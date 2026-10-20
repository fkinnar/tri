use std::env;
use std::path::PathBuf;

/// Ce qu'il faut pour compiler le shim C et lier LibRaw.
struct LibRaw {
    include_paths: Vec<PathBuf>,
    link_paths: Vec<PathBuf>,
    libs: Vec<String>,
}

/// Linux / macOS : pkg-config.
fn probe_pkg_config() -> LibRaw {
    let lib = pkg_config::Config::new()
        .cargo_metadata(false) // on émet nous-mêmes les directives de lien
        .probe("libraw")
        .expect("libraw introuvable via pkg-config (ou définir LIBRAW_DIR)");
    LibRaw {
        include_paths: lib.include_paths,
        link_paths: lib.link_paths,
        libs: lib.libs,
    }
}

/// Windows (ou installation manuelle) : LIBRAW_DIR contient include/ et lib/
/// (ex. vcpkg/installed/x64-windows). Nom de la lib surchargeable par LIBRAW_LIB.
fn probe_dir(dir: PathBuf) -> LibRaw {
    let mut libs = vec![env::var("LIBRAW_LIB").unwrap_or_else(|_| "raw".into())];
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        libs.push("stdc++".into());
    }
    LibRaw {
        include_paths: vec![dir.join("include")],
        link_paths: vec![dir.join("lib")],
        libs,
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=LIBRAW_DIR");
    println!("cargo:rerun-if-env-changed=LIBRAW_LIB");
    println!("cargo:rerun-if-changed=csrc/libraw_shim.c");
    println!("cargo:rerun-if-env-changed=TRI_CHECK_ONLY");

    // Vérification du code Rust pour une autre plateforme (`cargo check --target
    // x86_64-pc-windows-msvc` depuis macOS ou Linux, voir doc/en/build.md) : ni shim C (il
    // faudrait le compilateur de la cible) ni LibRaw ; `cargo check` ne lie rien.
    if env::var_os("TRI_CHECK_ONLY").is_some() {
        return;
    }

    let libraw = match env::var_os("LIBRAW_DIR") {
        Some(dir) => probe_dir(dir.into()),
        None => probe_pkg_config(),
    };

    // Le shim d'abord : avec ld (Linux), une lib statique doit précéder celles dont elle dépend.
    cc::Build::new()
        .file("csrc/libraw_shim.c")
        .includes(&libraw.include_paths)
        .compile("tri_libraw_shim");

    let macos = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos");
    for path in &libraw.link_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
    }
    for name in &libraw.libs {
        // macOS : libc++ à la place de libstdc++
        let name = if macos && name == "stdc++" {
            "c++"
        } else {
            name.as_str()
        };
        println!("cargo:rustc-link-lib={name}");
    }
}
