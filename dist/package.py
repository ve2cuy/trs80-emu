"""Construit l'archive de distribution locale : dist/out/trs80-emu-<version>.zip

    wasm-pack build crates/web --target web --out-dir ../../www/pkg --no-pack
    python dist/package.py

Contenu : le site (www/), déjà compilé, sans ROM ni disquettes locales, plus les
lanceurs (start.bat, start.sh), README-LOCAL.md, LICENSE et NOTICE. Tout tient dans un
dossier trs80-emu-<version>/ : il suffit de le décompresser et de lancer start.
"""
import re
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WWW = ROOT / 'www'
DIST = ROOT / 'dist'

# Jamais dans l'archive : ROM, disquettes locales, fichiers de développement.
EXCLUDED_DIRS = {'rom', 'local', '__pycache__'}
EXCLUDED_FILES = {'.gitignore'}

CRLF = b'\r\n'
LF = b'\n'


def version() -> str:
    cargo = (ROOT / 'crates' / 'trs80' / 'Cargo.toml').read_text(encoding='utf-8')
    return re.search(r'^version = "([^"]+)"', cargo, re.M).group(1)


def main() -> int:
    if not (WWW / 'pkg' / 'trs80_web_bg.wasm').exists():
        sys.exit('www/pkg absent : lancer wasm-pack build crates/web --target web --out-dir ../../www/pkg --no-pack')
    v = version()
    top = f'trs80-emu-{v}'
    out = DIST / 'out' / f'{top}.zip'
    out.parent.mkdir(exist_ok=True)
    files = []
    for path in sorted(WWW.rglob('*')):
        rel = path.relative_to(WWW)
        if path.is_dir() or EXCLUDED_DIRS & set(rel.parts) or path.name in EXCLUDED_FILES:
            continue
        if rel.suffix.lower() in ('.rom', '.bin', '.tar'):
            continue  # par prudence : aucune ROM, même hors de rom/
        files.append((path, f'{top}/{rel.as_posix()}'))
    for name in ('README-LOCAL.md', 'start.bat', 'start.sh'):
        files.append((DIST / name, f'{top}/{name}'))
    for name in ('LICENSE', 'NOTICE'):
        files.append((ROOT / name, f'{top}/{name}'))
    with zipfile.ZipFile(out, 'w', zipfile.ZIP_DEFLATED) as z:
        for src, arc in files:
            info = zipfile.ZipInfo.from_file(src, arc)
            info.compress_type = zipfile.ZIP_DEFLATED
            data = src.read_bytes()
            if arc.endswith('.bat'):
                data = data.replace(CRLF, LF).replace(LF, CRLF)  # fins de ligne Windows
            elif arc.endswith('.sh'):
                data = data.replace(CRLF, LF)  # fins de ligne Unix
                info.external_attr = 0o100755 << 16  # exécutable sous macOS / Linux
            z.writestr(info, data)
    print(f'{out.relative_to(ROOT)} : {len(files)} fichiers, {out.stat().st_size // 1024} Ko')
    return 0


if __name__ == '__main__':
    sys.exit(main())
