//! # Viterbi Core (`viterbi-core`)
//!
//! Motor matemático de bajo nivel para Codificación Convolucional, Topología de Trellis 1D,
//! Decodificación por Algoritmo de Viterbi (Hard/Soft Decision) y Decodificación Secuencial de Fano.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod config;

// Los siguientes submódulos se habilitarán incrementalmente en los próximos turnos:
pub mod fsm;
pub mod encoder;
pub mod decoder;
// pub mod channel;

pub use config::{CodecConfig, ConfigError};
