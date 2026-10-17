#!/usr/bin/env bash
# Captures d'écran du manuel des testeurs (doc/manuel.md, doc/manual.md).
#
#   scripts/manual-screenshots.sh <dossier de photos> [--lang fr|en] [--out DOSSIER]
#                                 [--volume NOM | --demo-root DOSSIER] [--font FICHIER]
#                                 [--select N]
#
# - Les photos (et leurs sidecars) sont copiées sur un volume de démonstration : sur
#   macOS, une image disque montée comme une carte (« Photos » par défaut, --volume),
#   seul emplacement montré dans le panneau des dossiers (les autres volumes, le dossier
#   personnel et la racine sont exclus) ; ailleurs, ou avec --demo-root, dans
#   <demo-root>/<nom du dossier> (chemin neutre, sans nom d'utilisateur).
# - L'interface tourne avec sa propre configuration, son propre cache et un dossier de
#   données temporaire : ni les réglages, ni l'état d'affichage, ni le cache de
#   l'utilisateur ne sont touchés. Les photos d'origine ne sont jamais modifiées.
# - Captures : <out>/<lang>/{grid,view,rating,compare,help}.jpg (défaut :
#   doc/manual-images), numérotées pour grid et view.
#
# Prérequis : interface compilée (cargo build --release -p tri-gui), ImageMagick (magick).

set -euo pipefail

usage() {
    sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'
    exit 1
}

[ $# -ge 1 ] || usage
photos=$1
shift
lang=fr
out=doc/manual-images
demo_root=""
volume=Photos
font=""
select=0
while [ $# -gt 0 ]; do
    case $1 in
        --lang) lang=$2; shift 2 ;;
        --out) out=$2; shift 2 ;;
        --demo-root) demo_root=$2; volume=""; shift 2 ;;
        --volume) volume=$2; shift 2 ;;
        --font) font=$2; shift 2 ;;
        --select) select=$2; shift 2 ;;
        *) usage ;;
    esac
done

root=$(cd "$(dirname "$0")/.." && pwd)
bin="$root/target/release/tri-gui"
[ -x "$bin" ] || { echo "interface non compilée : cargo build --release -p tri-gui" >&2; exit 1; }
command -v magick >/dev/null || { echo "ImageMagick (magick) introuvable" >&2; exit 1; }
[ -d "$photos" ] || { echo "dossier introuvable : $photos" >&2; exit 1; }

if [ -z "$font" ]; then
    for f in /System/Library/Fonts/Supplemental/Arial\ Bold.ttf \
             /System/Library/Fonts/Supplemental/Arial.ttf \
             /usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf \
             /usr/share/fonts/TTF/DejaVuSans-Bold.ttf; do
        [ -f "$f" ] && { font=$f; break; }
    done
fi
[ -n "$font" ] || { echo "aucune police trouvée : --font FICHIER" >&2; exit 1; }

work=$(mktemp -d)
mounted=""
cleanup() {
    [ -n "$mounted" ] && hdiutil detach -quiet "$mounted" || true
    rm -rf "$work"
}
trap cleanup EXIT

exclude=""
if [ -n "$volume" ] && [ "$(uname)" = Darwin ]; then
    # Image disque montée dans /Volumes, comme une carte.
    [ -e "/Volumes/$volume" ] && { echo "/Volumes/$volume existe déjà : --volume AUTRE" >&2; exit 1; }
    size=$(( $(du -sm "$photos" | cut -f1) * 13 / 10 + 64 ))
    hdiutil create -quiet -size "${size}m" -fs APFS -volname "$volume" "$work/demo.dmg"
    hdiutil attach -quiet -nobrowse "$work/demo.dmg"
    mounted="/Volumes/$volume"
    demo="$mounted/$(basename "$photos")"
    cp -R "$photos" "$demo"
    # Seul ce volume dans le panneau des dossiers.
    for v in /Volumes/*; do
        [ "$v" = "$mounted" ] || exclude+="\"$(basename "$v")\", "
    done
    exclude+="\"$HOME\", \"/\""
else
    demo_root=${demo_root:-/tmp/tri-demo}
    demo="$demo_root/$(basename "$photos")"
    rm -rf "$demo"
    mkdir -p "$demo_root"
    cp -R "$photos" "$demo"
fi
cat > "$work/config.toml" <<EOF
[i18n]
language = "$lang"

[data]
dir = "$work/data"

[cache]
dir = "$work/cache"

[places]
exclude = [$exclude]

# Pas de jugement de netteté pour les captures : sur un dossier de photos variées (gros
# plans sur fond flou parmi des paysages), il marque à tort des photos nettes.
[sharpness]
background = false

[gui]
window = [1280.0, 800.0]
EOF

mkdir -p "$out/$lang"

# Une capture : nom, puis les étapes de TRI_GUI_ACTIONS (voir doc/gui.md).
shot() {
    local name=$1 actions=$2
    echo "capture : $lang/$name"
    TRI_GUI_SCREENSHOT="$out/$lang/$name.png" TRI_GUI_SELECT=$select \
        TRI_GUI_ACTIONS="$actions" "$bin" --config "$work/config.toml" "$demo" &
    local pid=$! n=0
    while kill -0 $pid 2>/dev/null; do
        sleep 1
        n=$((n + 1))
        if [ $n -gt 180 ]; then
            kill $pid
            echo "délai dépassé : $name" >&2
            exit 1
        fi
    done
}

# Pastilles numérotées : « x,y,n » en fractions de l'image (indépendant de la densité
# de l'écran).
annotate() {
    local file=$1
    shift
    local size w h
    w=$(magick identify -format %w "$file")
    h=$(magick identify -format %h "$file")
    size=$((w / 55))
    local args=()
    for spec in "$@"; do
        IFS=, read -r fx fy n <<< "$spec"
        local x y
        x=$(awk "BEGIN { printf \"%d\", $fx * $w }")
        y=$(awk "BEGIN { printf \"%d\", $fy * $h }")
        args+=(-fill '#ffaa28' -stroke black -strokewidth 2
               -draw "circle $x,$y $((x + size)),$y"
               -stroke none -fill black -font "$font" -pointsize $((size * 3 / 2))
               -gravity NorthWest -annotate "+$((x - size / 2))+$((y - size * 3 / 4))" "$n")
    done
    magick "$file" "${args[@]}" "$file"
}

# La grille en dernier : vignettes, aperçus et histogrammes sont alors dans le cache, la
# barre d'état n'affiche plus de progression.
shot view "core.view.open,wait:2500"
shot rating "rating_dialog,wait:500"
shot compare "core.select.visual,core.nav.next,core.nav.next,core.select.visual,core.view.compare,wait:2500"
shot help "core.app.help,wait:500"
shot grid "wait:1500"

# Pastilles à côté de ce qu'elles désignent, dans des zones vides (jamais sur un bouton) :
# 1 dossiers, 2 filtres, 3 grille, 4 bandeau de notation, 5 1:1 / flèches / avance auto,
# 6 histogramme et métadonnées.
annotate "$out/$lang/grid.png" \
    0.16,0.12,1 0.255,0.058,2 0.748,0.30,3 0.615,0.955,4 0.748,0.905,5 0.97,0.32,6
# 1 ce qui est affiché, 2 1:1 / flèches / avance auto, 3 histogramme et métadonnées.
annotate "$out/$lang/view.png" \
    0.42,0.03,1 0.615,0.955,2 0.97,0.32,3

# JPEG : quatre fois plus léger que le PNG pour des captures pleines de photos (le dépôt
# grossit à chaque nouvelle version du manuel).
for f in "$out/$lang"/*.png; do
    magick "$f" -quality 88 "${f%.png}.jpg"
    rm "$f"
done

echo "captures dans $out/$lang"
