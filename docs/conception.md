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

## Références

- Décodage des opcodes : <http://www.z80.info/decoding.htm>
- Comportements non documentés : « The Undocumented Z80 Documented » (Sean Young)
- Émulateur TRS-80 existant en TypeScript : Lawrence Kesteloot (`lkesteloot/trs80` sur GitHub)
