# Fichiers de test

| Fichier | Dans le dépôt | Origine |
| --- | --- | --- |
| `zexdoc.com`, `zexall.com` | oui | Z80 Instruction Exerciser, © 1994 Frank D. Cringle, licence GPL v2. Copie de [anotherlin/z80emu](https://github.com/anotherlin/z80emu/tree/master/testfiles). |
| `M1L2_1.3.bin` (ou `level2.rom`) | **non** | ROM Level II du TRS-80 Model 1 (12 Ko), © Tandy / Microsoft. À fournir soi-même. |

La ROM est exclue par `.gitignore` : ne jamais la publier. Sans elle, le test
`rom_boot` est simplement ignoré.
