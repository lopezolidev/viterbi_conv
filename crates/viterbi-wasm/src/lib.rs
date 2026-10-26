//! # Componente 8: Interfaz WebAssembly (`viterbi-wasm`)
//!
//! Expone el núcleo matemático de Viterbi al ecosistema JavaScript/TypeScript 
//! para la simulación interactiva en el navegador mediante `wasm-bindgen`.

use std::sync::Arc;
use wasm_bindgen::prelude::*;
use viterbi_core::config::CodecConfig;
use viterbi_core::fsm::TrellisTopology;
use viterbi_core::encoder::ConvolutionalEncoder;
use viterbi_core::decoder::viterbi::ViterbiEngine;
use viterbi_core::decoder::ChannelDecoder;

/// Snapshot de telemetría mapeado automáticamente a un objeto de JavaScript.
#[wasm_bindgen]
pub struct WasmEncoderTelemetry {
    pub output_symbol: u8,
    pub window: u32,
    pub next_state: u32,
}

/// Motor Viterbi unificado exportado al contexto WebAssembly.
#[wasm_bindgen]
pub struct WasmViterbiEngine {
    encoder: ConvolutionalEncoder,
    decoder: ViterbiEngine,
}

#[wasm_bindgen]
impl WasmViterbiEngine {
    /// Inicializa una nueva instancia del motor. Lanza un error JS si la configuración es inválida.
    #[wasm_bindgen(constructor)]
    pub fn new(n: u8, k: u8, constraint_len: u8, polys: &[u32]) -> Result<WasmViterbiEngine, JsError> {
        let config = CodecConfig::new(n, k, constraint_len, polys.to_vec())
            .map_err(|e| JsError::new(&e.to_string()))?;
        
        let config_arc = Arc::new(config);
        let topology = Arc::new(TrellisTopology::new(config_arc.clone()));
        
        let encoder = ConvolutionalEncoder::new(config_arc);
        let decoder = ViterbiEngine::new(topology);

        Ok(Self { encoder, decoder })
    }

    /// Reinicia los estados internos de los búferes y el registro de desplazamiento.
    pub fn reset_engine(&mut self) {
        self.encoder.reset();
        self.decoder.reset();
    }

    /// Codifica un arreglo de bits (Uint8Array en JS) y retorna el flujo codificado.
    pub fn encode_frame(&mut self, input_bits: &[u8]) -> Vec<u8> {
        let mut output = vec![0; input_bits.len()];
        self.encoder.encode_stream(input_bits, &mut output);
        output
    }

    /// Ejecuta un solo paso de reloj en el codificador y extrae la telemetría en vivo.
    pub fn encode_step_with_telemetry(&mut self, input_symbol: u8) -> WasmEncoderTelemetry {
        let telemetry = self.encoder.step(input_symbol);
        WasmEncoderTelemetry {
            output_symbol: telemetry.output_symbol,
            window: telemetry.window,
            next_state: telemetry.next_state,
        }
    }

    /// Decodifica un arreglo de símbolos en decisión dura y retorna los bits corregidos.
    pub fn decode_frame(&mut self, received_symbols: &[u8]) -> Vec<u8> {
        let mut output = vec![0; received_symbols.len()];
        self.decoder.decode(received_symbols, &mut output);
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_wrapper_integration() {
        // Inicializamos R=1/2, K=3 con polinomios G=[7,5]
        let polys = [0b111, 0b101];
        let mut wasm_engine = WasmViterbiEngine::new(2, 1, 3, &polys)
            .expect("La configuración WASM debe inicializarse correctamente");
        
        // Tramo con zero-tailing
        let input = [1, 0, 1, 1, 0, 0];
        
        // Simulación Pura (Sin ruido)
        let encoded = wasm_engine.encode_frame(&input);
        let decoded = wasm_engine.decode_frame(&encoded);
        
        assert_eq!(decoded, input, "FFI: Los bits decodificados deben coincidir con la entrada original");

        // Prueba de telemetría iterativa
        wasm_engine.reset_engine();
        let telemetry = wasm_engine.encode_step_with_telemetry(1);
        assert_eq!(telemetry.output_symbol, 3); // Para input=1 en estado=0 con G=[7,5] -> v_t = 3
    }
}

