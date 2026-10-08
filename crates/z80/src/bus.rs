/// Ce que le Z80 voit du reste de la machine : 64 Ko de mémoire et 64 K ports d'E/S.
///
/// La machine (TRS-80, banc d'essai, etc.) implémente ce trait et décide de ce
/// qui se trouve à chaque adresse : ROM, RAM, clavier, vidéo, contrôleur de disquettes...
pub trait Bus {
    /// Lecture d'un octet en mémoire.
    fn read(&mut self, addr: u16) -> u8;

    /// Écriture d'un octet en mémoire.
    fn write(&mut self, addr: u16, val: u8);

    /// Lecture d'un port d'E/S (`IN`). Le port est sur 16 bits : l'octet haut
    /// vient de A ou de B selon l'instruction, comme sur le vrai bus d'adresses.
    fn input(&mut self, _port: u16) -> u8 {
        0xFF
    }

    /// Écriture sur un port d'E/S (`OUT`).
    fn output(&mut self, _port: u16, _val: u8) {}
}
