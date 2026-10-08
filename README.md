# 🖥️ trs80-emu

Émulateur du **TRS-80 Model 1**, écrit en Rust, destiné à s'exécuter dans un
fureteur web (WebAssembly).

> Projet en cours : le cœur Z80 est terminé et validé; la machine TRS-80 et la
> couche web sont les prochaines étapes. Voir [docs/conception.md](docs/conception.md).

## Structure

| Dossier | Rôle |
| --- | --- |
| `crates/z80` | Cœur Z80 `no_std` : réutilisable en natif, en WebAssembly ou sur microcontrôleur |
| `docs/` | Notes de conception |

## Prérequis

- [Rust](https://rustup.rs) (stable)
- Pour la future version web : `rustup target add wasm32-unknown-unknown`

## Tests

```bash
# Tests rapides : instructions, démarrage de la ROM (si présente)
cargo test

# Validation complète du Z80 (ZEXDOC et ZEXALL) : plusieurs minutes
cargo test --release -p z80 --test zex -- --ignored --nocapture
```

Le test de démarrage utilise la ROM Level II, qui **n'est pas fournie** (droit
d'auteur Tandy / Microsoft) : voir [crates/z80/tests/roms/README.md](crates/z80/tests/roms/README.md).

## Auteur

Alain Boudreault (VE2CUY) — [ve2cuy.github.io](https://ve2cuy.github.io/)
