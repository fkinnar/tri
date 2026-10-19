#!/usr/bin/env python3
"""Icône de tri (choisie le 19/10/2026 parmi trois pistes) : un « t » minuscule épais,
blanc cassé, sur un carré arrondi gris anthracite, avec l'étoile jaune de la note.

Dessinée en grand (suréchantillonnage ×4) puis réduite, pour des bords nets à toute
taille. Usage : make-icon.py <dossier> [tailles…] (défaut : 1024 256 64 32) ; écrit
icon-<taille>.png. Demande Pillow (`pip install pillow`).
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


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    out = sys.argv[1]
    sizes = [int(s) for s in sys.argv[2:]] or [1024, 256, 64, 32]
    for size in sizes:
        icon(size).save(f"{out}/icon-{size}.png")
        print(f"{out}/icon-{size}.png")


if __name__ == "__main__":
    main()
