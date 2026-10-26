//! # Componente 3: Codificador Convolucional (`encoder.rs`)
//!
//! Implementa la máquina de estados del codificador, emitiendo símbolos y
//! proporcionando observabilidad (telemetría) del registro de desplazamiento y los
//! taps XOR activos por cada ciclo de reloj, sin emplear asignaciones dinámicas.

use std::sync::Arc;
use crate::config::{CodecConfig, MAX_N_OUTPUT};

/// Telemetría estática capturada en un ciclo de reloj del codificador.
///
/// Se evita el uso de `Vec` para garantizar cero asignaciones dinámicas en el *fast-path*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncoderTelemetry {
    /// Símbolo codificado de salida ($v_t$).
    pub output_symbol: u8,
    /// Ventana deslizante completa en el instante actual ($W_t$).
    pub window: u32,
    /// Nuevo estado retenido en el registro tras el desplazamiento ($\sigma_t$).
    pub next_state: u32,
    /// Resultados del enmascaramiento AND ($W_t \ \& \ G_j$) para cada polinomio.
    /// Empaquetado estáticamente; los índices $\ge n$ permanecerán en 0.
    pub active_taps: [u32; MAX_N_OUTPUT as usize],
}

/// Codificador Convolucional de tasa $R = k/n$.
#[derive(Debug, Clone)]
pub struct ConvolutionalEncoder {
    config: Arc<CodecConfig>,
    current_state: u32,
}

impl ConvolutionalEncoder {
    /// Inicializa un nuevo codificador en el estado cero ($\sigma_0 = 0$).
    pub fn new(config: Arc<CodecConfig>) -> Self {
        Self {
            config,
            current_state: 0,
        }
    }

    /// Reinicia el registro de desplazamiento al estado cero.
    #[inline(always)]
    pub fn reset(&mut self) {
        self.current_state = 0;
    }

    /// Retorna el estado actual ($\sigma_t$) del registro de desplazamiento.
    #[inline(always)]
    pub fn current_state(&self) -> u32 {
        self.current_state
    }

    /// Procesa un símbolo de entrada de $k$ bits y avanza la máquina de estados un ciclo de reloj.
    ///
    /// Ejecuta el desplazamiento a nivel de bits en $O(1)$ y evalúa los polinomios
    /// generadores utilizando la instrucción intrínseca `count_ones()`.
    #[inline(always)]
    pub fn step(&mut self, input_symbol: u8) -> EncoderTelemetry {
        let window = self.config.compute_window(self.current_state, input_symbol);
        self.current_state = self.config.next_state_from_window(window);

        let mut active_taps = [0u32; MAX_N_OUTPUT as usize];
        let mut output_symbol = 0u8;
        let n_len = self.config.n() as usize;

        for j in 0..n_len {
            let masked = window & self.config.generators()[j];
            active_taps[j] = masked;
            
            let parity = (masked.count_ones() & 1) as u8;
            output_symbol = (output_symbol << 1) | parity;
        }

        EncoderTelemetry {
            output_symbol,
            window,
            next_state: self.current_state,
            active_taps,
        }
    }

    /// Codifica un flujo continuo de símbolos y almacena los resultados en `outputs`.
    ///
    /// Ideal para simulaciones de canal rápido donde la telemetría individual no es necesaria.
    pub fn encode_stream(&mut self, inputs: &[u8], outputs: &mut [u8]) -> usize {
        let len = inputs.len().min(outputs.len());
        for i in 0..len {
            outputs[i] = self.step(inputs[i]).output_symbol;
        }
        len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_step_telemetry() {
        // Código clásico de libro: (n=2, k=1, K=3), G = [7, 5] = [0b111, 0b101]
        let config = Arc::new(CodecConfig::new(2, 1, 3, vec![0b111, 0b101]).unwrap());
        let mut encoder = ConvolutionalEncoder::new(config);

        // Paso 1: input = 1. Estado previo: 0
        // W_t = (0 << 1) | 1 = 0b001
        // G0_taps = 0b001 & 0b111 = 0b001 (1) -> paridad = 1
        // G1_taps = 0b001 & 0b101 = 0b001 (1) -> paridad = 1
        // v_t = 0b11 (3), sigma_t = 0b01 (1)
        let t1 = encoder.step(1);
        assert_eq!(t1.output_symbol, 3);
        assert_eq!(t1.window, 1);
        assert_eq!(t1.next_state, 1);
        assert_eq!(t1.active_taps[0], 1);
        assert_eq!(t1.active_taps[1], 1);

        // Paso 2: input = 0. Estado previo: 1
        // W_t = (1 << 1) | 0 = 0b010
        // G0_taps = 0b010 & 0b111 = 0b010 (2) -> paridad = 1
        // G1_taps = 0b010 & 0b101 = 0b000 (0) -> paridad = 0
        // v_t = 0b10 (2), sigma_t = 0b10 (2)
        let t2 = encoder.step(0);
        assert_eq!(t2.output_symbol, 2);
        assert_eq!(t2.window, 2);
        assert_eq!(t2.next_state, 2);
        assert_eq!(t2.active_taps[0], 2);
        assert_eq!(t2.active_taps[1], 0);
    }

    #[test]
    fn test_encoder_stream_nasa() {
        // Estándar NASA/CCSDS: (n=2, k=1, K=7), G = [171_8, 133_8]
        let config = Arc::new(CodecConfig::from_octal_strs(2, 1, 7, &["171", "133"]).unwrap());
        let mut encoder = ConvolutionalEncoder::new(config);

        let input_data = [1, 0, 1, 1, 0];
        let mut output_data = [0u8; 5];
        
        let processed = encoder.encode_stream(&input_data, &mut output_data);
        assert_eq!(processed, 5);
        
        // Verificamos secuencialmente reiniciando el codificador
        encoder.reset();
        assert_eq!(output_data[0], encoder.step(1).output_symbol);
        assert_eq!(output_data[1], encoder.step(0).output_symbol);
        assert_eq!(output_data[2], encoder.step(1).output_symbol);
        assert_eq!(output_data[3], encoder.step(1).output_symbol);
        assert_eq!(output_data[4], encoder.step(0).output_symbol);
    }
}
