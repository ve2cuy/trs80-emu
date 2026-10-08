# 🖥️ trs80-emu

🇬🇧 [English version](README.md)

Émulateur du **TRS-80 Model 1**, écrit en Rust, qui s'exécute dans un fureteur
web (WebAssembly).

> Version 0.3 : ROM Level II, BASIC, texte et semi-graphiques, programmes `.CMD` et
> cassettes `.CAS`, collage de texte, horloge à 40 Hz, et **VE2CUY Invaders**, un jeu
> écrit en assembleur Z80 pour ce projet. À venir : disquettes.
> Voir [docs/conception.md](docs/conception.md).

**▶️ Essayer en ligne : <https://ve2cuy.github.io/trs80-emu/>**

## ROM

Aucune ROM n'est hébergée dans ce dépôt : elles sont © Tandy / Microsoft. La liste
déroulante de la page ([`www/roms.json`](www/roms.json)) les télécharge, au moment où
on les choisit, depuis le dépôt tiers
[kiwisincebirth/TRS-80-ROMS](https://github.com/kiwisincebirth/TRS-80-ROMS) :

| ROM | Téléchargement |
| --- | --- |
| Level II 1.3 et 1.2 (Tandy, officielles) | Direct, à un commit épinglé, empreinte SHA-256 vérifiée |
| Level II 1.3 avec correctifs, Enhanced Level II 1.4 (kiwisincebirth) | Depuis l'archive `.tar` de leur release, que l'utilisateur télécharge puis ouvre dans la page (GitHub n'autorise pas le fureteur à la télécharger lui-même) |

À la première visite, l'émulateur démarre avec la Level II 1.3. On peut aussi charger
son propre fichier (« Load ROM file… »). Lien direct : `?rom=level2-1.3`. La ROM choisie
est conservée dans le fureteur.

## Polices

La liste « Font » change le dessin du texte : la police d'origine du TRS-80 (pixels) ou
une police à chasse fixe : rétro (VT323, Press Start 2P, ...), moderne (IBM Plex Mono,
JetBrains Mono, ...) ou installée sur le système (Consolas, Courier New). Les polices
sont condensées pour la grille étroite de 64 × 16; les semi-graphiques ne changent pas.
Les polices web viennent de Google Fonts, téléchargées seulement si on les choisit.
Lien direct : `?font=vt323`.

## Lancer l'émulateur localement

Prérequis (une seule fois) :

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

Compiler, puis servir le dossier `www/` :

```bash
wasm-pack build crates/web --target web --out-dir ../../www/pkg --no-pack
cd www
python -m http.server 8080
```

Ouvrir <http://localhost:8080>, choisir une ROM dans la liste (ou votre fichier), puis
répondre à `MEM SIZE?` avec ENTRÉE.

- Un serveur local est nécessaire : un fureteur refuse de charger un module
  WebAssembly ouvert directement en `file://`.
- En développement, une ROM copiée dans `www/rom/level2.rom` (exclue de Git) est
  chargée automatiquement.

### Programmes

- **Liste intégrée** : VE2CUY Invaders ([source](asm/invaders.asm)) et des jeux dont
  l'auteur a autorisé la redistribution (Sea Dragon, Armored Patrol). Voir
  [www/programs/README.md](www/programs/README.md) avant d'en ajouter.
- **« Load program (.CMD, .CAS)… »** : n'importe quel programme de votre disque; rien
  n'est envoyé. Une cassette (`.CAS`) est chargée directement en mémoire : un programme
  en langage machine est lancé, un programme BASIC est exécuté.
- **Collage de texte** : Ctrl+V sur l'écran, ou « Type text… », tape le texte au clavier
  du TRS-80 (par exemple un programme BASIC).
- Lien direct vers un programme : `?program=ve2cuy-invaders`.

Les appels de fichiers de TRSDOS sont remplacés par des routines minimales : les
programmes conçus pour disquette démarrent, mais rien n'est enregistré.

### Clavier

| TRS-80 | PC |
| --- | --- |
| ENTER | Entrée |
| BREAK | Échap ou Fin |
| CLEAR | Origine ou Suppr |
| ← (effacer) | Retour arrière ou flèche gauche |
| → (tabulation) | Tab ou flèche droite |

Les symboles se tapent comme sur un PC (`"`, `*`, `+`, ...) : l'émulateur
s'occupe de la touche MAJ du TRS-80, dont la disposition est différente.

## Structure

| Dossier | Rôle |
| --- | --- |
| `crates/z80` | Cœur Z80 `no_std` : réutilisable en natif, en WebAssembly ou sur microcontrôleur |
| `crates/trs80` | La machine (`no_std`) : carte mémoire, clavier, vidéo, rendu en pixels |
| `crates/web` | Liaison WebAssembly (`wasm-bindgen`) |
| `www/` | La page web : `index.html`, `main.js`, `style.css` |
| `www/programs/` | Programmes libres de droits et leur liste (`index.json`) |
| `asm/` | Sources en assembleur Z80 (VE2CUY Invaders), assemblées avec zmac |
| `docs/` | Notes de conception |

## Tests

```bash
# Tests rapides : instructions Z80, démarrage de la ROM, session BASIC (si ROM présente)
cargo test

# Validation complète du Z80 (ZEXDOC et ZEXALL) : environ 30 secondes
cargo test --release -p z80 --test zex -- --ignored --nocapture
```

Les tests qui démarrent la ROM Level II l'attendent dans `crates/trs80/tests/roms/`.
Elle **n'est pas fournie** (droit d'auteur Tandy / Microsoft) : voir
[crates/trs80/tests/roms/README.md](crates/trs80/tests/roms/README.md).

## Auteur

Alain Boudreault (VE2CUY) — [ve2cuy.github.io](https://ve2cuy.github.io/)
