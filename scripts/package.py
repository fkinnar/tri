#!/usr/bin/env python3
"""Paquets prêts à installer de tri (19/10/2026), pour que personne n'ait à compiler.

- macOS : `tri.app` (interface, ligne de commande dans `Contents/MacOS/tri`), avec ses
  bibliothèques dans `Contents/Frameworks`, signé « ad hoc », dans un `.dmg`.
- Linux : archive `.tar.gz` (`bin/`, `lib/`, `share/`) avec `install.sh` (dans `~/.local`
  par défaut, ou `--prefix`, `--uninstall`).
- Windows : installateur `.msi` (WiX 5 : `Program Files\\tri`, menu Démarrer) et zip
  « portable », avec les DLL de LibRaw (vcpkg) et le runtime Visual C++.

Binaires **sans HEIC** (brevets HEVC, `doc/en/licensing.md`), avec libjpeg-turbo. LibRaw et
ses dépendances sont **livrées à part** (`.dylib` / `.so`, liaison dynamique : la LGPL
permet de les remplacer), avec leurs licences et les notices des crates Rust
(`cargo about`). Voir `doc/en/packaging.md`.

Usage : package.py [--out DOSSIER] [--no-build] [--no-archive] [--sign IDENTITÉ]
"""
import argparse
import datetime
import os
import platform
import re
import shutil
import struct
import subprocess
import sys
import tarfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FEATURES = ["--no-default-features", "--features", "tri/turbojpeg,tri-gui/turbojpeg"]
BINARIES = ["tri-gui", "tri"]
# Linux : bibliothèques livrées avec tri (le reste vient du système). LibRaw change de
# numéro d'une distribution à l'autre (libraw.so.23, .so.25…) : c'est la raison d'être de
# l'archive ; ses dépendances moins répandues la suivent.
LINUX_BUNDLED = re.compile(r"^lib(raw|lcms2|jpeg|gomp)\b")
# Sources des bibliothèques LGPL livrées (obligation de la LGPL, voir licensing.md).
LGPL_SOURCES = {
    "libraw": "https://www.libraw.org/data/LibRaw-{version}.tar.gz",
}


def run(cmd, **kw):
    print("+", " ".join(str(c) for c in cmd), flush=True)
    return subprocess.run(cmd, check=True, **kw)


def output(cmd):
    return subprocess.run(cmd, check=True, capture_output=True, text=True).stdout


def version():
    text = (ROOT / "gui" / "Cargo.toml").read_text()
    return re.search(r'^version = "([^"]+)"', text, re.M).group(1)


def package_version():
    """Version écrite dans les noms des paquets et leurs métadonnées : celle de
    `Cargo.toml` ; pour un build de nuit (`TRI_BUILD_CHANNEL=nightly`, posé par la CI),
    suivie de `-nightly-AAAAMMJJ-<commit>`, sinon son `.dmg` portait le même nom que la
    version publiée (retour du 25/10/2026)."""
    ver = version()
    if os.environ.get("TRI_BUILD_CHANNEL") != "nightly":
        return ver
    commit = os.environ.get("GITHUB_SHA") or output(["git", "rev-parse", "HEAD"]).strip()
    day = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%d")
    return f"{ver}-nightly-{day}-{commit[:7]}"


def build(target_dir):
    """Compilation à part (`target/package`) : le binaire de développement (avec HEIC)
    n'est pas écrasé."""
    env = dict(os.environ)
    if sys.platform.startswith("linux"):
        # Les bibliothèques livrées sont cherchées dans ../lib, à côté du binaire. RPATH
        # et non RUNPATH (--disable-new-dtags) : le RUNPATH ne vaut que pour les
        # dépendances directes, et libjpeg / lcms2, dépendances de LibRaw, n'étaient pas
        # trouvées sur une distribution sans LibRaw (vérifié sur Debian 13 et Fedora 42).
        flags = env.get("RUSTFLAGS", "")
        env["RUSTFLAGS"] = (
            flags + " -C link-arg=-Wl,--disable-new-dtags,-rpath,$ORIGIN/../lib"
        ).strip()
    run(
        ["cargo", "build", "--release", "--workspace", "--target-dir", str(target_dir)]
        + FEATURES,
        cwd=ROOT,
        env=env,
    )
    return target_dir / "release"


def rust_notices(dest):
    """Notices des crates Rust compilés dans le binaire (licences, copyrights)."""
    if shutil.which("cargo-about") is None:
        sys.exit("cargo-about manquant : cargo install cargo-about --locked --features cli")
    run(
        ["cargo", "about", "generate", "--workspace", "-c", "about.toml", "-o", str(dest)]
        + FEATURES
        + ["scripts/about.hbs"],
        cwd=ROOT,
    )


def common_files(dest):
    for name in ["LICENSE", "README.md", "README.fr.md", "CHANGELOG.md",
                 "CHANGELOG.fr.md"]:
        shutil.copy2(ROOT / name, dest / name)


# --- macOS ------------------------------------------------------------------------------


def mac_deps(binary):
    """Bibliothèques non système dont dépend `binary` (chemins absolus)."""
    deps = []
    for line in output(["otool", "-L", str(binary)]).splitlines()[1:]:
        path = line.strip().split(" (")[0]
        if path.startswith(("/usr/lib/", "/System/")) or path.startswith("@"):
            continue
        deps.append(Path(path))
    return deps


def mac_collect(binaries):
    """Toutes les bibliothèques à livrer, dépendances des dépendances comprises."""
    found, todo = {}, list(binaries)
    while todo:
        for dep in mac_deps(todo.pop()):
            if dep.name not in found:
                found[dep.name] = dep
                todo.append(dep.resolve())
    return found


def mac_formula(lib):
    """Formule Homebrew d'une bibliothèque (`/opt/homebrew/opt/<formule>/lib/…`)."""
    parts = lib.parts
    for marker in ["Cellar", "opt"]:
        # Dernière occurrence : `/opt/homebrew/opt/<formule>` ou `…/Cellar/<formule>`.
        idx = [i for i, p in enumerate(parts) if p == marker]
        if idx and idx[-1] + 1 < len(parts) and parts[idx[-1] - 1] != "/":
            return parts[idx[-1] + 1]
    return None


def mac_icns(resources):
    """`tri.icns` depuis `gui/assets/icon-1024.png` (sips + iconutil)."""
    iconset = resources / "tri.iconset"
    iconset.mkdir()
    source = ROOT / "gui" / "assets" / "icon-1024.png"
    for size in [16, 32, 128, 256, 512]:
        for scale in [1, 2]:
            px = size * scale
            suffix = "@2x" if scale == 2 else ""
            out = iconset / f"icon_{size}x{size}{suffix}.png"
            run(["sips", "-z", str(px), str(px), str(source), "--out", str(out)],
                stdout=subprocess.DEVNULL)
    run(["iconutil", "-c", "icns", str(iconset), "-o", str(resources / "tri.icns")])
    shutil.rmtree(iconset)


INFO_PLIST = """<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>tri</string>
  <key>CFBundleDisplayName</key><string>tri</string>
  <key>CFBundleIdentifier</key><string>io.github.fkinnar.tri</string>
  <key>CFBundleExecutable</key><string>tri-gui</string>
  <key>CFBundleIconFile</key><string>tri</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>{short}</string>
  <key>CFBundleVersion</key><string>{full}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.photography</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>GPL-3.0-or-later</string>
</dict>
</plist>
"""


def package_macos(release, out, ver, archive, identity):
    arch = platform.machine()
    stage = out / f"tri-{ver}-macos-{arch}"
    shutil.rmtree(stage, ignore_errors=True)
    app = stage / "tri.app"
    macos, frameworks = app / "Contents" / "MacOS", app / "Contents" / "Frameworks"
    resources = app / "Contents" / "Resources"
    for d in [macos, frameworks, resources / "licenses"]:
        d.mkdir(parents=True)
    short = re.match(r"\d+\.\d+\.\d+", ver).group(0)
    (app / "Contents" / "Info.plist").write_text(INFO_PLIST.format(short=short, full=ver))
    binaries = []
    for name in BINARIES:
        shutil.copy2(release / name, macos / name)
        binaries.append(macos / name)
    mac_icns(resources)

    # Bibliothèques : copiées, puis chaque référence réécrite vers Frameworks.
    libs = mac_collect(binaries)
    for name, path in libs.items():
        dest = frameworks / name
        shutil.copy2(path.resolve(), dest)
        dest.chmod(0o755)
        run(["install_name_tool", "-id", f"@rpath/{name}", str(dest)])
    for target in binaries + [frameworks / n for n in libs]:
        for dep in mac_deps(target):
            if dep.name in libs:
                run(["install_name_tool", "-change", str(dep), f"@rpath/{dep.name}",
                     str(target)])
    for binary in binaries:
        run(["install_name_tool", "-add_rpath", "@executable_path/../Frameworks",
             str(binary)])
    for lib in libs:
        run(["install_name_tool", "-add_rpath", "@loader_path", str(frameworks / lib)])

    # Licences des bibliothèques livrées (fichiers de leur formule Homebrew).
    licenses = resources / "licenses"
    native = []
    for name, path in sorted(libs.items()):
        formula = mac_formula(path)
        if not formula:
            continue
        prefix = Path(output(["brew", "--prefix", formula]).strip())
        info = output(["brew", "list", "--versions", formula]).split()
        native.append((name, formula, info[-1] if len(info) > 1 else "?"))
        for f in prefix.iterdir():
            if re.match(r"(LICENSE|COPYING|COPYRIGHT|NOTICE)", f.name, re.I) and f.is_file():
                (licenses / formula).mkdir(exist_ok=True)
                shutil.copy2(f, licenses / formula / f.name)
    write_native_notice(licenses, native)
    rust_notices(licenses / "rust-crates.html")
    common_files(resources)

    # Signature (obligatoire sur Apple Silicon après install_name_tool) : « ad hoc » par
    # défaut, sans identité Apple, Gatekeeper demande alors une confirmation au premier
    # lancement ; avec un certificat « Developer ID Application » (--sign), runtime
    # renforcé et horodatage, prêt pour la notarisation (doc/en/packaging.md).
    sign = ["codesign", "--force", "--sign", identity]
    if identity != "-":
        sign += ["--options", "runtime", "--timestamp"]
    for lib in libs:
        run(sign + [str(frameworks / lib)])
    run(sign + ["--deep", str(app)])
    run(["codesign", "--verify", "--deep", "--strict", str(app)])

    common_files(stage)
    (stage / "Applications").symlink_to("/Applications")
    if archive:
        dmg = out / f"{stage.name}.dmg"
        dmg.unlink(missing_ok=True)
        run(["hdiutil", "create", "-volname", f"tri {ver}", "-srcfolder", str(stage),
             "-format", "UDZO", "-ov", str(dmg)])
        return dmg
    return stage


# --- Linux ------------------------------------------------------------------------------


def linux_deps(binary):
    """Bibliothèques résolues par le chargeur (`ldd`) : nom → chemin."""
    deps = {}
    for line in output(["ldd", str(binary)]).splitlines():
        m = re.match(r"\s*(\S+) => (/\S+)", line)
        if m:
            deps[m.group(1)] = Path(m.group(2))
    return deps


def linux_package_of(path):
    """Paquet Debian / Ubuntu qui fournit `path` (pour sa licence et sa version)."""
    try:
        pkg = output(["dpkg", "-S", str(path.resolve())]).split(":")[0]
        ver = output(["dpkg-query", "-W", "-f=${Version}", pkg]).strip()
        return pkg, ver
    except (subprocess.CalledProcessError, FileNotFoundError):
        return None, "?"


DESKTOP = """[Desktop Entry]
Type=Application
Name=tri
GenericName=RAW viewer
GenericName[fr]=Visionneuse RAW
Comment=Fast RAW viewer for culling photo shoots
Comment[fr]=Visionneuse RAW rapide pour trier une séance photo
Exec=tri-gui %f
Icon=tri
Terminal=false
Categories=Graphics;Photography;Viewer;
StartupWMClass=tri
"""

INSTALL_SH = r"""#!/bin/sh
# Installe tri dans ~/.local (ou --prefix DOSSIER) ; --uninstall le retire.
set -e
prefix="$HOME/.local"
action=install
while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) prefix="$2"; shift 2 ;;
    --uninstall) action=uninstall; shift ;;
    *) echo "usage : $0 [--prefix DOSSIER] [--uninstall]"; exit 1 ;;
  esac
done
here=$(cd "$(dirname "$0")" && pwd)
dest="$prefix/lib/tri"
if [ "$action" = uninstall ]; then
  rm -rf "$dest" "$prefix/bin/tri-gui" "$prefix/bin/tri" \
    "$prefix/share/applications/tri.desktop" \
    "$prefix/share/icons/hicolor/256x256/apps/tri.png"
  echo "tri retiré de $prefix"
  exit 0
fi
mkdir -p "$dest" "$prefix/bin" "$prefix/share/applications" \
  "$prefix/share/icons/hicolor/256x256/apps"
rm -rf "$dest"
cp -R "$here" "$dest"
ln -sf "$dest/bin/tri-gui" "$prefix/bin/tri-gui"
ln -sf "$dest/bin/tri" "$prefix/bin/tri"
sed "s|^Exec=tri-gui|Exec=$dest/bin/tri-gui|" "$here/share/applications/tri.desktop" \
  > "$prefix/share/applications/tri.desktop"
cp "$here/share/icons/hicolor/256x256/apps/tri.png" \
  "$prefix/share/icons/hicolor/256x256/apps/tri.png"
command -v update-desktop-database >/dev/null 2>&1 \
  && update-desktop-database "$prefix/share/applications" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 \
  && gtk-update-icon-cache -q -t "$prefix/share/icons/hicolor" || true
echo "tri installé dans $dest ($prefix/bin/tri-gui)"
"""


def package_linux(release, out, ver, archive):
    arch = platform.machine()
    stage = out / f"tri-{ver}-linux-{arch}"
    shutil.rmtree(stage, ignore_errors=True)
    bindir, libdir, licenses = stage / "bin", stage / "lib", stage / "licenses"
    apps = stage / "share" / "applications"
    icons = stage / "share" / "icons" / "hicolor" / "256x256" / "apps"
    for d in [bindir, libdir, licenses, apps, icons]:
        d.mkdir(parents=True)
    for name in BINARIES:
        shutil.copy2(release / name, bindir / name)
        dynamic = output(["readelf", "-d", str(bindir / name)])
        if not re.search(r"\(RPATH\).*\$ORIGIN/\.\./lib", dynamic):
            sys.exit(f"{name} : RPATH $ORIGIN/../lib absent (compilé sans package.py ?)")

    # Bibliothèques livrées : la liste LINUX_BUNDLED, trouvées dans les dépendances des
    # binaires et de LibRaw.
    deps = {}
    for name in BINARIES:
        deps.update(linux_deps(bindir / name))
    libs = {n: p for n, p in deps.items() if LINUX_BUNDLED.match(n)}
    native = []
    for name, path in sorted(libs.items()):
        shutil.copy2(path.resolve(), libdir / name)
        pkg, ver_pkg = linux_package_of(path)
        native.append((name, pkg or "?", ver_pkg))
        copyright = Path("/usr/share/doc") / (pkg or "") / "copyright"
        if pkg and copyright.is_file():
            (licenses / pkg).mkdir(exist_ok=True)
            shutil.copy2(copyright, licenses / pkg / "copyright")
    # Vérification : avec les bibliothèques livrées, plus rien de manquant.
    missing = [l for l in output(["ldd", str(bindir / "tri-gui")]).splitlines()
               if "not found" in l]
    if missing:
        sys.exit("bibliothèques introuvables : " + "; ".join(missing))
    write_native_notice(licenses, native)
    rust_notices(licenses / "rust-crates.html")

    (apps / "tri.desktop").write_text(DESKTOP)
    shutil.copy2(ROOT / "gui" / "assets" / "icon-256.png", icons / "tri.png")
    install = stage / "install.sh"
    install.write_text(INSTALL_SH)
    install.chmod(0o755)
    common_files(stage)
    if archive:
        tgz = out / f"{stage.name}.tar.gz"
        with tarfile.open(tgz, "w:gz") as tar:
            tar.add(stage, arcname=stage.name)
        return tgz
    return stage


# --- Windows ----------------------------------------------------------------------------

# Runtime Visual C++ : présent sur beaucoup de PC, pas sur tous ; livré à côté de
# l'exécutable (déploiement « app-local », permis par Microsoft pour ces fichiers).
VC_RUNTIME = re.compile(r"^(msvcp140|vcruntime140|vcomp140|concrt140)(_\w+)?\.dll$", re.I)
# Identifiant fixe du produit : les versions suivantes remplacent la précédente.
UPGRADE_CODE = "ee6387a7-4751-488b-9066-99b3b6e6e358"


def pe_imports(path):
    """DLL importées par un exécutable ou une DLL Windows (table d'import du PE), lues
    sans outil externe (`dumpbin` demande l'environnement de Visual Studio)."""
    data = path.read_bytes()
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    sections_count = struct.unpack_from("<H", data, pe + 6)[0]
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    optional = pe + 24
    magic = struct.unpack_from("<H", data, optional)[0]
    directories = optional + (112 if magic == 0x20B else 96)
    import_rva = struct.unpack_from("<I", data, directories + 8)[0]
    sections = []
    table = optional + optional_size
    for k in range(sections_count):
        size, rva, _, raw = struct.unpack_from("<IIII", data, table + 40 * k + 8)
        sections.append((rva, size, raw))

    def offset(rva):
        for start, size, raw in sections:
            if start <= rva < start + max(size, 1):
                return rva - start + raw
        raise ValueError(f"{path.name} : adresse {rva:#x} hors des sections")

    names, at = [], offset(import_rva) if import_rva else None
    while at is not None:
        name_rva = struct.unpack_from("<I", data, at + 12)[0]
        if name_rva == 0:
            break
        start = offset(name_rva)
        names.append(data[start:data.index(b"\0", start)].decode("ascii"))
        at += 20
    return names


def vc_redist_dirs():
    """Dossiers du runtime Visual C++ redistribuable de Visual Studio (vswhere)."""
    vswhere = Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / \
        "Microsoft Visual Studio" / "Installer" / "vswhere.exe"
    if not vswhere.is_file():
        return []
    found = output([str(vswhere), "-latest", "-products", "*", "-find",
                    r"VC\Redist\MSVC\*\x64\Microsoft.VC*.*"]).splitlines()
    return [Path(f) for f in found if Path(f).is_dir()]


def windows_collect(binaries, search):
    """DLL à livrer (nom → chemin), dépendances des dépendances comprises : celles des
    dossiers `search` (vcpkg, runtime Visual C++) ; les DLL du système restent. Arrêt si
    une DLL n'est trouvée nulle part."""
    system = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32"
    found, todo, missing = {}, list(binaries), []
    while todo:
        for name in pe_imports(todo.pop()):
            key = name.lower()
            if key in found or key.startswith(("api-ms-", "ext-ms-")):
                continue
            local = next((d / name for d in search if (d / name).is_file()), None)
            if local:
                found[key] = local
                todo.append(local)
            elif not (system / name).is_file():
                missing.append(name)
    if missing:
        sys.exit("DLL introuvables : " + ", ".join(sorted(set(missing))))
    return found


def windows_ico(dest):
    """`tri.ico` : l'icône 256 px en PNG dans un conteneur ICO (accepté depuis Vista)."""
    png = (ROOT / "gui" / "assets" / "icon-256.png").read_bytes()
    header = struct.pack("<HHH", 0, 1, 1)
    entry = struct.pack("<BBBBHHII", 0, 0, 0, 0, 1, 32, len(png), 6 + 16)
    dest.write_bytes(header + entry + png)


WXS = r"""<?xml version="1.0" encoding="utf-8"?>
<!-- Installateur de tri (scripts/package.py, doc/en/packaging.md). -->
<Wix xmlns="http://wixtoolset.org/schemas/v4/wxs">
  <Package Name="tri" Manufacturer="tri" Version="{msi_version}"
           UpgradeCode="{upgrade_code}" Scope="perMachine">
    <SummaryInformation Description="tri {version}" />
    <MajorUpgrade AllowSameVersionUpgrades="yes"
                  DowngradeErrorMessage="A newer version of tri is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <Icon Id="tri.ico" SourceFile="{stage}\tri.ico" />
    <Property Id="ARPPRODUCTICON" Value="tri.ico" />
    <Property Id="ARPURLINFOABOUT" Value="https://github.com/fkinnar/tri" />
    <StandardDirectory Id="ProgramFiles64Folder">
      <Directory Id="INSTALLFOLDER" Name="tri" />
    </StandardDirectory>
    <Feature Id="Main">
      <Files Directory="INSTALLFOLDER" Include="{stage}\**" />
      <Component Id="StartMenu" Directory="ProgramMenuFolder">
        <Shortcut Id="TriShortcut" Name="tri" Description="RAW viewer"
                  Target="[INSTALLFOLDER]tri-gui.exe" WorkingDirectory="INSTALLFOLDER"
                  Icon="tri.ico" />
        <RegistryValue Root="HKLM" Key="Software\tri" Name="StartMenu" Type="integer"
                       Value="1" KeyPath="yes" />
      </Component>
    </Feature>
  </Package>
</Wix>
"""


def package_windows(release, out, ver, archive):
    stage = out / f"tri-{ver}-windows-x64"
    shutil.rmtree(stage, ignore_errors=True)
    licenses = stage / "licenses"
    licenses.mkdir(parents=True)
    binaries = []
    for name in BINARIES:
        shutil.copy2(release / f"{name}.exe", stage / f"{name}.exe")
        binaries.append(stage / f"{name}.exe")

    # DLL : LibRaw et ses dépendances (vcpkg, LIBRAW_DIR\bin), runtime Visual C++.
    libraw = os.environ.get("LIBRAW_DIR")
    if not libraw:
        sys.exit("LIBRAW_DIR non défini (dossier vcpkg installed\\x64-windows)")
    vcpkg = Path(libraw)
    search = [vcpkg / "bin"] + vc_redist_dirs()
    dlls = windows_collect(binaries, search)
    for key, path in sorted(dlls.items()):
        shutil.copy2(path, stage / path.name)
    if not any(VC_RUNTIME.match(p.name) for p in dlls.values()):
        print("attention : runtime Visual C++ non livré (pas de dossier Redist trouvé)")

    # Licences des ports vcpkg installés (share\<port>\copyright), runtime compris.
    native = []
    for copyright in sorted((vcpkg / "share").glob("*/copyright")):
        port = copyright.parent.name
        (licenses / port).mkdir(exist_ok=True)
        shutil.copy2(copyright, licenses / port / "copyright")
        native.append((port, f"vcpkg {port}", windows_port_version(vcpkg, port)))
    write_native_notice(licenses, native)
    rust_notices(licenses / "rust-crates.html")
    common_files(stage)
    windows_ico(stage / "tri.ico")
    print("DLL livrées : " + ", ".join(p.name for p in dlls.values()))
    if not archive:
        return stage

    portable = out / f"{stage.name}.zip"
    with zipfile.ZipFile(portable, "w", zipfile.ZIP_DEFLATED) as z:
        for f in sorted(stage.rglob("*")):
            z.write(f, Path(stage.name) / f.relative_to(stage))
    # Installateur : WiX 5 (`dotnet tool install --global wix`), version MSI numérique
    # (1.0.0 pour 1.0.0-alpha.2 : AllowSameVersionUpgrades remplace quand même).
    wxs = out / "tri.wxs"
    msi_version = re.match(r"\d+\.\d+\.\d+", ver).group(0)
    wxs.write_text(WXS.format(msi_version=msi_version, version=ver, stage=stage,
                              upgrade_code=UPGRADE_CODE), encoding="utf-8")
    msi = out / f"{stage.name}.msi"
    if shutil.which("wix") is None:
        sys.exit("WiX manquant : dotnet tool install --global wix --version 5.0.2")
    run(["wix", "build", "-arch", "x64", "-o", str(msi), str(wxs)])
    return msi


def windows_port_version(vcpkg, port):
    """Version d'un port vcpkg, lue dans son `vcpkg.spdx.json`."""
    spdx = vcpkg / "share" / port / "vcpkg.spdx.json"
    if spdx.is_file():
        m = re.search(r'"versionInfo"\s*:\s*"([^"]+)"', spdx.read_text(encoding="utf-8"))
        if m:
            return m.group(1)
    return "?"


# --- Commun -----------------------------------------------------------------------------


def write_native_notice(licenses, native):
    """`licenses/README.md` : bibliothèques livrées, versions, et où trouver le source
    des LGPL (obligation de la LGPL : source de la version livrée)."""
    lines = [
        "# Bibliothèques livrées avec tri",
        "",
        "Liées dynamiquement : elles peuvent être remplacées par une autre version.",
        "Licences dans les sous-dossiers ; crates Rust dans `rust-crates.html`.",
        "",
        "| Fichier | Paquet | Version |",
        "|---|---|---|",
    ]
    lines += [f"| {n} | {p} | {v} |" for n, p, v in native]
    lines += ["", "## Sources (LGPL)", ""]
    for name, pkg, ver in native:
        for key, url in LGPL_SOURCES.items():
            if key in pkg.lower():
                upstream = re.match(r"(?:\d+:)?(\d+\.\d+\.\d+)", ver)
                v = upstream.group(1) if upstream else ver
                lines.append(f"- {pkg} {v} : {url.format(version=v)}")
    lines.append("")
    (licenses / "README.md").write_text("\n".join(lines), encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description="Paquets prêts à installer de tri.")
    parser.add_argument("--out", type=Path, default=ROOT / "target" / "dist")
    parser.add_argument("--no-build", action="store_true",
                        help="reprendre la dernière compilation de target/package")
    parser.add_argument("--no-archive", action="store_true",
                        help="dossier seulement (pas de .dmg / .tar.gz / .msi)")
    parser.add_argument("--sign", default="-", metavar="IDENTITÉ",
                        help="macOS : certificat de signature (défaut : ad hoc)")
    args = parser.parse_args()
    target_dir = ROOT / "target" / "package"
    release = target_dir / "release" if args.no_build else build(target_dir)
    args.out.mkdir(parents=True, exist_ok=True)
    ver = package_version()
    if sys.platform == "darwin":
        result = package_macos(release, args.out, ver, not args.no_archive, args.sign)
    elif sys.platform.startswith("linux"):
        result = package_linux(release, args.out, ver, not args.no_archive)
    elif sys.platform == "win32":
        result = package_windows(release, args.out, ver, not args.no_archive)
    else:
        sys.exit(f"système non pris en charge : {sys.platform}")
    print(f"paquet : {result}")


if __name__ == "__main__":
    main()
