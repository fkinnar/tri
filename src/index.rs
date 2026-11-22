//! Index de recherche des palettes de tous les dossiers (#561, étape 1) : pas une
//! bibliothèque, un **cache de recherche qui se répare de lui-même**.
//!
//! - Seulement des données recalculables (les palettes) : l'index peut être effacé à tout
//!   moment, les recherches sont alors partielles jusqu'au prochain calcul. Les données de
//!   l'utilisateur restent dans les sidecars XMP.
//! - **Identité par le contenu** : chaque photo a une empreinte ([`fingerprint`] : taille et
//!   hachage du début et de la fin du fichier). Une palette ne dépend que du contenu (et,
//!   pour un rendu développé, de l'état du sidecar : [`crate::preview::stamp`]) ; une photo
//!   déplacée garde la sienne, son nouveau chemin est noté quand `tri` la revoit.
//! - Rien n'est suivi (déplacements, copies, suppressions) : un chemin qui ne mène plus à
//!   rien se voit à l'affichage d'un résultat (étapes suivantes).
//!
//! Rempli au fil des palettes prêtes (calculées ou trouvées dans le cache), par un fil à
//! part ([`start`], [`note`]) : le calcul ne l'attend jamais. Base SQLite dans le dossier
//! des données de `tri` (pas le cache, qu'on vide). Voir `doc/en/index.md`.

use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;

use crate::cache::{Cache, Level};
use crate::media::MediaFile;
use crate::palette::{Palette, Source};

/// Version du schéma : une autre version, la base est refaite (rien n'y est irremplaçable).
const SCHEMA: i64 = 1;

/// Section `[index]` : index de recherche des palettes (#561).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IndexSettings {
    /// Noter les palettes dans l'index (recherche dans tous les dossiers).
    pub enabled: bool,
    /// Octets lus au début et à la fin d'un fichier pour son empreinte (Ko).
    pub fingerprint_kb: u32,
    /// Palettes notées par écriture (une transaction) au plus.
    pub batch: usize,
}

impl Default for IndexSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            fingerprint_kb: 32,
            batch: 256,
        }
    }
}

/// Empreinte du contenu d'un fichier : sa taille et un hachage BLAKE3 (tronqué à 16 octets)
/// de ses `chunk` premiers et derniers octets. Assez pour reconnaître une photo déplacée ou
/// copiée (une photo modifiée change de taille ou de contenu à l'un des bouts : en-tête
/// EXIF, aperçu, fin des données) ; lit au plus `2 × chunk` octets.
pub fn fingerprint(path: &Path, chunk: usize) -> io::Result<(u64, [u8; 16])> {
    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_le_bytes());
    let mut buf = vec![0u8; chunk.min(size as usize)];
    file.read_exact(&mut buf)?;
    hasher.update(&buf);
    if size > chunk as u64 {
        let tail = chunk.min((size - chunk as u64) as usize);
        file.seek(SeekFrom::End(-(tail as i64)))?;
        buf.resize(tail, 0);
        file.read_exact(&mut buf)?;
        hasher.update(&buf);
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    Ok((size, out))
}

/// Une palette à noter : le fichier tel qu'il était (chemin, taille, date en ns), son
/// empreinte, la source, le tampon de ses paramètres et la palette (format du cache).
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Chemin absolu.
    pub path: PathBuf,
    /// Taille (octets).
    pub size: u64,
    /// Date de modification (ns depuis 1970), celle des clés du cache.
    pub modified: u128,
    /// Empreinte du contenu ([`fingerprint`]).
    pub fingerprint: [u8; 16],
    /// Source de la palette.
    pub source: Source,
    /// Paramètres de la palette ([`crate::preview::stamp`]).
    pub stamp: String,
    /// La palette ([`Palette::to_bytes`]).
    pub palette: Vec<u8>,
}

/// Une palette de l'index, avec les chemins connus de son contenu (plusieurs : copies).
#[derive(Clone, Debug)]
pub struct Indexed {
    /// Empreinte du contenu.
    pub fingerprint: [u8; 16],
    /// Chemins connus, avec leur taille et leur date (clés du cache sans relevé).
    pub paths: Vec<(PathBuf, u64, u128)>,
    /// Paramètres de la palette.
    pub stamp: String,
    /// La palette.
    pub palette: Palette,
}

/// La base de l'index.
pub struct Index {
    db: Connection,
}

impl Index {
    /// Ouvre (ou crée) l'index du fichier `path`. Une base d'un autre schéma, ou illisible,
    /// est refaite : l'index n'est jamais la seule copie de rien.
    pub fn open(path: &Path) -> rusqlite::Result<Index> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match Self::try_open(path) {
            Ok(index) => Ok(index),
            Err(_) => {
                let _ = std::fs::remove_file(path);
                for suffix in ["-wal", "-shm"] {
                    let mut side = path.as_os_str().to_owned();
                    side.push(suffix);
                    let _ = std::fs::remove_file(side);
                }
                Self::try_open(path)
            }
        }
    }

    /// Index en mémoire (tests, mesures).
    pub fn in_memory() -> rusqlite::Result<Index> {
        Self::setup(Connection::open_in_memory()?)
    }

    fn try_open(path: &Path) -> rusqlite::Result<Index> {
        let db = Connection::open(path)?;
        // Écritures en fond pendant les lectures (recherche) ; rien d'irremplaçable : une
        // écriture perdue à une coupure de courant est refaite au prochain calcul.
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "NORMAL")?;
        Self::setup(db)
    }

    fn setup(db: Connection) -> rusqlite::Result<Index> {
        let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version != SCHEMA {
            db.execute_batch(
                "DROP TABLE IF EXISTS paths;
                 DROP TABLE IF EXISTS palettes;
                 CREATE TABLE paths (
                     path BLOB PRIMARY KEY,
                     size INTEGER NOT NULL,
                     modified INTEGER NOT NULL,
                     fingerprint BLOB NOT NULL
                 );
                 CREATE INDEX paths_fingerprint ON paths(fingerprint);
                 CREATE TABLE palettes (
                     fingerprint BLOB NOT NULL,
                     source TEXT NOT NULL,
                     stamp TEXT NOT NULL,
                     palette BLOB NOT NULL,
                     PRIMARY KEY (fingerprint, source)
                 );",
            )?;
            db.pragma_update(None, "user_version", SCHEMA)?;
        }
        Ok(Index { db })
    }

    /// Empreinte de `path` si l'index le connaît à cette taille et cette date.
    pub fn known(&self, path: &Path, size: u64, modified: u128) -> Option<[u8; 16]> {
        let fp: Vec<u8> = self
            .db
            .query_row(
                "SELECT fingerprint FROM paths WHERE path = ?1 AND size = ?2 AND modified = ?3",
                params![path_bytes(path), size as i64, ns(modified)],
                |r| r.get(0),
            )
            .optional()
            .ok()??;
        fp.try_into().ok()
    }

    /// Vrai si l'index a la palette de `source` de ce contenu, à ces paramètres.
    pub fn has(&self, fingerprint: &[u8; 16], source: Source, stamp: &str) -> bool {
        self.db
            .query_row(
                "SELECT 1 FROM palettes WHERE fingerprint = ?1 AND source = ?2 AND stamp = ?3",
                params![&fingerprint[..], source.id(), stamp],
                |_| Ok(()),
            )
            .optional()
            .ok()
            .flatten()
            .is_some()
    }

    /// Note `entries` (une transaction) : le chemin (remplace ce qu'il était), la palette
    /// de son contenu (remplace celle d'autres paramètres).
    pub fn record(&mut self, entries: &[Entry]) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        {
            let mut path = tx.prepare(
                "INSERT OR REPLACE INTO paths (path, size, modified, fingerprint)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            let mut palette = tx.prepare(
                "INSERT OR REPLACE INTO palettes (fingerprint, source, stamp, palette)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for e in entries {
                path.execute(params![
                    path_bytes(&e.path),
                    e.size as i64,
                    ns(e.modified),
                    &e.fingerprint[..]
                ])?;
                palette.execute(params![
                    &e.fingerprint[..],
                    e.source.id(),
                    e.stamp,
                    e.palette
                ])?;
            }
        }
        tx.commit()
    }

    /// Toutes les palettes de `source`, avec les chemins connus de leur contenu (sans
    /// chemin : contenu oublié, pas rendu). Lues d'un coup pour une recherche en mémoire.
    pub fn palettes(&self, source: Source) -> rusqlite::Result<Vec<Indexed>> {
        let mut by_fp: std::collections::HashMap<Vec<u8>, usize> = Default::default();
        let mut out: Vec<Indexed> = Vec::new();
        let mut stmt = self.db.prepare(
            "SELECT p.fingerprint, p.stamp, p.palette, f.path, f.size, f.modified
             FROM palettes p JOIN paths f ON f.fingerprint = p.fingerprint
             WHERE p.source = ?1",
        )?;
        let rows = stmt.query_map(params![source.id()], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Vec<u8>>(2)?,
                r.get::<_, Vec<u8>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?;
        for row in rows {
            let (fp, stamp, bytes, path, size, modified) = row?;
            let path = (bytes_path(&path), size as u64, modified.max(0) as u128);
            if let Some(&i) = by_fp.get(&fp) {
                out[i].paths.push(path);
                continue;
            }
            let (Ok(fingerprint), Some(palette)) =
                (fp.clone().try_into(), Palette::from_bytes(&bytes))
            else {
                continue;
            };
            by_fp.insert(fp, out.len());
            out.push(Indexed {
                fingerprint,
                paths: vec![path],
                stamp,
                palette,
            });
        }
        Ok(out)
    }

    /// Nombre de chemins et de palettes notés.
    pub fn counts(&self) -> rusqlite::Result<(usize, usize)> {
        let count = |table: &str| -> rusqlite::Result<usize> {
            self.db
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| {
                    r.get::<_, i64>(0)
                })
                .map(|n| n as usize)
        };
        Ok((count("paths")?, count("palettes")?))
    }
}

/// Date en ns, dans un entier SQLite (jusqu'en 2262).
fn ns(modified: u128) -> i64 {
    i64::try_from(modified).unwrap_or(i64::MAX)
}

/// Chemin en octets : ceux du système (noms non UTF-8 compris) ; UTF-16 sous Windows.
fn path_bytes(path: &Path) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes().to_vec()
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str()
            .encode_wide()
            .flat_map(u16::to_le_bytes)
            .collect()
    }
}

/// Inverse de [`path_bytes`].
fn bytes_path(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        let wide: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        PathBuf::from(std::ffi::OsString::from_wide(&wide))
    }
}

/// Une palette prête à noter (fil d'écriture).
struct Note {
    file: MediaFile,
    source: Source,
}

/// Message au fil d'écriture : une palette à noter, ou « préviens quand tout ce qui
/// précède est écrit » ([`flush`]).
enum Message {
    Note(Note),
    Flush(Sender<()>),
}

/// Fil d'écriture de l'index, une fois [`start`] appelé.
static WRITER: OnceLock<Sender<Message>> = OnceLock::new();

/// Démarre le fil d'écriture de l'index `db` : il relit les palettes prêtes dans `cache`
/// (paramètres `preview`). Sans effet s'il tourne déjà ou si l'index est coupé
/// (`settings.enabled`). Renvoie l'erreur d'ouverture de la base.
pub fn start(
    db: &Path,
    cache: Cache,
    preview: crate::preview::Settings,
    settings: IndexSettings,
) -> rusqlite::Result<()> {
    if !settings.enabled || WRITER.get().is_some() {
        return Ok(());
    }
    let index = Index::open(db)?;
    let (tx, rx) = mpsc::channel();
    if WRITER.set(tx).is_err() {
        return Ok(());
    }
    let shared = Arc::new((cache, preview, settings));
    std::thread::Builder::new()
        .name("tri-index".into())
        .spawn(move || write_loop(index, &rx, &shared))
        .map(|_| ())
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

/// Signale une palette prête (calculée ou trouvée dans le cache) : notée dans l'index en
/// fond. Sans index démarré, rien.
pub fn note(file: &MediaFile, source: Source) {
    if let Some(tx) = WRITER.get() {
        let _ = tx.send(Message::Note(Note {
            file: file.clone(),
            source,
        }));
    }
}

/// Attend que les palettes signalées jusqu'ici soient écrites (ligne de commande, avant de
/// quitter), au plus `timeout`. Sans index démarré, rien.
pub fn flush(timeout: Duration) {
    if let Some(tx) = WRITER.get() {
        let (done, wait) = mpsc::channel();
        if tx.send(Message::Flush(done)).is_ok() {
            let _ = wait.recv_timeout(timeout);
        }
    }
}

fn write_loop(
    mut index: Index,
    rx: &Receiver<Message>,
    shared: &(Cache, crate::preview::Settings, IndexSettings),
) {
    let (cache, preview, settings) = shared;
    while let Ok(first) = rx.recv() {
        // Par lots : un dossier déjà calculé annonce toutes ses palettes d'un coup.
        let mut notes = Vec::new();
        let mut flushes = Vec::new();
        let mut next = Some(first);
        while let Some(message) = next.take() {
            match message {
                Message::Note(n) => notes.push(n),
                // Écrire ce qui précède, puis prévenir.
                Message::Flush(done) => {
                    flushes.push(done);
                    break;
                }
            }
            if notes.len() >= settings.batch.max(1) {
                break;
            }
            next = rx.recv_timeout(Duration::from_millis(50)).ok();
        }
        let entries: Vec<Entry> = notes
            .iter()
            .filter_map(|n| entry(&index, n, cache, preview, settings))
            .collect();
        if !entries.is_empty()
            && let Err(e) = index.record(&entries)
        {
            crate::log::warning("index", &format!("palettes not recorded: {e}"), "");
        }
        for done in flushes {
            let _ = done.send(());
        }
    }
}

/// L'entrée à noter pour `note`, `None` si l'index l'a déjà (même chemin, même fichier,
/// mêmes paramètres) ou si le fichier ou la palette ne se lisent pas.
fn entry(
    index: &Index,
    note: &Note,
    cache: &Cache,
    preview: &crate::preview::Settings,
    settings: &IndexSettings,
) -> Option<Entry> {
    let file = &note.file;
    let path = std::path::absolute(&file.path).ok()?;
    let (size, modified) = crate::cache::lookup_stamp(&path)??;
    let level = Level::Palette(note.source);
    let stamp = crate::preview::stamp(file, level, preview);
    let known = index.known(&path, size, modified);
    if known.is_some_and(|fp| index.has(&fp, note.source, &stamp)) {
        return None;
    }
    let fingerprint = match known {
        Some(fp) => fp,
        None => {
            fingerprint(&path, settings.fingerprint_kb as usize * 1024)
                .ok()?
                .1
        }
    };
    let key = crate::preview::key(file, level, preview).ok()?;
    let palette = cache.get_bytes(&key)?;
    Some(Entry {
        path,
        size,
        modified,
        fingerprint,
        source: note.source,
        stamp,
        palette,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, fp: u8, source: Source, stamp: &str, palette: &Palette) -> Entry {
        Entry {
            path: PathBuf::from(path),
            size: 100,
            modified: 1_700_000_000_000_000_000,
            fingerprint: [fp; 16],
            source,
            stamp: stamp.into(),
            palette: palette.to_bytes(),
        }
    }

    fn palette(l: f32) -> Palette {
        Palette {
            swatches: vec![crate::palette::Swatch {
                oklab: [l, 0.1, 0.0],
                share: 1.0,
            }],
            coverage: crate::palette::Coverage::from_samples(&[[l, 0.1, 0.0]; 4]),
        }
    }

    #[test]
    fn a_moved_photo_keeps_its_palette_and_a_copy_has_two_paths() {
        let mut index = Index::in_memory().unwrap();
        let red = palette(0.5);
        index
            .record(&[entry("/a/x.raf", 1, Source::Camera, "s", &red)])
            .unwrap();
        assert_eq!(
            index.known(Path::new("/a/x.raf"), 100, 1_700_000_000_000_000_000),
            Some([1; 16])
        );
        assert_eq!(
            index.known(Path::new("/a/x.raf"), 101, 1_700_000_000_000_000_000),
            None
        );
        assert!(index.has(&[1; 16], Source::Camera, "s"));
        assert!(!index.has(&[1; 16], Source::Camera, "autres réglages"));
        assert!(!index.has(&[1; 16], Source::Raw, "s"));
        // Déplacée (même contenu ailleurs) : un chemin de plus, la même palette.
        index
            .record(&[entry("/b/x.raf", 1, Source::Camera, "s", &red)])
            .unwrap();
        let found = index.palettes(Source::Camera).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].paths.len(), 2);
        assert_eq!(found[0].palette, red);
        assert_eq!(index.counts().unwrap(), (2, 1));
        // Recalculée à d'autres réglages : remplacée, pas en double.
        let blue = palette(0.7);
        index
            .record(&[entry("/b/x.raf", 1, Source::Camera, "t", &blue)])
            .unwrap();
        let found = index.palettes(Source::Camera).unwrap();
        assert_eq!((found.len(), found[0].stamp.as_str()), (1, "t"));
        assert_eq!(found[0].palette, blue);
        assert!(index.palettes(Source::Raw).unwrap().is_empty());
    }

    #[test]
    fn the_fingerprint_reads_both_ends_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.bin");
        let mut data = vec![7u8; 300_000];
        std::fs::write(&path, &data).unwrap();
        let (size, a) = fingerprint(&path, 1024).unwrap();
        assert_eq!(size, 300_000);
        // Le milieu change : même empreinte (pas lu) ; un bout change : une autre.
        data[150_000] = 1;
        std::fs::write(&path, &data).unwrap();
        assert_eq!(fingerprint(&path, 1024).unwrap().1, a);
        data[299_999] = 1;
        std::fs::write(&path, &data).unwrap();
        assert_ne!(fingerprint(&path, 1024).unwrap().1, a);
        // Plus petit qu'un bout.
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(fingerprint(&path, 1024).unwrap().0, 3);
    }

    #[test]
    fn a_base_of_another_schema_is_rebuilt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.sqlite");
        std::fs::write(&path, b"pas une base").unwrap();
        let mut index = Index::open(&path).unwrap();
        index
            .record(&[entry("/a/x.raf", 1, Source::Raw, "s", &palette(0.5))])
            .unwrap();
        drop(index);
        let index = Index::open(&path).unwrap();
        assert_eq!(index.counts().unwrap(), (1, 1), "relue telle quelle");
    }

    #[test]
    fn non_utf8_and_odd_paths_round_trip() {
        let path = PathBuf::from("/photos/été 2026/IMG #1.raf");
        assert_eq!(bytes_path(&path_bytes(&path)), path);
    }
}
