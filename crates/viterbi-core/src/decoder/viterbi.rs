//! # Componente 5: Motor Viterbi y Unidad ACS (`decoder/viterbi.rs`)
//!
//! Implementa el algoritmo de Viterbi utilizando una matriz circular aplanada para el traceback
//! y aritmética de saturación para la Métrica de Ruta (PM).

use std::sync::Arc;
use crate::fsm::TrellisTopology;
use super::{ChannelDecoder, DecoderDiagnostic, ChainStatus};

/// Decodificador de Viterbi optimizado para ejecución en streaming o por bloques.
pub struct ViterbiEngine {
    topology: Arc<TrellisTopology>,
    /// Búfer de métricas de ruta del paso t-1 (Ping-Pong 0)
    pm_prev: Vec<u32>,
    /// Búfer de métricas de ruta del paso t (Ping-Pong 1)
    pm_curr: Vec<u32>,
    /// Búfer circular aplanado para el historial de transiciones (almacena el estado previo).
    /// Indexado como `idx = (t % L) * N_s + state`.
    survivor_ring: Vec<u16>,
    /// Búfer paralelo para reconstruir los bits de entrada emitidos.
    input_ring: Vec<u8>,
    /// Búfer de observabilidad para reportar ambigüedades en la unidad ACS.
    tie_detected: Vec<bool>,
    /// Profundidad del búfer circular de traceback ($L = 5K$).
    traceback_depth: usize,
    /// Índice de tiempo absoluto $t$.
    t: usize,
}

impl ViterbiEngine {
    /// Preasigna el motor de Viterbi instanciando arreglos en memoria $O(1)$ basados
    /// en la profundidad del grafo y las métricas topológicas.
    pub fn new(topology: Arc<TrellisTopology>) -> Self {
        let config = &topology.config;
        let num_states = config.num_states();
        let l_depth = config.default_traceback_depth();
        let ring_size = l_depth * num_states;

        let mut pm_prev = vec![u32::MAX; num_states];
        // La condición de frontera topológica siempre comienza en el estado 0.
        pm_prev[0] = 0;

        Self {
            topology,
            pm_prev,
            pm_curr: vec![0; num_states],
            survivor_ring: vec![0; ring_size],
            input_ring: vec![0; ring_size],
            tie_detected: vec![false; ring_size],
            traceback_depth: l_depth,
            t: 0,
        }
    }
}

impl ChannelDecoder for ViterbiEngine {
    fn reset(&mut self) {
        self.pm_prev.fill(u32::MAX);
        self.pm_prev[0] = 0;
        self.t = 0;
        // No es necesario limpiar los anillos (rings); se sobreescriben cíclicamente.
    }

    fn decode(&mut self, received_symbols: &[u8], output_bits: &mut [u8]) -> DecoderDiagnostic {
        let num_states = self.topology.config.num_states();
        let l_depth = self.traceback_depth;
        let output_mask = self.topology.config.output_mask();
        
        let mut total_tie_breaks = 0;
        let num_steps = received_symbols.len();
        let out_len = output_bits.len();
        let min_len = num_steps.min(out_len);

        for step in 0..min_len {
            // Se asume decisión dura empaquetada. Se enmascara para ignorar bits de ruido espurio.
            let z_t = received_symbols[step] & output_mask;
            
            let mut global_min_pm = u32::MAX;
            
            // Unidad Add-Compare-Select (ACS)
            for state in 0..num_states {
                let mut min_pm_candidate = u32::MAX;
                let mut best_prev = 0;
                let mut best_input = 0;
                let mut tie = false;

                let branches = self.topology.backward_branches(state);
                
                for branch in branches {
                    let pm_prev_val = self.pm_prev[branch.prev_state];
                    if pm_prev_val == u32::MAX { continue; } // Nodo inalcanzable

                    // Métrica de Rama (Branch Metric) usando Distancia de Hamming en GF(2)
                    let bm = (z_t ^ branch.output_symbol).count_ones() as u32;
                    let pm_candidate = pm_prev_val.saturating_add(bm);

                    if pm_candidate < min_pm_candidate {
                        min_pm_candidate = pm_candidate;
                        best_prev = branch.prev_state;
                        best_input = branch.input_symbol;
                        tie = false;
                    } else if pm_candidate == min_pm_candidate {
                        // Telemetría: Colisión de métricas
                        tie = true;
                    }
                }

                self.pm_curr[state] = min_pm_candidate;
                
                let idx = (self.t % l_depth) * num_states + state;
                self.survivor_ring[idx] = best_prev as u16;
                self.input_ring[idx] = best_input;
                self.tie_detected[idx] = tie;

                if tie {
                    total_tie_breaks += 1;
                }

                if min_pm_candidate < global_min_pm {
                    global_min_pm = min_pm_candidate;
                }
            }

            // Normalización para prevenir desbordamiento entero en tramas continuas (sliding window)
            for state in 0..num_states {
                if self.pm_curr[state] != u32::MAX {
                    self.pm_curr[state] -= global_min_pm;
                }
            }

            // Rotación de Ping-Pong Buffers
            self.pm_prev.copy_from_slice(&self.pm_curr);

            // Operación de Traceback Continuo si el buffer está lleno
            if self.t >= l_depth - 1 {
                let mut curr_state = 0;
                let mut min_val = u32::MAX;
                for s in 0..num_states {
                    if self.pm_curr[s] < min_val {
                        min_val = self.pm_curr[s];
                        curr_state = s;
                    }
                }

                let mut trace_state = curr_state;
                let mut decoded_symbol = 0;
                
                // Recorremos retrospectivamente la profundidad L
                for d in 0..l_depth {
                    let back_t = self.t - d;
                    let idx = (back_t % l_depth) * num_states + trace_state;
                    decoded_symbol = self.input_ring[idx];
                    trace_state = self.survivor_ring[idx] as usize;
                }
                
                // Escribimos el símbolo resuelto en la cabeza de la ventana deslizante
                let emit_t = self.t + 1 - l_depth;
                if emit_t < out_len {
                    output_bits[emit_t] = decoded_symbol;
                }
            }
            
            self.t += 1;
        }

        // Finalización: Purgado de los bits remanentes en el búfer
        let mut curr_state = 0;
        let mut min_val = u32::MAX;
        for s in 0..num_states {
            if self.pm_curr[s] < min_val {
                min_val = self.pm_curr[s];
                curr_state = s;
            }
        }

        let final_best_state = curr_state;
        let flush_count = self.t.min(l_depth - 1);
        let mut trace_state = curr_state;
        
        for d in 0..flush_count {
            let back_t = self.t - 1 - d;
            let idx = (back_t % l_depth) * num_states + trace_state;
            let u = self.input_ring[idx];
            trace_state = self.survivor_ring[idx] as usize;
            
            if back_t < out_len {
                output_bits[back_t] = u;
            }
        }

        let status = if final_best_state == 0 {
            ChainStatus::Valid
        } else {
            ChainStatus::Corrupt
        };

        DecoderDiagnostic {
            status,
            total_tie_breaks,
            final_best_state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CodecConfig;
    use crate::encoder::ConvolutionalEncoder;

    #[test]
    fn test_viterbi_codec_integration_and_error_correction() {
        let config = Arc::new(CodecConfig::from_octal_strs(2, 1, 7, &["171", "133"]).unwrap());
        let topology = Arc::new(TrellisTopology::new(config.clone()));
        let mut encoder = ConvolutionalEncoder::new(config);
        
        // Mensaje de 10 bits seguido de 6 bits de zero-tailing (K-1) para forzar terminación en el estado 0.
        let input_bits = [1, 0, 1, 1, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0];
        let mut encoded = vec![0; input_bits.len()];
        encoder.encode_stream(&input_bits, &mut encoded);
        
        // Simulador de canal BSC: inyectamos un error de bit (Hamming = 1) en el cuarto símbolo
        encoded[3] ^= 0b01; 
        
        let mut viterbi = ViterbiEngine::new(topology);
        let mut decoded = vec![0; input_bits.len()];
        
        let diagnostic = viterbi.decode(&encoded, &mut decoded);
        
        // Verificación end-to-end
        assert_eq!(decoded, input_bits, "El motor falló corrigiendo el error inyectado");
        assert_eq!(diagnostic.status, ChainStatus::Valid);
        assert_eq!(diagnostic.final_best_state, 0);
    }

    #[test]
    fn test_viterbi_corrupt_tail_detection() {
        let config = Arc::new(CodecConfig::new(2, 1, 3, vec![0b111, 0b101]).unwrap());
        let topology = Arc::new(TrellisTopology::new(config.clone()));
        let mut encoder = ConvolutionalEncoder::new(config);
        
        // Trama cortada abruptamente (sin zero-tailing para K=3)
        let input_bits = [1, 1, 0, 1];
        let mut encoded = vec![0; input_bits.len()];
        encoder.encode_stream(&input_bits, &mut encoded);
        
        let mut viterbi = ViterbiEngine::new(topology);
        let mut decoded = vec![0; input_bits.len()];
        
        let diagnostic = viterbi.decode(&encoded, &mut decoded);
        
        assert_eq!(decoded, input_bits);
        // Debe ser detectado mediante la telemetría como un estado final ambiguo
        assert_eq!(diagnostic.status, ChainStatus::Corrupt);
        assert_ne!(diagnostic.final_best_state, 0);
    }
}

