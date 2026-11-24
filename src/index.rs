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
const SCHEMA: i64 = 4;

/// Section `[index]` : index de recherche des palettes (#561).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IndexSettings {
    /// Noter les palettes dans l'index (recherche dans tous les dossiers).
    pub enabled: bool,
    /// Octets lus au début et à la fin d'un fichier pour son empreinte (Ko).
    pub fingerprint_kb: u32,
    /// Palettes notées par écriture (une transaction) au plus.
    pub batch: usize,
    /// Racines surveillées ([`crate::roots`]) : dossiers parcourus en fond (`~/…`
    /// accepté).
    pub roots: Vec<String>,
    /// Parcourir les racines au lancement.
    pub walk_at_launch: bool,
    /// Parcourir un disque amovible quand on le branche (jamais une carte d'appareil).
    pub walk_plugged: bool,
    /// Reparcourir les racines après tant de minutes sans rien faire (0 : jamais).
    pub walk_idle_minutes: u64,
    /// Niveaux de dossiers au plus sous une racine.
    pub walk_depth: usize,
    /// Pause entre deux dossiers d'un parcours (ms), pour ne rien gêner.
    pub walk_pause_ms: u64,
}

impl Default for IndexSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            fingerprint_kb: 32,
            batch: 256,
            roots: Vec::new(),
            walk_at_launch: true,
            walk_plugged: true,
            walk_idle_minutes: 10,
            walk_depth: 16,
            walk_pause_ms: 2,
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
        // Écrit aussi ailleurs (le fil d'écriture, une réparation) : on attend son tour.
        db.busy_timeout(Duration::from_secs(5))?;
        // Écritures en fond pendant les lectures (recherche) ; rien d'irremplaçable : une
        // écriture perdue à une coupure de courant est refaite au prochain calcul.
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "NORMAL")?;
        Self::setup(db)
    }

    fn setup(db: Connection) -> rusqlite::Result<Index> {
        let version: i64 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        // Les colonnes ajoutées depuis : la base gardée (tout ce qu'on a vu).
        let added: &[&str] = match version {
            2 => &[
                "ALTER TABLE paths ADD COLUMN signature BLOB",
                "ALTER TABLE paths ADD COLUMN signature_stamp TEXT",
                "ALTER TABLE paths ADD COLUMN notes INTEGER",
                "CREATE INDEX paths_size ON paths(size)",
            ],
            3 => &[
                "ALTER TABLE paths ADD COLUMN notes INTEGER",
                "CREATE INDEX paths_size ON paths(size)",
            ],
            _ => &[],
        };
        if !added.is_empty() {
            for statement in added {
                db.execute(statement, [])?;
            }
            db.pragma_update(None, "user_version", SCHEMA)?;
        } else if version != SCHEMA {
            db.execute_batch(
                "DROP TABLE IF EXISTS paths;
                 DROP TABLE IF EXISTS palettes;
                 CREATE TABLE paths (
                     path BLOB PRIMARY KEY,
                     size INTEGER NOT NULL,
                     modified INTEGER NOT NULL,
                     fingerprint BLOB,
                     signature BLOB,
                     signature_stamp TEXT,
                     notes INTEGER
                 );
                 CREATE INDEX paths_fingerprint ON paths(fingerprint);
                 CREATE INDEX paths_size ON paths(size);
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

    /// Empreinte de `path` si l'index le connaît à cette taille et cette date, et l'a
    /// déjà lue (une photo seulement vue n'en a pas encore).
    pub fn known(&self, path: &Path, size: u64, modified: u128) -> Option<[u8; 16]> {
        let fp: Option<Vec<u8>> = self
            .db
            .query_row(
                "SELECT fingerprint FROM paths WHERE path = ?1 AND size = ?2 AND modified = ?3",
                params![path_bytes(path), size as i64, ns(modified)],
                |r| r.get(0),
            )
            .optional()
            .ok()??;
        fp?.try_into().ok()
    }

    /// Note les photos vues dans `dir` sur `depth` niveaux (1 : le dossier seul), avec leur
    /// taille et leur date ; oublie celles de ces niveaux qui n'y sont plus (supprimées,
    /// déplacées : revues ailleurs, elles y seront notées). Une photo changée (autre
    /// taille ou date) perd son empreinte, relue au prochain calcul de palette. Les
    /// développements (`IMG@dt-01.RAF`) ne sont pas des fichiers : pas notés.
    pub fn seen(
        &mut self,
        dir: &Path,
        depth: usize,
        photos: &[(PathBuf, u64, u128)],
    ) -> rusqlite::Result<()> {
        let keep: std::collections::HashSet<Vec<u8>> =
            photos.iter().map(|(p, _, _)| path_bytes(p)).collect();
        let tx = self.db.transaction()?;
        {
            // Sous `dir` : les chemins qui commencent par les octets de `dir`, puis gardés
            // composant par composant (`Path::starts_with` : `/ab` n'est pas sous `/a`, et
            // sous Windows `/` et `\` se valent).
            let low = path_bytes(dir);
            let mut high = low.clone();
            if let Some(last) = high.last_mut() {
                *last += 1;
            }
            let mut stmt = tx.prepare("SELECT path FROM paths WHERE path >= ?1 AND path < ?2")?;
            let below: Vec<Vec<u8>> = stmt
                .query_map(params![low, high], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            let mut forget = tx.prepare("DELETE FROM paths WHERE path = ?1")?;
            for bytes in below {
                let path = bytes_path(&bytes);
                let Ok(rest) = path.strip_prefix(dir) else {
                    continue;
                };
                let levels = rest.components().count();
                if levels >= 1 && levels <= depth && !keep.contains(&bytes) {
                    forget.execute(params![bytes])?;
                }
            }
            let mut upsert = tx.prepare(
                "INSERT INTO paths (path, size, modified, fingerprint) VALUES (?1, ?2, ?3, NULL)
                 ON CONFLICT(path) DO UPDATE SET size = ?2, modified = ?3,
                     fingerprint = CASE WHEN size = ?2 AND modified = ?3
                                        THEN fingerprint ELSE NULL END,
                     signature = CASE WHEN size = ?2 AND modified = ?3
                                      THEN signature ELSE NULL END",
            )?;
            for (path, size, modified) in photos {
                upsert.execute(params![path_bytes(path), *size as i64, ns(*modified)])?;
            }
        }
        tx.commit()
    }

    /// Les photos connues directement dans `dir` (pas ses sous-dossiers) : nom, taille,
    /// empreinte de contenu si elle a été lue. Pour vérifier qu'un dossier retrouvé est
    /// bien le même ([`crate::locate::verify`]).
    pub fn photos_in(&self, dir: &Path) -> rusqlite::Result<Vec<crate::locate::Sample>> {
        let low = path_bytes(dir);
        let mut high = low.clone();
        if let Some(last) = high.last_mut() {
            *last += 1;
        }
        let mut stmt = self
            .db
            .prepare("SELECT path, size, fingerprint FROM paths WHERE path >= ?1 AND path < ?2")?;
        let rows = stmt.query_map(params![low, high], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<Vec<u8>>>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (bytes, size, fp) = row?;
            let path = bytes_path(&bytes);
            if path.parent() != Some(dir) {
                continue;
            }
            let Some(name) = path.file_name() else {
                continue;
            };
            out.push(crate::locate::Sample {
                name: name.to_os_string(),
                size: size as u64,
                fingerprint: fp.and_then(|f| f.try_into().ok()),
            });
        }
        Ok(out)
    }

    /// Les chemins connus dont la taille est l'une de `sizes` (pour reconnaître un dossier
    /// déplacé à ses photos, [`crate::locate::recognise`]).
    pub fn with_sizes(&self, sizes: &[u64]) -> rusqlite::Result<Vec<(PathBuf, u64)>> {
        let mut out = Vec::new();
        let mut stmt = self
            .db
            .prepare("SELECT path, size FROM paths WHERE size = ?1")?;
        for &size in sizes {
            let rows = stmt.query_map(params![size as i64], |r| {
                Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, i64>(1)?))
            })?;
            for row in rows {
                let (bytes, size) = row?;
                out.push((bytes_path(&bytes), size as u64));
            }
        }
        Ok(out)
    }

    /// Un dossier déplacé (`from` → `to`, « Retrouver… », #561) : ses chemins et ceux de
    /// ses sous-dossiers suivent, avec leurs empreintes (palettes, similitude) ; un chemin
    /// déjà connu à la nouvelle place garde la sienne. Renvoie le nombre de chemins
    /// déplacés.
    pub fn relocate(&mut self, from: &Path, to: &Path) -> rusqlite::Result<usize> {
        let low = path_bytes(from);
        let mut high = low.clone();
        if let Some(last) = high.last_mut() {
            *last += 1;
        }
        let tx = self.db.transaction()?;
        let mut moved = 0;
        {
            let mut stmt = tx.prepare("SELECT path FROM paths WHERE path >= ?1 AND path < ?2")?;
            let below: Vec<Vec<u8>> = stmt
                .query_map(params![low, high], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            let mut exists = tx.prepare("SELECT 1 FROM paths WHERE path = ?1")?;
            let mut update = tx.prepare("UPDATE paths SET path = ?2 WHERE path = ?1")?;
            let mut forget = tx.prepare("DELETE FROM paths WHERE path = ?1")?;
            for bytes in below {
                let path = bytes_path(&bytes);
                let Ok(rest) = path.strip_prefix(from) else {
                    continue;
                };
                let new = path_bytes(&to.join(rest));
                if exists.exists(params![new])? {
                    forget.execute(params![bytes])?;
                } else {
                    update.execute(params![bytes, new])?;
                }
                moved += 1;
            }
        }
        tx.commit()?;
        Ok(moved)
    }

    /// Des photos déplacées ou copiées par `tri` (`copy`) : (avant, après, taille et date
    /// de l'après). Déplacée, la photo garde tout ce que l'index en savait ; copiée, la
    /// copie est notée avec l'empreinte de contenu de l'original (ses palettes suivent,
    /// son empreinte de similitude sera recalculée). Les chemins inconnus sont ignorés.
    pub fn carry(&mut self, items: &[Carried], copy: bool) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        {
            let mut fingerprint_of = tx.prepare("SELECT fingerprint FROM paths WHERE path = ?1")?;
            let mut forget = tx.prepare("DELETE FROM paths WHERE path = ?1")?;
            let mut update = tx.prepare(
                "UPDATE paths SET path = ?2,
                     size = COALESCE(?3, size), modified = COALESCE(?4, modified)
                 WHERE path = ?1",
            )?;
            let mut insert = tx.prepare(
                "INSERT OR REPLACE INTO paths (path, size, modified, fingerprint)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (from, to, stamp) in items {
                let (from_b, to_b) = (path_bytes(from), path_bytes(to));
                let Some(fp) = fingerprint_of
                    .query_row(params![from_b], |r| r.get::<_, Option<Vec<u8>>>(0))
                    .optional()?
                else {
                    continue;
                };
                if copy {
                    if let Some((size, modified)) = stamp {
                        insert.execute(params![to_b, *size as i64, ns(*modified), fp])?;
                    }
                } else {
                    forget.execute(params![to_b])?;
                    update.execute(params![
                        from_b,
                        to_b,
                        stamp.map(|s| s.0 as i64),
                        stamp.map(|s| ns(s.1))
                    ])?;
                }
            }
        }
        tx.commit()
    }

    /// Note les notes de photos (étoiles, drapeau, couleurs) : celles qu'on montrera si la
    /// photo devient injoignable (dossier renommé, disque débranché). Les chemins
    /// inconnus sont ignorés.
    pub fn record_notes(&mut self, items: &[(PathBuf, crate::xmp::Notes)]) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        {
            let mut update =
                tx.prepare("UPDATE paths SET notes = ?2 WHERE path = ?1 AND notes IS NOT ?2")?;
            for (path, notes) in items {
                update.execute(params![path_bytes(path), pack_notes(notes)])?;
            }
        }
        tx.commit()
    }

    /// Les notes connues des photos directement dans `dir`.
    pub fn notes_in(&self, dir: &Path) -> rusqlite::Result<Vec<(PathBuf, crate::xmp::Notes)>> {
        let low = path_bytes(dir);
        let mut high = low.clone();
        if let Some(last) = high.last_mut() {
            *last += 1;
        }
        let mut stmt = self.db.prepare(
            "SELECT path, notes FROM paths WHERE path >= ?1 AND path < ?2 AND notes IS NOT NULL",
        )?;
        let rows = stmt.query_map(params![low, high], |r| {
            Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, i64>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (bytes, packed) = row?;
            let path = bytes_path(&bytes);
            if path.parent() == Some(dir) {
                out.push((path, unpack_notes(packed)));
            }
        }
        Ok(out)
    }

    /// Vrai si l'index a l'empreinte de similitude de `path` à cette taille, cette date et
    /// ces paramètres.
    pub fn has_signature(&self, path: &Path, size: u64, modified: u128, stamp: &str) -> bool {
        self.db
            .query_row(
                "SELECT 1 FROM paths WHERE path = ?1 AND size = ?2 AND modified = ?3
                 AND signature IS NOT NULL AND signature_stamp = ?4",
                params![path_bytes(path), size as i64, ns(modified), stamp],
                |_| Ok(()),
            )
            .optional()
            .ok()
            .flatten()
            .is_some()
    }

    /// Note des empreintes de similitude (une transaction) : (chemin, taille, date,
    /// paramètres, empreinte au format du cache). Le chemin est noté s'il ne l'était pas ;
    /// son empreinte de contenu reste si le fichier n'a pas changé.
    pub fn record_signatures(
        &mut self,
        items: &[(PathBuf, u64, u128, String, Vec<u8>)],
    ) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        {
            let mut upsert = tx.prepare(
                "INSERT INTO paths (path, size, modified, fingerprint, signature, signature_stamp)
                 VALUES (?1, ?2, ?3, NULL, ?4, ?5)
                 ON CONFLICT(path) DO UPDATE SET
                     fingerprint = CASE WHEN size = ?2 AND modified = ?3
                                        THEN fingerprint ELSE NULL END,
                     size = ?2, modified = ?3, signature = ?4, signature_stamp = ?5",
            )?;
            for (path, size, modified, stamp, bytes) in items {
                upsert.execute(params![
                    path_bytes(path),
                    *size as i64,
                    ns(*modified),
                    bytes,
                    stamp
                ])?;
            }
        }
        tx.commit()
    }

    /// Les empreintes de similitude connues (chemin, empreinte au format du cache), pour
    /// « Toutes les photos » : « semblables » partout sans relire le cache.
    pub fn signatures(&self) -> rusqlite::Result<Vec<(PathBuf, Vec<u8>)>> {
        let mut stmt = self
            .db
            .prepare("SELECT path, signature FROM paths WHERE signature IS NOT NULL")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?;
        rows.map(|r| r.map(|(p, s)| (bytes_path(&p), s))).collect()
    }

    /// Les photos connues, sous `under` s'il est donné (tous niveaux), sinon toutes :
    /// chemin, taille, date. Pour « Toutes les photos » (#561), sans parcourir les
    /// dossiers.
    pub fn photos(&self, under: Option<&Path>) -> rusqlite::Result<Vec<(PathBuf, u64, u128)>> {
        let mut stmt = self.db.prepare("SELECT path, size, modified FROM paths")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (bytes, size, modified) = row?;
            let path = bytes_path(&bytes);
            if under.is_none_or(|d| path.starts_with(d)) {
                out.push((path, size as u64, modified.max(0) as u128));
            }
        }
        Ok(out)
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
            // Le chemin : son empreinte de contenu ; son empreinte de similitude reste si le
            // fichier n'a pas changé.
            let mut path = tx.prepare(
                "INSERT INTO paths (path, size, modified, fingerprint) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(path) DO UPDATE SET
                     signature = CASE WHEN size = ?2 AND modified = ?3
                                      THEN signature ELSE NULL END,
                     size = ?2, modified = ?3, fingerprint = ?4",
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
/// Notes en un entier : étoiles (bits 0–3), drapeau (4–5), couleurs (une par bit, 8–12).
fn pack_notes(notes: &crate::xmp::Notes) -> i64 {
    use crate::xmp::{Color, Flag};
    let flag = match notes.flag {
        Flag::None => 0,
        Flag::Accepted => 1,
        Flag::Rejected => 2,
    };
    let colors = Color::ALL
        .iter()
        .enumerate()
        .filter(|(_, c)| notes.colors.contains(c))
        .fold(0i64, |bits, (i, _)| bits | 1 << i);
    i64::from(notes.stars.min(15)) | flag << 4 | colors << 8
}

/// L'inverse de [`pack_notes`].
fn unpack_notes(packed: i64) -> crate::xmp::Notes {
    use crate::xmp::{Color, Flag};
    crate::xmp::Notes {
        stars: (packed & 0xf) as u8,
        flag: match (packed >> 4) & 3 {
            1 => Flag::Accepted,
            2 => Flag::Rejected,
            _ => Flag::None,
        },
        colors: Color::ALL
            .iter()
            .enumerate()
            .filter(|(i, _)| packed >> 8 & 1 << i != 0)
            .map(|(_, c)| *c)
            .collect(),
    }
}

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
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect();
        PathBuf::from(std::ffi::OsString::from_wide(&wide))
    }
}

/// Un fichier suivi par [`Index::carry`] : avant, après, taille et date de l'après.
pub type Carried = (PathBuf, PathBuf, Option<(u64, u128)>);

/// Une palette prête à noter (fil d'écriture).
struct Note {
    file: MediaFile,
    source: Source,
}

/// Message au fil d'écriture : une palette à noter, ou « préviens quand tout ce qui
/// précède est écrit » ([`flush`]).
enum Message {
    Note(Note),
    /// Une empreinte de similitude prête (dans le cache).
    Signature(MediaFile),
    /// Des fichiers déplacés ou copiés par `tri` ([`carry`]).
    Carry(Vec<(PathBuf, PathBuf)>, bool),
    /// Des notes lues ou écrites ([`note_notes`]).
    Notes(Vec<(PathBuf, crate::xmp::Notes)>),
    Seen {
        dir: PathBuf,
        depth: usize,
        files: Vec<MediaFile>,
    },
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
    // Une carte d'appareil n'est pas indexée (retour du 22/11/2026 : des fichiers bruts,
    // avant leur copie ailleurs) ; un développement non plus (un chemin virtuel, qui
    // n'existe pas sur le disque : pris comme photo témoin, il faisait échouer la
    // reconnaissance d'un dossier renommé).
    if crate::locate::is_camera_card(&file.path) || crate::media::is_development(&file.path) {
        return;
    }
    if let Some(tx) = WRITER.get() {
        let _ = tx.send(Message::Note(Note {
            file: file.clone(),
            source,
        }));
    }
}

/// Signale une empreinte de similitude prête (calculée ou trouvée dans le cache) : notée
/// dans l'index en fond (« semblables » dans « Toutes les photos »). Sans index démarré,
/// ou pour un développement (il a celle de sa photo), rien.
pub fn note_signature(file: &MediaFile) {
    if crate::media::is_development(&file.path) || crate::locate::is_camera_card(&file.path) {
        return;
    }
    if let Some(tx) = WRITER.get() {
        let _ = tx.send(Message::Signature(file.clone()));
    }
}

/// Signale les notes lues ou écrites de photos joignables : gardées dans l'index, pour
/// les montrer (et filtrer dessus) si la photo devient injoignable. Sans index démarré,
/// rien.
pub fn note_notes(notes: Vec<(PathBuf, crate::xmp::Notes)>) {
    if notes.is_empty() {
        return;
    }
    if let Some(tx) = WRITER.get() {
        let _ = tx.send(Message::Notes(notes));
    }
}

/// Signale des fichiers déplacés, renommés ou copiés (`copy`) par `tri` : l'index les
/// suit en fond (« Toutes les photos » les garde à leur nouvelle place). Sans index
/// démarré, rien.
pub fn carry(files: &[(PathBuf, PathBuf)], copy: bool) {
    if let Some(tx) = WRITER.get() {
        let _ = tx.send(Message::Carry(files.to_vec(), copy));
    }
}

/// Signale les images d'un dossier ouvert ou calculé (`dir`, `depth` niveaux) : notées
/// dans l'index en fond (« Toutes les photos », #561), celles qui n'y sont plus oubliées.
/// Sans index démarré, rien.
pub fn seen(dir: &Path, depth: usize, files: &[MediaFile]) {
    if crate::locate::is_camera_card(dir) {
        return;
    }
    if let Some(tx) = WRITER.get() {
        let _ = tx.send(Message::Seen {
            dir: dir.to_path_buf(),
            depth,
            files: files.to_vec(),
        });
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
        let mut signatures: Vec<MediaFile> = Vec::new();
        let mut flushes = Vec::new();
        let mut next = Some(first);
        while let Some(message) = next.take() {
            match message {
                Message::Note(n) => notes.push(n),
                Message::Signature(f) => signatures.push(f),
                Message::Notes(notes) => {
                    // Seulement des photos là (une injoignable n'a que des notes vides).
                    let items: Vec<(PathBuf, crate::xmp::Notes)> = notes
                        .into_iter()
                        .filter(|(p, _)| p.is_file())
                        .filter_map(|(p, n)| Some((std::path::absolute(&p).ok()?, n)))
                        .collect();
                    if let Err(e) = index.record_notes(&items) {
                        crate::log::warning("index", &format!("notes not recorded: {e}"), "");
                    }
                }
                Message::Carry(files, copy) => {
                    let items: Vec<Carried> = files
                        .into_iter()
                        .filter(|(_, to)| !crate::locate::is_camera_card(to))
                        .filter_map(|(from, to)| {
                            let from = std::path::absolute(&from).ok()?;
                            let to = std::path::absolute(&to).ok()?;
                            let stamp = crate::cache::read_stamp(&to);
                            Some((from, to, stamp))
                        })
                        .collect();
                    if let Err(e) = index.carry(&items, copy) {
                        crate::log::warning("index", &format!("files not followed: {e}"), "");
                    }
                }
                // Un dossier qui n'existe plus (renommé, disque débranché) n'est pas « vu
                // vide » : ses photos restent, grisées dans « Toutes les photos » (retour du
                // 22/11/2026 : rouvert au lancement, il effaçait tout ce qu'on en savait).
                Message::Seen { dir, .. } if !dir.is_dir() => {}
                Message::Seen { dir, depth, files } => {
                    let photos: Vec<(PathBuf, u64, u128)> = files
                        .iter()
                        .filter(|f| !crate::media::is_development(&f.path))
                        .filter_map(|f| {
                            let path = std::path::absolute(&f.path).ok()?;
                            let (size, modified) = crate::cache::lookup_stamp(&path)??;
                            Some((path, size, modified))
                        })
                        .collect();
                    let dir = std::path::absolute(&dir).unwrap_or(dir);
                    if let Err(e) = index.seen(&dir, depth, &photos) {
                        crate::log::warning("index", &format!("folder not recorded: {e}"), "");
                    }
                }
                // Écrire ce qui précède, puis prévenir.
                Message::Flush(done) => {
                    flushes.push(done);
                    break;
                }
            }
            if notes.len() + signatures.len() >= settings.batch.max(1) {
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
        let items: Vec<(PathBuf, u64, u128, String, Vec<u8>)> = signatures
            .iter()
            .filter_map(|f| signature_item(&index, f, cache, preview))
            .collect();
        if !items.is_empty()
            && let Err(e) = index.record_signatures(&items)
        {
            crate::log::warning("index", &format!("signatures not recorded: {e}"), "");
        }
        for done in flushes {
            let _ = done.send(());
        }
    }
}

/// L'empreinte de similitude de `file` à noter, `None` si l'index l'a déjà (même fichier,
/// mêmes paramètres) ou si elle ne se lit pas.
fn signature_item(
    index: &Index,
    file: &MediaFile,
    cache: &Cache,
    preview: &crate::preview::Settings,
) -> Option<(PathBuf, u64, u128, String, Vec<u8>)> {
    let path = std::path::absolute(&file.path).ok()?;
    let (size, modified) = crate::cache::lookup_stamp(&path)??;
    let stamp = crate::preview::stamp(file, Level::Similarity, preview);
    if index.has_signature(&path, size, modified, &stamp) {
        return None;
    }
    let key = crate::preview::key(file, Level::Similarity, preview).ok()?;
    let bytes = cache.get_bytes(&key)?;
    Some((path, size, modified, stamp, bytes))
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
            perceived: None,
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
    fn photos_seen_are_listed_and_missing_ones_forgotten_at_their_depth() {
        let mut index = Index::in_memory().unwrap();
        let p = |s: &str| PathBuf::from(s);
        let photos = |list: &[&str]| -> Vec<(PathBuf, u64, u128)> {
            list.iter()
                .map(|s| (p(s), 100, 1_700_000_000_000_000_000))
                .collect()
        };
        index
            .seen(
                Path::new("/a"),
                usize::MAX,
                &photos(&["/a/1.raf", "/a/2.raf", "/a/sub/3.raf"]),
            )
            .unwrap();
        index
            .seen(Path::new("/ab"), 1, &photos(&["/ab/4.raf"]))
            .unwrap();
        assert_eq!(index.photos(None).unwrap().len(), 4);
        assert_eq!(index.photos(Some(Path::new("/a"))).unwrap().len(), 3);
        // Rouvert sans sous-dossiers : `2` a disparu ; `sub/3` n'est pas regardé, gardé ;
        // `/ab` (même début de nom) n'est pas sous `/a`.
        index
            .seen(Path::new("/a"), 1, &photos(&["/a/1.raf"]))
            .unwrap();
        let mut all: Vec<PathBuf> = index
            .photos(None)
            .unwrap()
            .into_iter()
            .map(|e| e.0)
            .collect();
        all.sort();
        assert_eq!(all, vec![p("/a/1.raf"), p("/a/sub/3.raf"), p("/ab/4.raf")]);
    }

    #[test]
    fn a_photo_seen_keeps_its_fingerprint_unless_it_changed() {
        let mut index = Index::in_memory().unwrap();
        let red = palette(0.5);
        index
            .record(&[entry("/a/x.raf", 1, Source::Camera, "s", &red)])
            .unwrap();
        let seen = |size: u64| vec![(PathBuf::from("/a/x.raf"), size, 1_700_000_000_000_000_000)];
        // Revue telle quelle : empreinte et palette gardées.
        index.seen(Path::new("/a"), 1, &seen(100)).unwrap();
        assert_eq!(index.palettes(Source::Camera).unwrap().len(), 1);
        // Modifiée : empreinte oubliée (relue au prochain calcul), palette plus rendue.
        index.seen(Path::new("/a"), 1, &seen(101)).unwrap();
        assert_eq!(
            index.known(Path::new("/a/x.raf"), 101, 1_700_000_000_000_000_000),
            None
        );
        assert!(index.palettes(Source::Camera).unwrap().is_empty());
        assert_eq!(index.photos(None).unwrap().len(), 1, "toujours vue");
    }

    #[test]
    fn signatures_are_kept_while_the_file_is_the_same() {
        let mut index = Index::in_memory().unwrap();
        let t = 1_700_000_000_000_000_000u128;
        let p = PathBuf::from("/a/x.raf");
        index
            .record_signatures(&[(p.clone(), 100, t, "s".into(), vec![1, 2, 3])])
            .unwrap();
        assert!(index.has_signature(&p, 100, t, "s"));
        assert!(!index.has_signature(&p, 100, t, "autres réglages"));
        // Une palette notée ensuite, puis le dossier revu tel quel : l'empreinte reste.
        index
            .record(&[entry("/a/x.raf", 1, Source::Camera, "p", &palette(0.5))])
            .unwrap();
        index
            .seen(Path::new("/a"), 1, &[(p.clone(), 100, t)])
            .unwrap();
        assert_eq!(
            index.signatures().unwrap(),
            vec![(p.clone(), vec![1, 2, 3])]
        );
        // Le fichier change : oubliée.
        index
            .seen(Path::new("/a"), 1, &[(p.clone(), 101, t)])
            .unwrap();
        assert!(index.signatures().unwrap().is_empty());
    }

    #[test]
    fn moved_photos_keep_everything_and_copies_share_the_fingerprint() {
        let mut index = Index::in_memory().unwrap();
        let red = palette(0.5);
        index
            .record(&[entry("/a/x.raf", 7, Source::Camera, "s", &red)])
            .unwrap();
        let t = 1_700_000_000_000_000_000u128;
        index
            .record_signatures(&[(PathBuf::from("/a/x.raf"), 100, t, "s".into(), vec![9])])
            .unwrap();
        index
            .carry(
                &[(PathBuf::from("/a/x.raf"), PathBuf::from("/b/x.raf"), None)],
                false,
            )
            .unwrap();
        assert_eq!(
            index.signatures().unwrap(),
            vec![(PathBuf::from("/b/x.raf"), vec![9])]
        );
        index
            .carry(
                &[(
                    PathBuf::from("/b/x.raf"),
                    PathBuf::from("/c/x.raf"),
                    Some((100, t)),
                )],
                true,
            )
            .unwrap();
        let mut all: Vec<PathBuf> = index
            .photos(None)
            .unwrap()
            .into_iter()
            .map(|e| e.0)
            .collect();
        all.sort();
        assert_eq!(
            all,
            vec![PathBuf::from("/b/x.raf"), PathBuf::from("/c/x.raf")]
        );
        assert_eq!(
            index.palettes(Source::Camera).unwrap().len(),
            1,
            "une palette, deux chemins"
        );
        // Inconnu : ignoré.
        index
            .carry(
                &[(PathBuf::from("/z.raf"), PathBuf::from("/y.raf"), None)],
                false,
            )
            .unwrap();
        assert_eq!(index.photos(None).unwrap().len(), 2);
    }

    #[test]
    fn notes_are_kept_and_an_older_index_is_migrated() {
        use crate::xmp::{Color, Flag, Notes};
        let mut index = Index::in_memory().unwrap();
        let t = 1_700_000_000_000_000_000u128;
        let p = PathBuf::from("/a/x.raf");
        index
            .seen(Path::new("/a"), 1, &[(p.clone(), 1, t)])
            .unwrap();
        let notes = Notes {
            stars: 4,
            flag: Flag::Rejected,
            colors: vec![Color::Red, Color::Purple],
        };
        index.record_notes(&[(p.clone(), notes.clone())]).unwrap();
        assert_eq!(
            index.notes_in(Path::new("/a")).unwrap(),
            vec![(p.clone(), notes)]
        );
        // Une base du schéma 2 : complétée, pas refaite.
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE paths (path BLOB PRIMARY KEY, size INTEGER NOT NULL,
                 modified INTEGER NOT NULL, fingerprint BLOB);
             CREATE TABLE palettes (fingerprint BLOB NOT NULL, source TEXT NOT NULL,
                 stamp TEXT NOT NULL, palette BLOB NOT NULL, PRIMARY KEY (fingerprint, source));
             INSERT INTO paths VALUES (X'2F612F782E726166', 1, 1, NULL);",
        )
        .unwrap();
        db.pragma_update(None, "user_version", 2).unwrap();
        let mut old = Index::setup(db).unwrap();
        assert_eq!(old.photos(None).unwrap().len(), 1, "gardée");
        old.record_notes(&[(p.clone(), Notes::default())]).unwrap();
        assert!(old.signatures().unwrap().is_empty());
    }

    #[test]
    fn a_moved_folder_takes_its_paths_and_their_data_along() {
        let mut index = Index::in_memory().unwrap();
        let red = palette(0.5);
        index
            .record(&[
                entry("/old/x.raf", 1, Source::Camera, "s", &red),
                entry("/old/sub/y.raf", 2, Source::Camera, "s", &red),
                entry("/older/z.raf", 3, Source::Camera, "s", &red),
            ])
            .unwrap();
        let t = 1_700_000_000_000_000_000u128;
        index
            .record_signatures(&[(PathBuf::from("/old/x.raf"), 100, t, "s".into(), vec![9])])
            .unwrap();
        let samples = index.photos_in(Path::new("/old")).unwrap();
        assert_eq!(samples.len(), 1, "le dossier seul");
        assert_eq!(samples[0].fingerprint, Some([1; 16]));
        assert_eq!(
            index
                .relocate(Path::new("/old"), Path::new("/new"))
                .unwrap(),
            2
        );
        let mut all: Vec<PathBuf> = index
            .photos(None)
            .unwrap()
            .into_iter()
            .map(|e| e.0)
            .collect();
        all.sort();
        assert_eq!(
            all,
            vec![
                PathBuf::from("/new/sub/y.raf"),
                PathBuf::from("/new/x.raf"),
                PathBuf::from("/older/z.raf")
            ],
            "/older n'est pas sous /old"
        );
        assert_eq!(
            index.signatures().unwrap(),
            vec![(PathBuf::from("/new/x.raf"), vec![9])]
        );
        assert_eq!(
            index.palettes(Source::Camera).unwrap().len(),
            3,
            "palettes suivies"
        );
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
