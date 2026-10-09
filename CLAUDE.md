# trs80-emu — instructions pour Claude

Émulateur TRS-80 Model 1 en Rust, cible finale : WebAssembly dans un fureteur,
publié sur GitHub Pages. Lire d'abord `docs/conception.md` (décisions, carte
mémoire, étapes et état d'avancement).

`ressources.txt` (non versionné) contient l'historique de la conversation de
départ, si plus de contexte est nécessaire.

## Conventions

- Échanges avec l'utilisateur, documentation, commentaires et messages de commit : **en français**.
  Identificateurs du code : en anglais.
- `crates/z80` reste `no_std` et sans dépendance : il doit pouvoir tourner sur STM32.
  Le processeur ne connaît la machine qu'à travers le trait `Bus`.
- Toute nouvelle instruction ou correction du Z80 s'accompagne d'un test dans
  `crates/z80/tests/instructions.rs`.
- `asm/` : sources Z80, assemblées avec le zmac de l'utilisateur
  (`Z:\0 - Documentation\Réseau et équipement maison\TRS-80\ASM-Tool-Chain\zmac.exe`);
  copier `zout/invaders.cmd` et `zout/invaders.500.cas` dans `www/programs/`.
- `crates/z80/tests/zex/` : ZEXDOC et ZEXALL (GPL, versionnés). Le cœur Z80 ne dépend
  d'aucun fichier de `crates/trs80`.
- Licence Apache 2.0; les fichiers de tiers sont listés dans `NOTICE` (à tenir à jour).
- Listes de « Boot a disk » : `python www/disks/make_index.py` (publiées et locales);
  la GitHub Action lance `--check`.
- Disquettes publiées (`www/disks/`) : seulement avec une autorisation vérifiable; LDOS 5.3.1
  exige de conserver l'avis de MISOSYS (voir `www/disks/README.md`).
- **Ne jamais committer de ROM TRS-80** (droit d'auteur, y compris les versions modifiées
  comme la 1.4 de kiwisincebirth). Pour les tests : `crates/trs80/tests/roms/`, exclu par
  `.gitignore`. Dans la page : seulement des liens vers un tiers (`www/roms.json`).
- Programmes de `www/programs/` : seulement avec une autorisation de redistribution
  vérifiable (voir `www/programs/README.md`).
- `cargo run -p trs80 --example screen -- <rom> [secondes] [texte] [prog.cmd]` affiche
  l'écran en texte : pratique pour tester une ROM ou un programme sans fureteur.

## Commandes

```bash
cargo test                                                        # tests rapides
cargo test --release -p z80 --test zex -- --ignored --nocapture   # ZEXDOC / ZEXALL
wasm-pack build crates/web --target web --out-dir ../../www/pkg --no-pack
cd www && python serve.py                                         # http://localhost:8080, sans cache
python dist/package.py        # archive locale dist/out/trs80-emu-<version>.zip (après wasm-pack)
```

- Version publiée : `dist/package.py` lit la version dans `crates/trs80/Cargo.toml`;
  l'archive (sans ROM ni `disks/local/`) est jointe à une release GitHub `v<version>`.
  `dist/README-LOCAL.md`, `start.bat`, `start.sh` y sont copiés.

- `www/rom/level2.rom` (exclu de Git) est chargé d'office en développement;
  `?frames=N` exécute N images au chargement (utile pour les captures headless,
  où `requestAnimationFrame` n'avance pas).

- Sous Windows, `cargo` est dans `%USERPROFILE%\.cargo\bin`; Rust est aussi installé sous WSL.
- `gh` (GitHub CLI) est disponible seulement sous WSL; dépôt : `ve2cuy/trs80-emu`.
