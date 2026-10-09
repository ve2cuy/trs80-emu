"""Génère les listes de « Boot a disk » à partir des fichiers présents :

    www/disks/index.json        disquettes publiées (sur GitHub et le site public)
    www/disks/local/index.json  disquettes locales (dossier exclu de Git)

Usage (depuis n'importe où) :

    python www/disks/make_index.py           # met à jour les deux index
    python www/disks/make_index.py --check   # vérifie seulement (code 1 si pas à jour)

- Chaque image (.dsk, .dmk, .jv1, .jv3, .imd) sans entrée en reçoit une : identifiant unique
  (aussi utilisé par le lien ?disk=...) et titre tirés du nom de fichier.
- Les entrées existantes sont conservées telles quelles (titre, description, licence...) :
  on peut les compléter à la main, le script ne les écrase pas.
- Les entrées dont le fichier a disparu sont retirées.
"""
import json
import re
import sys
from pathlib import Path

DISKS = Path(__file__).resolve().parent
FOLDERS = [
    (DISKS, 'published'),
    (DISKS / 'local', 'local'),
]
EXTENSIONS = {'.dsk', '.dmk', '.jv1', '.jv3', '.imd'}


def slug(name: str) -> str:
    """Identifiant à partir d'un nom de fichier : minuscules, chiffres et tirets."""
    s = re.sub(r'[^a-z0-9]+', '-', Path(name).stem.lower()).strip('-')
    return s or 'disk'


def title(name: str) -> str:
    """Titre lisible à partir d'un nom de fichier (ex. trsdos-27dd.dsk -> TRSDOS 27DD)."""
    return re.sub(r'[-_]+', ' ', Path(name).stem).strip().upper() or name


def load(index: Path) -> list:
    if not index.exists():
        return []
    try:
        entries = json.loads(index.read_text(encoding='utf-8-sig'))
    except json.JSONDecodeError as e:
        sys.exit(f'{index} : JSON invalide ({e}). Corrigez-le avant de relancer.')
    if not isinstance(entries, list):
        sys.exit(f'{index} : une liste JSON est attendue.')
    return entries


def main() -> int:
    check = '--check' in sys.argv[1:]
    # Les identifiants doivent être uniques dans les deux listes (la page les réunit).
    taken = set()
    plans = []
    for folder, kind in FOLDERS:
        index = folder / 'index.json'
        entries = load(index)
        present = sorted(
            (p.name for p in folder.iterdir() if p.is_file() and p.suffix.lower() in EXTENSIONS),
            key=str.lower,
        ) if folder.exists() else []
        kept = [e for e in entries if e.get('file') in present]
        removed = [e.get('file') for e in entries if e.get('file') not in present]
        taken.update(e['id'] for e in kept if 'id' in e)
        known = {e.get('file') for e in kept}
        added = []
        for name in present:
            if name in known:
                continue
            base = slug(name) if kind == 'published' else f'local-{slug(name)}'
            ident, n = base, 2
            while ident in taken:
                ident, n = f'{base}-{n}', n + 1
            taken.add(ident)
            added.append({'id': ident, 'file': name, 'title': title(name)})
        plans.append((folder, kind, index, kept + added, added, removed, entries))

    stale = False
    for folder, kind, index, new, added, removed, old in plans:
        if not folder.exists():
            continue
        changed = new != old or not index.exists()
        stale |= changed
        label = 'publiées' if kind == 'published' else 'locales'
        print(f'{index.relative_to(DISKS.parent.parent)} ({label}) : {len(new)} disquette(s)')
        for e in added:
            print(f'  + {e["file"]}  (id « {e["id"]} »)')
        for f in removed:
            print(f'  - {f}  (fichier absent)')
        if kind == 'published' and added:
            print('  ATTENTION : ces disquettes seront publiées sur GitHub et sur le site.')
            print('  Vérifiez que leur redistribution est autorisée; sinon, placez-les dans disks/local/.')
        if changed and not check:
            index.write_text(json.dumps(new, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')
    if check and stale:
        print('Index pas à jour : lancez python www/disks/make_index.py')
        return 1
    if not stale:
        print('Index déjà à jour.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
