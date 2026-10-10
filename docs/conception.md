# Conception de l'émulateur TRS-80 Model 1

Résumé des décisions prises au démarrage du projet (2026-10-08).

## Objectif

Un émulateur du TRS-80 Model 1 qui s'exécute dans un fureteur web, hébergé sur
GitHub Pages à côté du blogue (<https://ve2cuy.github.io/blog/>). Il remplace
l'ancien émulateur Flash, qui ne fonctionne plus dans les fureteurs actuels.

## Choix techniques

- **Rust compilé en WebAssembly.** À 1,77 MHz, la performance ne l'exige pas;
  le vrai gain est la **réutilisation** : le même cœur Z80 peut tourner dans le
  fureteur (Wasm), en natif sur PC et sur microcontrôleur (STM32, en `no_std`).
- **Séparation stricte** :
  - `crates/z80` : le processeur seul, `no_std`, sans aucune dépendance. Il ne
    connaît la machine qu'à travers le trait `Bus` (mémoire et ports d'E/S).
  - `crates/trs80` : la machine (carte mémoire, clavier, vidéo et rendu en pixels), `no_std`.
  - `crates/web` : une mince couche `wasm-bindgen`; JavaScript (`www/main.js`) s'occupe du
    `<canvas>`, du clavier, du stockage de la ROM et de la boucle d'affichage.
- **Mémoire partagée Wasm ↔ JavaScript** : JavaScript lit directement la mémoire
  vidéo (3C00-3FFF) dans la mémoire Wasm, sans copie, pour dessiner l'écran.

## Carte mémoire du Model 1

```
0000-2FFF  ROM Level II (12 Ko)
37E0-37EF  Interruptions, sélection de lecteur, contrôleur WD1771 (interface d'expansion)
3800-3BFF  Clavier : matrice 8 × 8, une rangée par bit d'adresse
3C00-3FFF  Vidéo : 64 × 16 caractères
4000-FFFF  RAM (16 Ko à 48 Ko)
Port FF    Cassette (et son)
```

Particularité vidéo : sans la modification minuscules, la mémoire vidéo n'a pas
de bit 6. La ROM écrit les majuscules en 00h-1Fh (« M » = 0Dh) et le générateur de
caractères les affiche comme 40h-5Fh. Les codes 80h-FFh sont des blocs
semi-graphiques 2 × 3.

## Boucle d'exécution

À chaque image (`requestAnimationFrame`, 60 Hz) : exécuter environ 29 600 T-states
(1 774 000 / 60), puis redessiner l'écran. Le mode turbo exécute plus de cycles
par image.

## ROM

La ROM Level II est protégée par le droit d'auteur (Tandy / Microsoft) : **elle
n'est jamais publiée dans le dépôt**. Pour les tests, elle est placée localement
dans `crates/trs80/tests/roms/` (exclue par `.gitignore`). Dans le fureteur,
l'utilisateur chargera son propre fichier ROM, conservé ensuite dans IndexedDB.

## Étapes

1. ✅ Mise en place du dépôt et de la chaîne Rust (Windows et WSL, cible `wasm32-unknown-unknown`).
2. ✅ Cœur Z80 : jeu d'instructions complet (CB, DD, ED, FD, DDCB/FDCB), indicateurs
   non documentés X/Y, registre WZ (MEMPTR), durées en T-states, interruptions
   (modes 0, 1, 2 et NMI), HALT, registre R.
3. ✅ Validation : tests unitaires, ZEXDOC / ZEXALL, démarrage de la ROM jusqu'à « MEM SIZE? ».
4. ✅ Crate `trs80` : carte mémoire (48 Ko de RAM), clavier (matrice et MAJ par symbole),
   vidéo 64 × 16 et mode 32 caractères, semi-graphiques, police 5 × 7 maison.
   Validé par une session BASIC complète (`PRINT (2+2)*3`).
   ⬜ Reste : interruption 40 Hz de l'interface d'expansion (horloge, `TIME$`).
5. ✅ Couche web : `wasm-pack`, `<canvas>` 384 × 192 affiché en 4:3, ROM choisie par
   l'utilisateur et conservée dans IndexedDB, Reset, Turbo ×10.
7a. ✅ Cassettes `.CAS` (SYSTEM et BASIC, chargées directement en mémoire; BASIC : liens
   de lignes et pointeurs recalculés), frappe automatique (collage, file de 16 Ko,
   ×4 pendant la frappe), interface d'expansion avec horloge à 40 Hz (IM 1, verrou 37E0h).
   Magnétophone (`tape.rs`, d'après xtrs) : lecture à 500 bauds par l'entrée cassette
   (bascule du bit 7 de FFh; Model III : fronts en interruptions, port E0h bits 0-1, niveau
   au bit 0 de FFh). Moteur : bit 2 de FFh (Model I), bit 1 de ECh (Model III/4). La suite
   d'une cassette SYSTEM y reste pour les chargeurs à plusieurs étapes; sur Model III/4,
   4211h = 0 met la ROM en basse vitesse (cassette à 500 bauds).
   BASIC Level I (ROM de 4 Ko, Model I) : cassettes à 250 bauds (mêmes impulsions, durées
   doublées), lues par la ROM (CLOAD tapé, puis RUN si l'invite revient). `cas::normalize`
   convertit les cassettes à 1500 bauds (train de bits brut à 9 bits par octet compris) et
   répare l'en-tête BASIC 53h D3h D3h; la lecture SYSTEM tolère les sommes de contrôle
   fausses et l'absence d'adresse de lancement; une cassette d'un autre format est mise au
   magnétophone.
7b. ✅ VE2CUY Invaders (`asm/invaders.asm`, zmac) : jeu bilingue choisi à l'accueil,
   tampon d'écran, environ 31 images par seconde; testé par `tests/invaders.rs`.
7c. ✅ Son : sortie cassette (port FFh, bits 0-1) échantillonnée au fil des cycles
   (`audio.rs`, moyenne entre échantillons + passe-haut), jouée par WebAudio; effets
   sonores dans VE2CUY Invaders.
7d. ✅ Disquettes : contrôleur WD1771 (`fdc.rs`) et images JV1/JV3/DMK (`disk.rs`, avec
   `alloc`); LDOS 5.3.1 fourni (`www/disks/`). Démarrent : LDOS 5.3.1, TRSDOS 2.1/2.3,
   NEWDOS 3.0, NEWDOS/80, DOSPLUS 3.5. Reste : formatage.
7f. ✅ Doubleurs de densité Percom (commandes FEh/FFh) et Radio Shack (80h/A0h dans le
   registre de secteur), comme xtrs : le contrôleur actif (WD1771 ou WD1791) décide de la
   densité des secteurs trouvés et du codage du type de secteur. Déplacements de tête
   minutés (6 à 20 ms par piste) : l'interruption de fin arrive après la commande.
   Démarrent en plus : TRSDOS 2.7DD, DBLDOS 4.2. Journal du contrôleur : `fdc_trace()`.
7g. ✅ Formatage (« écrire la piste ») : secteurs tirés du flux de la piste; disquette vierge;
   export JV3 des disquettes reformatées et des DMK. RESET : le contrôleur exécute un
   Restore (la ROM, en 0696h, démarre en BASIC si l'état vaut 00h). LDOS 5.3.1 double
   densité fabriqué avec LDOS (`examples/make_ldos_dd.rs`) et publié.
7e. ✅ Licence Apache 2.0 (`LICENSE`, `NOTICE` pour les fichiers de tiers).
6. ✅ Chargement de programmes `.CMD` (écriture directe en mémoire, après avoir amené
   le BASIC à « READY »), routines minimales à la place des appels de fichiers TRSDOS,
   liste de programmes libres de droits (`www/programs/`). Interface en anglais.
   ⬜ Reste : fichiers `.CAS` (cassette).
6b. ✅ Liste de ROM (`www/roms.json`) : liens vers le dépôt tiers kiwisincebirth/TRS-80-ROMS,
   jamais d'hébergement. 1.2 et 1.3 officielles téléchargées directement (commit épinglé,
   SHA-256 vérifié); 1.3P et 1.4 extraites de l'archive `.tar` de leur release, ouverte par
   l'utilisateur (les archives de release GitHub n'ont pas d'en-tête CORS).
   Les quatre ROM démarrent; la 1.4 (« Enhanced Level II BASIC ») n'a plus de cassette.
   Première visite : démarrage automatique avec la 1.3 officielle.
6c. ✅ Polices : police d'origine (pixels, Rust) ou polices à chasse fixe (`www/fonts.js`).
   Rust dessine alors seulement les blocs graphiques (`render_graphics`) et la page dessine
   le texte depuis un atlas de 64 glyphes, sur un canvas de 1536 × 1152. Redessin seulement
   si la mémoire vidéo change. Clavier : chaque touche reste enfoncée au moins 3 images.
7. ⬜ Confort : Reset, Turbo, collage de texte, sauvegarde et restauration de l'état.
8. ⬜ Facultatif : modification minuscules, lecture de piste, Model III.
9. ✅ Déploiement GitHub Pages par une GitHub Action (`.github/workflows/pages.yml`) :
   <https://ve2cuy.github.io/trs80-emu/>
10. ✅ Autres modèles (`Model` dans `crates/trs80/src/lib.rs`, liste « Modèle » de la page) :
   - Model III : ROM de 14 Ko, ports E0h-FFh, NMI du contrôleur, horloge à 30 Hz.
   - Model 4 : port 84h (plans de mémoire, banques de 128 Ko, 80 × 24), 4 MHz, 60 Hz.
   - Model II : ROM d'amorçage de 2 Ko visible jusqu'à une écriture en F9h, vidéo en F800h
     (port FFh bit 7), FD1791 (E4h-E7h, choix en EFh) servi par le DMA Z80 (F8h, `dma.rs`),
     fin de commande par le PIO (E0h-E3h), clavier par le canal 3 du CTC (F0h-F3h), tous en
     mode 2 (priorité : DMA, clavier, PIO); horloge à 60 Hz par NMI (lue en FEh). Le
     contrôleur termine seul un secteur que le DMA ne lit qu'en partie (données perdues).
     Disquettes IMD. Essayé avec TRSDOS-II 2.0a.
11. ✅ Disque dur Radio Shack (Model I, III et 4, `hard.rs`) : WD1010 sur C0h-CFh, comme
   xtrs (MIT) : un secteur de 256 octets par commande, 32 secteurs par piste; images Reed
   (.hdv) dont l'en-tête donne les secteurs par cylindre (d'où les têtes); l'image s'allonge
   à l'écriture. Pilotes RSHARD5/RSHARD6 de MISOSYS (rshard.dsk, publiée); essayé avec
   LDOS 5.3.1 (Model I et III) et TRSDOS 6.2.1 (Model 4) : SYSTEM, RSFORM, COPY, DIR.
12. ✅ RS-232 et modem (Model I, III et 4) : UART des ports E8h-EBh (`serial.rs`), octets reçus
   au rythme de la vitesse choisie; Model III/4 : interruptions sur front, bit 5 de E0h pour
   la réception (le pilote RS232T de LDOS l'autorise), bit 4 pour l'émission, acquittées par
   la lecture de E0h. Modem Hayes dans la page (`www/modem.js`) et relais WebSocket -> telnet
   en Python sans dépendance (`server/telnet-relay`). Essayé avec LCOMM de LDOS 5.3.1 sur
   bbs.electrodrome.net.
13. ✅ Model II : disque dur et RS-232. CTC complet (`dma.rs`) : temporisateurs (pré-diviseur
   16 ou 256, constante, interruption au passage à zéro) et compteurs; l'amorce de
   TRSDOS-II 4.x teste les canaux 0 à 2 (« BOOT ERROR CT » sinon). Disque dur : le WD1010 de
   `hard.rs` avec la taille de secteur de CEh (512 octets), un second CTC sur C4h-C7h dont le
   canal 0 compte les fins de commande (interruption environ 1 ms après la commande, le DOS
   remettant son indicateur à zéro juste après l'avoir lancée), le DMA servi par C8h (DRQ),
   une unité absente « pas prête » et une image vierge non formatée (la ROM affiche alors
   « BOOT ERROR HN »). Essayé avec TRSDOS-HD 4.0 : INIT, démarrage sur le disque dur, COPY,
   DIR. RS-232 : Z80 SIO (`sio.rs`, F4h-F7h), canal A relié au modem; vitesse donnée par les
   canaux 0 et 1 du CTC et le diviseur de WR4; vecteur modifié par l'état. Essayé avec
   OMNITERM 1.10 (300 bauds) sur bbs.electrodrome.net.
14. ✅ Carte graphique haute résolution Radio Shack (Model III et 4, `hires.rs`), comme xtrs
   (MIT) : 128 × 256 octets, X, Y, donnée avec avance automatique, mode (83h), défilement du
   Model 4 (8Ch-8Dh). Image de 640 × 240, texte superposé en ou exclusif. Essayé avec BASICG
   sous TRSDOS 6.2.1. Polices de la page : aussi en 80 × 24 (Model II, Model 4), avec
   minuscules, vidéo inversée et [ \ ] ^ du Model II.
15. ✅ Caractères spéciaux C0h-FFh des Model III et 4 (dessins originaux en 5 × 7, équivalents
   Unicode pour le copier-coller); carte son Orchestra-90 / 85 (deux convertisseurs 8 bits
   mélangés au haut-parleur 1 bit). Essayé avec ORCH90 (Gypsy Rondo à quatre voix).
16. ✅ Model II : mode 40 colonnes (bit 4 du port FFh), caractères doublés en largeur. Essayé
   avec la démonstration des logiciels du Model II (avertissement d'imprimante). Restent les 32
   « business graphics » (04h-1Fh), dont le dessin n'est pas documenté dans les manuels
   consultés.

## Références

- Décodage des opcodes : <http://www.z80.info/decoding.htm>
- Comportements non documentés : « The Undocumented Z80 Documented » (Sean Young)
- Émulateur TRS-80 existant en TypeScript : Lawrence Kesteloot (`lkesteloot/trs80` sur GitHub)
- Documentation du Model II (manuels techniques, BASIC, cartes) : <https://github.com/pski/model2archive/tree/master/Hardware>
