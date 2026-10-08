; =============================================================================
; VE2CUY INVADERS - un jeu inspire de Space Invaders pour le TRS-80 Model I
; Par VE2CUY (Alain Boudreault), 2026.
;
; Assemblage :  zmac invaders.asm       -> zout/invaders.cmd
; Essai       :  trs80gp -m1 zout/invaders.cmd, ou l'emulateur trs80-emu
;
; Machine : TRS-80 Model I, Level II BASIC, 16 Ko de RAM ou plus, sans DOS.
; Les textes du jeu sont en anglais ou en francais, au choix a l'ecran
; d'accueil (le Model I n'a pas d'accents).
;
; Principes :
;   - Tout est dessine dans un tampon de 1 Ko (BUF), puis copie a l'ecran en
;     une seule instruction LDIR : pas de scintillement.
;   - Graphismes semi-graphiques : un caractere 80h-BFh = 2 x 3 pixels
;     (bit 0 haut-gauche, 1 haut-droite, 2 milieu-gauche, 3 milieu-droite,
;     4 bas-gauche, 5 bas-droite). Chaque sprite fait 3 caracteres = 6 x 3 pixels.
;   - Le clavier est lu directement dans sa matrice (3801h-3880h).
;   - Le son passe par la sortie cassette (bits 0-1 du port FFh), basculee par le
;     processeur lui-meme : le jeu s'arrete un instant pendant chaque effet,
;     comme dans les jeux TRS-80 d'epoque.
; =============================================================================

VIDEO   EQU     3C00H           ; memoire video : 16 lignes x 64 colonnes
BUF     EQU     7000H           ; tampon d'ecran (1 Ko)
STACK   EQU     7000H           ; la pile descend sous le tampon

KROW0   EQU     3801H           ; @ A B C D E F G
KROW6   EQU     3840H           ; ENTER CLEAR BREAK HAUT BAS GAUCHE DROITE ESPACE
KALL    EQU     38FFH           ; toutes les rangees a la fois
K_E     EQU     20H             ; bit de la touche E (rangee 0)
K_F     EQU     40H             ; bit de la touche F (rangee 0)
K_BREAK EQU     04H             ; bits de la rangee 6
K_LEFT  EQU     20H
K_RIGHT EQU     40H
K_SPACE EQU     80H

ROWS    EQU     4               ; rangees d'envahisseurs (une ligne sur deux)
COLS    EQU     8               ; colonnes d'envahisseurs
NINV    EQU     ROWS*COLS
UFOLINE EQU     1               ; ligne de la soucoupe
SHLINE  EQU     13              ; ligne des boucliers
PLINE   EQU     15              ; ligne du canon
NBOMBS  EQU     3               ; bombes ennemies simultanees
DEATHT  EQU     45              ; duree de l'explosion du canon (images)
FRDELAY EQU     250             ; temporisation par image (tours de boucle de 26 cycles)

        ORG     5200H

; =============================================================================
; Programme principal
; =============================================================================
START:  DI
        LD      SP,STACK
        LD      HL,0
        LD      (HISCORE),HL
        LD      (SCORE),HL
        LD      A,3
        LD      (LIVES),A
        LD      A,R
        LD      (SEED),A

LANGSEL:                        ; ecran d'accueil : choix de la langue
        CALL    TITLE
GOMENU: CALL    MENU            ; Z = jouer, NZ = changer de langue
        JR      NZ,LANGSEL
        CALL    GAME
        JR      GOMENU

; -----------------------------------------------------------------------------
; Ecran d'accueil bilingue : attend E (English) ou F (Francais).
; -----------------------------------------------------------------------------
TITLE:  CALL    WAITREL
        CALL    CLSBUF
        LD      HL,T_TITLE
        LD      D,2
        CALL    PRCENT
        LD      HL,T_BYPAR
        LD      D,4
        CALL    PRCENT
        CALL    PARADE          ; une rangee d'envahisseurs decoratifs, ligne 7
        LD      HL,T_PRESSE
        LD      D,11
        CALL    PRCENT
        LD      HL,T_PRESSF
        LD      D,12
        CALL    PRCENT
        CALL    SHOW
TITLE1: CALL    RANDOM          ; brasse le generateur pendant l'attente
        LD      A,(KROW0)
        LD      B,0
        AND     K_E
        JR      NZ,TITLE2
        INC     B
        LD      A,(KROW0)
        AND     K_F
        JR      Z,TITLE1
TITLE2: LD      A,B
        LD      (LANG),A
        RET

; -----------------------------------------------------------------------------
; Menu dans la langue choisie : points, touches. ESPACE = jouer (Z),
; BREAK = retour au choix de la langue (NZ).
; -----------------------------------------------------------------------------
MENU:   CALL    WAITREL
        CALL    CLSBUF
        CALL    HUD
        LD      HL,T_TITLE
        LD      D,2
        CALL    PRCENT
        LD      A,S_BY
        CALL    STRING
        LD      D,3
        CALL    PRCENT
        ; Tableau des points : sprite, puis texte
        LD      HL,SPR_A1
        LD      DE,5*256+22
        CALL    PUT3
        LD      HL,T_30
        LD      DE,5*256+27
        CALL    PRINT
        LD      HL,SPR_B1
        LD      DE,7*256+22
        CALL    PUT3
        LD      HL,T_20
        LD      DE,7*256+27
        CALL    PRINT
        LD      HL,SPR_C1
        LD      DE,9*256+22
        CALL    PUT3
        LD      HL,T_10
        LD      DE,9*256+27
        CALL    PRINT
        LD      HL,SPR_UFO
        LD      DE,11*256+22
        CALL    PUT3
        LD      A,S_MYST
        CALL    STRING
        LD      DE,11*256+27
        CALL    PRINT
        LD      A,S_KEYS
        CALL    STRING
        LD      D,13
        CALL    PRCENT
        LD      A,S_START
        CALL    STRING
        LD      D,14
        CALL    PRCENT
        LD      A,S_LANG
        CALL    STRING
        LD      D,15
        CALL    PRCENT
        CALL    SHOW
MENU1:  CALL    RANDOM
        LD      A,(KROW6)
        BIT     2,A             ; BREAK
        RET     NZ              ; NZ : changer de langue
        AND     K_SPACE
        JR      Z,MENU1
        XOR     A               ; Z : jouer
        RET

; =============================================================================
; Une partie
; =============================================================================
GAME:   CALL    WAITREL
        LD      HL,0
        LD      (SCORE),HL
        LD      A,3
        LD      (LIVES),A
        LD      A,1
        LD      (WAVE),A

NEWWAVE:
        CALL    INITWAVE
        CALL    DRAW            ; la formation, sous la banniere
        LD      A,S_WAVE        ; « WAVE n » / « VAGUE n » pendant une seconde
        CALL    STRING
        CALL    BANNER
        LD      A,(WAVE)
        LD      L,A
        LD      H,0
        CALL    NUM5
        LD      HL,NUMSTR+3     ; deux chiffres suffisent
        LD      DE,8*256+38
        CALL    PRINT
        CALL    SHOW
        LD      B,40
        CALL    PAUSE

GLOOP:  LD      HL,FRAME
        INC     (HL)
        LD      A,(KROW6)       ; BREAK : retour au menu
        AND     K_BREAK
        RET     NZ
        LD      A,(DEAD)
        OR      A
        JR      NZ,GDYING
        CALL    PLAYER
        CALL    MISSILE
        CALL    BOMBS
        CALL    DROP
        CALL    UFO
        CALL    MARCH
        LD      A,(INVADED)     ; les envahisseurs ont atteint les boucliers
        OR      A
        JR      NZ,GOVER
        LD      A,(ALIVECNT)    ; vague terminee ?
        OR      A
        JR      NZ,GDRAW
        CALL    DRAW            ; laisse finir la derniere explosion
        CALL    SHOW
        LD      B,20
        CALL    PAUSE
        LD      HL,WAVE
        INC     (HL)
        JR      NEWWAVE

GDYING: DEC     A               ; explosion du canon en cours
        LD      (DEAD),A
        JR      NZ,GDRAW
        LD      A,(LIVES)
        OR      A
        JR      Z,GOVER
        LD      A,30            ; nouveau canon au centre
        LD      (PX),A

GDRAW:  CALL    DRAW
        CALL    SHOW
        CALL    DELAY
        JR      GLOOP

GOVER:  CALL    DRAW            ; fin de partie
        LD      A,S_OVER
        CALL    STRING
        CALL    BANNER
        CALL    SHOW
        LD      HL,(SCORE)      ; nouveau record ?
        LD      DE,(HISCORE)
        OR      A
        SBC     HL,DE
        JR      C,GOVER1
        LD      HL,(SCORE)
        LD      (HISCORE),HL
GOVER1: LD      B,100
        JP      PAUSE           ; puis retour au menu

; -----------------------------------------------------------------------------
; Prepare une vague : formation complete, boucliers neufs, plus de projectiles.
; -----------------------------------------------------------------------------
INITWAVE:
        LD      HL,ALIVE
        LD      B,NINV
IW1:    LD      (HL),1
        INC     HL
        DJNZ    IW1
        LD      A,NINV
        LD      (ALIVECNT),A
        LD      A,4
        LD      (FX),A
        LD      A,(WAVE)        ; chaque vague commence un peu plus bas
        DEC     A
        CP      2
        JR      C,IW2
        LD      A,2
IW2:    ADD     A,2
        LD      (FY),A
        LD      A,1
        LD      (FDIR),A
        LD      (MOVECNT),A
        XOR     A
        LD      (ANIM),A
        LD      (MACT),A
        LD      (UFOACT),A
        LD      (DEAD),A
        LD      (INVADED),A
        LD      HL,BOMBTAB
        LD      B,NBOMBS*3
IW3:    LD      (HL),A
        INC     HL
        DJNZ    IW3
        LD      A,30
        LD      (PX),A
        ; Boucliers : 4 blocs de 6 caracteres pleins
        LD      HL,SHIELDS
        LD      B,64
IW4:    LD      (HL),' '
        INC     HL
        DJNZ    IW4
        LD      IX,SHPOS
        LD      C,4
IW5:    LD      E,(IX+0)
        LD      D,0
        LD      HL,SHIELDS
        ADD     HL,DE
        LD      B,6
IW6:    LD      (HL),0BFH
        INC     HL
        DJNZ    IW6
        INC     IX
        DEC     C
        JR      NZ,IW5
        RET

SHPOS:  DEFB    7,21,37,51

; =============================================================================
; Logique du jeu (une image)
; =============================================================================

; Canon : deplacement et tir.
PLAYER: LD      A,(KROW6)
        LD      C,A
        LD      A,(PX)
        BIT     5,C             ; GAUCHE
        JR      Z,PL1
        OR      A
        JR      Z,PL1
        DEC     A
PL1:    BIT     6,C             ; DROITE
        JR      Z,PL2
        CP      61
        JR      NC,PL2
        INC     A
PL2:    LD      (PX),A
        BIT     7,C             ; ESPACE : tir, un seul missile a la fois
        RET     Z
        LD      A,(MACT)
        OR      A
        RET     NZ
        INC     A
        LD      (MACT),A
        LD      A,(PX)
        INC     A
        LD      (MX),A
        LD      A,PLINE-1
        LD      (MY),A
        JP      SNDFIRE

; Missile du joueur : monte d'une ligne par image et touche ce qu'il rencontre.
MISSILE:
        LD      A,(MACT)
        OR      A
        RET     Z
        LD      A,(MY)
        DEC     A
        LD      (MY),A
        JP      Z,MSOFF         ; sorti par le haut
        ; Soucoupe
        CP      UFOLINE
        JR      NZ,MS1
        LD      A,(UFOACT)
        OR      A
        JR      Z,MS1
        LD      A,(UFOX)
        LD      B,A
        LD      A,(MX)
        SUB     B
        CP      3
        JR      NC,MS1
        XOR     A
        LD      (UFOACT),A
        CALL    RANDOM          ; 50 a 300 points
        AND     7
        CP      6
        JR      C,MSU1
        SUB     4
MSU1:   INC     A
        LD      B,A
        LD      DE,50
        LD      HL,0
MSU2:   ADD     HL,DE
        DJNZ    MSU2
        CALL    ADDSCORE
        CALL    SNDUFO
        JP      MSOFF
MS1:    ; Boucliers
        LD      A,(MY)
        CP      SHLINE
        JR      NZ,MS2
        LD      A,(MX)
        CALL    HITSHIELD
        JP      NZ,MSOFF
MS2:    ; Envahisseurs : rangee = (MY - FY) / 2, colonne = (MX - FX) / 4
        LD      A,(FY)
        LD      B,A
        LD      A,(MY)
        SUB     B
        RET     C
        BIT     0,A             ; ligne vide entre deux rangees
        RET     NZ
        SRL     A
        CP      ROWS
        RET     NC
        LD      D,A             ; D = rangee
        LD      A,(FX)
        LD      B,A
        LD      A,(MX)
        SUB     B
        RET     C
        CP      COLS*4
        RET     NC
        LD      B,A
        AND     3
        CP      3               ; l'espace entre deux envahisseurs
        RET     Z
        LD      A,B
        SRL     A
        SRL     A
        LD      E,A             ; E = colonne
        CALL    INVADDR         ; HL = ALIVE[rangee*8 + colonne]
        LD      A,(HL)
        CP      1
        RET     NZ
        LD      (HL),5          ; explosion pendant quelques images
        LD      HL,ALIVECNT
        DEC     (HL)
        LD      A,D             ; points selon la rangee
        LD      HL,30
        OR      A
        JR      Z,MS3
        LD      HL,20
        CP      2
        JR      C,MS3
        LD      HL,10
MS3:    CALL    ADDSCORE
        CALL    SNDHIT
MSOFF:  XOR     A
        LD      (MACT),A
        RET

; HL = adresse de ALIVE pour la rangee D et la colonne E.
INVADDR:
        LD      A,D
        ADD     A,A
        ADD     A,A
        ADD     A,A
        ADD     A,E
        LD      C,A
        LD      B,0
        LD      HL,ALIVE
        ADD     HL,BC
        RET

; Projectile en colonne A sur la ligne des boucliers : abime le bouclier.
; NZ si un bouclier a ete touche.
HITSHIELD:
        LD      C,A
        LD      B,0
        LD      HL,SHIELDS
        ADD     HL,BC
        LD      A,(HL)
        CP      ' '
        RET     Z
        CP      0BFH            ; plein -> abime -> detruit
        LD      A,0A5H
        JR      Z,HS1
        LD      A,' '
HS1:    LD      (HL),A
        OR      1               ; NZ
        RET

; Bombes ennemies : descendent une image sur deux.
BOMBS:  LD      A,(FRAME)
        AND     1
        RET     NZ
        LD      IX,BOMBTAB
        LD      B,NBOMBS
BB1:    PUSH    BC
        LD      A,(IX+0)
        OR      A
        JR      Z,BBNEXT
        INC     (IX+2)
        LD      A,(IX+2)
        CP      SHLINE
        JR      NZ,BB2
        LD      A,(IX+1)
        CALL    HITSHIELD
        JR      NZ,BBOFF
        JR      BBNEXT
BB2:    CP      PLINE
        JR      C,BBNEXT
        JR      NZ,BBOFF        ; sous l'ecran
        LD      A,(PX)          ; le canon est-il touche ?
        LD      B,A
        LD      A,(IX+1)
        SUB     B
        CP      3
        JR      NC,BBNEXT
        CALL    KILLED
BBOFF:  LD      (IX+0),0
BBNEXT: LD      DE,3
        ADD     IX,DE
        POP     BC
        DJNZ    BB1
        RET

; Le canon est detruit.
KILLED: CALL    SNDDEATH
        LD      A,DEATHT
        LD      (DEAD),A
        LD      HL,LIVES
        DEC     (HL)
        XOR     A
        LD      (MACT),A
        LD      HL,BOMBTAB
        LD      B,NBOMBS*3
KL1:    LD      (HL),A
        INC     HL
        DJNZ    KL1
        RET

; Largage d'une bombe par l'envahisseur le plus bas d'une colonne au hasard.
DROP:   LD      A,(FRAME)
        AND     7
        RET     NZ
        CALL    RANDOM
        CP      150
        RET     NC
        LD      IX,BOMBTAB      ; une place libre ?
        LD      B,NBOMBS
        LD      DE,3
DR1:    LD      A,(IX+0)
        OR      A
        JR      Z,DR2
        ADD     IX,DE
        DJNZ    DR1
        RET
DR2:    CALL    RANDOM
        AND     7
        LD      E,A             ; E = colonne
        LD      D,ROWS-1        ; de la rangee du bas vers le haut
DR3:    PUSH    DE
        CALL    INVADDR
        POP     DE
        LD      A,(HL)
        CP      1
        JR      Z,DR4
        DEC     D
        JP      P,DR3
        RET                     ; colonne vide
DR4:    LD      (IX+0),1
        LD      A,D
        ADD     A,A
        LD      B,A
        LD      A,(FY)
        ADD     A,B
        INC     A
        LD      (IX+2),A        ; ligne sous l'envahisseur
        LD      A,E
        ADD     A,A
        ADD     A,A
        LD      B,A
        LD      A,(FX)
        ADD     A,B
        INC     A
        LD      (IX+1),A        ; colonne du milieu de l'envahisseur
        RET

; Soucoupe mystere : apparait au hasard, traverse la ligne 1.
UFO:    LD      A,(UFOACT)
        OR      A
        JR      NZ,UF2
        CALL    RANDOM
        OR      A
        RET     NZ              ; environ une chance sur 256 par image
        LD      A,(FRAME)
        RRCA
        LD      A,1             ; vers la droite, depuis la gauche
        LD      B,0
        JR      NC,UF1
        LD      A,0FFH          ; vers la gauche, depuis la droite
        LD      B,61
UF1:    LD      (UFODIR),A
        LD      A,B
        LD      (UFOX),A
        LD      A,1
        LD      (UFOACT),A
        RET
UF2:    LD      A,(FRAME)
        AND     1
        RET     NZ
        LD      A,(UFODIR)
        LD      B,A
        LD      A,(UFOX)
        ADD     A,B
        CP      62              ; sortie (et 0FFh en partant a gauche)
        JR      NC,UF3
        LD      (UFOX),A
        RET
UF3:    XOR     A
        LD      (UFOACT),A
        RET

; La formation avance d'un pas quand son compteur arrive a zero : plus il reste
; d'envahisseurs, plus elle est lente. Au bord, elle descend et change de sens.
MARCH:  LD      A,(ALIVECNT)    ; formation detruite : plus rien ne bouge
        OR      A
        RET     Z
        LD      HL,MOVECNT
        DEC     (HL)
        RET     NZ
        LD      A,(ALIVECNT)
        SRL     A
        SRL     A
        INC     A
        LD      (HL),A
        LD      A,(ANIM)
        XOR     1
        LD      (ANIM),A
        CALL    SNDSTEP
        CALL    EXTENT          ; B = col. min, C = col. max, D = rangee max
        LD      A,(FDIR)
        DEC     A
        JR      NZ,MA1
        LD      A,C             ; vers la droite : FX + 4*cmax + 2 < 63 ?
        ADD     A,A
        ADD     A,A
        LD      C,A
        LD      A,(FX)
        ADD     A,C
        CP      61
        JR      NC,MADOWN
        INC     A
        SUB     C
        LD      (FX),A
        JR      MACHK
MA1:    LD      A,B             ; vers la gauche : FX + 4*cmin > 0 ?
        ADD     A,A
        ADD     A,A
        LD      B,A
        LD      A,(FX)
        ADD     A,B
        JR      Z,MADOWN
        LD      A,(FX)
        DEC     A
        LD      (FX),A
        JR      MACHK
MADOWN: LD      HL,FY
        INC     (HL)
        LD      A,(FDIR)
        NEG
        LD      (FDIR),A
MACHK:  LD      A,D             ; la rangee du bas atteint-elle les boucliers ?
        ADD     A,A
        LD      D,A
        LD      A,(FY)
        ADD     A,D
        CP      SHLINE
        RET     C
        LD      A,1
        LD      (INVADED),A
        RET

; Etendue de la formation vivante : B = colonne min, C = colonne max,
; D = rangee max. Appelee seulement si au moins un envahisseur est vivant.
EXTENT: LD      B,0FFH          ; colonne min
        LD      C,0             ; colonne max
        XOR     A
        LD      (EXROW),A       ; rangee max
        LD      HL,ALIVE
        LD      E,0             ; rangee
XT1:    LD      D,0             ; colonne
XT2:    LD      A,(HL)
        CP      1
        JR      NZ,XT5
        LD      A,D
        CP      B
        JR      NC,XT3
        LD      B,A
XT3:    CP      C
        JR      C,XT4
        LD      C,A
XT4:    LD      A,E
        LD      (EXROW),A
XT5:    INC     HL
        INC     D
        LD      A,D
        CP      COLS
        JR      NZ,XT2
        INC     E
        LD      A,E
        CP      ROWS
        JR      NZ,XT1
        LD      A,(EXROW)
        LD      D,A
        RET

; =============================================================================
; Dessin d'une image dans le tampon
; =============================================================================
DRAW:   CALL    CLSBUF
        CALL    HUD
        ; Soucoupe
        LD      A,(UFOACT)
        OR      A
        JR      Z,DW1
        LD      A,(UFOX)
        LD      E,A
        LD      D,UFOLINE
        LD      HL,SPR_UFO
        CALL    PUT3
DW1:    ; Envahisseurs
        LD      IX,ALIVE
        LD      D,0             ; rangee
DW2:    LD      E,0             ; colonne
DW3:    LD      A,(IX+0)
        OR      A
        JR      Z,DW6
        PUSH    DE
        CP      1
        JR      Z,DW4
        DEC     A               ; explosion : decompte jusqu'a disparaitre
        CP      1
        JR      NZ,DW3B
        XOR     A
DW3B:   LD      (IX+0),A
        LD      HL,SPR_EXP
        JR      DW5
DW4:    LD      A,D             ; sprite selon la rangee et l'animation
        ADD     A,A
        LD      HL,ANIM
        ADD     A,(HL)
        ADD     A,A
        LD      C,A
        LD      B,0
        LD      HL,ROWSPR
        ADD     HL,BC
        LD      A,(HL)
        INC     HL
        LD      H,(HL)
        LD      L,A
DW5:    LD      A,E             ; position : ligne FY + 2*rangee, colonne FX + 4*col
        ADD     A,A
        ADD     A,A
        LD      B,A
        LD      A,(FX)
        ADD     A,B
        LD      E,A
        LD      A,D
        ADD     A,A
        LD      B,A
        LD      A,(FY)
        ADD     A,B
        LD      D,A
        CALL    PUT3
        POP     DE
DW6:    INC     IX
        INC     E
        LD      A,E
        CP      COLS
        JR      NZ,DW3
        INC     D
        LD      A,D
        CP      ROWS
        JR      NZ,DW2
        ; Boucliers
        LD      HL,SHIELDS
        LD      DE,BUF+SHLINE*64
        LD      BC,64
        LDIR
        ; Canon, ou son explosion
        LD      A,(PX)
        LD      E,A
        LD      D,PLINE
        LD      HL,SPR_PLY
        LD      A,(DEAD)
        OR      A
        JR      Z,DW7
        LD      HL,SPR_EXP
        AND     4               ; clignote
        JR      Z,DW7
        LD      HL,SPR_EXP2
DW7:    CALL    PUT3
        ; Missile
        LD      A,(MACT)
        OR      A
        JR      Z,DW8
        LD      A,(MX)
        LD      E,A
        LD      A,(MY)
        LD      D,A
        CALL    BUFADDR
        LD      (HL),95H        ; trait vertical (colonne de gauche)
DW8:    ; Bombes
        LD      IX,BOMBTAB
        LD      B,NBOMBS
DW9:    PUSH    BC
        LD      A,(IX+0)
        OR      A
        JR      Z,DW10
        LD      E,(IX+1)
        LD      D,(IX+2)
        CALL    BUFADDR
        LD      (HL),0AAH       ; trait vertical (colonne de droite)
DW10:   LD      DE,3
        ADD     IX,DE
        POP     BC
        DJNZ    DW9
        RET

; Ligne 0 : score, record, vague et vies.
HUD:    LD      A,S_SCORE
        CALL    STRING
        LD      DE,0*256+0
        CALL    PRINT
        LD      HL,(SCORE)
        CALL    NUM5
        LD      HL,NUMSTR
        LD      DE,0*256+7
        CALL    PRINT
        LD      A,S_HI
        CALL    STRING
        LD      DE,0*256+17
        CALL    PRINT
        LD      HL,(HISCORE)
        CALL    NUM5
        LD      HL,NUMSTR
        LD      DE,0*256+26
        CALL    PRINT
        LD      A,S_LIVES
        CALL    STRING
        LD      DE,0*256+50
        CALL    PRINT
        LD      A,(LIVES)
        ADD     A,'0'
        LD      DE,0*256+57
        CALL    BUFADDR
        LD      (HL),A
        RET

; Rangee d'envahisseurs decoratifs (ecran d'accueil).
PARADE: LD      HL,SPR_A1
        LD      DE,7*256+14
        CALL    PUT3
        LD      HL,SPR_B1
        LD      DE,7*256+20
        CALL    PUT3
        LD      HL,SPR_C1
        LD      DE,7*256+26
        CALL    PUT3
        LD      HL,SPR_UFO
        LD      DE,7*256+32
        CALL    PUT3
        LD      HL,SPR_C2
        LD      DE,7*256+38
        CALL    PUT3
        LD      HL,SPR_B2
        LD      DE,7*256+44
        CALL    PUT3
        LD      HL,SPR_A2
        LD      DE,7*256+50
        JP      PUT3

; Texte HL encadre au centre de la ligne 8 (vague, fin de partie).
BANNER: PUSH    HL
        LD      HL,BUF+7*64+16  ; efface les lignes 7 a 9, colonnes 16 a 47
        LD      C,3
BN1:    LD      B,32
BN2:    LD      (HL),' '
        INC     HL
        DJNZ    BN2
        LD      DE,32
        ADD     HL,DE
        DEC     C
        JR      NZ,BN1
        POP     HL
        LD      D,8
        JP      PRCENT

; =============================================================================
; Utilitaires
; =============================================================================

; Efface le tampon (espaces). Astuce de la pile : SP pointe sur la fin du tampon
; et chaque PUSH y ecrit deux espaces, environ 5 fois plus vite qu'un LDIR.
; Sans danger : les interruptions sont desactivees (DI au demarrage).
CLSBUF: LD      (SAVESP),SP
        LD      SP,BUF+1024
        LD      HL,2020H        ; deux espaces
        LD      B,64            ; 64 x 8 PUSH = 1024 octets
CB1:    PUSH    HL
        PUSH    HL
        PUSH    HL
        PUSH    HL
        PUSH    HL
        PUSH    HL
        PUSH    HL
        PUSH    HL
        DJNZ    CB1
        LD      SP,(SAVESP)
        RET

; Copie le tampon a l'ecran.
SHOW:   LD      HL,BUF
        LD      DE,VIDEO
        LD      BC,1024
        LDIR
        RET

; HL = BUF + D*64 + E (detruit BC).
BUFADDR:
        LD      L,D
        LD      H,0
        ADD     HL,HL
        ADD     HL,HL
        ADD     HL,HL
        ADD     HL,HL
        ADD     HL,HL
        ADD     HL,HL
        LD      C,E
        LD      B,0
        ADD     HL,BC
        LD      BC,BUF
        ADD     HL,BC
        RET

; Ecrit la chaine HL (terminee par 0) en ligne D, colonne E.
PRINT:  PUSH    HL
        CALL    BUFADDR
        EX      DE,HL
        POP     HL
PR1:    LD      A,(HL)
        OR      A
        RET     Z
        LD      (DE),A
        INC     HL
        INC     DE
        JR      PR1

; Ecrit la chaine HL centree sur la ligne D.
PRCENT: PUSH    HL
        LD      B,0             ; longueur
PC1:    LD      A,(HL)
        OR      A
        JR      Z,PC2
        INC     B
        INC     HL
        JR      PC1
PC2:    LD      A,64
        SUB     B
        SRL     A
        LD      E,A
        POP     HL
        JR      PRINT

; Dessine le sprite HL (3 caracteres) en ligne D, colonne E.
PUT3:   PUSH    HL
        CALL    BUFADDR
        EX      DE,HL
        POP     HL
        LDI
        LDI
        LDI
        RET

; Ajoute HL au score.
ADDSCORE:
        LD      DE,(SCORE)
        ADD     HL,DE
        LD      (SCORE),HL
        RET

; Convertit HL en 5 chiffres dans NUMSTR (termine par 0).
NUM5:   LD      IX,NUMSTR
        LD      DE,10000
        CALL    DIGIT
        LD      DE,1000
        CALL    DIGIT
        LD      DE,100
        CALL    DIGIT
        LD      DE,10
        CALL    DIGIT
        LD      A,L
        ADD     A,'0'
        LD      (IX+0),A
        LD      (IX+1),0
        RET
DIGIT:  LD      A,'0'-1
DG1:    INC     A
        OR      A
        SBC     HL,DE
        JR      NC,DG1
        ADD     HL,DE
        LD      (IX+0),A
        INC     IX
        RET

; HL = texte numero A dans la langue choisie.
STRING: ADD     A,A
        LD      C,A
        LD      B,0
        LD      HL,STR_EN
        LD      A,(LANG)
        OR      A
        JR      Z,ST1
        LD      HL,STR_FR
ST1:    ADD     HL,BC
        LD      A,(HL)
        INC     HL
        LD      H,(HL)
        LD      L,A
        RET

; Nombre pseudo-aleatoire dans A (registre R et generateur a decalage).
RANDOM: PUSH    BC
        LD      A,(SEED)
        LD      B,A
        RRCA
        RRCA
        RRCA
        XOR     1FH
        ADD     A,B
        SBC     A,0FFH
        LD      (SEED),A
        LD      B,A
        LD      A,R
        XOR     B
        POP     BC
        RET

; Attend que toutes les touches soient relachees.
WAITREL:
        LD      A,(KALL)
        OR      A
        JR      NZ,WAITREL
        RET

; Temporisation d'une image de jeu.
DELAY:  LD      BC,FRDELAY
DL1:    DEC     BC
        LD      A,B
        OR      C
        JR      NZ,DL1
        RET

; Pause de B trentiemes de seconde (26 cycles par tour, 1,774 MHz).
PAUSE:  PUSH    BC
        LD      BC,2270
PA1:    DEC     BC
        LD      A,B
        OR      C
        JR      NZ,PA1
        POP     BC
        DJNZ    PAUSE
        RET

; =============================================================================
; Son : sortie cassette (port FFh, bits 0-1 : 1 = haut, 2 = bas, 0 = repos).
; Le bit 3 reste a 0 (sinon l'ecran passerait en 32 colonnes). IX est preserve.
; =============================================================================

; Onde carree : H = demi-periode (tours de DJNZ de 13 cycles), L = demi-periodes.
BEEP:   LD      A,1
BP1:    OUT     (0FFH),A
        LD      B,H
BP2:    DJNZ    BP2
        XOR     3               ; 1 <-> 2
        DEC     L
        JR      NZ,BP1
        XOR     A
        OUT     (0FFH),A
        RET

; Bruit : L = impulsions de niveau aleatoire, H = duree d'une impulsion.
NOISE:  CALL    RANDOM
        AND     1
        INC     A               ; 1 ou 2
        OUT     (0FFH),A
        LD      B,H
NS1:    DJNZ    NS1
        DEC     L
        JR      NZ,NOISE
        XOR     A
        OUT     (0FFH),A
        RET

; Tir : glissando descendant (« piou »).
SNDFIRE:
        LD      H,6
SF1:    LD      L,4
        PUSH    HL
        CALL    BEEP
        POP     HL
        INC     H
        INC     H
        LD      A,H
        CP      30
        JR      C,SF1
        RET

; Envahisseur touche : bref bruit.
SNDHIT: LD      HL,10*256+90
        JP      NOISE

; Soucoupe touchee : deux notes.
SNDUFO: LD      HL,20*256+60
        CALL    BEEP
        LD      HL,12*256+100
        JP      BEEP

; Canon detruit : long bruit qui descend.
SNDDEATH:
        LD      HL,60*256+250
        CALL    NOISE
        LD      HL,90*256+200
        JP      NOISE

; Pas de la formation : quatre notes graves, en boucle.
SNDSTEP:
        LD      A,(STEPNOTE)
        INC     A
        AND     3
        LD      (STEPNOTE),A
        LD      C,A
        LD      B,0
        LD      HL,STEPS
        ADD     HL,BC
        LD      H,(HL)
        LD      L,6
        JP      BEEP

STEPS:  DEFB    150,165,180,195

; =============================================================================
; Sprites : 3 caracteres semi-graphiques (6 x 3 pixels)
; =============================================================================
                                ; ..XX..  .XXXX.  X.XX.X
SPR_A1: DEFB    98H,0BFH,0A4H
                                ; ..XX..  .XXXX.  .X..X.
SPR_A2: DEFB    0A8H,8FH,94H
                                ; .X..X.  XXXXXX  X.XX.X
SPR_B1: DEFB    9EH,0BCH,0ADH
                                ; X....X  XXXXXX  .X..X.
SPR_B2: DEFB    0ADH,8CH,9EH
                                ; XXXXXX  X.XX.X  X....X
SPR_C1: DEFB    97H,8FH,0ABH
                                ; XXXXXX  X.XX.X  .X..X.
SPR_C2: DEFB    0A7H,8FH,9BH
                                ; .XXXX.  XXXXXX  .X..X.
SPR_UFO:
        DEFB    0AEH,8FH,9DH
                                ; ..XX..  XXXXXX  XXXXXX
SPR_PLY:
        DEFB    0BCH,0BFH,0BCH
SPR_EXP:
        DEFB    99H,0A6H,99H    ; eclats
SPR_EXP2:
        DEFB    0A6H,99H,0A6H

; Sprite de chaque rangee (deux images d'animation par rangee).
ROWSPR: DEFW    SPR_A1,SPR_A2       ; 30 points
        DEFW    SPR_B1,SPR_B2       ; 20 points
        DEFW    SPR_C1,SPR_C2       ; 10 points
        DEFW    SPR_C1,SPR_C2       ; 10 points

; =============================================================================
; Textes (majuscules, sans accents : le Model I n'a ni minuscules ni accents)
; =============================================================================
T_TITLE:  DEFB  'V E 2 C U Y    I N V A D E R S',0
T_BYPAR:  DEFB  'BY / PAR  VE2CUY',0
T_PRESSE: DEFB  'PRESS  E  FOR ENGLISH',0
T_PRESSF: DEFB  'APPUYEZ SUR  F  POUR LE FRANCAIS',0
T_30:     DEFB  '= 30 POINTS',0
T_20:     DEFB  '= 20 POINTS',0
T_10:     DEFB  '= 10 POINTS',0

S_SCORE EQU     0               ; numeros des textes traduits
S_HI    EQU     1
S_LIVES EQU     2
S_WAVE  EQU     3
S_OVER  EQU     4
S_START EQU     5
S_KEYS  EQU     6
S_MYST  EQU     7
S_LANG  EQU     8
S_BY    EQU     9

STR_EN: DEFW    E_SCORE,E_HI,E_LIVES,E_WAVE,E_OVER,E_START,E_KEYS,E_MYST,E_LANG,E_BY
STR_FR: DEFW    F_SCORE,F_HI,F_LIVES,F_WAVE,F_OVER,F_START,F_KEYS,F_MYST,F_LANG,F_BY

E_SCORE: DEFB   'SCORE',0
E_HI:    DEFB   'HI-SCORE',0
E_LIVES: DEFB   'LIVES',0
E_WAVE:  DEFB   'W A V E',0
E_OVER:  DEFB   'G A M E   O V E R',0
E_START: DEFB   'PRESS SPACE TO START',0
E_KEYS:  DEFB   'ARROWS: MOVE     SPACE: FIRE     BREAK: QUIT',0
E_MYST:  DEFB   '= ? MYSTERY',0
E_LANG:  DEFB   'BREAK: CHANGE LANGUAGE',0
E_BY:    DEFB   'BY VE2CUY',0

F_SCORE: DEFB   'SCORE',0
F_HI:    DEFB   'RECORD',0
F_LIVES: DEFB   'VIES',0
F_WAVE:  DEFB   'V A G U E',0
F_OVER:  DEFB   'P A R T I E   T E R M I N E E',0
F_START: DEFB   'APPUYEZ SUR ESPACE POUR JOUER',0
F_KEYS:  DEFB   'FLECHES: DEPLACER   ESPACE: TIRER   BREAK: QUITTER',0
F_MYST:  DEFB   '= ? MYSTERE',0
F_LANG:  DEFB   'BREAK: CHANGER DE LANGUE',0
F_BY:    DEFB   'PAR VE2CUY',0

; =============================================================================
; Variables
; =============================================================================
LANG:     DEFS  1               ; 0 = anglais, 1 = francais
SEED:     DEFS  1
SCORE:    DEFS  2
HISCORE:  DEFS  2
LIVES:    DEFS  1
WAVE:     DEFS  1
FRAME:    DEFS  1
FX:       DEFS  1               ; colonne de la formation
FY:       DEFS  1               ; ligne de la formation
FDIR:     DEFS  1               ; 1 = droite, 0FFh = gauche
ANIM:     DEFS  1
MOVECNT:  DEFS  1
ALIVECNT: DEFS  1
INVADED:  DEFS  1
EXROW:    DEFS  1
ALIVE:    DEFS  NINV            ; 0 = detruit, 1 = vivant, 2-5 = explosion
PX:       DEFS  1               ; colonne du canon
DEAD:     DEFS  1               ; images d'explosion restantes
MACT:     DEFS  1               ; missile : actif, colonne, ligne
MX:       DEFS  1
MY:       DEFS  1
UFOACT:   DEFS  1
UFOX:     DEFS  1
UFODIR:   DEFS  1
BOMBTAB:  DEFS  NBOMBS*3        ; par bombe : active, colonne, ligne
SHIELDS:  DEFS  64              ; la ligne des boucliers
NUMSTR:   DEFS  6
STEPNOTE: DEFS  1               ; note du prochain pas de la formation
SAVESP:   DEFS  2               ; pile sauvegardee pendant CLSBUF

        END     START
