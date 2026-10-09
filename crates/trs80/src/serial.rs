//! Port série RS-232-C (carte Radio Shack du Model I, intégré aux Model III et 4) : une
//! UART TR1602 sur les ports E8h-EBh, comme dans xtrs.
//!
//! - E8h : lecture : état du modem (CTS, DSR, porteuse : toujours présents); écriture :
//!   réinitialisation.
//! - E9h : lecture : interrupteurs de configuration (Model I); écriture : vitesse (quartet bas :
//!   réception, haut : émission, codes 0 à 15 de 50 à 19 200 bauds).
//! - EAh : lecture : état de l'UART (bit 7 : octet reçu, bit 6 : émetteur libre); écriture :
//!   format (longueur, parité, arrêt), DTR, RTS.
//! - EBh : données (lecture : octet reçu; écriture : octet à émettre).
//!
//! L'autre bout (le « modem » de la page, puis Internet) échange des octets par
//! `Serial::send` et `Serial::take_output`. Les octets reçus arrivent au rythme de la
//! vitesse choisie, un à la fois : le suivant attend que le programme ait lu le précédent.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

const RECEIVED: u8 = 0x80;
const SENT: u8 = 0x40;
/// État du modem : CTS, DSR et porteuse (CD).
const MODEM: u8 = 0x80 | 0x40 | 0x20;
/// Interrupteurs du Model I : 9600 bauds, 8 bits, sans parité (valeur de xtrs).
const SWITCHES: u8 = 0x07 | 0x08 | 0x60;
/// Bits des interruptions du Model III et 4 (port E0h) : le pilote RS232T de LDOS autorise
/// 20h pour sa réception par interruption.
const IRQ_RECEIVED: u8 = 0x20;
const IRQ_SENT: u8 = 0x10;
const BAUDS: [u32; 16] = [50, 75, 110, 134, 150, 300, 600, 1200, 1800, 2000, 2400, 3600, 4800, 7200, 9600, 19200];

pub(crate) struct Serial {
    status: u8,
    control: u8,
    baud: u8,
    data: u8,
    /// Vers l'autre bout (émis par le TRS-80), et depuis l'autre bout (à recevoir).
    output: VecDeque<u8>,
    input: VecDeque<u8>,
    /// Fin de l'émission en cours, et prochaine réception possible (T-states).
    sending_until: u64,
    next_receive: u64,
    /// Interruptions en attente (Model III et 4) : levées à l'arrivée d'un octet ou à la
    /// fin d'une émission, effacées par la lecture du port E0h ou l'accès aux données. (Une
    /// interruption « émetteur libre » permanente bloquerait LDOS dans son répartiteur.)
    irq: u8,
}

impl Serial {
    pub(crate) fn new() -> Self {
        Serial {
            status: SENT,
            control: 0x6E,
            baud: 0xEE,
            data: 0,
            output: VecDeque::new(),
            input: VecDeque::new(),
            sending_until: 0,
            next_receive: 0,
            irq: 0,
        }
    }

    /// Durée d'un caractère (bit de départ, données, parité, arrêt), en T-states.
    fn char_time(&self, clock_hz: u32, baud_code: u8) -> u64 {
        let word = [5, 7, 6, 8][((self.control >> 5) & 3) as usize];
        let parity = if self.control & 0x08 != 0 { 0 } else { 1 };
        let stop = if self.control & 0x10 != 0 { 2 } else { 1 };
        let bits = 1 + word + parity + stop;
        clock_hz as u64 * bits / BAUDS[baud_code as usize & 15] as u64
    }

    /// Vitesse d'émission choisie par le programme (bauds).
    pub(crate) fn baud(&self) -> u32 {
        BAUDS[(self.baud >> 4) as usize]
    }

    /// Avance le temps : fin d'émission, octet suivant à recevoir.
    pub(crate) fn tick(&mut self, now: u64, clock_hz: u32) {
        if self.status & SENT == 0 && now >= self.sending_until {
            self.status |= SENT;
            self.irq |= IRQ_SENT;
        }
        if self.status & RECEIVED == 0 && now >= self.next_receive
            && let Some(b) = self.input.pop_front()
        {
            self.data = b;
            self.status |= RECEIVED;
            self.irq |= IRQ_RECEIVED;
            self.next_receive = now + self.char_time(clock_hz, self.baud & 0x0F);
        }
    }

    pub(crate) fn read(&mut self, port: u8) -> u8 {
        match port & 3 {
            0 => MODEM,
            1 => SWITCHES,
            2 => self.status,
            _ => {
                self.status &= !RECEIVED;
                self.irq &= !IRQ_RECEIVED;
                self.data
            }
        }
    }

    pub(crate) fn write(&mut self, port: u8, val: u8, now: u64, clock_hz: u32) {
        match port & 3 {
            0 => self.status = (self.status & RECEIVED) | SENT,
            1 => self.baud = val,
            2 => self.control = val,
            _ => {
                if self.output.len() < 65_536 {
                    self.output.push_back(val);
                }
                self.status &= !SENT;
                self.irq &= !IRQ_SENT;
                self.sending_until = now + self.char_time(clock_hz, self.baud >> 4);
            }
        }
    }

    /// Interruptions du Model III et 4 (bits du port E0h) : 20h octet reçu, 10h émission finie.
    pub(crate) fn interrupts(&self) -> u8 {
        self.irq
    }

    /// Lecture du port E0h : les interruptions signalées sont acquittées.
    pub(crate) fn acknowledge(&mut self) {
        self.irq = 0;
    }

    /// Octets venus de l'autre bout, à recevoir par le TRS-80.
    pub(crate) fn send(&mut self, bytes: &[u8]) {
        self.input.extend(bytes);
    }

    /// Octets émis par le TRS-80 depuis le dernier appel.
    pub(crate) fn take_output(&mut self) -> Vec<u8> {
        self.output.drain(..).collect()
    }

    /// Octets reçus de l'autre bout, pas encore lus par le TRS-80.
    pub(crate) fn pending_input(&self) -> usize {
        self.input.len() + (self.status & RECEIVED != 0) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLOCK: u32 = 1_774_080;

    #[test]
    fn bytes_go_both_ways_at_the_chosen_speed() {
        let mut s = Serial::new();
        s.write(0xE9, 0x55, 0, CLOCK); // 300 bauds
        s.write(0xEA, 0x6C, 0, CLOCK); // 8 bits, sans parité, 1 arrêt
        // Émission : l'émetteur est occupé le temps d'un caractère (10 bits à 300 bauds).
        s.write(0xEB, b'A', 0, CLOCK);
        assert_eq!(s.read(0xEA) & SENT, 0);
        s.tick(CLOCK as u64 * 10 / 300 + 1, CLOCK);
        assert_eq!(s.read(0xEA) & SENT, SENT);
        assert_eq!(s.take_output(), b"A");
        // Réception : un octet à la fois, au rythme de la ligne.
        s.send(b"OK");
        s.tick(100_000, CLOCK);
        assert_eq!(s.read(0xEA) & RECEIVED, RECEIVED);
        assert_eq!(s.read(0xEB), b'O');
        s.tick(100_001, CLOCK);
        assert_eq!(s.read(0xEA) & RECEIVED, 0, "le suivant attend le temps d'un caractère");
        s.tick(200_000, CLOCK);
        assert_eq!(s.read(0xEB), b'K');
        assert_eq!(s.pending_input(), 0);
    }
}
