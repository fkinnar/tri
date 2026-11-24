//! Index de recherche des palettes (#561) : rempli par les calculs, une photo déplacée
//! garde sa palette ; mesure sur 100 000 palettes synthétiques (`--ignored`).

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use tri::compute::Kind;
use tri::index::{Entry, Index, IndexSettings};
use tri::palette::{Coverage, Palette, Source, SourceKind, Swatch};
use tri::scheduler::CachedPreviews;

fn producer(cache: &tri::cache::Cache) -> CachedPreviews {
    CachedPreviews {
        cache: cache.clone(),
        settings: tri::preview::Settings::default(),
        developers: tri::develop::Developers::none().into(),
    }
}

fn compute_colours(dir: &Path, producer: &CachedPreviews) {
    let files = tri::media::scan(dir, 1).unwrap();
    let s = tri::compute::run(
        &files,
        Kind::Palette(SourceKind::Camera),
        producer,
        &AtomicBool::new(false),
        &|_, _| {},
    );
    assert_eq!(s.failed, 0, "{:?}", s.error);
}

/// Un seul test démarre le fil d'écriture (il est propre au processus).
#[test]
fn computed_palettes_are_indexed_and_a_moved_photo_keeps_its_own() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a"), dir.path().join("b"));
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    image::RgbImage::from_pixel(40, 30, image::Rgb([200, 40, 30]))
        .save(a.join("rouge.jpg"))
        .unwrap();
    image::RgbImage::from_pixel(40, 30, image::Rgb([30, 60, 200]))
        .save(a.join("bleu.jpg"))
        .unwrap();
    let cache = tri::cache::Cache::open(dir.path().join("cache")).unwrap();
    let producer = producer(&cache);
    let db = dir.path().join("data").join("index.sqlite");
    tri::index::start(
        &db,
        cache.clone(),
        producer.settings,
        IndexSettings::default(),
    )
    .unwrap();
    compute_colours(&a, &producer);
    tri::index::flush(Duration::from_secs(30));
    let index = Index::open(&db).unwrap();
    let found = index.palettes(Source::Camera).unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(index.counts().unwrap(), (2, 2));
    // Déjà là : recalculer ne note rien de neuf.
    compute_colours(&a, &producer);
    tri::index::flush(Duration::from_secs(30));
    assert_eq!(index.counts().unwrap(), (2, 2));
    // Déplacée ailleurs (hors de tri) : revue là-bas, même contenu, même palette ; un
    // chemin de plus (l'ancien ne mène plus à rien, vu à l'affichage d'un résultat).
    std::fs::rename(a.join("rouge.jpg"), b.join("rouge.jpg")).unwrap();
    compute_colours(&b, &producer);
    tri::index::flush(Duration::from_secs(30));
    let found = index.palettes(Source::Camera).unwrap();
    assert_eq!(found.len(), 2, "toujours deux contenus");
    let red = found
        .iter()
        .find(|p| p.paths.iter().any(|(p, _, _)| p.ends_with("b/rouge.jpg")))
        .expect("nouveau chemin noté");
    assert_eq!(red.paths.len(), 2, "{:?}", red.paths);
    // Dossiers ouverts (#561, « Toutes les photos ») : chaque photo vue est notée ; l'ancien
    // chemin de la photo déplacée, absent de `a`, est oublié.
    tri::index::seen(&a, 1, &tri::media::scan(&a, 1).unwrap());
    tri::index::seen(&b, 1, &tri::media::scan(&b, 1).unwrap());
    tri::index::flush(Duration::from_secs(30));
    let mut photos: Vec<String> = index
        .photos(Some(dir.path()))
        .unwrap()
        .into_iter()
        .map(|(p, _, _)| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    photos.sort();
    assert_eq!(photos, ["bleu.jpg", "rouge.jpg"]);
    let red = index
        .palettes(Source::Camera)
        .unwrap()
        .into_iter()
        .find(|p| p.paths.iter().any(|(p, _, _)| p.ends_with("b/rouge.jpg")))
        .unwrap();
    assert_eq!(red.paths.len(), 1, "l'ancien chemin oublié");
    // Empreintes de similitude calculées : notées aussi (« semblables » partout).
    let files = tri::media::scan(&b, 1).unwrap();
    tri::compute::run(
        &files,
        Kind::Signatures,
        &producer,
        &AtomicBool::new(false),
        &|_, _| {},
    );
    tri::index::flush(Duration::from_secs(30));
    let signatures = index.signatures().unwrap();
    assert!(
        signatures.iter().any(|(p, _)| p.ends_with("b/rouge.jpg")),
        "{signatures:?}"
    );
    // Les notes signalées : gardées (une photo absente, non : ses notes sont vides).
    let five = tri::xmp::Notes {
        stars: 5,
        ..Default::default()
    };
    tri::index::note_notes(vec![
        (b.join("rouge.jpg"), five.clone()),
        (b.join("absente.jpg"), five.clone()),
    ]);
    tri::index::flush(Duration::from_secs(30));
    let b_abs = std::path::absolute(&b).unwrap();
    assert_eq!(
        index.notes_in(&b_abs).unwrap(),
        vec![(b_abs.join("rouge.jpg"), five)]
    );
    // Le dossier renommé dans le système, puis rouvert (vide) : rien d'oublié.
    let renamed = dir.path().join("b renommé");
    std::fs::rename(&b, &renamed).unwrap();
    tri::index::seen(&b, 1, &[]);
    tri::index::flush(Duration::from_secs(30));
    assert!(
        index
            .photos(None)
            .unwrap()
            .iter()
            .any(|(p, _, _)| p.ends_with("b/rouge.jpg")),
        "gardée, pour être grisée puis retrouvée"
    );
    // Une photo déplacée par `tri` vers un dossier jamais ouvert : l'index la suit.
    let elsewhere = dir.path().join("jamais ouvert");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let plan = tri::files::plan_transfer(
        &[a.join("bleu.jpg")],
        &elsewhere,
        tri::files::TransferMode::Move,
        &tri::files::FileSettings::default(),
    )
    .unwrap();
    tri::files::transfer(&plan, |_, _| {}).unwrap();
    tri::index::flush(Duration::from_secs(30));
    let paths: Vec<std::path::PathBuf> = index
        .photos(None)
        .unwrap()
        .into_iter()
        .map(|e| e.0)
        .collect();
    assert!(
        paths.iter().any(|p| p.ends_with("jamais ouvert/bleu.jpg"))
            && !paths.iter().any(|p| p.ends_with("a/bleu.jpg")),
        "{paths:?}"
    );
}

/// Petit générateur pseudo-aléatoire (reproductible, sans dépendance).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Palette synthétique proche d'une vraie : 6 couleurs, couverture de 96 × 96 points
/// autour d'elles (un peu de bruit, comme une photo).
fn synthetic(rng: &mut Lcg) -> Palette {
    let centres: Vec<[f32; 3]> = (0..6)
        .map(|_| {
            [
                0.2 + 0.7 * rng.next(),
                (rng.next() - 0.5) * 0.4,
                (rng.next() - 0.5) * 0.4,
            ]
        })
        .collect();
    let samples: Vec<[f32; 3]> = (0..96 * 96)
        .map(|i| {
            let c = centres[i % 6];
            let n = |rng: &mut Lcg, s: f32| (rng.next() - 0.5) * s;
            [
                c[0] + n(rng, 0.15),
                c[1] + n(rng, 0.06),
                c[2] + n(rng, 0.06),
            ]
        })
        .collect();
    Palette {
        swatches: centres
            .iter()
            .map(|&oklab| Swatch {
                oklab,
                share: 1.0 / 6.0,
            })
            .collect(),
        coverage: Coverage::from_samples(&samples),
        perceived: None,
    }
}

/// Mesure (#561, étape 1) : écrire, relire et parcourir 100 000 palettes.
/// `cargo test --release --test index -- --ignored --nocapture`
#[test]
#[ignore]
fn measure_one_hundred_thousand_palettes() {
    const N: usize = 100_000;
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("index.sqlite");
    let mut index = Index::open(&db).unwrap();
    let mut rng = Lcg(42);
    let start = Instant::now();
    let mut bytes = 0usize;
    let mut batch = Vec::new();
    for i in 0..N {
        let palette = synthetic(&mut rng).to_bytes();
        bytes += palette.len();
        let mut fingerprint = [0u8; 16];
        fingerprint[..8].copy_from_slice(&(i as u64).to_le_bytes());
        batch.push(Entry {
            path: format!("/Volumes/Photos/{:04}/DSCF{:04}.RAF", i / 500, i % 10_000).into(),
            size: 30_000_000 + i as u64,
            modified: 1_700_000_000_000_000_000 + i as u128,
            fingerprint,
            source: Source::Camera,
            stamp: "params".into(),
            palette,
        });
        if batch.len() == 256 {
            index.record(&batch).unwrap();
            batch.clear();
        }
    }
    index.record(&batch).unwrap();
    let written = start.elapsed();
    drop(index);
    let file = std::fs::metadata(&db).unwrap().len();
    let start = Instant::now();
    let index = Index::open(&db).unwrap();
    let all = index.palettes(Source::Camera).unwrap();
    let loaded = start.elapsed();
    assert_eq!(all.len(), N);
    let reference = synthetic(&mut rng);
    let start = Instant::now();
    let close = all
        .iter()
        .filter(|p| tri::palette::distance(&reference, &p.palette, 2.0) < 0.12)
        .count();
    let similar = start.elapsed();
    let colour = [0.6, 0.12, 0.05];
    let start = Instant::now();
    let with_colour = all
        .iter()
        .filter(|p| p.palette.coverage.share_within(colour, 0.1, 2.0) > 0.005)
        .count();
    let colour_search = start.elapsed();
    println!(
        "{N} palettes : {:.0} o en moyenne ({:.0} Mo de palettes), base {:.0} Mo",
        bytes as f64 / N as f64,
        bytes as f64 / 1e6,
        file as f64 / 1e6
    );
    println!("écriture (lots de 256) : {written:.2?}");
    println!("lecture de tout : {loaded:.2?}");
    println!("palettes proches : {similar:.2?} ({close} trouvées)");
    println!("contient cette couleur : {colour_search:.2?} ({with_colour} trouvées)");
}
