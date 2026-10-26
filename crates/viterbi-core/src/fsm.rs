//! # Componente 2: Topología del Trellis (FSM)
//!
//! Precomputación de tablas de transición unidimensionales para garantizar
//! localidad de caché y eliminar cálculos redundantes durante la codificación
//! y la unidad ACS (Add-Compare-Select) de Viterbi.

use std::sync::Arc;
use crate::config::CodecConfig;

/// Representa una transición directa en el Trellis: `(Estado, Entrada) -> (Siguiente Estado, Salida)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForwardBranch {
    /// Siguiente estado resultante de la transición.
    pub next_state: usize,
    /// Símbolo de salida $v_t$ generado (empaquetado en bits).
    pub output_symbol: u8,
}

/// Representa una transición inversa en el Trellis: `Siguiente Estado -> (Estado Previo, Entrada)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackwardBranch {
    /// Estado previo desde el cual se originó la transición.
    pub prev_state: usize,
    /// Símbolo de entrada $u_t$ que provocó la transición.
    pub input_symbol: u8,
    /// Símbolo de salida $v_t$ generado, útil para precalcular la métrica de rama (BM) en ACS.
    pub output_symbol: u8,
}

/// Topología estática del código convolucional.
///
/// Pre-calcula y aplana el grafo del Trellis en vectores unidimensionales para
/// cumplir con la restricción de cero asignaciones dinámicas y máxima localidad en caché L1.
#[derive(Debug, Clone)]
pub struct TrellisTopology {
    /// Referencia compartida a la configuración inmutable del código.
    pub config: Arc<CodecConfig>,
    /// Tabla de transiciones directas.
    /// Tamaño: $N_s \times 2^k$. Indexada por: `estado * 2^k + entrada`.
    forward_table: Vec<ForwardBranch>,
    /// Tabla de transiciones inversas (ramas incidentes a un nodo).
    /// Tamaño: $N_s \times 2^k$. Indexada por: `siguiente_estado * 2^k + m`.
    backward_table: Vec<BackwardBranch>,
}

impl TrellisTopology {
    /// Pre-calcula el Trellis completo en tiempo $O(2^{k \cdot K})$.
    ///
    /// Se ejecuta una única vez durante la fase de inicialización. Toda la memoria
    /// necesaria para el grafo es preasignada aquí.
    pub fn new(config: Arc<CodecConfig>) -> Self {
        let num_states = config.num_states();
        let num_inputs = config.num_inputs();
        let num_branches = config.num_branches_per_step();

        // Preasignación estricta de memoria (Regla de Cero Asignaciones Dinámicas en Fast-Path)
        let mut forward_table = vec![ForwardBranch { next_state: 0, output_symbol: 0 }; num_branches];
        let mut backward_table = vec![
            BackwardBranch { prev_state: 0, input_symbol: 0, output_symbol: 0 }; 
            num_branches
        ];
        
        // Rastreador temporal para empaquetar las ramas inversas de forma contigua
        let mut backward_counters = vec![0; num_states];

        for state in 0..num_states {
            for input in 0..num_inputs {
                let window = config.compute_window(state as u32, input as u8);
                let next_state = config.next_state_from_window(window) as usize;
                let output = config.eval_output_from_window(window);

                // Indexación aritmética aplanada: f(r, c) = r * C + c
                let forward_idx = state * num_inputs + input;
                forward_table[forward_idx] = ForwardBranch {
                    next_state,
                    output_symbol: output,
                };

                let m = backward_counters[next_state];
                let backward_idx = next_state * num_inputs + m;
                backward_table[backward_idx] = BackwardBranch {
                    prev_state: state,
                    input_symbol: input as u8,
                    output_symbol: output,
                };
                backward_counters[next_state] += 1;
            }
        }

        Self {
            config,
            forward_table,
            backward_table,
        }
    }

    /// Retorna la transición directa (Siguiente Estado, Salida) en $O(1)$.
    #[inline(always)]
    pub fn forward_branch(&self, state: usize, input: usize) -> ForwardBranch {
        self.forward_table[state * self.config.num_inputs() + input]
    }

    /// Retorna un slice con todas las transiciones que convergen en `next_state` en $O(1)$.
    ///
    /// Ideal para iterar rápidamente en la unidad ACS sin requerir el uso de `Vec`.
    #[inline(always)]
    pub fn backward_branches(&self, next_state: usize) -> &[BackwardBranch] {
        let start = next_state * self.config.num_inputs();
        let end = start + self.config.num_inputs();
        &self.backward_table[start..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topology_classic_k3() {
        // G0 = 7_8, G1 = 5_8. K = 3, k = 1, n = 2. N_s = 4.
        let config = Arc::new(CodecConfig::new(2, 1, 3, vec![0b111, 0b101]).unwrap());
        let topology = TrellisTopology::new(config.clone());

        // Verificamos dimensiones aplanadas
        assert_eq!(topology.forward_table.len(), 8); // 4 estados * 2 entradas
        assert_eq!(topology.backward_table.len(), 8);

        // Prueba de Transición Directa (Forward)
        // Estado 0b00 (0), Entrada 1 -> Siguiente Estado 0b01 (1), Salida 3
        let branch = topology.forward_branch(0, 1);
        assert_eq!(branch.next_state, 1);
        assert_eq!(branch.output_symbol, 3); // 0b11

        // Prueba de Transición Inversa (Backward) para convergencia en el nodo 0b01 (1)
        // En un Trellis radix-2, el estado 0b01 puede provenir de:
        // - Estado 0b00 (0) con entrada 1
        // - Estado 0b10 (2) con entrada 1
        let incoming = topology.backward_branches(1);
        assert_eq!(incoming.len(), 2);
        
        let mut prev_states: Vec<usize> = incoming.iter().map(|b| b.prev_state).collect();
        prev_states.sort();
        assert_eq!(prev_states, vec![0, 2]);

        // Verificamos los detalles completos del origen desde el estado 2
        let from_state_2 = incoming.iter().find(|b| b.prev_state == 2).unwrap();
        assert_eq!(from_state_2.input_symbol, 1);
        // Si prev=0b10, input=1 -> W_t = (2 << 1) | 1 = 5 (0b101). 
        // Salida: popcnt(0b101 & 0b111)=0, popcnt(0b101 & 0b101)=0 -> v = 0b00 (0)
        assert_eq!(from_state_2.output_symbol, 0); 
    }

    #[test]
    fn test_topology_nasa_voyager() {
        let config = Arc::new(CodecConfig::from_octal_strs(2, 1, 7, &["171", "133"]).unwrap());
        let topology = TrellisTopology::new(config.clone());

        assert_eq!(topology.forward_table.len(), 128);
        assert_eq!(topology.backward_table.len(), 128);

        // Estado 63 (0b111111), Entrada 0
        // Siguiente Estado -> W_t = (63 << 1) | 0 = 126 (0b1111110)
        // Sigma_t = 126 & 63 = 62 (0b111110)
        let branch = topology.forward_branch(63, 0);
        assert_eq!(branch.next_state, 62);
        
        let incoming = topology.backward_branches(62);
        assert_eq!(incoming.len(), 2);
    }
}

