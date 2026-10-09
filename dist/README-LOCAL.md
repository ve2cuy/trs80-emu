# TRS-80 Model I emulator — running it on your computer

🇫🇷 [Version française plus bas](#version-française)

This archive contains the complete emulator, already built (Rust compiled to
WebAssembly). Nothing to compile: you only need **Python 3** and a web browser.

Online version: <https://ve2cuy.github.io/trs80-emu/> — source code:
<https://github.com/ve2cuy/trs80-emu>

## Start

- **Windows**: double-click `start.bat`.
- **macOS / Linux**: run `./start.sh` in a terminal (or `sh start.sh`).
- **Any system**: in this folder, run `python serve.py` (or `python3 serve.py`), then
  open <http://localhost:8080>.

The page opens in your browser. To stop the emulator, close the server window (or
press Ctrl+C in it).

Why a server? Browsers refuse to load a WebAssembly module from a file opened
directly (`file://`). `serve.py` is a tiny local web server, reachable only from
your own computer (127.0.0.1); it also disables caching, so an update shows up with
a simple reload.

Python is not installed? Get it from <https://www.python.org/downloads/> (on
Windows, tick "Add python.exe to PATH" during installation).

## ROM

TRS-80 ROMs are © Tandy / Microsoft and are not included.

- On the first start, the Level II 1.3 ROM is downloaded automatically from
  <https://github.com/kiwisincebirth/TRS-80-ROMS> (internet needed once; the ROM is
  then kept by your browser).
- Or choose your own file with "Load ROM file…".
- **Without internet**: copy your 12 KB Level II ROM to `rom/level2.rom` in this
  folder; it is loaded automatically.

## Disks and programs

- "Boot a disk" offers LDOS 5.3.1 (single density, and a complete double-density
  system). These disks are freely redistributable (see `NOTICE`).
- **Your own disks** (TRSDOS, NEWDOS...): copy them to `disks/local/`, then run
  `python disks/make_index.py` and reload the page: they appear in "Boot a disk",
  marked "local". Any `.DSK`/`.DMK` image can also be inserted with "Insert…".
- "Load program (.CMD, .CAS, .BAS)…" runs a program, a cassette image or a BASIC
  listing; "Type text…" or Ctrl+V on the screen types text (e.g. a BASIC program).
- The interface language (English, French, Spanish, Chinese) is chosen with the globe
  at the top right.

## Nothing works? (scripts blocked)

The page needs JavaScript and WebAssembly. If the screen shows a message about blocked
scripts, or the menu does not respond:

- **Brave**: click the lion icon in the address bar and turn Shields down for this site
  (or turn off "Block scripts"), then reload.
- **NoScript, uBlock Origin** or another blocker: allow `localhost` (or the site), then
  reload.

## Version française

Cette archive contient l'émulateur complet, déjà compilé (Rust compilé en WebAssembly).
Rien à compiler : il suffit de **Python 3** et d'un fureteur.

### Démarrer

- **Windows** : double-cliquer sur `start.bat`.
- **macOS / Linux** : lancer `./start.sh` dans un terminal (ou `sh start.sh`).
- **Tout système** : dans ce dossier, lancer `python serve.py` (ou `python3 serve.py`),
  puis ouvrir <http://localhost:8080>.

La page s'ouvre dans le fureteur. Pour arrêter l'émulateur, fermer la fenêtre du serveur
(ou y appuyer sur Ctrl+C).

Pourquoi un serveur ? Les fureteurs refusent de charger un module WebAssembly depuis un
fichier ouvert directement (`file://`). `serve.py` est un petit serveur web local,
accessible seulement depuis votre ordinateur (127.0.0.1); il désactive aussi le cache.

Python n'est pas installé ? Le télécharger sur <https://www.python.org/downloads/> (sous
Windows, cocher « Add python.exe to PATH » à l'installation).

### ROM

Les ROM du TRS-80 sont © Tandy / Microsoft et ne sont pas incluses.

- Au premier démarrage, la ROM Level II 1.3 est téléchargée automatiquement depuis
  <https://github.com/kiwisincebirth/TRS-80-ROMS> (connexion requise une fois; le fureteur
  la conserve ensuite).
- Ou choisir son propre fichier avec « Load ROM file… ».
- **Sans connexion** : copier votre ROM Level II (12 Ko) dans `rom/level2.rom`, dans ce
  dossier; elle est chargée automatiquement.

### Disquettes et programmes

- « Boot a disk » propose LDOS 5.3.1 (simple densité, et système complet en double
  densité), redistribuables (voir `NOTICE`).
- **Vos propres disquettes** (TRSDOS, NEWDOS...) : les copier dans `disks/local/`, lancer
  `python disks/make_index.py` et recharger la page : elles apparaissent dans « Boot a
  disk », marquées « local ». Toute image `.DSK`/`.DMK` peut aussi être insérée avec
  « Insert… ».
- « Charger un programme (.CMD, .CAS, .BAS)… » lance un programme, une cassette ou un
  listing BASIC; « Taper du texte… » ou Ctrl+V sur l'écran tape du texte (ex. un
  programme BASIC).
- La langue de l'interface (anglais, français, espagnol, chinois) se choisit avec le globe
  en haut à droite.

### Rien ne fonctionne ? (scripts bloqués)

La page a besoin de JavaScript et de WebAssembly. Si l'écran affiche un message sur des
scripts bloqués, ou si le menu ne répond pas :

- **Brave** : cliquer sur l'icône du lion dans la barre d'adresse et désactiver les
  boucliers pour ce site (ou « Bloquer les scripts »), puis recharger.
- **NoScript, uBlock Origin** ou un autre bloqueur : autoriser `localhost` (ou le site),
  puis recharger.

---

Licence : Apache 2.0 (`LICENSE`); fichiers de tiers : `NOTICE`. Auteur : Alain Boudreault (VE2CUY).
