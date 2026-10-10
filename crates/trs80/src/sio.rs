//! Ports série RS-232-C du Model II : un Z80 SIO (canaux A et B) sur les ports F4h-F7h.
//!
//! - F4h : données du canal A; F5h : données du canal B;
//! - F6h : commande / état du canal A; F7h : commande / état du canal B.
//!
//! Comme pour l'UART des autres modèles (`serial.rs`), le canal A est relié au « modem » de
//! la page par `Sio::send` et `Sio::take_output`; le canal B n'a rien au bout. Les octets
//! reçus arrivent un à la fois : le suivant attend que le programme ait lu le précédent, et
//! le temps d'un caractère. La vitesse vient du CTC : son canal 0 (temporisateur) cadence le
//! canal A, son canal 1 le canal B, divisés encore par 1, 16, 32 ou 64 selon WR4 (bits 7-6).
//! Ex. : OMNITERM, constante 52 et pré-diviseur 16, puis x16 : 4 MHz / 832 / 16 = 300 bauds.
//! Sans horloge programmée, 9600 bauds.
//!
//! Registres : WR0 choisit le registre suivant et porte les commandes (remise à zéro du
//! canal, des interruptions d'état, de l'interruption d'émission, réception de la première
//! touche); WR1 autorise les interruptions (bit 0 : état, bit 1 : émission, bit 2 du canal B :
//! l'état modifie le vecteur, bits 4-3 : réception); WR2 (canal B) est le vecteur; WR3 bit 0
//! autorise la réception. RR0 : octet reçu (bit 0), interruption en attente (bit 1, canal A),
//! émetteur libre (bit 2), porteuse (bit 3) et CTS (bit 5) toujours présents; RR1 : tout est
//! émis (bit 0); RR2 (canal B) : le vecteur.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

/// Vitesse sans horloge programmée.
const BAUD: u32 = 9600;

#[derive(Default)]
struct Channel {
    wr: [u8; 8],
    pointer: u8,
    rx_data: u8,
    rx_full: bool,
    /// Mode « interruption sur le premier caractère » : armé par la commande 4 de WR0.
    rx_first: bool,
    tx_busy: bool,
    tx_until: u64,
    next_receive: u64,
    rx_pending: bool,
    tx_pending: bool,
    ext_pending: bool,
    output: VecDeque<u8>,
    input: VecDeque<u8>,
}

impl Channel {
    fn reset(&mut self) {
        let (output, input) = (core::mem::take(&mut self.output), core::mem::take(&mut self.input));
        *self = Channel { output, input, ..Channel::default() };
    }

    fn rx_mode(&self) -> u8 {
        (self.wr[1] >> 3) & 3
    }
}

#[derive(Default)]
pub(crate) struct Sio {
    ch: [Channel; 2],
    /// Période de l'horloge de chaque canal (T-states), donnée par le CTC.
    clock: [Option<u32>; 2],
    hz: u32,
}

impl Sio {
    /// RESET de la machine : registres remis à zéro (les octets en route sont gardés).
    pub(crate) fn reset(&mut self) {
        self.ch.iter_mut().for_each(Channel::reset);
    }

    /// Horloges des canaux (périodes des canaux 0 et 1 du CTC) et fréquence du processeur.
    pub(crate) fn set_clocks(&mut self, clock: [Option<u32>; 2], hz: u32) {
        self.clock = clock;
        self.hz = hz;
    }

    /// Période d'un bit du canal `n` (T-states), à défaut d'horloge celle de 9600 bauds.
    fn bit_time(&self, n: usize) -> u64 {
        let mult = [1, 16, 32, 64][(self.ch[n].wr[4] >> 6) as usize];
        match self.clock[n] {
            Some(p) => p as u64 * mult,
            None => self.hz.max(1) as u64 / BAUD as u64,
        }
    }

    /// Durée d'un caractère (départ, 8 bits, arrêt).
    fn char_time(&self, n: usize) -> u64 {
        self.bit_time(n) * 10
    }

    pub(crate) fn read(&mut self, port: u8) -> u8 {
        let n = (port & 1) as usize;
        if port & 2 == 0 {
            let c = &mut self.ch[n];
            c.rx_full = false;
            c.rx_pending = false;
            return c.rx_data;
        }
        let any = self.ch.iter().any(|c| c.rx_pending || c.tx_pending || c.ext_pending);
        let vector = self.vector();
        let c = &mut self.ch[n];
        let reg = core::mem::take(&mut c.pointer);
        match reg {
            0 => c.rx_full as u8 | ((any && n == 0) as u8) << 1 | (!c.tx_busy as u8) << 2 | 0x08 | 0x20,
            1 => !c.tx_busy as u8,
            2 if n == 1 => vector,
            _ => 0,
        }
    }

    pub(crate) fn write(&mut self, port: u8, val: u8, now: u64) {
        let n = (port & 1) as usize;
        let char_time = self.char_time(n);
        let c = &mut self.ch[n];
        if port & 2 == 0 {
            if c.output.len() < 65_536 {
                c.output.push_back(val);
            }
            c.tx_busy = true;
            c.tx_pending = false;
            c.tx_until = now + char_time;
            return;
        }
        if c.pointer != 0 {
            c.wr[c.pointer as usize] = val;
            c.pointer = 0;
            return;
        }
        c.pointer = val & 7;
        match (val >> 3) & 7 {
            2 => c.ext_pending = false,
            3 => c.reset(),
            4 => c.rx_first = true,
            5 => c.tx_pending = false,
            _ => {}
        }
    }

    /// Avance le temps : fin d'émission, octet suivant à recevoir.
    pub(crate) fn tick(&mut self, now: u64) {
        let times = [self.char_time(0), self.char_time(1)];
        for (c, char_time) in self.ch.iter_mut().zip(times) {
            if c.tx_busy && now >= c.tx_until {
                c.tx_busy = false;
                c.tx_pending = c.wr[1] & 0x02 != 0;
            }
            if !c.rx_full && c.wr[3] & 0x01 != 0 && now >= c.next_receive
                && let Some(b) = c.input.pop_front()
            {
                c.rx_data = b;
                c.rx_full = true;
                c.next_receive = now + char_time;
                match c.rx_mode() {
                    1 if c.rx_first => {
                        c.rx_first = false;
                        c.rx_pending = true;
                    }
                    2 | 3 => c.rx_pending = true,
                    _ => {}
                }
            }
        }
    }

    /// Vecteur programmé (WR2 du canal B).
    fn vector(&self) -> u8 {
        self.ch[1].wr[2]
    }

    /// Interruption à demander, par ordre de priorité (canal A puis B; réception, émission,
    /// état) : (source, vecteur). Si l'état modifie le vecteur (WR1 bit 2 du canal B), les
    /// bits 3-1 donnent la cause : B émission 0, état 1, réception 2; A émission 4, état 5,
    /// réception 6.
    pub(crate) fn request(&self) -> Option<(usize, u8)> {
        let codes = [(0, 6u8), (1, 4), (2, 5)];
        for n in 0..2 {
            let c = &self.ch[n];
            for (kind, code) in codes {
                let pending = [c.rx_pending, c.tx_pending, c.ext_pending][kind];
                if pending {
                    let code = if n == 0 { code } else { code - 4 };
                    let v = if self.ch[1].wr[1] & 0x04 != 0 { (self.vector() & 0xF1) | code << 1 } else { self.vector() };
                    return Some((n * 3 + kind, v));
                }
            }
        }
        None
    }

    /// Le processeur a accepté l'interruption `source` (de `request`).
    pub(crate) fn acknowledge(&mut self, source: usize) {
        let c = &mut self.ch[source / 3];
        match source % 3 {
            0 => c.rx_pending = false,
            1 => c.tx_pending = false,
            _ => c.ext_pending = false,
        }
    }

    /// Octets venus de l'autre bout, à recevoir sur le canal A.
    pub(crate) fn send(&mut self, bytes: &[u8]) {
        self.ch[0].input.extend(bytes);
    }

    /// Octets émis sur le canal A depuis le dernier appel.
    pub(crate) fn take_output(&mut self) -> Vec<u8> {
        self.ch[0].output.drain(..).collect()
    }

    /// Octets reçus de l'autre bout, pas encore lus par le TRS-80.
    pub(crate) fn pending_input(&self) -> usize {
        self.ch[0].input.len() + self.ch[0].rx_full as usize
    }

    /// Vitesse du canal A (bauds).
    pub(crate) fn baud(&self) -> u32 {
        (self.hz as u64 / self.bit_time(0).max(1)) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLOCK: u32 = 4_000_000;

    #[test]
    fn channel_a_talks_with_interrupts() {
        let mut s = Sio::default();
        s.set_clocks([None, None], CLOCK);
        // Canal B : vecteur 70h, l'état modifie le vecteur. Canal A : réception (toutes les
        // touches) et émission par interruption.
        s.write(0xF7, 0x02, 0);
        s.write(0xF7, 0x70, 0);
        s.write(0xF7, 0x01, 0);
        s.write(0xF7, 0x04, 0);
        s.write(0xF6, 0x03, 0);
        s.write(0xF6, 0xC1, 0);
        s.write(0xF6, 0x01, 0);
        s.write(0xF6, 0x12, 0);
        // Réception : interruption, vecteur 70h | 6 << 1.
        s.send(b"OK");
        s.tick(1);
        assert_eq!(s.request(), Some((0, 0x7C)));
        assert_eq!(s.read(0xF6) & 1, 1);
        assert_eq!(s.read(0xF4), b'O');
        assert_eq!(s.request(), None);
        s.tick(2);
        assert_eq!(s.read(0xF6) & 1, 0, "le suivant attend le temps d'un caractère");
        s.tick(10_000);
        assert_eq!(s.read(0xF4), b'K');
        assert_eq!(s.pending_input(), 0);
        // Émission : l'émetteur est occupé, puis libre avec une interruption (vecteur 78h).
        s.write(0xF4, b'A', 20_000);
        assert_eq!(s.read(0xF6) & 4, 0);
        s.tick(30_000);
        assert_eq!(s.read(0xF6) & 4, 4);
        assert_eq!(s.request(), Some((1, 0x78)));
        s.acknowledge(1);
        assert_eq!(s.request(), None);
        assert_eq!(s.take_output(), b"A");
    }
}
