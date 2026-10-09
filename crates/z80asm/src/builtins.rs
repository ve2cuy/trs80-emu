//! Symboles prédéfinis : routines de la ROM Level II et services (SVC) de LDOS 5.3.1 pour
//! le Model I. Un programme peut les appeler sans les définir (CALL @DSPLY); une définition
//! du programme (@DSPLY EQU ...) a priorité.

/// Un symbole prédéfini.
#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub name: &'static str,
    pub value: u16,
    /// Service de LDOS (sinon routine de la ROM, utilisable aussi sans DOS).
    pub ldos: bool,
}

const fn rom(name: &'static str, value: u16) -> Builtin {
    Builtin { name, value, ldos: false }
}

const fn ldos(name: &'static str, value: u16) -> Builtin {
    Builtin { name, value, ldos: true }
}

pub const BUILTINS: &[Builtin] = &[
    // ROM Level II
    rom("@KBD", 0x002B),   // lit le clavier sans attendre : A = touche ou 0
    rom("@DSP", 0x0033),   // affiche le caractère de A
    rom("@PRT", 0x003B),   // imprime le caractère de A
    rom("@KEYIN", 0x0040), // saisit une ligne : HL = tampon, B = longueur maximale
    rom("@KEY", 0x0049),   // attend une touche : A
    rom("@PAUSE", 0x0060), // attend BC × 14,7 µs
    rom("@CLS", 0x01C9),   // efface l'écran
    // LDOS 5.3.1, Model I
    ldos("@EXIT", 0x402D),  // retour à LDOS
    ldos("@ABORT", 0x4030), // abandon, retour à LDOS
    ldos("@ERROR", 0x4409), // affiche l'erreur A
    ldos("@DEBUG", 0x440D), // entre dans DEBUG
    ldos("@FSPEC", 0x441C), // HL = nom de fichier, DE = FCB
    ldos("@INIT", 0x4420),  // ouvre ou crée : DE = FCB, HL = tampon, B = LRL
    ldos("@OPEN", 0x4424),  // ouvre un fichier existant
    ldos("@CLOSE", 0x4428), // ferme : DE = FCB
    ldos("@KILL", 0x442C),  // supprime : DE = FCB
    ldos("@LOAD", 0x4430),  // charge un programme : DE = FCB
    ldos("@RUN", 0x4433),   // charge et lance un programme
    ldos("@READ", 0x4436),  // lit un enregistrement
    ldos("@WRITE", 0x4439), // écrit un enregistrement
    ldos("@VER", 0x443C),   // écrit et vérifie
    ldos("@REW", 0x443F),   // revient au début du fichier
    ldos("@POSN", 0x4442),  // se place à l'enregistrement BC
    ldos("@BKSP", 0x4445),  // recule d'un enregistrement
    ldos("@PEOF", 0x4448),  // se place à la fin du fichier
    ldos("@DSPLY", 0x4467), // affiche le message HL (terminé par 0DH ou 03H)
    ldos("@PRINT", 0x446A), // imprime le message HL
    ldos("@TIME", 0x446D),  // HL = tampon de 8 octets : HH:MM:SS
    ldos("@DATE", 0x4470),  // HL = tampon de 8 octets : MM/DD/YY
];

/// Adresses du Model III (LDOS 5.1) qui diffèrent sur le Model I : (nom, Model III, Model I).
pub const MODEL3: &[(&str, u16, u16)] = &[("@TIME", 0x3036, 0x446D), ("@DATE", 0x3033, 0x4470)];

pub fn find(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.name == name)
}
