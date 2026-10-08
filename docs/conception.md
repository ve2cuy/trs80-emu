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
dans `crates/z80/tests/roms/` (exclue par `.gitignore`). Dans le fureteur,
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
6. ⬜ Chargement de programmes `.CAS` et `.CMD` (écriture directe en mémoire); boucle
   de développement avec zmac.
7. ⬜ Confort : Reset, Turbo, collage de texte, sauvegarde et restauration de l'état.
8. ⬜ Facultatif : son de la cassette (WebAudio), modification minuscules, disquettes
   (WD1771, images `.DSK` / `.DMK`), Model III.
9. ✅ Déploiement GitHub Pages par une GitHub Action (`.github/workflows/pages.yml`) :
   <https://ve2cuy.github.io/trs80-emu/>

## Références

- Décodage des opcodes : <http://www.z80.info/decoding.htm>
- Comportements non documentés : « The Undocumented Z80 Documented » (Sean Young)
- Émulateur TRS-80 existant en TypeScript : Lawrence Kesteloot (`lkesteloot/trs80` sur GitHub)
