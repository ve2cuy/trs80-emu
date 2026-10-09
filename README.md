# 🖥️ trs80-emu

🇫🇷 [Version française](README_FR.md)

An emulator of the **TRS-80 Model I**, written in Rust, that runs in a web
browser (WebAssembly).

> Version 1.0: Level II ROM, BASIC, text and semigraphics, `.CMD` programs and
> `.CAS` cassettes, **floppy disks** (LDOS, TRSDOS, NEWDOS...), **sound**, pasting
> text, 40 Hz clock, and **VE2CUY Invaders**, a game written in Z80 assembly for
> this project.
> See [docs/conception.md](docs/conception.md) (in French).

**▶️ Try it online: <https://ve2cuy.github.io/trs80-emu/>**

<p align="center">
  <img src="docs/interface.png" alt="The emulator in its dark theme: side menu (Machine, Disks), LDOS 5.3.1 double density booted on the TRS-80 screen, and the card describing the disk" width="700">
  <br><em>LDOS 5.3.1 double density booted from drive 0, with the side menu and the disk's card.</em>
</p>

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

## Interface

- **Side menu**: Machine (ROM, expansion interface, sound, disk drive sounds), Programs,
  Disks, My library, Repository, Display (font, theme), Keyboard, About. On a computer,
  it can be reduced to a column of icons (round button on its edge); on a phone, it is
  a drawer opened with ☰.
- **Light and dark themes**: follows the system by default; choose in Display, or with
  the sun / moon button at the bottom of the menu.
- **Preferences** (theme, menu, open sections, sound, Turbo, repository...) are kept
  in the browser.
- **Program card**: under the screen, the name of the program or disk being run, with
  its description when known (built-in list, repository, or the name and copyright
  records of a `.CMD` file).
- **Disk drive sounds** (Machine): the motor hum and the steps of the head, synthesized
  from the controller's activity.
- **Drag and drop** a disk, program or BASIC listing onto the screen to run it.

### My library

Disks, programs (`.CMD`, `.CAS`) and BASIC listings (`.BAS`, text or tokenized) can be
kept in the browser (IndexedDB, on this device only; nothing is uploaded): "Add files…",
the "Keep" button of a drive (with the changes made by the DOS) or of the "Type text"
panel, or automatically for the files you open ("Keep the files I open"). From the
library: Boot or insert a disk, Run a program, download or delete.

### External repository

Files can come from your device (Load…, Insert…, My library) or from a repository on the
Web, by default `https://ve2cuy.com/trs80`, which can be changed in the Repository section.
It has four folders, `rom/`, `disk/`, `cmd/` and `bas/`, each with an `index.json` listing
its files, in the same format as [`www/disks/index.json`](www/disks/index.json):

```json
[
  { "file": "invaders.cmd", "title": "VE2CUY Invaders", "year": 2026, "authors": "VE2CUY",
    "description": "…", "controls": "Arrows and space", "license": "…" },
  "other.cmd"
]
```

Only `file` is required (a plain file name is also accepted). The server must allow
cross-origin requests (CORS header `Access-Control-Allow-Origin: *`), since the page is
served from another address.

## Running the emulator locally (release 1.0)

No compilation needed: download the ready-to-run archive and you only need
**Python 3** and a web browser.

1. Download `trs80-emu-1.0.0.zip` from the
   [releases page](https://github.com/ve2cuy/trs80-emu/releases/latest) and unzip it.
2. Start it:
   - **Windows**: double-click `start.bat`;
   - **macOS / Linux**: `./start.sh` in a terminal;
   - **any system**: `python serve.py` in the folder, then open <http://localhost:8080>.
3. The page opens; the Level II 1.3 ROM is downloaded automatically the first time
   (or copy your own ROM to `rom/level2.rom` to work offline).

To stop it, close the server window (or press Ctrl+C). Details, including your own
disks in `disks/local/`: `README-LOCAL.md`, in the archive
([dist/README-LOCAL.md](dist/README-LOCAL.md)).

## Building from source

Prerequisites (once):

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

Build, then serve the `www/` folder:

```bash
wasm-pack build crates/web --target web --out-dir ../../www/pkg --no-pack
cd www
python serve.py        # no caching; or: python -m http.server 8080
```

Open <http://localhost:8080>, choose a ROM in the list (or your own file), then
answer `MEM SIZE?` with ENTER.

- A local server is required: browsers refuse to load a WebAssembly module
  opened directly from `file://`.
- During development, a ROM copied to `www/rom/level2.rom` (ignored by Git) is
  loaded automatically.

### Programs

- **Built-in list**: VE2CUY Invaders ([source](asm/invaders.asm)) and games whose
  author allowed redistribution (Sea Dragon, Armored Patrol). Read
  [www/programs/README.md](www/programs/README.md) (in French) before adding one.
- **"Load program (.CMD, .CAS, .BAS)…"**: any program from your disk; nothing is uploaded.
  A cassette (`.CAS`) is loaded directly into memory: a machine-language tape is
  started, a BASIC tape is run.
- **Pasting text**: Ctrl+V on the screen, or "Type text…", types the text on the
  TRS-80 keyboard (for example a BASIC program).
- Direct link to a program: `?program=ve2cuy-invaders`.

TRSDOS file calls are replaced by minimal routines: programs made for floppy disk
start, but nothing is saved.

### Disks

- **"Boot a disk"**: LDOS 5.3.1, freely redistributable, in single density and as a complete
  double-density system (see [www/disks/README.md](www/disks/README.md)).
  At boot, enter a date from its era, e.g. `10/08/91`. Direct link: `?disk=ldos-531`.
- **Local disks** (development): disks you may not publish (e.g. TRSDOS) go in
  `www/disks/local/`; this folder is ignored by Git and its disks appear in "Boot a
  disk" marked "local" (with `python serve.py`).
- **Disk lists**: after adding or removing images in `www/disks/` or `www/disks/local/`,
  run `python www/disks/make_index.py`. It updates both `index.json` files (new images
  get an entry, entries of missing images are removed, existing titles and
  descriptions are kept). Only put images you may redistribute in `www/disks/`.
- **Drives 0 to 3**: "Insert…" any JV1, JV3 or DMK image, "Blank" for an unformatted disk
  (format it from the DOS, e.g. `FORMAT :1`), "Eject", and "Save" to download the disk
  (a reformatted disk or a DMK image is saved as JV3). Drive 0 is the boot drive: inserting a disk
  there restarts the TRS-80 on it.
- WD1771 controller of the Model I expansion interface, plus the **Percom and Radio
  Shack double-density doublers** (WD1791). Tested: LDOS 5.3.1, TRSDOS 2.1, 2.3 and
  2.7DD, NEWDOS 3.0, NEWDOS/80, DOSPLUS 3.5, DBLDOS 4.2. Formatting works. As on the real
  machine, a disk whose track 0 is double density cannot boot.

### Sound

The Model I has no sound chip: programs toggle the cassette output (port FFh), as
VE2CUY Invaders does. The emulator turns it into audio (WebAudio). Sound starts at
the first key press or click (a browser rule); untick "Sound" to mute.

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

On a tablet or phone, tap the screen (or "⌨ Keyboard") to show the on-screen keyboard;
buttons under the screen give BREAK, CLEAR, the arrows and ENTER.

## Structure

| Folder | Role |
| --- | --- |
| `crates/z80` | `no_std` Z80 core: reusable natively, in WebAssembly or on a microcontroller |
| `crates/trs80` | The machine (`no_std`): memory map, keyboard, video, pixel rendering |
| `crates/web` | WebAssembly binding (`wasm-bindgen`) |
| `www/` | The web page: `index.html`, `main.js`, `style.css` |
| `www/programs/` | Freely redistributable programs and their list (`index.json`) |
| `asm/` | Z80 assembly sources (VE2CUY Invaders), built with zmac |
| `dist/` | Release packaging (`package.py`), launchers and local instructions |
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

## License

[Apache License 2.0](LICENSE). Third-party files (ZEXDOC/ZEXALL, the public-domain
games, LDOS) keep their own terms: see [NOTICE](NOTICE). TRS-80 ROMs are not included.

## Author

Alain Boudreault (VE2CUY) — [ve2cuy.github.io](https://ve2cuy.github.io/)
