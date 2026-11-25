//! Retrouver un dossier déplacé (#561, « Retrouver… ») : une photo de « Toutes les
//! photos » dont le dossier n'est plus là (déplacé sur un autre disque, renommé…). On
//! cherche un dossier qui contient **les mêmes photos**, on le vérifie, puis l'index, le
//! cache et les données de tri suivent (l'appelant, avec [`crate::index::Index::relocate`],
//! [`crate::cache::Cache::relocate`] et [`crate::store::relocate`]).
//!
//! - **Recherche** ([`find`]) : d'abord l'ancien chemin posé sur chaque racine donnée
//!   (disques, dossier personnel, racines surveillées), en partant de la fin
//!   (`…/2026-11-18_Sarah`, `…/2026/2026-11-18_Sarah`…) : un dossier déplacé garde souvent
//!   sa structure, et ces essais ne coûtent rien ; puis les dossiers du même nom jusqu'à
//!   `depth` niveaux sous chaque racine.
//! - **Vérification** ([`verify`]) : quelques photos témoins de l'ancien dossier (la
//!   première, celle du milieu, la dernière) doivent y être, à la même taille, et au même
//!   contenu quand l'index en connaît l'empreinte. Un dossier homonyme aux autres photos
//!   n'est pas pris.
//! - **Cartes d'appareil** ([`is_camera_card`]) : jamais indexées (des fichiers bruts,
//!   avant leur copie ailleurs).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Une photo témoin de l'ancien dossier : son nom, sa taille, son empreinte de contenu si
/// l'index l'a lue ([`crate::index::fingerprint`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sample {
    /// Nom du fichier.
    pub name: OsString,
    /// Taille (octets).
    pub size: u64,
    /// Empreinte du contenu.
    pub fingerprint: Option<[u8; 16]>,
}

/// Vrai si `path` est sur une carte d'appareil : il passe par un dossier `DCIM` (la norme
/// des cartes, DCF). Ce qui s'y trouve n'est pas indexé.
pub fn is_camera_card(path: &Path) -> bool {
    path.components()
        .any(|c| c.as_os_str().eq_ignore_ascii_case("DCIM"))
}

/// Vrai si le dossier injoignable `dir` est sur un disque débranché (le volume entier
/// manque : rien à retrouver, il suffit de le rebrancher), faux s'il manque sur un disque
/// présent (déplacé ou renommé). Le premier parent qui existe est un dossier de montage
/// (`/Volumes`, `/media/<utilisateur>`, `/run/media/<utilisateur>`, `/mnt`), ou rien
/// n'existe (une lettre de lecteur absente sous Windows).
pub fn unplugged(dir: &Path) -> bool {
    let Some(existing) = dir.ancestors().skip(1).find(|a| a.is_dir()) else {
        return true;
    };
    let mounts = ["/Volumes", "/media", "/run/media", "/mnt"];
    // Sous un dossier de montage, ou sous un dossier de montage d'un autre système
    // (`/Volumes` d'un index fait sur macOS, lu sous Linux).
    if mounts
        .iter()
        .any(|m| existing == Path::new(m) || (dir.starts_with(m) && !Path::new(m).is_dir()))
    {
        return true;
    }
    // `/media/<utilisateur>`, `/run/media/<utilisateur>` : un niveau de plus.
    existing
        .parent()
        .is_some_and(|p| p == Path::new("/media") || p == Path::new("/run/media"))
}

/// Les témoins à vérifier parmi `photos` : la première, celle du milieu et la dernière
/// (par nom), sans doublon.
pub fn pick_samples(mut photos: Vec<Sample>) -> Vec<Sample> {
    photos.sort_by(|a, b| a.name.cmp(&b.name));
    let n = photos.len();
    let mut picked: Vec<usize> = [0, n / 2, n.saturating_sub(1)]
        .into_iter()
        .filter(|&i| i < n)
        .collect();
    picked.dedup();
    picked.into_iter().map(|i| photos[i].clone()).collect()
}

/// Vrai si le dossier `candidate` contient les photos témoins `samples` : au moins les
/// deux tiers d'entre elles (une photo supprimée entre-temps n'empêche rien), au même
/// nom, à la même taille, et au même contenu quand l'empreinte est connue (`chunk` :
/// octets lus à chaque bout, comme l'index) ; une seule présente mais **différente**
/// suffit à refuser. Sans témoin : faux (rien pour en juger).
pub fn verify(candidate: &Path, samples: &[Sample], chunk: usize) -> bool {
    let mut found = 0;
    for s in samples {
        let path = candidate.join(&s.name);
        let Ok(meta) = std::fs::metadata(&path) else {
            continue;
        };
        if !meta.is_file() || meta.len() != s.size {
            return false;
        }
        if let Some(fp) = s.fingerprint
            && !crate::index::fingerprint(&path, chunk).is_ok_and(|(_, read)| read == fp)
        {
            return false;
        }
        found += 1;
    }
    !samples.is_empty() && found * 3 >= samples.len() * 2
}

/// Cherche l'ancien dossier `old` (ses témoins `samples`) sous les racines `roots`, les
/// plus probables d'abord, chaque candidat vérifié dès qu'il se présente ([`verify`]) :
/// l'ancien chemin posé sur chaque racine, de sa fin vers son début (un dossier déplacé
/// garde souvent sa structure) ; puis les dossiers du même nom jusqu'à `depth` niveaux
/// (dossiers cachés et cartes d'appareil écartés). `None` si aucun ; `stop` interrompt.
pub fn find(
    old: &Path,
    samples: &[Sample],
    roots: &[PathBuf],
    depth: usize,
    chunk: usize,
    stop: &AtomicBool,
) -> Option<PathBuf> {
    let name = old.file_name()?;
    let parts: Vec<&std::ffi::OsStr> = old
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s),
            _ => None,
        })
        .collect();
    let mut tried: Vec<PathBuf> = Vec::new();
    let mut good = |p: PathBuf| {
        if p == old || tried.contains(&p) || !p.is_dir() {
            return None;
        }
        tried.push(p.clone());
        verify(&p, samples, chunk).then_some(p)
    };
    // L'ancien chemin sur chaque racine, de sa fin vers son début.
    for take in 1..=parts.len() {
        for root in roots {
            let mut p = root.clone();
            for part in &parts[parts.len() - take..] {
                p.push(part);
            }
            if let Some(found) = good(p) {
                return Some(found);
            }
        }
    }
    // Puis les dossiers du même nom, en descendant.
    for root in roots {
        let walk = walkdir::WalkDir::new(root)
            .max_depth(depth.max(1))
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                e.depth() == 0
                    || (e.file_type().is_dir()
                        && !e.file_name().to_string_lossy().starts_with('.')
                        && !e.file_name().eq_ignore_ascii_case("DCIM"))
            });
        for entry in walk.flatten() {
            if stop.load(Ordering::Relaxed) {
                return None;
            }
            if entry.depth() > 0
                && entry.file_name() == name
                && let Some(found) = good(entry.into_path())
            {
                return Some(found);
            }
        }
    }
    None
}

/// Un dossier ouvert que l'index ne connaît pas encore (`dir`, ses photos `files`) est-il
/// un dossier connu devenu injoignable (renommé ou déplacé hors de `tri`) ? Cherche, parmi
/// les photos connues de même taille et de même nom, un ancien dossier qui n'existe plus
/// et dont les photos témoins sont toutes dans `dir` ([`verify`]) ; si oui, l'index, le
/// cache et les données de tri le suivent ([`relocate`]) et l'ancien dossier est rendu.
/// `data` : dossier de données (l'index y est), `chunk` : comme [`verify`].
pub fn recognise(
    data: &Path,
    cache: Option<&crate::cache::Cache>,
    dir: &Path,
    files: &[crate::media::MediaFile],
    chunk: usize,
) -> Option<PathBuf> {
    let db = data.join("index.sqlite");
    if !db.exists() || is_camera_card(dir) {
        return None;
    }
    let dir = std::path::absolute(dir).ok()?;
    let index = crate::index::Index::open(&db).ok()?;
    if !index.photos_in(&dir).ok()?.is_empty() {
        return None;
    }
    // Quelques photos du dossier ouvert, choisies par leur nom ; seule leur taille est
    // relevée (25/11/2026 : relever celle des 2820 photos d'un dossier sur un NAS, une
    // par une, retenait l'ouverture plusieurs minutes).
    let names: Vec<Sample> = files
        .iter()
        .filter(|f| !crate::media::is_development(&f.path))
        .filter_map(|f| {
            let path = std::path::absolute(&f.path).ok()?;
            if path.parent() != Some(dir.as_path()) {
                return None;
            }
            Some(Sample {
                name: path.file_name()?.to_os_string(),
                size: 0,
                fingerprint: None,
            })
        })
        .collect();
    let sample: Vec<(OsString, u64)> = pick_samples(names)
        .into_iter()
        .filter_map(|s| {
            let size = std::fs::metadata(dir.join(&s.name)).ok()?.len();
            Some((s.name, size))
        })
        .collect();
    let sizes: Vec<u64> = sample.iter().map(|s| s.1).collect();
    let mut olds: Vec<PathBuf> = index
        .with_sizes(&sizes)
        .ok()?
        .into_iter()
        .filter(|(p, size)| {
            p.file_name()
                .is_some_and(|n| sample.iter().any(|(m, s)| m == n && s == size))
        })
        .filter_map(|(p, _)| p.parent().map(Path::to_path_buf))
        .filter(|old| *old != dir)
        .collect();
    olds.sort();
    olds.dedup();
    if olds.is_empty() {
        return None;
    }
    let mut why = Vec::new();
    let old = olds.into_iter().find(|old| {
        if old.is_dir() {
            why.push(format!("{} still exists", old.display()));
            return false;
        }
        let samples: Vec<Sample> = index
            .photos_in(old)
            .unwrap_or_default()
            .into_iter()
            .filter(|s| !crate::developments::looks_like(&s.name))
            .collect();
        let ok = verify(&dir, &pick_samples(samples), chunk);
        if !ok {
            why.push(format!("{}: other photos", old.display()));
        }
        ok
    });
    let Some(old) = old else {
        crate::log::info(
            "index",
            &format!("{} not recognised", dir.display()),
            &why.join("; "),
        );
        return None;
    };
    crate::log::info(
        "index",
        &format!("{} recognised as {}", dir.display(), old.display()),
        "",
    );
    drop(index);
    if let Err(e) = relocate(Some(data), cache, &old, &dir) {
        crate::log::warning("index", &format!("folder not followed: {e}"), "");
    }
    Some(old)
}

/// Le dossier `from` retrouvé en `to` : l'index (`data/index.sqlite`), le cache et les
/// données de tri gardées sur la machine (`data`) suivent, sous-dossiers compris. Renvoie
/// le nombre de photos de l'index déplacées, ou la première erreur (le reste est quand
/// même fait).
pub fn relocate(
    data: Option<&Path>,
    cache: Option<&crate::cache::Cache>,
    from: &Path,
    to: &Path,
) -> Result<usize, String> {
    let mut moved = 0;
    let mut error = None;
    if let Some(data) = data {
        let db = data.join("index.sqlite");
        if db.exists() {
            match crate::index::Index::open(&db).and_then(|mut i| i.relocate(from, to)) {
                Ok(n) => moved = n,
                Err(e) => error = Some(e.to_string()),
            }
        }
        crate::store::relocate(data, from, to);
    }
    if let Some(cache) = cache
        && let Err(e) = cache.relocate(from, to)
    {
        error.get_or_insert(e.to_string());
    }
    match error {
        Some(e) => Err(e),
        None => Ok(moved),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn sample(name: &str, bytes: &[u8]) -> Sample {
        Sample {
            name: name.into(),
            size: bytes.len() as u64,
            fingerprint: None,
        }
    }

    #[test]
    fn development_names_are_told_apart() {
        use crate::developments::looks_like;
        assert!(looks_like("DSCF0001@dt.RAF".as_ref()));
        assert!(looks_like("DSCF0001@art-02.RAF".as_ref()));
        assert!(!looks_like("DSCF0001.RAF".as_ref()));
        assert!(!looks_like("moi@home.jpg".as_ref()));
    }

    #[test]
    fn cards_are_recognised_by_their_dcim_folder() {
        assert!(is_camera_card(Path::new(
            "/Volumes/SD/DCIM/100_FUJI/DSCF0001.RAF"
        )));
        assert!(is_camera_card(Path::new("/media/u/card/dcim")));
        assert!(!is_camera_card(Path::new(
            "/Volumes/Photos/2026/DSCF0001.RAF"
        )));
    }

    #[test]
    fn a_moved_folder_is_found_with_its_structure_then_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (b"photo a".as_slice(), b"photo bb".as_slice());
        // Déplacé d'un disque à l'autre, même structure sous la racine.
        let old = dir.path().join("disque1/Photos/2026/2026-11-18_Sarah");
        let new = dir.path().join("disque2/Photos/2026/2026-11-18_Sarah");
        write(&new.join("A.RAF"), a);
        write(&new.join("B.RAF"), b);
        // Un homonyme aux autres photos, plus haut : pas pris.
        write(&dir.path().join("disque2/2026-11-18_Sarah/A.RAF"), b"autre");
        let samples = vec![sample("A.RAF", a), sample("B.RAF", b)];
        let roots = vec![dir.path().join("disque2")];
        let stop = AtomicBool::new(false);
        let found = find(&old, &samples, &roots, 1, 1024, &stop);
        assert_eq!(found.as_deref(), Some(new.as_path()));
        // Rangé ailleurs, autre structure : trouvé par son nom en descendant.
        let elsewhere = dir.path().join("disque3/Archives/Sarah/2026-11-18_Sarah");
        write(&elsewhere.join("A.RAF"), a);
        write(&elsewhere.join("B.RAF"), b);
        let roots = vec![dir.path().join("disque3")];
        assert_eq!(
            find(&old, &samples, &roots, 1, 1024, &stop),
            None,
            "trop profond"
        );
        assert_eq!(
            find(&old, &samples, &roots, 3, 1024, &stop).as_deref(),
            Some(elsewhere.as_path())
        );
    }

    #[test]
    fn the_fingerprint_settles_same_size_lookalikes() {
        let dir = tempfile::tempdir().unwrap();
        let candidate = dir.path().join("x");
        write(&candidate.join("A.RAF"), b"abcd");
        let mut s = sample("A.RAF", b"abcd");
        s.fingerprint = Some(
            crate::index::fingerprint(&candidate.join("A.RAF"), 1024)
                .unwrap()
                .1,
        );
        assert!(verify(&candidate, std::slice::from_ref(&s), 1024));
        // Même taille, autre contenu.
        write(&candidate.join("A.RAF"), b"abce");
        assert!(!verify(&candidate, std::slice::from_ref(&s), 1024));
        // Deux témoins sur trois suffisent (une photo supprimée avant le renommage).
        write(&candidate.join("A.RAF"), b"abcd");
        write(&candidate.join("B.RAF"), b"bb");
        let three = [s.clone(), sample("B.RAF", b"bb"), sample("C.RAF", b"cc")];
        assert!(verify(&candidate, &three, 1024));
        let two_missing = [s, sample("D.RAF", b"dd"), sample("C.RAF", b"cc")];
        assert!(!verify(&candidate, &two_missing, 1024));
        assert!(
            !verify(&candidate, &[], 1024),
            "sans témoin, rien n'est pris"
        );
    }

    #[test]
    #[cfg(unix)]
    fn an_unplugged_drive_is_told_from_a_moved_folder() {
        assert!(unplugged(Path::new("/Volumes/Disque absent/Photos/2026")));
        let dir = tempfile::tempdir().unwrap();
        assert!(
            !unplugged(&dir.path().join("déplacé/2026")),
            "le disque est là"
        );
    }

    #[test]
    fn relocating_moves_the_index_entries() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        let t = 1_700_000_000_000_000_000u128;
        {
            let mut index = crate::index::Index::open(&data.join("index.sqlite")).unwrap();
            let seen = [(PathBuf::from("/old/a.raf"), 1, t)];
            index.seen(Path::new("/old"), 1, &seen).unwrap();
        }
        let moved = relocate(Some(&data), None, Path::new("/old"), Path::new("/new"));
        assert_eq!(moved, Ok(1));
        let index = crate::index::Index::open(&data.join("index.sqlite")).unwrap();
        let photos: Vec<PathBuf> = index
            .photos(None)
            .unwrap()
            .into_iter()
            .map(|e| e.0)
            .collect();
        assert_eq!(photos, vec![PathBuf::from("/new/a.raf")]);
    }

    #[test]
    fn a_renamed_folder_is_recognised_when_opened() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        let old = dir.path().join("TEST3");
        let new = dir.path().join("TEST4");
        write(&old.join("A.RAF"), b"photo a");
        write(&old.join("B.RAF"), b"photo bb");
        let files = |d: &Path| crate::media::scan(d, 1).unwrap();
        {
            let mut index = crate::index::Index::open(&data.join("index.sqlite")).unwrap();
            let seen: Vec<(PathBuf, u64, u128)> = files(&old)
                .iter()
                .map(|f| (f.path.clone(), std::fs::metadata(&f.path).unwrap().len(), 1))
                .collect();
            index.seen(&old, 1, &seen).unwrap();
        }
        // Tant que l'ancien existe : rien (une copie n'est pas un renommage).
        std::fs::create_dir_all(&new).unwrap();
        for name in ["A.RAF", "B.RAF"] {
            std::fs::copy(old.join(name), new.join(name)).unwrap();
        }
        assert_eq!(recognise(&data, None, &new, &files(&new), 1024), None);
        std::fs::remove_dir_all(&old).unwrap();
        assert_eq!(
            recognise(&data, None, &new, &files(&new), 1024),
            Some(old.clone())
        );
        let index = crate::index::Index::open(&data.join("index.sqlite")).unwrap();
        assert_eq!(index.photos_in(&new).unwrap().len(), 2, "l'index a suivi");
        assert_eq!(
            recognise(&data, None, &new, &files(&new), 1024),
            None,
            "déjà connu"
        );
    }

    #[test]
    fn samples_are_the_first_the_middle_and_the_last() {
        let names = ["e", "a", "c", "b", "d"];
        let picked: Vec<String> = pick_samples(names.iter().map(|n| sample(n, b"x")).collect())
            .into_iter()
            .map(|s| s.name.to_string_lossy().into_owned())
            .collect();
        assert_eq!(picked, ["a", "c", "e"]);
        assert_eq!(pick_samples(vec![sample("a", b"x")]).len(), 1);
    }
}
