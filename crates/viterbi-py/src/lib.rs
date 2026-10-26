//! # Componente 9: Interfaz Python y Acelerador de Simulación (`viterbi-py`)
//!
//! Expone el motor a Python utilizando PyO3 e integra Rayon para paralelizar
//! masivamente las simulaciones de la Tasa de Error de Bit (BER).

use std::sync::Arc;
use pyo3::prelude::*;
use rayon::prelude::*;

use viterbi_core::config::CodecConfig;
use viterbi_core::fsm::TrellisTopology;
use viterbi_core::encoder::ConvolutionalEncoder;
use viterbi_core::decoder::viterbi::ViterbiEngine;
use viterbi_core::decoder::ChannelDecoder;
use viterbi_core::channel::{AwgnChannel, XorShift32};

#[pyclass(unsendable)]
pub struct PyViterbiEngine {
    config_arc: Arc<CodecConfig>,
    topology_arc: Arc<TrellisTopology>,
    encoder: ConvolutionalEncoder,
    decoder: ViterbiEngine,
}

#[pymethods]
impl PyViterbiEngine {
    /// Inicializa el motor de Viterbi desde Python.
    /// Ejemplo: `engine = PyViterbiEngine(2, 1, 7, [0o171, 0o133])`
    #[new]
    pub fn new(n: u8, k: u8, constraint_len: u8, polys: Vec<u32>) -> PyResult<Self> {
        let config = CodecConfig::new(n, k, constraint_len, polys)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        
        let config_arc = Arc::new(config);
        let topology_arc = Arc::new(TrellisTopology::new(config_arc.clone()));
        
        let encoder = ConvolutionalEncoder::new(config_arc.clone());
        let decoder = ViterbiEngine::new(topology_arc.clone());

        Ok(Self {
            config_arc,
            topology_arc,
            encoder,
            decoder,
        })
    }

    /// Reinicia los buffers internos del codificador y decodificador.
    pub fn reset(&mut self) {
        self.encoder.reset();
        self.decoder.reset();
    }

    /// Codifica una lista de enteros en Python (0s y 1s).
    pub fn encode(&mut self, input_bits: Vec<u8>) -> Vec<u8> {
        let mut output = vec![0; input_bits.len()];
        self.encoder.encode_stream(&input_bits, &mut output);
        output
    }

    /// Decodifica una lista de símbolos binarios recibidos (Hard Decision).
    pub fn decode(&mut self, received_symbols: Vec<u8>) -> Vec<u8> {
        let mut output = vec![0; received_symbols.len()];
        self.decoder.decode(&received_symbols, &mut output);
        output
    }

    /// Simula la Tasa de Error de Bit (BER) para múltiples puntos de Eb/N0 (en dB).
    /// Ejecuta el proceso de modulación, canal AWGN y decodificación en paralelo empleando Rayon.
    pub fn simulate_ber(&self, eb_n0_db_range: Vec<f32>, bits_per_point: usize, seed: u32) -> Vec<f32> {
        let code_rate = self.config_arc.code_rate() as f32;

        eb_n0_db_range
            .into_par_iter()
            .enumerate()
            .map(|(i, snr)| {
                // Se instancian motores locales en la memoria caché del hilo actual (L1/L2).
                // Al clonar los Arc de la topología, evitamos recalcular el Trellis en cada núcleo.
                let mut local_encoder = ConvolutionalEncoder::new(self.config_arc.clone());
                let mut local_decoder = ViterbiEngine::new(self.topology_arc.clone());
                let mut channel = AwgnChannel::new(snr, code_rate, seed.wrapping_add(i as u32));
                let mut prng = XorShift32::new(seed.wrapping_add(snr.to_bits()));

                let mut tx_bits = vec![0u8; bits_per_point];
                for bit in tx_bits.iter_mut() {
                    *bit = (prng.next_u32() & 1) as u8;
                }

                let mut encoded = vec![0u8; tx_bits.len()];
                local_encoder.encode_stream(&tx_bits, &mut encoded);

                // Modulación BPSK implícita + Inyección de Ruido AWGN
                channel.transmit_hard_decision(&mut encoded);

                let mut rx_bits = vec![0u8; tx_bits.len()];
                local_decoder.decode(&encoded, &mut rx_bits);

                let mut error_count = 0;
                for (tx, rx) in tx_bits.iter().zip(rx_bits.iter()) {
                    if tx != rx {
                        error_count += 1;
                    }
                }

                (error_count as f32) / (bits_per_point as f32)
            })
            .collect()
    }
}

/// Módulo de inicialización de la extensión nativa para Python.
#[pymodule]
fn viterbi_py(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyViterbiEngine>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_engine_initialization() {
        // Validación de inicialización exitosa a través del wrapper de Python
        let engine = PyViterbiEngine::new(2, 1, 3, vec![0b111, 0b101]);
        assert!(engine.is_ok());
    }
}

