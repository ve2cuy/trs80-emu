//! Son : la sortie cassette (bits 0-1 du port FFh) sert de haut-parleur.
//! Les programmes font du son en basculant ces bits; on en tire des échantillons audio.
//!
//! Le niveau de sortie est moyenné entre deux échantillons (filtre « boîte »), puis un
//! filtre passe-haut retire la composante continue (pas de clic quand le son s'arrête).

/// Échantillons conservés entre deux lectures par l'hôte (environ 0,18 s à 44,1 kHz).
pub const AUDIO_CAPACITY: usize = 8192;
/// Volume : le signal du Model I est carré, on le garde modéré.
const VOLUME: f32 = 0.25;

pub(crate) struct Audio {
    /// Fréquence d'échantillonnage; 0 = son désactivé.
    rate: u32,
    samples: [f32; AUDIO_CAPACITY],
    len: usize,
    /// Somme niveau × cycles depuis le dernier échantillon, et nombre de cycles.
    sum: f32,
    cycles: u32,
    /// Avance vers le prochain échantillon, en « cycles × fréquence » (sans erreur cumulée).
    phase: u64,
    prev_in: f32,
    prev_out: f32,
    /// Fréquence du processeur (T-states par seconde).
    clock: u64,
}

impl Audio {
    /// Fréquence du processeur de la machine (T-states par seconde).
    pub(crate) fn set_clock(&mut self, hz: u32) {
        self.clock = hz as u64;
    }

    pub(crate) fn new() -> Self {
        Audio {
            rate: 0,
            samples: [0.0; AUDIO_CAPACITY],
            len: 0,
            sum: 0.0,
            cycles: 0,
            phase: 0,
            prev_in: 0.0,
            prev_out: 0.0,
            clock: crate::CLOCK_HZ as u64,
        }
    }

    pub(crate) fn set_rate(&mut self, rate: u32) {
        self.rate = rate;
        self.len = 0;
    }

    /// Niveau (-1, 0 ou +1) correspondant aux bits 0-1 du port FFh.
    pub(crate) fn level(bits: u8) -> f32 {
        match bits & 3 {
            1 => 1.0,
            2 => -1.0,
            _ => 0.0,
        }
    }

    /// Le niveau `level` a duré `t` T-states.
    #[inline]
    pub(crate) fn advance(&mut self, level: f32, t: u32) {
        if self.rate == 0 {
            return;
        }
        self.sum += level * t as f32;
        self.cycles += t;
        self.phase += t as u64 * self.rate as u64;
        while self.phase >= self.clock {
            self.phase -= self.clock;
            let x = if self.cycles > 0 { self.sum / self.cycles as f32 } else { level };
            self.sum = 0.0;
            self.cycles = 0;
            // Passe-haut d'ordre 1 : y = x - x[n-1] + 0,995·y[n-1]
            let y = x - self.prev_in + 0.995 * self.prev_out;
            self.prev_in = x;
            self.prev_out = y;
            if self.len < AUDIO_CAPACITY {
                self.samples[self.len] = y * VOLUME;
                self.len += 1;
            }
        }
    }

    pub(crate) fn samples(&self) -> &[f32] {
        &self.samples[..self.len]
    }

    pub(crate) fn clear(&mut self) {
        self.len = 0;
    }
}
