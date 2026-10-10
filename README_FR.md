# 🖥️ trs80-emu

🇬🇧 [English version](README.md)

Émulateur du **TRS-80 Model 1**, écrit en Rust, qui s'exécute dans un fureteur
web (WebAssembly).

> Version 1.1 : ROM Level II, BASIC, texte et semi-graphiques, programmes `.CMD` et
> cassettes `.CAS`, **disquettes** (LDOS, TRSDOS, NEWDOS...), **son**, collage de texte,
> horloge à 40 Hz, et **VE2CUY Invaders**, un jeu écrit en assembleur Z80 pour ce projet.
> Nouveau en 1.1 : menu latéral, thèmes clair et sombre, quatre langues, bibliothèque de
> fichiers, dépôt externe, bruit des lecteurs, clavier sur tablette.
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

## Modèles

La liste « Modèle » (section Machine) choisit l'ordinateur; lien direct `?model=3` :

- **Model I** : ROM Level II (12 Ko), interface d'expansion (horloge à 40 Hz, disquettes).
- **Model III** : ROM de 14 Ko (Model III rév. C, téléchargée chez kiwisincebirth/TRS-80-ROMS),
  minuscules, contrôleur de disquettes WD1793 sur les ports F0h-F4h avec NMI, horloge à 30 Hz.
  Essayé avec TRSDOS 1.3.
- **Model 4** : la ROM du Model III plus 128 Ko de RAM en banques, les quatre plans de mémoire
  et l'écran de 80 × 24 (port 84h), 4 MHz et horloge à 60 Hz. TRSDOS 6 / LS-DOS démarre en mode
  Model 4 (essayé avec TRSDOS 6.2.1).
- **Model II** (`?model=2`) : une autre machine. ROM d'amorçage de 2 Ko (non proposée au
  téléchargement : chargez la vôtre avec « Charger une ROM… »), 64 Ko de RAM, écran de 80 × 24
  avec vidéo inversée, clavier ASCII (Ctrl + lettre donne les codes de contrôle, Fin = BREAK),
  4 MHz, horloge à 60 Hz par NMI, contrôleur FD1791 pour disquettes de 8 pouces servi par un DMA
  Z80, interruptions en mode 2 (DMA, CTC avec ses temporisateurs, PIO, SIO), disque dur et
  RS-232 (voir plus bas). Images de disquette IMD (ImageDisk) ou DMK. Essayé avec TRSDOS-II
  2.0a (qui n'accepte que les années 1980 à 1999), TRSDOS-II 4.2 et 4.4 (leur amorce teste la
  mémoire, le DMA, le PIO et le CTC) et TRSDOS-HD 4.0. Mode 40 colonnes (bit 4 du port FFh :
  caractères doublés en largeur), comme dans l'avertissement d'imprimante de la démonstration
  des logiciels du Model II.

Pas encore émulés : les 32 caractères « business graphics » du Model II (codes 04h-1Fh,
affichés comme les lettres 44h-5Fh; 00h-03h, les triangles de son logo, sont émulés); le jeu
de caractères alternatif des Model III et 4 (katakana ou caractères internationaux, bit 3 du
port ECh) : C0h-FFh montrent toujours les caractères spéciaux. Les
disquettes système des Model III et 4 ne sont pas publiées ici (droit d'auteur) : mettre les
vôtres dans `www/disks/local/`, avec `"model": 2`, `3` ou `4` dans `index.json`.

## Interface

- **Menu latéral** : Machine (ROM, interface d'expansion, son, bruit des lecteurs), Programs,
  Disks, My library, Repository, Display (police, thème), Keyboard, About. Sur un ordinateur,
  on peut le réduire à une colonne d'icônes (bouton rond sur son bord); sur un téléphone,
  c'est un tiroir qu'on ouvre avec ☰.
- **Thèmes clair et sombre** : celui du système par défaut; au choix dans Display, ou avec le
  bouton soleil / lune en haut à droite.
- **Langues** : anglais, français, espagnol et chinois simplifié, au choix avec le globe en
  haut à droite (celle du fureteur par défaut). Lien direct : `?lang=fr`. Les textes sont dans
  [`www/i18n.js`](www/i18n.js); les listes de programmes, de disquettes et du dépôt peuvent
  traduire leurs champs dans `"i18n": { "fr": { "description": "…" } }`.
- **Préférences** (thème, menu, sections ouvertes, son, Turbo, dépôt...) conservées dans le
  fureteur.
- **Mettre un disque dans un lecteur** : la même commande « Mettre dans… » partout
  (disquettes de la section Disquettes, Ma bibliothèque, Dépôt), numérotée comme les
  lecteurs : « Lecteur 0 — démarrer » redémarre le TRS-80 sur la disquette, les lecteurs 1 à
  3 reçoivent des disquettes de données, HD1 / HD2 les images de disque dur (`.hdv`). Choisir
  un disque dans une liste n'affiche que sa fiche; rien ne démarre avant « Mettre dans… ».
- **Barre des lecteurs** : sous l'écran, ce qui est dans les lecteurs 0 à 3 et les disques
  durs (● : le DOS y a écrit), avec leur géométrie (pistes, faces, secteurs par piste,
  taille, densité; la piste 0 quand elle diffère; cylindres et têtes d'un disque dur).
  Pendant un accès, l'opération, la piste et le secteur (cylindre, tête et secteur d'un
  disque dur) s'affichent un instant. Chaque disque a un bouton d'éjection; un clic ailleurs
  ouvre la section Disquettes. Une disquette des lecteurs 1, 2 ou 3 dont le DOS du lecteur 0
  ne peut pas lire le format s'affiche en rouge, avec les deux formats (TRSDOS-II 2.0 et 4.x,
  TRSDOS 1.3 / 2.7DD, LDOS / LS-DOS / TRSDOS 6), reconnus à leur géométrie : taille des
  secteurs des disquettes de 8 pouces, numéro du premier secteur (1 ou 0) des autres.
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
open »). Depuis la bibliothèque : mettre un disque dans un lecteur (« Mettre dans… »),
lancer un programme, télécharger ou supprimer.

### Disque dur

La section Disquettes offre deux disques durs (HD1 et HD2) : l'interface Radio Shack, un
contrôleur Western Digital WD1010 sur les ports C0h-CFh, celle des pilotes **RSHARD** de
MISOSYS et des cartes FreHD. Les images sont au format Reed (`.hdv`) de xtrs, trs80gp et
FreHD; « Nouveau » crée un disque vide de 10 Mo (306 cylindres, 4 têtes), « Télécharger »
l'enregistre avec ses fichiers. Le disque reste branché quand le TRS-80 redémarre ou passe
d'un Model I, III ou 4 à l'autre.

Le DOS a besoin du pilote RSHARD : la disquette « Pilotes RSHARD » (liste Disquettes, mise
dans le lecteur 1) contient RSHARD5/RSFORM5 pour LDOS 5.3 et RSHARD6/RSFORM6 pour LS-DOS /
TRSDOS 6.

```
SYSTEM (DRIVE=2,DISABLE,DRIVER="RSHARD5")    ENTRÉE partout, sauf « partition's number of heads » : 4
RSFORM5 :2 (NAME="RIGID1",MPW="PASSWORD")     formater (Y), aucune piste bloquée (N)
DIR :2
```

- Sur le Model I, la disquette des pilotes est en double densité : démarrez LDOS 5.3.1 double
  densité pour la lire.
- Gardez 4 têtes : avec 2 têtes, RSFORM5 finit sur « DATA RECORD NOT FOUND DURING WRITE »
  (il écrit sur une 3e tête; xtrs réagit pareil).
- Non émulés, comme dans xtrs : les commandes de plusieurs secteurs et le DMA du contrôleur
  (les pilotes RSHARD ne s'en servent pas).

Essayé avec LDOS 5.3.1 sur le Model I (disquette double densité) et le Model III, et TRSDOS
6.2.1 sur le Model 4. Enregistrez la configuration avec `SYSTEM (SYSGEN)` (LDOS 5.3 n'a pas de programme
SYSGEN) ou `SYSGEN` (TRSDOS 6) pour que le pilote se charge au démarrage, puis gardez la
disquette système et le disque dur dans Ma bibliothèque (ou téléchargez-les) pour en garder
une copie.

**Reprise de session** (interrupteur « Retrouver les disques au rechargement », section
Machine, actif par défaut) : les disquettes et les disques durs en place, avec leurs
modifications (SYSGEN, fichiers copiés…), sont enregistrés dans le fureteur pour chaque
modèle. Au rechargement de la page, ou en revenant à ce modèle, ils sont remis dans les
lecteurs et le TRS-80 redémarre sur le lecteur 0.

**Model II** : le même contrôleur WD1010, avec ce que le Model II y ajoute : secteurs de 512
octets, un CTC sur la carte d'interface (ports C4h-C7h) dont le canal 0 interrompt à la fin de
chaque commande, données transférées par le DMA Z80, et une ROM d'amorçage qui essaie le disque
dur avant les disquettes. Le système est **TRSDOS-HD** (Tandy, non fourni) : « Nouveau » crée
un disque de 8,4 Mo (256 cylindres, 4 têtes); la ROM d'amorçage affiche alors
`BOOT ERROR HN` (disque non formaté) : appuyez sur ÉCHAP pour démarrer la disquette TRSDOS-HD,
puis `INIT` formate le disque dur (lecteur 4) et y copie le système. Le Model II démarre
ensuite sur le disque dur (`TRSDOS-HD Ready`, `DIR :4`). Une image de disque dur du Model II
ne passe pas aux autres modèles, ni l'inverse.

### Carte graphique (Model III et 4)

La carte graphique haute résolution Radio Shack (26-1125 pour le Model III, 26-1126 pour le
Model 4) est branchée par défaut (interrupteur « Carte graphique (640 × 240) », section
Machine) : 640 × 240 points, 32 Ko de mémoire sur les ports 80h-83h (X, Y, donnée avec avance
automatique, mode), plus les registres de défilement 8Ch-8Dh du Model 4, comme dans xtrs.
Quand un programme affiche le graphique, l'écran montre l'image de 640 × 240 avec le texte
par-dessus (ou exclusif). Essayé avec BASICG 01.01.00 sous TRSDOS 6.2.1 (non fourni) : la
disquette de BASICG au lecteur 1, `BASICG`, puis `SCREEN 0`, `CLR`, `CIRCLE (320,120),100`,
`LINE (0,0)-(639,239)`. Essayé aussi avec BASICG 1.0 sous TRSDOS 1.3 sur le Model III, qui
montre le graphique pendant un programme et revient au texte dès qu'il écrit (READY) :
`10 SCREEN 0:CLR:CIRCLE (320,120),100`, `20 GOTO 20`, `RUN`.

### Carte son Orchestra

L'Orchestra-90 (Model III et 4, ports 79h à gauche et 75h à droite) et l'Orchestra-85 (Model
I, ports B9h et B5h) de Software Affair : deux convertisseurs 8 bits, mélangés au haut-parleur
1 bit (mono). Essayé avec ORCH90 sous LDOS 5.3.1 sur le Model III (non fourni) : `ORCH90`, `N`
(horloge normale; `Y` sur un Model 4 à 4 MHz), `4` voix, `N`, puis BREAK et `G GYPSY:1` (lire,
compiler et jouer un morceau).

### Modem et BBS (RS-232)

Le port série RS-232 (UART des ports E8h-EBh sur les Model I, III et 4; port A du Z80 SIO,
ports F4h-F7h, sur le Model II) est relié à un modem Hayes
virtuel qui joint les BBS par telnet, à travers un relais WebSocket
([`server/telnet-relay`](server/telnet-relay/README.md)), car un fureteur ne peut pas ouvrir de
connexion telnet. La section Modem propose environ 850 BBS (`www/bbs.json`, tirés de
<https://www.telnetbbsguide.com/bbs/list/brief/>) : choisissez-en un, puis **Composer** tape
`ATDT hôte[:port]` sur le TRS-80, dans LCOMM ou COMM. Le relais permet la même liste.

```
SET *KI KI
SET *CL RS232T (BAUD=2400,WORD=8,PARITY=OFF)               Model III
SET *CL RS232R (BAUD=2400,WORD=8,PARITY=OFF,BREAK=255)     Model I
LCOMM *CL
ATDT bbs.electrodrome.net                                   dans LCOMM : CONNECT, puis le BBS
```

TRSDOS 6.2.1 (Model 4, 80 colonnes) :

```
SET *CL COM/DVR
SETCOM (BAUD=2400,WORD=8,PARITY=OFF)
COMM *CL
ATDT bbs.electrodrome.net
```


`+++` (avec une pause avant et après) revient au mode commande du modem, `ATO` retourne en
ligne, `ATH` ou le bouton Raccrocher (section Modem) termine l'appel. Le modem retire les
séquences ANSI (couleurs, curseur) que le TRS-80 ne peut pas afficher (interrupteur dans la
section Modem). Sur le Model I, `BREAK=255` contourne un défaut du pilote RS232R de LDOS
5.3.1, qui sinon perd chaque caractère reçu. Essayé avec LDOS 5.3.1 (Model I et III) et TRSDOS 6.2.1 (Model 4).

Model II : lancez un programme de terminal sur le canal A (essayé avec OMNITERM 1.10 sous
TRSDOS-II 4.2, non fourni), puis `ATDT bbs.electrodrome.net`. La vitesse de la ligne vient du
CTC, comme sur la vraie machine : OMNITERM démarre à 300 bauds (changez-la dans ses réglages
pour aller plus vite), et un programme qui n'affiche pas aussi vite que la ligne reçoit perd
des caractères, comme il le ferait sur le vrai Model II.

### Dépôt externe

Les fichiers viennent de l'appareil (Load…, Insert…, My library) ou d'un dépôt sur le Web,
par défaut `https://ve2cuy.com/trs80`, modifiable dans la section Repository. Il comporte
six dossiers, `rom/`, `disk/`, `cmd/`, `bas/`, `asm/` (sources assembleur, ouvertes dans
l'atelier) et `cas/` (cassettes), chacun avec un `index.json` qui liste ses
fichiers, dans le même format que [`www/disks/index.json`](www/disks/index.json) :

```json
[
  { "file": "invaders.cmd", "title": "VE2CUY Invaders", "year": 2026, "authors": "VE2CUY",
    "description": "…", "controls": "Flèches et espace", "license": "…" },
  "autre.cmd"
]
```

Seul `file` est obligatoire (un simple nom de fichier est aussi accepté). `model` (un nombre
ou une liste) réserve un fichier à certains modèles, et `category` (`games`, `education`,
`finance`, `office`, `programming`, `utilities`, `graphics`, `music`, `communications`,
`science`, `systems`, `demo`, `other`) alimente le filtre par catégorie; un champ de recherche
filtre aussi par titre, nom et description. Le dépôt par défaut compte environ 20 000
fichiers classés ainsi. Le serveur doit
permettre les requêtes d'une autre origine (en-tête CORS `Access-Control-Allow-Origin: *`),
puisque la page est servie depuis une autre adresse.

Les fichiers sont classés par modèle : `model` indique le modèle (1, 2, 3, 4, ou une liste
comme `[3, 4]`) et la section Repository ne liste que les fichiers du modèle choisi (« Tous
les modèles » les montre tous). Le dépôt par défaut a un sous-dossier par modèle, nommé dans
`file` :

```
rom/index.json    rom/model1/level2.rom    rom/model3/model3.rom    ...
disk/index.json   disk/model1/ldos-531.dsk disk/model2/trsdos20a-m2.imd ...
```

```json
[{ "file": "model3/trsdos13-m3.dsk", "model": 3, "title": "TRSDOS 1.3 (Model III)" }]
```

### Scripts bloqués (Brave, NoScript…)

La page a besoin de JavaScript et de WebAssembly. Si les scripts sont bloqués, l'écran
explique la marche à suivre, en quatre langues : dans **Brave**, cliquer sur l'icône du lion
dans la barre d'adresse et désactiver les boucliers pour le site (ou « Bloquer les
scripts »), puis recharger. Un téléchargement arrêté par un bloqueur (la ROM, un fichier du
dépôt) est aussi signalé comme tel.

## Lancer l'émulateur localement (version 1.1)

Rien à compiler : téléchargez l'archive prête à l'emploi; il suffit de **Python 3** et
d'un fureteur.

1. Télécharger `trs80-emu-1.1.0.zip` sur la
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
- **Copier-coller** : Ctrl+V sur l'écran, le bouton Coller de la barre d'outils, ou « Type
  text… », tape le texte au clavier du TRS-80 (par exemple un programme BASIC ou des
  commandes du DOS). Ctrl+C sur l'écran (rien de sélectionné dans la page), ou le bouton
  Copier, copie le texte de l'écran.
- Lien direct vers un programme : `?program=ve2cuy-invaders`.

Les appels de fichiers de TRSDOS sont remplacés par des routines minimales : les
programmes conçus pour disquette démarrent, mais rien n'est enregistré.

### Disquettes

- **« Disquettes de ce modèle »** : LDOS 5.3.1, redistribuable, en simple densité et en système complet
  double densité (voir [www/disks/README.md](www/disks/README.md)).
  Au démarrage, entrer une date de son époque, ex. `10/08/91`. Lien direct : `?disk=ldos-531`.
- **Disquettes locales** (développement) : les disquettes qu'on ne peut pas publier
  (ex. TRSDOS) vont dans `www/disks/local/`; ce dossier est exclu de Git et ses
  disquettes apparaissent dans « Disquettes de ce modèle », marquées « local » (avec
  `python serve.py`).
- **Listes de disquettes** : après avoir ajouté ou retiré des images dans `www/disks/` ou
  `www/disks/local/`, lancer `python www/disks/make_index.py`. Il met à jour les deux
  `index.json` (une entrée pour chaque nouvelle image, retrait des images absentes, titres
  et descriptions existants conservés). Ne placer dans `www/disks/` que des images dont la
  redistribution est autorisée.
- **Lecteurs 0 à 3** : « Insert… » pour une image JV1, JV3, DMK ou IMD, « Blank » pour une disquette
  vierge (à formater depuis le DOS, ex. `FORMAT :1`), « Eject », et « Save » pour récupérer la
  disquette (une disquette reformatée ou une image DMK ou IMD est enregistrée en JV3). Le lecteur 0 sert au
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

Model II : sa touche CAPS est enfoncée au départ, car TRSDOS-II n'accepte ses commandes qu'en
majuscules (`dir` donne `ERROR 31`); Verr. Maj la bascule, pour taper en minuscules dans un
programme de terminal.

### Assembleur Z80

Le bouton `</>` en haut à droite ouvre un atelier de développement à côté de l'écran du TRS-80 :

- **Éditeur** avec couleurs et numéros de ligne. Syntaxe des assembleurs de l'époque
  (EDTASM, zmac) : étiquettes en colonne 1 ou terminées par `:`, `ORG`, `EQU`, `DB`/`DEFB`,
  `DW`/`DEFW`, `DS`/`DEFS`, `END départ`; nombres `4467H`, `0x4467`, `$4467`, `%1010`, `'A'`.
  Toutes les instructions documentées du Z80; le code produit est identique à celui de zmac
  (vérifié sur toutes les instructions et sur VE2CUY Invaders).
- **Assembler** (F9) : chaque erreur est expliquée, et un bouton **Corriger** propose la ligne
  corrigée quand c'est possible : instruction ou symbole mal orthographié, nombre hexadécimal
  sans chiffre en tête (`FFH` → `0FFH`), `JR` trop loin (→ `JP`), opérandes invalides (avec les
  formes valides), adresse Model III d'un service de LDOS, `ORG` manquant...
- **Services de LDOS** sans définition : `CALL @DSPLY`, `@TIME`, `@DATE`, `@EXIT`, `@KEY`,
  services de fichiers (`@FSPEC`, `@INIT`, `@OPEN`, `@READ`, `@WRITE`, `@CLOSE`...) et routines
  de la ROM sont prédéfinis, aux adresses de LDOS 5.3.1 sur le Model I (vérifiées par les tests
  sous LDOS). La liste est dans l'atelier; un `EQU` du programme a priorité.
- **Exécuter** (F5) ou **Déboguer** : le programme est chargé et lancé comme une commande du
  DOS; son `RET` ou `JP @EXIT` ramène à LDOS (ou au BASIC sans disquette). Un clic sur un numéro
  de ligne pose un **point d'arrêt**; **Pas** (F10) exécute une instruction et passe par-dessus
  les appels à LDOS et à la ROM. Les **registres**, les indicateurs, la pile et les octets en
  PC et en (HL) sont affichés à chaque pas.
- **Ouvrir / Enregistrer** sur la disquette du lecteur choisi : `NOM/ASM` (source, aussi au
  format EDTASM en lecture) et `NOM/CMD` (programme, lançable depuis LDOS : `NOM`). La disquette
  système LDOS en simple densité est pleine : utiliser celle en double densité, ou une disquette
  de données.

## Structure

| Dossier | Rôle |
| --- | --- |
| `crates/z80` | Cœur Z80 `no_std` : réutilisable en natif, en WebAssembly ou sur microcontrôleur |
| `crates/z80asm` | Assembleur Z80 `no_std` : diagnostics avec corrections, fichiers `.CMD`, symboles de LDOS |
| `crates/trs80` | La machine (`no_std`) : carte mémoire, clavier, vidéo, disquettes, fichiers LDOS, débogueur |
| `crates/web` | Liaison WebAssembly (`wasm-bindgen`) |
| `www/` | La page web : `index.html`, `main.js` (émulateur), `ide.js` (assembleur), `i18n.js`, `style.css` |
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
