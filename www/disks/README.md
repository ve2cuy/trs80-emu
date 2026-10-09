# Disquettes fournies avec l'émulateur

Comme pour les programmes, seules des disquettes dont la redistribution est
**explicitement autorisée** sont publiées ici.

| Fichier | Contenu | Format |
| --- | --- | --- |
| `ldos-531.dsk` | LDOS 5.3.1 pour le Model I, disquette système | JV1, 35 pistes, simple densité |
| `ldos-531-dd.dsk` | LDOS 5.3.1 complet (fichiers des deux disquettes LDOS), double densité | JV3, 40 cylindres, piste 0 en simple densité |

## Ajouter une disquette

Copier l'image dans ce dossier (publiée) ou dans `local/` (non publiée), puis :

    python www/disks/make_index.py

Le script met à jour `index.json` et `local/index.json` : une entrée pour chaque nouvelle
image (titre tiré du nom de fichier, à compléter au besoin), retrait des images absentes.
Les titres, descriptions et licences déjà écrits sont conservés. La GitHub Action vérifie
que `index.json` correspond aux fichiers publiés (`make_index.py --check`).

## LDOS 5.3.1

Copyright 1991 MISOSYS, Inc. Fichier `ld1-531.dsk` de l'archive `ld1-531.zip`
publiée par Tim Mann (<https://tim-mann.org/misosys.html>), sans astérisque
(aucune exception). Avis de distribution, conservé tel quel comme l'exige la
permission :

> Roy Soltoff holds copyright or distribution rights to the software
> and documentation in the list below.  Roy grants free permission to
> everyone to download and use this software and documentation and to
> redistribute it to others, provided this notice is retained.  All
> other rights are reserved.  Specific exceptions apply to files marked
> with an asterisk (*) and are detailed within those files.  Hartforth
> is available with the permission of Andrew Graham.  LDOS/LS-DOS are
> available with the permission of William Schroeder.

`ldos-531-dd.dsk` a été fabriquée dans cet émulateur, avec LDOS lui-même, selon la marche
à suivre du manuel (commande SOLE) : `FDUBL (TANDY)`, `FORMAT :1 (SYSTEM)` en double
densité, `BACKUP :0 :1 (SYS,INV)`, `SYSTEM (SYSGEN,DRIVE=1)`, `SOLE :1`, puis les fichiers
de la deuxième disquette (`ld1x-531.dsk` de la même archive) avec `BACKUP :2 :1 (INV)`.
C'est une copie de LDOS, distribuée sous le même avis. Pour la refaire :

    cargo run --release -p trs80 --example make_ldos_dd -- <rom> www/disks/ldos-531.dsk www/disks/ldos-531-dd.dsk ld1x-531.dsk

Au démarrage, LDOS 5.3.1 demande la date : il n'accepte que des années de son
époque (ex. `10/08/91`).

## Compatibilité de l'émulateur

Contrôleur WD1771 du Model I et doubleurs Percom et Radio Shack (WD1791), images JV1,
JV3 et DMK. Fonctionnent : LDOS 5.3.1, TRSDOS 2.1, 2.3 et 2.7DD, NEWDOS 3.0, NEWDOS/80,
DOSPLUS 3.5, DBLDOS 4.2. Le formatage (« écrire la piste ») fonctionne : bouton « Blank »
pour insérer une disquette vierge, puis FORMAT dans le DOS. Une disquette dont la piste 0
est en double densité ne démarre pas, comme sur un vrai Model I.
