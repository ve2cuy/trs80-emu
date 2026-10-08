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
- **Ne jamais committer de ROM TRS-80** (droit d'auteur). Elles vont dans
  `crates/*/tests/roms/`, exclu par `.gitignore`.

## Commandes

```bash
cargo test                                                        # tests rapides
cargo test --release -p z80 --test zex -- --ignored --nocapture   # ZEXDOC / ZEXALL
```

- Sous Windows, `cargo` est dans `%USERPROFILE%\.cargo\bin`; Rust est aussi installé sous WSL.
- `gh` (GitHub CLI) est disponible seulement sous WSL; dépôt : `ve2cuy/trs80-emu`.
