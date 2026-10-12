// Aide-mémoire des commandes système, par DOS. Chaque commande a été essayée dans
// l'émulateur avec le vrai DOS (exemple doscmd, tests disks, dosfs, hard, printer) ou
// figure dans les procédures vérifiées du README (disque dur, port série). Les noms de
// fichier (NOTE/TXT...) sont des exemples à remplacer. Les descriptions sont dans i18n.js
// (clés cheat.<clé>).

export const CHEATSHEET = [
  {
    id: 'ldos',
    title: 'LDOS 5.3 — Model I, III',
    sections: [
      ['drives', [
        ['DIR :1', 'dir1'], ['FREE', 'free'], ['DEVICE', 'device'],
        ['SYSTEM (DRIVE=2,ENABLE)', 'enable'], ['SYSTEM (SYSGEN)', 'sysgen'],
      ]],
      ['files', [
        ['COPY NOTE/TXT:0 NOTE/TXT:1', 'copy'], ['RENAME NOTE/TXT TO NEW/TXT', 'rename'],
        ['KILL NOTE/TXT', 'kill'], ['LIST NOTE/TXT', 'list'],
      ]],
      ['hard', [
        ['SYSTEM (DRIVE=2,DISABLE,DRIVER="RSHARD5")', 'hardDriver'],
        ['RSFORM5 :2 (NAME="RIGID1",MPW="PASSWORD")', 'hardFormat'],
        ['DIR :2', 'hardDir'], ['SYSTEM (SYSGEN)', 'sysgen'],
      ]],
      ['serial', [
        ['SET *KI KI', 'serialKi'],
        ['SET *CL RS232R (BAUD=2400,WORD=8,PARITY=OFF,BREAK=255)', 'serialM1'],
        ['SET *CL RS232T (BAUD=2400,WORD=8,PARITY=OFF)', 'serialM3'],
        ['LCOMM *CL', 'lcomm'],
      ]],
      ['printer', [
        ['LIST NOTE/TXT (P)', 'printList'], ['COPY NOTE/TXT *PR', 'printCopy'],
      ]],
    ],
  },
  {
    id: 'trsdos6',
    title: 'TRSDOS 6.2 / LS-DOS 6.3 — Model 4',
    sections: [
      ['drives', [
        ['DIR :1', 'dir1'], ['FREE', 'free'], ['DEVICE', 'device'],
        ['SYSTEM (DRIVE=2,ENABLE)', 'enable'], ['SYSGEN', 'sysgen'],
      ]],
      ['files', [
        ['COPY NOTE/TXT:0 NOTE/TXT:1', 'copy'], ['RENAME NOTE/TXT TO NEW/TXT', 'rename'],
        ['REMOVE NOTE/TXT', 'kill'], ['LIST NOTE/TXT', 'list'],
      ]],
      ['hard', [
        ['SYSTEM (DRIVE=2,DISABLE,DRIVER="RSHARD6")', 'hardDriver'],
        ['RSFORM6 :2 (NAME="RIGID1",MPW="PASSWORD")', 'hardFormat'],
        ['DIR :2', 'hardDir'], ['SYSGEN', 'sysgen'],
      ]],
      ['serial', [
        ['SET *CL COM/DVR', 'serialDrv'],
        ['SETCOM (BAUD=2400,WORD=8,PARITY=OFF)', 'setcom'],
        ['COMM *CL', 'comm'],
      ]],
      ['printer', [
        ['LIST NOTE/TXT (P)', 'printList'], ['COPY NOTE/TXT *PR', 'printCopy'],
      ]],
    ],
  },
  {
    id: 'trsdos13',
    title: 'TRSDOS 1.3 — Model III',
    sections: [
      ['drives', [['DIR :1', 'dir1'], ['FREE', 'free'], ['LIB', 'lib']]],
      ['files', [
        ['COPY NOTE/TXT:0 TO NOTE/TXT:1', 'copy'], ['RENAME NOTE/TXT TO NEW/TXT', 'rename'],
        ['KILL NOTE/TXT', 'kill'], ['LIST NOTE/TXT (ASCII)', 'listAscii'],
      ]],
      ['serial', [
        ['SETCOM', 'setcomShow'], ['SETCOM (BAUD=2400,WORD=8,STOP=1)', 'setcom'], ['SETCOM (NOWAIT)', 'nowait'],
      ]],
      ['printer', [['LIST NOTE/TXT (ASCII,PRT)', 'printList']]],
    ],
  },
  {
    id: 'trsdos23',
    title: 'TRSDOS 2.3 — Model I',
    sections: [
      ['drives', [['DIR', 'dir'], ['FREE', 'free'], ['LIB', 'lib']]],
      ['files', [
        ['COPY NOTE/TXT TO NEW/TXT', 'copySame'], ['RENAME NOTE/TXT TO NEW/TXT', 'rename'],
        ['KILL NOTE/TXT', 'kill'], ['LIST NOTE/TXT', 'list'],
      ]],
      ['printer', [['PRINT NOTE/TXT', 'printList']]],
    ],
  },
  {
    id: 'trsdos2',
    title: 'TRSDOS-II / TRSDOS-HD — Model II',
    sections: [
      ['drives', [
        ['DIR', 'dir'], ['FREE', 'free'], ['LIB', 'lib'],
        ['FORMAT 1 {ID=DATADISK,ABS}', 'format20'], ['FORMAT :1 {ABS}', 'format44'],
      ]],
      ['files', [
        ['COPY HELLO:0 TO HELLO:1', 'copy'], ['FCOPY :0 {DIR}', 'fcopyDir'], ['FCOPY :0 TO :4 {ALL}', 'fcopyAll'],
      ]],
      ['printer', [
        ['LIST HELLO PRT', 'printList20'], ['LIST HELLO {PRT}', 'printList44'], ['DIR {PRT}', 'printDir'],
      ]],
      ['hard', [
        ['FORMAT :4 HDS=4 CYL=256 {ABS}', 'hardFormat44'], ['FILES :4 {SYS}', 'filesSys'],
        ['COPY HELLO:4 TO :0', 'copyHard'], ['INIT', 'hardInit'], ['DIR :4', 'hardDir'],
      ]],
    ],
  },
  {
    id: 'basic',
    title: 'BASIC — Model I, III, 4',
    sections: [
      ['printer', [['LPRINT "TRS-80";6*7', 'lprint'], ['LLIST', 'llist']]],
    ],
  },
];

/** Aide-mémoire qui convient au DOS reconnu au lecteur 0, sinon au modèle. */
export function defaultCheatsheet(dosName, model) {
  const name = dosName ?? '';
  if (name.startsWith('LDOS')) return 'ldos';
  if (name.startsWith('TRSDOS 6') || name.startsWith('LS-DOS')) return 'trsdos6';
  if (name.startsWith('TRSDOS 1')) return 'trsdos13';
  if (name.startsWith('TRSDOS-II')) return 'trsdos2';
  if (name.startsWith('TRSDOS 2')) return 'trsdos23';
  return { 1: 'ldos', 2: 'trsdos2', 3: 'ldos', 4: 'trsdos6' }[model] ?? 'ldos';
}
