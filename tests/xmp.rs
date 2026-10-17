//! Tests d'intégration du module xmp : choix du sidecar, création, réécriture, sécurité.

use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use tri::xmp::{self, Color, Convention, Flag, Notes, XmpError};

fn image(dir: &Path, name: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, b"image").unwrap();
    path
}

fn notes(stars: u8, flag: Flag, colors: &[Color]) -> Notes {
    Notes {
        stars,
        flag,
        colors: colors.to_vec(),
    }
}

fn set_mtime(path: &Path, secs_ago: u64) {
    let file = fs::File::options().write(true).open(path).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(secs_ago))
        .unwrap();
}

#[test]
fn without_sidecar_notes_are_empty_and_a_new_one_follows_the_convention() {
    let dir = tempfile::tempdir().unwrap();
    let raf = image(dir.path(), "DSCF0001.RAF");
    assert_eq!(xmp::read(&raf).unwrap(), Notes::default());

    let n = notes(2, Flag::Accepted, &[Color::Red]);
    let written = xmp::write(&raf, &n, Convention::Darktable).unwrap();
    assert_eq!(written, dir.path().join("DSCF0001.RAF.xmp"));
    assert_eq!(xmp::read(&raf).unwrap(), n);

    let other = image(dir.path(), "DSCF0002.RAF");
    let written = xmp::write(&other, &n, Convention::Adobe).unwrap();
    assert_eq!(written, dir.path().join("DSCF0002.xmp"));
    assert_eq!(xmp::read(&other).unwrap(), n);
}

#[test]
fn the_most_recent_sidecar_is_read_and_written() {
    let dir = tempfile::tempdir().unwrap();
    let raf = image(dir.path(), "DSCF0003.RAF");
    xmp::write(&raf, &notes(1, Flag::None, &[]), Convention::Darktable).unwrap();
    xmp::write(&raf, &notes(4, Flag::None, &[]), Convention::Adobe).unwrap();
    // Les deux existent maintenant ? Non : le second write a trouvé le premier, plus récent.
    assert!(!dir.path().join("DSCF0003.xmp").exists());

    // Deux sidecars, l'Adobe plus récent : c'est lui qui compte.
    fs::copy(
        dir.path().join("DSCF0003.RAF.xmp"),
        dir.path().join("DSCF0003.xmp"),
    )
    .unwrap();
    xmp::write(&raf, &notes(5, Flag::None, &[]), Convention::Darktable).unwrap();
    set_mtime(&dir.path().join("DSCF0003.RAF.xmp"), 3600);
    let adobe = dir.path().join("DSCF0003.xmp");
    let n = notes(2, Flag::Rejected, &[]);
    fs::write(
        &adobe,
        xmp::apply(&fs::read_to_string(&adobe).unwrap(), &n).unwrap(),
    )
    .unwrap();
    assert_eq!(xmp::read(&raf).unwrap(), n);
    assert_eq!(xmp::sidecar_path(&raf, Convention::Darktable), adobe);
}

#[test]
fn uppercase_extension_is_found() {
    let dir = tempfile::tempdir().unwrap();
    let raf = image(dir.path(), "DSCF0004.RAF");
    let upper = dir.path().join("DSCF0004.RAF.XMP");
    xmp::write(&raf, &notes(3, Flag::None, &[]), Convention::Darktable).unwrap();
    fs::rename(dir.path().join("DSCF0004.RAF.xmp"), &upper).unwrap();
    // Sur un système de fichiers insensible à la casse (macOS), les deux noms désignent le
    // même fichier : dans tous les cas, les notes sont lues.
    assert_eq!(xmp::read(&raf).unwrap().stars, 3);
    assert!(xmp::existing_sidecars(&raf).iter().any(|p| {
        p.to_string_lossy()
            .eq_ignore_ascii_case(&upper.to_string_lossy())
    }));
}

#[test]
fn unchanged_notes_do_not_touch_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let raf = image(dir.path(), "DSCF0005.RAF");
    let n = notes(3, Flag::None, &[Color::Green]);
    let path = xmp::write(&raf, &n, Convention::Darktable).unwrap();
    set_mtime(&path, 3600);
    let before = fs::metadata(&path).unwrap().modified().unwrap();
    xmp::write(&raf, &n, Convention::Darktable).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
}

#[test]
fn a_malformed_sidecar_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let raf = image(dir.path(), "DSCF0006.RAF");
    let sidecar = dir.path().join("DSCF0006.RAF.xmp");
    let broken = "<x:xmpmeta><rdf:RDF><rdf:Description xmp:Rating=\"2\"></x:xmpmeta>";
    fs::write(&sidecar, broken).unwrap();
    assert!(matches!(xmp::read(&raf), Err(XmpError::Malformed(_))));
    assert!(xmp::write(&raf, &notes(5, Flag::None, &[]), Convention::Darktable).is_err());
    assert_eq!(fs::read_to_string(&sidecar).unwrap(), broken, "intact");
}

#[test]
fn an_adobe_sidecar_is_shared_by_a_raw_and_its_jpeg() {
    let dir = tempfile::tempdir().unwrap();
    let raf = image(dir.path(), "DSCF0007.RAF");
    let jpg = image(dir.path(), "DSCF0007.JPG");
    let n = notes(4, Flag::Accepted, &[]);
    xmp::write(&raf, &n, Convention::Adobe).unwrap();
    assert_eq!(xmp::read(&jpg).unwrap(), n, "même IMAGE.xmp");
    // En convention darktable, chacun a le sien.
    let dir2 = tempfile::tempdir().unwrap();
    let raf2 = image(dir2.path(), "DSCF0008.RAF");
    let jpg2 = image(dir2.path(), "DSCF0008.JPG");
    xmp::write(&raf2, &n, Convention::Darktable).unwrap();
    assert_eq!(xmp::read(&jpg2).unwrap(), Notes::default());
}

#[test]
fn no_temporary_file_is_left_behind() {
    let dir = tempfile::tempdir().unwrap();
    let raf = image(dir.path(), "DSCF0009.RAF");
    xmp::write(&raf, &notes(1, Flag::None, &[]), Convention::Darktable).unwrap();
    let names: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
}

mod convert {
    use super::*;
    use tri::xmp::convert;

    #[test]
    fn darktable_to_adobe_copies_notes_and_keeps_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let raf = image(dir.path(), "IMG.RAF");
        let other = image(dir.path(), "NONE.RAF");
        let notes = Notes {
            stars: 4,
            flag: Flag::Accepted,
            colors: vec![Color::Blue],
        };
        let dt = tri::xmp::write(&raf, &notes, Convention::Darktable).unwrap();
        let before = fs::read_to_string(&dt).unwrap();

        let dry = convert([raf.as_path(), other.as_path()], Convention::Adobe, true);
        assert_eq!(dry.written, [dir.path().join("IMG.xmp")]);
        assert!(
            !dir.path().join("IMG.xmp").exists(),
            "simulation : rien écrit"
        );

        let report = convert([raf.as_path(), other.as_path()], Convention::Adobe, false);
        assert_eq!(report.written, [dir.path().join("IMG.xmp")]);
        assert_eq!(report.unchanged, 1, "image sans sidecar");
        assert_eq!(
            tri::xmp::read_sidecar(&dir.path().join("IMG.xmp")).unwrap(),
            notes
        );
        assert_eq!(fs::read_to_string(&dt).unwrap(), before, "source intacte");
        assert!(
            !dir.path().join("NONE.xmp").exists(),
            "pas de sidecar vide créé"
        );
    }

    #[test]
    fn adobe_pair_conflict_keeps_the_first_image() {
        let dir = tempfile::tempdir().unwrap();
        let raf = image(dir.path(), "IMG.RAF");
        let jpg = image(dir.path(), "IMG.JPG");
        tri::xmp::write(
            &raf,
            &Notes {
                stars: 5,
                ..Notes::default()
            },
            Convention::Darktable,
        )
        .unwrap();
        tri::xmp::write(
            &jpg,
            &Notes {
                stars: 2,
                ..Notes::default()
            },
            Convention::Darktable,
        )
        .unwrap();
        let report = convert([raf.as_path(), jpg.as_path()], Convention::Adobe, false);
        assert_eq!(report.conflicts, std::slice::from_ref(&jpg));
        assert_eq!(
            tri::xmp::read_sidecar(&dir.path().join("IMG.xmp"))
                .unwrap()
                .stars,
            5
        );
    }

    #[test]
    fn adobe_to_darktable_splits_a_shared_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let raf = image(dir.path(), "IMG.RAF");
        let jpg = image(dir.path(), "IMG.JPG");
        let notes = Notes {
            flag: Flag::Rejected,
            stars: 3,
            ..Notes::default()
        };
        tri::xmp::write(&raf, &notes, Convention::Adobe).unwrap();
        let report = convert([raf.as_path(), jpg.as_path()], Convention::Darktable, false);
        assert_eq!(report.written.len(), 2);
        assert_eq!(tri::xmp::read(&raf).unwrap(), notes);
        assert_eq!(tri::xmp::read(&jpg).unwrap(), notes);
    }

    #[test]
    fn up_to_date_target_becomes_the_most_recent() {
        let dir = tempfile::tempdir().unwrap();
        let raf = image(dir.path(), "IMG.RAF");
        let notes = Notes {
            stars: 1,
            ..Notes::default()
        };
        let adobe = tri::xmp::write(&raf, &notes, Convention::Adobe).unwrap();
        set_mtime(&adobe, 60);
        let dt = dir.path().join("IMG.RAF.xmp");
        tri::xmp::write_sidecar(&dt, &notes).unwrap();
        assert_eq!(tri::xmp::sidecar_path(&raf, Convention::Adobe), dt);

        let report = convert([raf.as_path()], Convention::Adobe, false);
        assert!(report.written.is_empty());
        assert_eq!(report.unchanged, 1);
        assert_eq!(tri::xmp::sidecar_path(&raf, Convention::Darktable), adobe);
    }

    #[test]
    fn malformed_source_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let raf = image(dir.path(), "IMG.RAF");
        fs::write(dir.path().join("IMG.RAF.xmp"), "<pas du xml").unwrap();
        let report = convert([raf.as_path()], Convention::Adobe, false);
        assert_eq!(report.errors.len(), 1);
        assert!(!dir.path().join("IMG.xmp").exists());
    }
}
