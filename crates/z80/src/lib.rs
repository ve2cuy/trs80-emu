//! Cœur Z80 pour l'émulateur TRS-80 Model 1.
//!
//! - `no_std` : aucune dépendance au système ni au web; le même code peut tourner
//!   en natif, en WebAssembly ou sur un microcontrôleur.
//! - Le processeur ne connaît le reste de la machine qu'à travers le trait [`Bus`].
//! - [`Cpu::step`] exécute une instruction complète et retourne sa durée en T-states.
//!
//! ```
//! use z80::{Bus, Cpu};
//!
//! struct Ram([u8; 65536]);
//! impl Bus for Ram {
//!     fn read(&mut self, addr: u16) -> u8 { self.0[addr as usize] }
//!     fn write(&mut self, addr: u16, val: u8) { self.0[addr as usize] = val; }
//! }
//!
//! let mut ram = Ram([0; 65536]);
//! ram.0[..3].copy_from_slice(&[0x3E, 0x2A, 0x76]); // LD A,42 ; HALT
//! let mut cpu = Cpu::new();
//! while !cpu.halted {
//!     cpu.step(&mut ram);
//! }
//! assert_eq!(cpu.a, 42);
//! ```
#![no_std]

mod bus;
mod cpu;
pub mod flags;

pub use bus::Bus;
pub use cpu::Cpu;
