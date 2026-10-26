//! # Componente 4: Interfaz del Decodificador y Diagnósticos (`decoder/mod.rs`)
//!
//! Define el Trait base `ChannelDecoder` que unifica las implementaciones de decodificadores
//! (Viterbi, Fano) y expone las estructuras de observabilidad para la telemetría del Trellis.

// El siguiente submódulo se habilitará en el Componente 5:
pub mod viterbi;

/// Estado final de la cadena decodificada tras el procesamiento de los bits de cola (zero-tailing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainStatus {
    /// La cadena terminó correctamente. El estado de mínimo costo (arg min PM) en T es 0.
    Valid,
    /// La cadena es irrecuperable. El estado de mínimo costo en T no es 0, lo que indica
    /// que los bits de cola no lograron forzar el registro al estado inicial.
    Corrupt,
}

impl Default for ChainStatus {
    fn default() -> Self {
        Self::Valid
    }
}

/// Métricas de diagnóstico y telemetría extraídas del proceso de decodificación.
/// 
/// Captura las banderas de ambigüedad (TieBreaks) de la unidad ACS y el estado final
/// para el análisis de observabilidad visual del Trellis.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DecoderDiagnostic {
    /// Estado de integridad de la trama decodificada.
    pub status: ChainStatus,
    /// Cantidad total de empates exactos (TieBreaks) detectados en la unidad ACS.
    /// Un empate ocurre cuando PM_a + BM_a == PM_b + BM_b.
    pub total_tie_breaks: usize,
    /// El estado (\sigma_i) que presentó la Métrica de Ruta (PM) mínima al finalizar.
    pub final_best_state: usize,
}

/// Interfaz unificada para los algoritmos de decodificación de canal.
pub trait ChannelDecoder {
    /// Reinicia las métricas de ruta y el estado interno del decodificador, preparándolo
    /// para procesar una nueva trama sin requerir reasignaciones de memoria.
    fn reset(&mut self);

    /// Decodifica una secuencia de símbolos ruidosos ($z_t$) y escribe los bits de información
    /// originales en `output_bits`.
    ///
    /// # Restricciones de Memoria
    /// Esta función no debe realizar asignaciones en el heap (Box, Vec::push) durante su
    /// ejecución. Los arreglos temporales (ping-pong buffers, survivor rings) deben
    /// haber sido preasignados durante la inicialización del motor.
    ///
    /// # Argumentos
    /// * `received_symbols` - Stream de símbolos recibidos del canal.
    /// * `output_bits` - Buffer preasignado mutado in-place con la secuencia decodificada.
    ///
    /// # Retorna
    /// Un `DecoderDiagnostic` con la telemetría de la ejecución.
    fn decode(&mut self, received_symbols: &[u8], output_bits: &mut [u8]) -> DecoderDiagnostic;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnostic_default_values() {
        let diagnostic = DecoderDiagnostic::default();
        
        assert_eq!(diagnostic.status, ChainStatus::Valid);
        assert_eq!(diagnostic.total_tie_breaks, 0);
        assert_eq!(diagnostic.final_best_state, 0);
    }

    #[test]
    fn test_chain_status_equality() {
        let status1 = ChainStatus::Valid;
        let status2 = ChainStatus::Corrupt;
        
        assert_ne!(status1, status2);
        assert_eq!(status1, ChainStatus::Valid);
    }
}

