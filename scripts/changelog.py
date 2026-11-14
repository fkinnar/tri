#!/usr/bin/env python3
"""Fragments de l'historique des versions (décidé le 14/11/2026).

Chaque pull request ajoute son entrée dans deux petits fichiers, `changelog.d/<PR>.md`
(anglais) et `changelog.d/<PR>.fr.md` (français), au lieu d'écrire en tête de
`CHANGELOG.md` / `CHANGELOG.fr.md` : deux PR ouvertes en même temps ne se gênent plus. Au
moment d'une version, la PR de la montée de version les rassemble dans les deux
historiques (`collect`) et les efface. Voir doc/en/build.md (« Changelog »).

    scripts/changelog.py check                 # chaque fragment a ses deux langues (CI)
    scripts/changelog.py show [--fr]           # aperçu des entrées en attente
    scripts/changelog.py collect [--version 1.0.0-beta.2 [--date 2026-11-21]] [--dry-run]

`collect` range les entrées en tête de la section « ## Unreleased » / « ## Non publié »
(créée au besoin), la plus récente PR d'abord ; avec `--version`, la section prend le
titre de la version (« ## 1.0.0-beta.2 — 2026-11-21 », « — 21/11/2026 » en français).
"""

import argparse
import datetime
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FRAGMENTS = "changelog.d"
# Langue → (historique, titre de la section en attente, format de date du titre).
LANGS = {
    "en": ("CHANGELOG.md", "## Unreleased", "%Y-%m-%d"),
    "fr": ("CHANGELOG.fr.md", "## Non publié", "%d/%m/%Y"),
}
IGNORED = {"README.md", "README.fr.md"}


def fragments(root):
    """{nom: {langue: chemin}} des fragments de `root/changelog.d`."""
    found = {}
    folder = root / FRAGMENTS
    if not folder.is_dir():
        return found
    for path in folder.iterdir():
        if path.name in IGNORED or path.suffix != ".md" or not path.is_file():
            continue
        stem = path.name[: -len(".md")]
        lang = "en"
        if stem.endswith(".fr"):
            stem, lang = stem[: -len(".fr")], "fr"
        found.setdefault(stem, {})[lang] = path
    return found


def order(names):
    """Les plus récentes d'abord : numéros de PR décroissants, noms sans numéro en tête."""
    def key(name):
        return (1, -int(name), "") if name.isdigit() else (0, 0, name)
    return sorted(names, key=key)


def problems(root):
    """Ce qui ne va pas dans les fragments (vide : tout va bien)."""
    out = []
    for name, langs in sorted(fragments(root).items()):
        for lang in LANGS:
            if lang not in langs:
                suffix = ".fr.md" if lang == "fr" else ".md"
                out.append(f"{FRAGMENTS}/{name}{suffix} is missing")
                continue
            text = langs[lang].read_text(encoding="utf-8").strip()
            if not text.startswith("- "):
                out.append(f"{langs[lang].relative_to(root)}: must start with a '- ' list item")
    return out


def entries(root, lang):
    """Texte des entrées en attente pour `lang`, dans l'ordre de l'historique."""
    found = fragments(root)
    parts = [found[n][lang].read_text(encoding="utf-8").strip() for n in order(found)]
    return "\n".join(parts)


def insert(text, heading, block, title=None):
    """`text` (un historique) avec `block` en tête des entrées de la section `heading`
    (créée avant la première version si absente), renommée `title` si donné."""
    lines = text.split("\n")
    start = next((i for i, l in enumerate(lines) if l.strip() == heading), None)
    if start is None:
        first = next((i for i, l in enumerate(lines) if l.startswith("## ")), len(lines))
        lines[first:first] = [heading, ""]
        start = first
    end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith("## ")), len(lines))
    at = next((i for i in range(start + 1, end) if lines[i].startswith("- ")), None)
    new = block.split("\n") if block else []
    if at is None:
        # Pas encore d'entrée : après le texte de la section, avant la suivante.
        at = end
        while at > start + 1 and lines[at - 1].strip() == "":
            at -= 1
        new = ([""] if lines[at - 1].strip() else []) + new
    lines[at:at] = new
    if title:
        lines[start] = title
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("check", help="each fragment has both languages")
    show = sub.add_parser("show", help="print the pending entries")
    show.add_argument("--fr", action="store_true", help="French entries")
    collect = sub.add_parser("collect", help="move the fragments into the changelogs")
    collect.add_argument("--version", help="title the section with this version")
    collect.add_argument("--date", help="release date, YYYY-MM-DD (default: today)")
    collect.add_argument("--dry-run", action="store_true", help="print, change nothing")
    parser.add_argument("--root", type=Path, default=ROOT, help=argparse.SUPPRESS)
    args = parser.parse_args()
    root = args.root

    issues = problems(root)
    if args.command == "check" or issues:
        for issue in issues:
            print(f"changelog: {issue}", file=sys.stderr)
        if issues:
            return 1
        print(f"changelog: {len(fragments(root))} pending entries, all in both languages")
        return 0

    if args.command == "show":
        print(entries(root, "fr" if args.fr else "en"))
        return 0

    found = fragments(root)
    if not found and not args.version:
        print("changelog: no pending entries")
        return 0
    date = datetime.date.fromisoformat(args.date) if args.date else datetime.date.today()
    for lang, (name, heading, fmt) in LANGS.items():
        path = root / name
        title = f"## {args.version} — {date.strftime(fmt)}" if args.version else None
        text = insert(path.read_text(encoding="utf-8"), heading, entries(root, lang), title)
        if args.dry_run:
            print(f"==> {name}\n{text[:3000]}")
        else:
            path.write_text(text, encoding="utf-8")
    if not args.dry_run:
        for langs in found.values():
            for path in langs.values():
                path.unlink()
        print(f"changelog: {len(found)} entries collected")
    return 0


if __name__ == "__main__":
    sys.exit(main())
