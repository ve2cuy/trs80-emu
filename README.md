# 🖥️ trs80-emu

🇫🇷 [Version française](README_FR.md)

An emulator of the **TRS-80 Model I**, written in Rust, that runs in a web
browser (WebAssembly).

> Version 0.2: boots the Level II ROM, BASIC from the keyboard, text and
> semigraphics, `.CMD` programs. Coming next: cassette, floppy disks.
> See [docs/conception.md](docs/conception.md) (in French).

**▶️ Try it online: <https://ve2cuy.github.io/trs80-emu/>**

## ROMs

No ROM is hosted in this repository: they are © Tandy / Microsoft. The drop-down
list on the page ([`www/roms.json`](www/roms.json)) downloads them, when you choose
one, from the third-party repository
[kiwisincebirth/TRS-80-ROMS](https://github.com/kiwisincebirth/TRS-80-ROMS):

| ROM | Download |
| --- | --- |
| Level II 1.3 and 1.2 (Tandy, official) | Direct, from a pinned commit, SHA-256 checksum verified |
| Level II 1.3 with bug fixes, Enhanced Level II 1.4 (kiwisincebirth) | From their release `.tar` archive, which you download and then open in the page (GitHub does not let the browser download it by itself) |

On the first visit, the emulator starts with Level II 1.3. You can also load your
own file ("Load ROM file…"). Direct link: `?rom=level2-1.3`. The chosen ROM is kept
in your browser.

## Fonts

The "Font" list changes how text is drawn: the original TRS-80 pixel font, or a
fixed-width (monospace) font: retro (VT323, Press Start 2P, ...), modern (IBM Plex
Mono, JetBrains Mono, ...) or installed on your system (Consolas, Courier New).
Fonts are condensed to fit the narrow 64 × 16 grid; semigraphics are unchanged.
Web fonts are downloaded from Google Fonts only when chosen. Direct link:
`?font=vt323`.

## Running the emulator locally

Prerequisites (once):

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

Build, then serve the `www/` folder:

```bash
wasm-pack build crates/web --target web --out-dir ../../www/pkg --no-pack
cd www
python -m http.server 8080
```

Open <http://localhost:8080>, choose a ROM in the list (or your own file), then
answer `MEM SIZE?` with ENTER.

- A local server is required: browsers refuse to load a WebAssembly module
  opened directly from `file://`.
- During development, a ROM copied to `www/rom/level2.rom` (ignored by Git) is
  loaded automatically.

### Programs

- **Built-in list**: games whose author allowed redistribution (Sea Dragon,
  Armored Patrol). Read [www/programs/README.md](www/programs/README.md) (in French)
  before adding one.
- **"Load CMD file…"**: any `.CMD` file from your disk; nothing is uploaded.
- Direct link to a program: `?program=seadragon`.

TRSDOS file calls are replaced by minimal routines: programs made for floppy disk
start, but nothing is saved.

### Keyboard

| TRS-80 | PC |
| --- | --- |
| ENTER | Enter |
| BREAK | Esc or End |
| CLEAR | Home or Delete |
| ← (backspace) | Backspace or Left arrow |
| → (tab) | Tab or Right arrow |

Type symbols as you would on a PC (`"`, `*`, `+`, ...): the emulator handles the
TRS-80 Shift key, whose layout is different.

## Structure

| Folder | Role |
| --- | --- |
| `crates/z80` | `no_std` Z80 core: reusable natively, in WebAssembly or on a microcontroller |
| `crates/trs80` | The machine (`no_std`): memory map, keyboard, video, pixel rendering |
| `crates/web` | WebAssembly binding (`wasm-bindgen`) |
| `www/` | The web page: `index.html`, `main.js`, `style.css` |
| `www/programs/` | Freely redistributable `.CMD` programs and their list (`index.json`) |
| `docs/` | Design notes (in French) |

## Tests

```bash
# Quick tests: Z80 instructions, ROM boot, BASIC session (if a ROM is present)
cargo test

# Full Z80 validation (ZEXDOC and ZEXALL): about 30 seconds
cargo test --release -p z80 --test zex -- --ignored --nocapture
```

The tests that boot the Level II ROM expect it in `crates/trs80/tests/roms/`.
It is **not included** (Tandy / Microsoft copyright): see
[crates/trs80/tests/roms/README.md](crates/trs80/tests/roms/README.md) (in French).

## Author

Alain Boudreault (VE2CUY) — [ve2cuy.github.io](https://ve2cuy.github.io/)
