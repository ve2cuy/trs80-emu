# 🖥️ trs80-emu

🇫🇷 [Version française](README_FR.md)

An emulator of the **TRS-80 Model I**, written in Rust, that runs in a web
browser (WebAssembly).

> Version 0.4: Level II ROM, BASIC, text and semigraphics, `.CMD` programs and
> `.CAS` cassettes, **floppy disks** (LDOS, TRSDOS, NEWDOS...), **sound**, pasting
> text, 40 Hz clock, and **VE2CUY Invaders**, a game written in Z80 assembly for
> this project.
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

- **Built-in list**: VE2CUY Invaders ([source](asm/invaders.asm)) and games whose
  author allowed redistribution (Sea Dragon, Armored Patrol). Read
  [www/programs/README.md](www/programs/README.md) (in French) before adding one.
- **"Load program (.CMD, .CAS)…"**: any program from your disk; nothing is uploaded.
  A cassette (`.CAS`) is loaded directly into memory: a machine-language tape is
  started, a BASIC tape is run.
- **Pasting text**: Ctrl+V on the screen, or "Type text…", types the text on the
  TRS-80 keyboard (for example a BASIC program).
- Direct link to a program: `?program=ve2cuy-invaders`.

TRSDOS file calls are replaced by minimal routines: programs made for floppy disk
start, but nothing is saved.

### Disks

- **"Boot a disk"**: LDOS 5.3.1, freely redistributable (see [www/disks/README.md](www/disks/README.md)).
  At boot, enter a date from its era, e.g. `10/08/91`. Direct link: `?disk=ldos-531`.
- **Local disks** (development): disks you may not publish (e.g. TRSDOS) go in
  `www/disks/local/` with an `index.json` like `www/disks/index.json`; this folder is
  ignored by Git and its disks appear in "Boot a disk" marked "local".
- **Drives 0 to 3**: "Insert…" any JV1, JV3 or DMK image, "Eject", and "Save" to download
  a disk modified by the TRS-80 (JV1 and JV3). Drive 0 is the boot drive: press Reset.
- WD1771 controller of the Model I expansion interface, plus the **Percom and Radio
  Shack double-density doublers** (WD1791). Tested: LDOS 5.3.1, TRSDOS 2.1, 2.3 and
  2.7DD, NEWDOS 3.0, NEWDOS/80, DOSPLUS 3.5, DBLDOS 4.2. Not yet: formatting. As on the
  real machine, a disk whose track 0 is double density cannot boot.

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

## Structure

| Folder | Role |
| --- | --- |
| `crates/z80` | `no_std` Z80 core: reusable natively, in WebAssembly or on a microcontroller |
| `crates/trs80` | The machine (`no_std`): memory map, keyboard, video, pixel rendering |
| `crates/web` | WebAssembly binding (`wasm-bindgen`) |
| `www/` | The web page: `index.html`, `main.js`, `style.css` |
| `www/programs/` | Freely redistributable programs and their list (`index.json`) |
| `asm/` | Z80 assembly sources (VE2CUY Invaders), built with zmac |
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
