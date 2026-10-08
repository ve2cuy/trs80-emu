# Programmes en assembleur Z80

## VE2CUY Invaders (`invaders.asm`)

Jeu inspiré de Space Invaders pour le TRS-80 Model I (Level II, 16 Ko, sans DOS).
Textes en anglais ou en français, au choix à l'écran d'accueil.

Assemblage avec [zmac](http://48k.ca/zmac.html) :

```bash
zmac invaders.asm
cp zout/invaders.cmd     ../www/programs/ve2cuy-invaders.cmd
cp zout/invaders.500.cas ../www/programs/ve2cuy-invaders.cas
```

`zout/` (sorties de zmac) est exclu de Git. Le test `cargo test -p trs80 --test invaders`
joue une partie dans l'émulateur (choix de la langue, menu, tirs, score).

Son : la sortie cassette (port FFh, bits 0-1) est basculée par le processeur pour le tir,
les explosions, la soucoupe et le pas de la formation (routines `BEEP` et `NOISE`).

Principes : affichage par tampon de 1 Ko copié à l'écran d'un seul `LDIR`, effacement
du tampon par la pile (`PUSH`), sprites semi-graphiques de 3 caractères (6 × 3 pixels),
lecture directe de la matrice du clavier. Environ 30 images par seconde.
