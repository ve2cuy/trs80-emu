# 🖥️ trs80-emu

🇬🇧 [English version](README.md)

Émulateur du **TRS-80 Model 1**, écrit en Rust, qui s'exécute dans un fureteur
web (WebAssembly).

> Version 1.0 : ROM Level II, BASIC, texte et semi-graphiques, programmes `.CMD` et
> cassettes `.CAS`, **disquettes** (LDOS, TRSDOS, NEWDOS...), **son**, collage de texte,
> horloge à 40 Hz, et **VE2CUY Invaders**, un jeu écrit en assembleur Z80 pour ce projet.
> Voir [docs/conception.md](docs/conception.md).

**▶️ Essayer en ligne : <https://ve2cuy.github.io/trs80-emu/>**

<p align="center">
  <img src="docs/interface.png" alt="L'émulateur en thème sombre : menu latéral (Machine, Disks), LDOS 5.3.1 double densité démarré sur l'écran du TRS-80, et la fiche de la disquette" width="700">
  <br><em>LDOS 5.3.1 double densité démarré depuis le lecteur 0, avec le menu latéral et la fiche de la disquette.</em>
</p>

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

## Interface

- **Menu latéral** : Machine (ROM, interface d'expansion, son, bruit des lecteurs), Programs,
  Disks, My library, Repository, Display (police, thème), Keyboard, About. Sur un ordinateur,
  on peut le réduire à une colonne d'icônes (bouton rond sur son bord); sur un téléphone,
  c'est un tiroir qu'on ouvre avec ☰.
- **Thèmes clair et sombre** : celui du système par défaut; au choix dans Display, ou avec le
  bouton soleil / lune au bas du menu.
- **Langues** : anglais, français, espagnol et chinois simplifié, au choix dans Affichage
  (celle du fureteur par défaut). Lien direct : `?lang=fr`. Les textes sont dans
  [`www/i18n.js`](www/i18n.js); les listes de programmes, de disquettes et du dépôt peuvent
  traduire leurs champs dans `"i18n": { "fr": { "description": "…" } }`.
- **Préférences** (thème, menu, sections ouvertes, son, Turbo, dépôt...) conservées dans le
  fureteur.
- **Fiche du programme** : sous l'écran, le nom du programme ou de la disquette en cours, avec
  sa description quand on la connaît (liste intégrée, dépôt, ou les enregistrements de nom et
  de droit d'auteur d'un fichier `.CMD`).
- **Bruit des lecteurs** (Machine) : le ronronnement du moteur et les pas de la tête,
  synthétisés d'après l'activité du contrôleur.
- **Glisser-déposer** une disquette, un programme ou un listing BASIC sur l'écran pour le lancer.

### Ma bibliothèque

Disquettes, programmes (`.CMD`, `.CAS`) et listings BASIC (`.BAS`, en texte ou tokenisés) se
conservent dans le fureteur (IndexedDB, sur cet appareil seulement; rien n'est envoyé) :
« Add files… », le bouton « Keep » d'un lecteur (avec les modifications faites par le DOS) ou
du panneau « Type text », ou automatiquement pour les fichiers ouverts (« Keep the files I
open »). Depuis la bibliothèque : démarrer ou insérer une disquette, lancer un programme,
télécharger ou supprimer.

### Dépôt externe

Les fichiers viennent de l'appareil (Load…, Insert…, My library) ou d'un dépôt sur le Web,
par défaut `https://ve2cuy.com/trs80`, modifiable dans la section Repository. Il comporte
quatre dossiers, `rom/`, `disk/`, `cmd/` et `bas/`, chacun avec un `index.json` qui liste ses
fichiers, dans le même format que [`www/disks/index.json`](www/disks/index.json) :

```json
[
  { "file": "invaders.cmd", "title": "VE2CUY Invaders", "year": 2026, "authors": "VE2CUY",
    "description": "…", "controls": "Flèches et espace", "license": "…" },
  "autre.cmd"
]
```

Seul `file` est obligatoire (un simple nom de fichier est aussi accepté). Le serveur doit
permettre les requêtes d'une autre origine (en-tête CORS `Access-Control-Allow-Origin: *`),
puisque la page est servie depuis une autre adresse.

## Lancer l'émulateur localement (version 1.0)

Rien à compiler : téléchargez l'archive prête à l'emploi; il suffit de **Python 3** et
d'un fureteur.

1. Télécharger `trs80-emu-1.0.0.zip` sur la
   [page des versions](https://github.com/ve2cuy/trs80-emu/releases/latest) et la
   décompresser.
2. Lancer :
   - **Windows** : double-cliquer sur `start.bat`;
   - **macOS / Linux** : `./start.sh` dans un terminal;
   - **tout système** : `python serve.py` dans le dossier, puis ouvrir <http://localhost:8080>.
3. La page s'ouvre; la ROM Level II 1.3 est téléchargée automatiquement la première fois
   (ou copiez votre ROM dans `rom/level2.rom` pour travailler hors ligne).

Pour arrêter, fermer la fenêtre du serveur (ou Ctrl+C). Détails, dont vos propres
disquettes dans `disks/local/` : `README-LOCAL.md`, dans l'archive
([dist/README-LOCAL.md](dist/README-LOCAL.md)).

## Compiler à partir des sources

Prérequis (une seule fois) :

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

Compiler, puis servir le dossier `www/` :

```bash
wasm-pack build crates/web --target web --out-dir ../../www/pkg --no-pack
cd www
python serve.py        # sans mise en cache; ou : python -m http.server 8080
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
- **« Load program (.CMD, .CAS, .BAS)… »** : n'importe quel programme de votre disque; rien
  n'est envoyé. Une cassette (`.CAS`) est chargée directement en mémoire : un programme
  en langage machine est lancé, un programme BASIC est exécuté.
- **Collage de texte** : Ctrl+V sur l'écran, ou « Type text… », tape le texte au clavier
  du TRS-80 (par exemple un programme BASIC).
- Lien direct vers un programme : `?program=ve2cuy-invaders`.

Les appels de fichiers de TRSDOS sont remplacés par des routines minimales : les
programmes conçus pour disquette démarrent, mais rien n'est enregistré.

### Disquettes

- **« Boot a disk »** : LDOS 5.3.1, redistribuable, en simple densité et en système complet
  double densité (voir [www/disks/README.md](www/disks/README.md)).
  Au démarrage, entrer une date de son époque, ex. `10/08/91`. Lien direct : `?disk=ldos-531`.
- **Disquettes locales** (développement) : les disquettes qu'on ne peut pas publier
  (ex. TRSDOS) vont dans `www/disks/local/`; ce dossier est exclu de Git et ses
  disquettes apparaissent dans « Boot a disk », marquées « local » (avec `python serve.py`).
- **Listes de disquettes** : après avoir ajouté ou retiré des images dans `www/disks/` ou
  `www/disks/local/`, lancer `python www/disks/make_index.py`. Il met à jour les deux
  `index.json` (une entrée pour chaque nouvelle image, retrait des images absentes, titres
  et descriptions existants conservés). Ne placer dans `www/disks/` que des images dont la
  redistribution est autorisée.
- **Lecteurs 0 à 3** : « Insert… » pour une image JV1, JV3 ou DMK, « Blank » pour une disquette
  vierge (à formater depuis le DOS, ex. `FORMAT :1`), « Eject », et « Save » pour récupérer la
  disquette (une disquette reformatée ou une image DMK est enregistrée en JV3). Le lecteur 0 sert au
  démarrage : y insérer une disquette redémarre le TRS-80 dessus.
- Contrôleur WD1771 de l'interface d'expansion du Model I, et **doubleurs de densité
  Percom et Radio Shack** (WD1791). Essayés : LDOS 5.3.1, TRSDOS 2.1, 2.3 et 2.7DD,
  NEWDOS 3.0, NEWDOS/80, DOSPLUS 3.5, DBLDOS 4.2. Le formatage fonctionne. Comme sur la
  vraie machine, une disquette dont la piste 0 est en double densité ne peut pas démarrer.

### Son

Le Model I n'a pas de puce sonore : les programmes basculent la sortie cassette (port FFh),
comme le fait VE2CUY Invaders. L'émulateur en tire du son (WebAudio). Le son démarre à la
première touche ou au premier clic (règle des fureteurs); décocher « Sound » pour le couper.

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

Sur une tablette ou un téléphone, toucher l'écran (ou « ⌨ Keyboard ») fait apparaître le
clavier virtuel; des boutons sous l'écran donnent BREAK, CLEAR, les flèches et ENTER.

## Structure

| Dossier | Rôle |
| --- | --- |
| `crates/z80` | Cœur Z80 `no_std` : réutilisable en natif, en WebAssembly ou sur microcontrôleur |
| `crates/trs80` | La machine (`no_std`) : carte mémoire, clavier, vidéo, rendu en pixels |
| `crates/web` | Liaison WebAssembly (`wasm-bindgen`) |
| `www/` | La page web : `index.html`, `main.js`, `style.css` |
| `www/programs/` | Programmes libres de droits et leur liste (`index.json`) |
| `asm/` | Sources en assembleur Z80 (VE2CUY Invaders), assemblées avec zmac |
| `dist/` | Empaquetage des versions (`package.py`), lanceurs et instructions locales |
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

## Licence

[Apache License 2.0](LICENSE). Les fichiers de tiers (ZEXDOC/ZEXALL, les jeux du domaine
public, LDOS) gardent leurs propres conditions : voir [NOTICE](NOTICE). Les ROM du TRS-80
ne sont pas incluses.

## Auteur

Alain Boudreault (VE2CUY) — [ve2cuy.github.io](https://ve2cuy.github.io/)
