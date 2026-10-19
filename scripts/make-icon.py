#!/usr/bin/env python3
"""Icône de tri (choisie le 19/10/2026 parmi trois pistes) : un « t » minuscule épais,
blanc cassé, sur un carré arrondi gris anthracite, avec l'étoile jaune de la note ; et le
mot-symbole « t★ri » de la barre du haut, dans le même trait.

Dessinés en grand (suréchantillonnage ×4) puis réduits, pour des bords nets à toute
taille. Usage : make-icon.py <dossier> [tailles…] (défaut : 1024 256 64 32) ; écrit
icon-<taille>.png, et wordmark-letters.png / wordmark-star.png (même taille : les lettres
en blanc, teintées par l'interface selon le thème ; l'étoile jaune, posée telle quelle).
Demande Pillow (`pip install pillow`).
"""
import math
import sys

from PIL import Image, ImageDraw

BASE = 1024
BG_TOP, BG_BOTTOM = (40, 46, 54), (20, 22, 26)
WHITE = (240, 240, 236, 255)
STAR = (250, 200, 70, 255)


def background(big):
    """Carré arrondi (forme des icônes macOS), léger dégradé vertical."""
    grad = Image.new("RGBA", (big, big))
    d = ImageDraw.Draw(grad)
    for y in range(big):
        t = y / big
        col = tuple(int(BG_TOP[i] + (BG_BOTTOM[i] - BG_TOP[i]) * t) for i in range(3))
        d.line([(0, y), (big, y)], fill=col + (255,))
    mask = Image.new("L", (big, big), 0)
    m = int(big * 0.09)
    ImageDraw.Draw(mask).rounded_rectangle(
        [m, m, big - m, big - m], radius=int(big * 0.2), fill=255
    )
    out = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    out.paste(grad, (0, 0), mask)
    return out


def icon(size):
    big = BASE * 4
    img = background(big)
    d = ImageDraw.Draw(img)
    cx, sw, top, y1, hook = big * 0.44, big * 0.13, big * 0.18, big * 0.58, big * 0.13
    # Fût : haut arrondi, bas droit qui se raccorde au crochet.
    d.rounded_rectangle([cx - sw / 2, top, cx + sw / 2, top + sw * 2], radius=sw / 2, fill=WHITE)
    d.rectangle([cx - sw / 2, top + sw, cx + sw / 2, y1 + 1], fill=WHITE)
    # Barre.
    bar_y = big * 0.34
    d.rounded_rectangle(
        [cx - big * 0.19, bar_y, cx + big * 0.21, bar_y + sw * 0.8], radius=sw * 0.4, fill=WHITE
    )
    # Crochet : quart de cercle de centre (cx + hook, y1), puis un bout droit arrondi.
    ro = hook + sw / 2
    d.arc([cx + hook - ro, y1 - ro, cx + hook + ro, y1 + ro], start=90, end=180, fill=WHITE, width=int(sw))
    ex, ey = cx + hook, y1 + hook
    tail = big * 0.06
    d.rectangle([ex - 2, ey - sw / 2, ex + tail, ey + sw / 2], fill=WHITE)
    d.ellipse([ex + tail - sw / 2, ey - sw / 2, ex + tail + sw / 2, ey + sw / 2], fill=WHITE)
    # Étoile de la note.
    sx, sy, r1, r2 = big * 0.72, big * 0.27, big * 0.1, big * 0.042
    points = []
    for k in range(10):
        a = -math.pi / 2 + k * math.pi / 5
        r = r1 if k % 2 == 0 else r2
        points.append((sx + r * math.cos(a), sy + r * math.sin(a)))
    d.polygon(points, fill=STAR)
    return img.resize((size, size), Image.LANCZOS)


def star(d, sx, sy, r1, r2, fill):
    points = []
    for k in range(10):
        a = -math.pi / 2 + k * math.pi / 5
        r = r1 if k % 2 == 0 else r2
        points.append((sx + r * math.cos(a), sy + r * math.sin(a)))
    d.polygon(points, fill=fill)


def wordmark(height):
    """Mot-symbole « t★ri » : (lettres en blanc, étoile jaune), deux images de même
    taille, fond transparent."""
    big = height * 4.0
    sw = big * 0.17
    w, h = int(big * 2.4), int(big)
    letters = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    stars = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d, ds = ImageDraw.Draw(letters), ImageDraw.Draw(stars)
    white = (255, 255, 255, 255)
    base, xtop, gap = big - 2, big * 0.38, sw * 0.75
    # t : fût, barre, crochet qui finit sur la ligne de base.
    hook, cx = big * 0.17, sw * 1.15
    y1 = base - sw / 2 - hook
    d.rounded_rectangle([cx - sw / 2, 0, cx + sw / 2, sw * 2], radius=sw / 2, fill=white)
    d.rectangle([cx - sw / 2, sw, cx + sw / 2, y1 + 1], fill=white)
    d.rounded_rectangle([cx - sw * 1.05, xtop, cx + sw * 1.2, xtop + sw * 0.82], radius=sw * 0.41, fill=white)
    ro = hook + sw / 2
    d.arc([cx + hook - ro, y1 - ro, cx + hook + ro, y1 + ro], start=90, end=180, fill=white, width=int(sw))
    ex, ey = cx + hook, y1 + hook
    d.ellipse([ex - sw / 2, ey - sw / 2, ex + sw / 2, ey + sw / 2], fill=white)
    t_right = max(cx + sw * 1.2, ex + sw / 2)
    # Étoile au-dessus, entre le t et le r.
    star(ds, cx + sw * 1.55, big * 0.15, big * 0.14, big * 0.058, STAR)
    # r : fût de la hauteur d'x à la base, épaule en quart de cercle vers la droite.
    rx, sh = t_right + gap + sw / 2, big * 0.16
    d.rounded_rectangle([rx - sw / 2, xtop, rx + sw / 2, base], radius=sw / 2, fill=white)
    cy, ro = xtop + sh, sh + sw / 2
    d.arc([rx + sh - ro, cy - ro, rx + sh + ro, cy + ro], start=180, end=270, fill=white, width=int(sw))
    d.ellipse([rx + sh - sw / 2, cy - sh - sw / 2, rx + sh + sw / 2, cy - sh + sw / 2], fill=white)
    # i : fût et point rond.
    ix = rx + sh + sw / 2 + gap + sw / 2
    d.rounded_rectangle([ix - sw / 2, xtop, ix + sw / 2, base], radius=sw / 2, fill=white)
    dot = sw * 1.05
    d.ellipse([ix - dot / 2, xtop - sw * 0.55 - dot, ix + dot / 2, xtop - sw * 0.55], fill=white)
    box = (0, 0, int(ix + sw / 2 + 6), h)
    size = (max(1, int((box[2]) / 4)), height)
    return (letters.crop(box).resize(size, Image.LANCZOS), stars.crop(box).resize(size, Image.LANCZOS))


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    out = sys.argv[1]
    sizes = [int(s) for s in sys.argv[2:]] or [1024, 256, 64, 32]
    for size in sizes:
        icon(size).save(f"{out}/icon-{size}.png")
        print(f"{out}/icon-{size}.png")
    letters, stars = wordmark(96)
    letters.save(f"{out}/wordmark-letters.png")
    stars.save(f"{out}/wordmark-star.png")
    print(f"{out}/wordmark-letters.png, {out}/wordmark-star.png")


if __name__ == "__main__":
    main()
