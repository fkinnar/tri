//! Tests de la ligne de commande : le vrai binaire `tri`, lancé sur des dossiers
//! temporaires. Chaque lancement a sa configuration (cache, données, langue anglaise) et
//! un dossier personnel temporaire : rien n'est lu ni écrit chez l'utilisateur, et rien
//! ne part à la corbeille (`--permanent`). Restent hors tests : `--reveal`, `--edit`,
//! `--eject` (le système) et `--check-update` (le réseau).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tri::xmp::{Convention, Notes};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/exif.jpg");

/// Un environnement isolé : dossier personnel, cache, données et configuration
/// temporaires.
struct Env {
    root: tempfile::TempDir,
}

impl Env {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let r = root.path();
        for d in ["home", "cache", "data", "photos"] {
            fs::create_dir(r.join(d)).unwrap();
        }
        // Chaînes littérales TOML : les `\` des chemins Windows restent tels quels.
        let config = format!(
            "[cache]\ndir = '{}'\n\n[data]\ndir = '{}'\n\n[i18n]\nlanguage = \"en\"\n",
            r.join("cache").display(),
            r.join("data").display()
        );
        fs::write(r.join("config.toml"), config).unwrap();
        Env { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }

    /// Dossier de photos du test.
    fn photos(&self) -> PathBuf {
        self.path("photos")
    }

    /// Copie la petite image de test sous `name` dans `dir`.
    fn image(&self, dir: &Path, name: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(name);
        fs::copy(FIXTURE, &path).unwrap();
        path
    }

    /// Lance `tri` avec la configuration du test.
    fn run<I, S>(&self, args: I) -> Output
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let home = self.path("home");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_tri"));
        cmd.arg("--config").arg(self.path("config.toml")).args(args);
        for var in [
            "HOME",
            "USERPROFILE",
            "APPDATA",
            "LOCALAPPDATA",
            "XDG_CACHE_HOME",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
        ] {
            cmd.env(var, &home);
        }
        cmd.output().unwrap()
    }
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// Le lancement a réussi ; sinon le test échoue avec sa sortie.
fn ok(o: Output) -> Output {
    assert!(
        o.status.success(),
        "échec ({:?})\nstdout : {}\nstderr : {}",
        o.status,
        stdout(&o),
        stderr(&o)
    );
    o
}

fn failed(o: Output) -> Output {
    assert!(!o.status.success(), "aurait dû échouer : {}", stdout(&o));
    o
}

fn arg(p: &Path) -> &std::ffi::OsStr {
    p.as_os_str()
}

/// Somme des entrées d'un tableau d'occupation du cache (`--cache-usage`, fin de
/// `--previews`).
fn entries_in(text: &str) -> usize {
    text.lines()
        .filter_map(|l| l.split_whitespace().nth(1)?.parse::<usize>().ok())
        .sum()
}

/// Entrées du cache (tous niveaux), d'après `--cache-usage`.
fn cache_entries(env: &Env) -> usize {
    entries_in(&stdout(&ok(env.run(["--cache-usage"]))))
}

#[test]
fn bad_arguments_print_the_usage_and_fail() {
    let env = Env::new();
    let out = failed(env.run(["--nope"]));
    // Arguments lus avant la configuration : dans la langue du système.
    assert!(stderr(&out).contains("--nope"), "{}", stderr(&out));
    assert!(stderr(&out).contains("--clear-cache"), "mode d'emploi");

    let photos = env.photos();
    let out = failed(env.run([arg(&photos), "extra".as_ref()]));
    assert!(stderr(&out).contains("extra\n"), "{}", stderr(&out));

    let out = failed(env.run([arg(&photos), "--convert-xmp".as_ref(), "capture".as_ref()]));
    assert!(stderr(&out).contains("capture"), "{}", stderr(&out));

    let out = failed(env.run(["--depth", "two"]));
    assert!(!stderr(&out).is_empty());

    // Sans dossier ni commande : le mode d'emploi.
    let out = failed(env.run(Vec::<&str>::new()));
    assert!(stderr(&out).contains("--cache-usage"));
}

#[test]
fn a_folder_is_listed_by_kind_and_depth() {
    let env = Env::new();
    let photos = env.photos();
    env.image(&photos, "a.jpg");
    fs::write(photos.join("b.RAF"), b"not really a raw").unwrap();
    env.image(&photos.join("sub"), "c.jpg");

    let out = stdout(&ok(env.run([arg(&photos)])));
    assert!(out.contains("a.jpg") && out.contains("b.RAF"), "{out}");
    assert!(!out.contains("c.jpg"), "profondeur 1 par défaut : {out}");

    let out = stdout(&ok(env.run([
        arg(&photos),
        "--depth".as_ref(),
        "2".as_ref(),
    ])));
    assert!(out.contains("c.jpg"), "{out}");

    let out = stdout(&ok(env.run([arg(&photos), "--raw".as_ref()])));
    assert!(out.contains("b.RAF") && !out.contains("a.jpg"), "{out}");

    let out = stdout(&ok(env.run([arg(&photos), "--flat".as_ref()])));
    assert!(out.contains("a.jpg") && !out.contains("b.RAF"), "{out}");
}

#[test]
fn previews_fill_the_cache_and_a_folder_is_cleared_alone() {
    let env = Env::new();
    let (one, two) = (env.photos().join("one"), env.photos().join("two"));
    env.image(&one, "a.jpg");
    env.image(&two, "b.jpg");
    assert_eq!(cache_entries(&env), 0);

    let out = stdout(&ok(env.run([arg(&one), "--previews".as_ref()])));
    assert!(out.contains("1 images (0 RAW)"), "{out}");
    let after_one = cache_entries(&env);
    assert!(after_one > 0, "aperçus en cache");
    // Le bilan final vient après les analyses de fond (netteté, empreintes) : il
    // compte tout ce qui est en cache.
    assert_eq!(entries_in(&out), after_one, "{out}");
    ok(env.run([arg(&two), "--previews".as_ref(), "--mixed".as_ref()]));
    let both = cache_entries(&env);
    assert_eq!(both, 2 * after_one, "autant d'entrées par image");

    // Un niveau seulement, puis tout le dossier : l'autre dossier ne bouge pas.
    let out = stdout(&ok(env.run([
        arg(&one),
        "--clear-cache".as_ref(),
        "thumbnail".as_ref(),
    ])));
    assert!(out.contains("cleared (Thumbnail): 1 entries"), "{out}");
    let out = stdout(&ok(env.run([arg(&one), "--clear-cache".as_ref()])));
    assert!(out.contains("cleared (all)"), "{out}");
    assert_eq!(cache_entries(&env), after_one, "le dossier two reste");

    // Avec `--depth`, les sous-dossiers aussi.
    let out = stdout(&ok(env.run([
        arg(&env.photos()),
        "--clear-cache".as_ref(),
        "--depth".as_ref(),
        "2".as_ref(),
    ])));
    assert!(out.contains(&format!("{after_one} entries")), "{out}");
    assert_eq!(cache_entries(&env), 0);

    let out = failed(env.run([arg(&env.path("missing")), "--clear-cache".as_ref()]));
    assert!(!stderr(&out).is_empty());
}

#[test]
fn the_whole_cache_or_one_level_is_cleared() {
    let env = Env::new();
    let photos = env.photos();
    env.image(&photos, "a.jpg");
    ok(env.run([arg(&photos), "--previews".as_ref()]));
    let all = cache_entries(&env);

    let out = stdout(&ok(env.run(["--clear-cache", "thumbnail"])));
    assert!(out.contains("cache cleared (Thumbnail)"), "{out}");
    assert_eq!(cache_entries(&env), all - 1);
    let out = stdout(&ok(env.run(["--clear-cache"])));
    assert!(out.contains("cache cleared (all)"), "{out}");
    assert_eq!(cache_entries(&env), 0);
}

#[test]
fn ratings_are_converted_to_the_other_convention() {
    let env = Env::new();
    let photos = env.photos();
    let image = env.image(&photos, "a.jpg");
    let notes = Notes {
        stars: 3,
        ..Notes::default()
    };
    let source = tri::xmp::write(&image, &notes, Convention::Darktable).unwrap();
    let before = fs::read_to_string(&source).unwrap();
    let adobe = photos.join("a.xmp");

    let out = stdout(&ok(env.run([
        arg(&photos),
        "--convert-xmp".as_ref(),
        "adobe".as_ref(),
        "--dry-run".as_ref(),
    ])));
    assert!(out.contains("a.xmp"), "{out}");
    assert!(!adobe.exists(), "--dry-run : rien écrit");

    ok(env.run([arg(&photos), "--convert-xmp".as_ref(), "adobe".as_ref()]));
    assert_eq!(tri::xmp::read_sidecar(&adobe).unwrap(), notes);
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        before,
        "source intacte"
    );
}

#[test]
fn an_image_is_renamed_with_its_sidecar() {
    let env = Env::new();
    let photos = env.photos();
    let image = env.image(&photos, "a.jpg");
    let notes = Notes {
        stars: 2,
        ..Notes::default()
    };
    tri::xmp::write(&image, &notes, Convention::Darktable).unwrap();

    let out = stdout(&ok(env.run([
        "--rename".as_ref(),
        arg(&image),
        "b".as_ref(),
        "--dry-run".as_ref(),
    ])));
    assert!(
        out.contains("b.jpg") && out.contains("nothing was changed"),
        "{out}"
    );
    assert!(image.exists());

    ok(env.run(["--rename".as_ref(), arg(&image), "b".as_ref()]));
    assert!(!image.exists());
    assert_eq!(tri::xmp::read(&photos.join("b.jpg")).unwrap(), notes);

    // Il faut un nom, et un seul.
    failed(env.run(["--rename".as_ref(), arg(&photos.join("b.jpg"))]));
}

#[test]
fn images_are_copied_moved_and_deleted() {
    let env = Env::new();
    let photos = env.photos();
    let (a, b) = (env.image(&photos, "a.jpg"), env.image(&photos, "b.jpg"));
    let dest = env.path("dest");
    fs::create_dir(&dest).unwrap();

    ok(env.run([
        "--copy".as_ref(),
        arg(&a),
        arg(&b),
        "--to".as_ref(),
        arg(&dest),
        "--dry-run".as_ref(),
    ]));
    assert!(!dest.join("a.jpg").exists(), "--dry-run");
    ok(env.run(["--copy".as_ref(), arg(&a), "--to".as_ref(), arg(&dest)]));
    assert!(a.exists() && dest.join("a.jpg").exists());
    ok(env.run(["--move".as_ref(), arg(&b), "--to".as_ref(), arg(&dest)]));
    assert!(!b.exists() && dest.join("b.jpg").exists());
    // Destination requise.
    failed(env.run(["--copy".as_ref(), arg(&a)]));

    // Un dossier entier, sous-dossiers compris.
    env.image(&photos.join("sub"), "c.jpg");
    ok(env.run(["--copy".as_ref(), arg(&photos), "--to".as_ref(), arg(&dest)]));
    assert!(dest.join("photos").join("sub").join("c.jpg").exists());

    // Suppression définitive seulement : jamais la corbeille de l'utilisateur.
    let out = stdout(&ok(env.run([
        "--delete".as_ref(),
        arg(&a),
        "--permanent".as_ref(),
        "--dry-run".as_ref(),
    ])));
    assert!(out.contains("deleted:"), "{out}");
    assert!(a.exists());
    ok(env.run(["--delete".as_ref(), arg(&a), "--permanent".as_ref()]));
    assert!(!a.exists());
}

#[test]
fn a_card_is_offloaded_and_only_what_is_new_is_copied() {
    let env = Env::new();
    let card = env.path("card");
    env.image(&card.join("DCIM").join("100"), "a.jpg");
    env.image(&card.join("DCIM").join("100"), "b.jpg");
    fs::write(card.join("notes.txt"), "ignored").unwrap();
    let root = env.path("archive");
    let session = root.join("session");
    let offload = |extra: &[&str]| {
        let mut args: Vec<&std::ffi::OsStr> = vec![
            "--offload".as_ref(),
            arg(&card),
            "--to".as_ref(),
            arg(&root),
            "--folder".as_ref(),
            "session".as_ref(),
        ];
        args.extend(extra.iter().map(std::ffi::OsStr::new));
        env.run(args)
    };

    let out = stdout(&ok(offload(&["--dry-run"])));
    assert!(
        out.contains("2 images") && out.contains("1 other files ignored"),
        "{out}"
    );
    assert!(!session.exists(), "--dry-run");

    ok(offload(&[]));
    assert!(session.join("a.jpg").exists() && session.join("b.jpg").exists());
    assert!(
        card.join("DCIM").join("100").join("a.jpg").exists(),
        "carte intacte"
    );

    // Une seconde fois : déjà là, puis effacé de la carte sur demande explicite.
    let out = stdout(&ok(offload(&["--delete-copied"])));
    assert!(out.contains("2 images already in the destination"), "{out}");
    assert!(!card.join("DCIM").join("100").join("a.jpg").exists());
    assert!(session.join("a.jpg").exists());
}

#[test]
fn analyses_run_on_a_folder() {
    let env = Env::new();
    let photos = env.photos();
    env.image(&photos, "a.jpg");
    env.image(&photos, "b.jpg");

    let out = stdout(&ok(env.run([arg(&photos), "--metadata".as_ref()])));
    assert!(
        out.contains("== a.jpg") && out.contains("== b.jpg"),
        "{out}"
    );
    let out = stdout(&ok(env.run([arg(&photos), "--exposure".as_ref()])));
    assert!(out.contains("a.jpg\t8 bits"), "{out}");
    let out = stdout(&ok(env.run([arg(&photos), "--sharpness".as_ref()])));
    assert!(out.contains("2 images, median"), "{out}");
    let out = stdout(&ok(env.run([arg(&photos), "--groups".as_ref()])));
    assert!(out.contains("2 images,"), "{out}");
    let out = stdout(&ok(env.run([
        arg(&photos),
        "--groups".as_ref(),
        "--threshold".as_ref(),
        "1".as_ref(),
    ])));
    assert!(
        out.contains("[2] a.jpg b.jpg"),
        "identiques, une pile : {out}"
    );
}

#[test]
fn a_jpeg_is_exported_as_is() {
    let env = Env::new();
    let photos = env.photos();
    let image = env.image(&photos, "a.jpg");
    let out_dir = env.path("export");

    let out = failed(env.run(["--export".as_ref(), "nope".as_ref(), arg(&image)]));
    assert!(
        stderr(&out).contains("unknown export preset: nope"),
        "{}",
        stderr(&out)
    );

    let args = |extra: &'static str| {
        let mut a: Vec<&std::ffi::OsStr> = vec![
            "--export".as_ref(),
            "asis".as_ref(),
            arg(&image),
            "--to".as_ref(),
            arg(&out_dir),
            "--libraw".as_ref(),
        ];
        if !extra.is_empty() {
            a.push(extra.as_ref());
        }
        a
    };
    ok(env.run(args("--dry-run")));
    assert!(!out_dir.exists(), "--dry-run");
    let out = stdout(&ok(env.run(args(""))));
    assert!(out.contains("without recompression"), "{out}");
    assert_eq!(
        fs::read(out_dir.join("a.jpg")).unwrap(),
        fs::read(&image).unwrap(),
        "copie à l'identique"
    );
}
