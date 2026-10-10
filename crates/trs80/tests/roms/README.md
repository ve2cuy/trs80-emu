# ROM TRS-80 pour les tests

Les ROM du TRS-80 sont © Tandy / Microsoft, y compris les versions modifiées
(ex. : Level II 1.4 de kiwisincebirth). Ce dossier est exclu par `.gitignore` :
**ne jamais y committer de ROM**. Seul ce README est versionné.

Les tests qui en ont besoin (`rom_boot`, `basic`, `programs`) cherchent
`M1L2_1.3.bin` ou `level2.rom` (ROM Level II, 12 Ko); sans elle, ils sont ignorés.
`tape` cherche aussi `level1.bin` (ROM Level I, 4 Ko : `test-roms/M1L1.bin`) et
`M3_REVC.bin` (Model III).

Source possible : [kiwisincebirth/TRS-80-ROMS](https://github.com/kiwisincebirth/TRS-80-ROMS)
(`test-roms/M1L2_1.3.bin`, ou l'archive `.tar` de ses releases).
