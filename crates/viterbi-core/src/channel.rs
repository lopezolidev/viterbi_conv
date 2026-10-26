//! # Componente 7: Simulador de Canal (`channel.rs`)
//!
//! Implementa los modelos de canal de comunicaciones (BSC y AWGN) para inyectar
//! ruido en el flujo de símbolos. Utiliza un PRNG embebido para garantizar el
//! determinismo en simulaciones y evitar dependencias dinámicas.

use std::f32::consts::PI;

/// Generador pseudoaleatorio determinista y ligero (Xorshift32).
pub struct XorShift32 {
    state: u32,
}

impl XorShift32 {
    /// Inicializa el PRNG con una semilla no nula.
    pub fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0xBAD_C0DE } else { seed },
        }
    }

    /// Genera el siguiente número pseudoaleatorio.
    #[inline(always)]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    /// Genera un valor de punto flotante uniformemente distribuido en el rango [0.0, 1.0).
    #[inline(always)]
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }

    /// Genera un par de variables aleatorias normales estándar (media 0, varianza 1)
    /// utilizando la transformada de Box-Muller.
    pub fn next_gaussian_pair(&mut self) -> (f32, f32) {
        let u1 = self.next_f32().max(f32::EPSILON);
        let u2 = self.next_f32();
        
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * PI * u2;
        
        (r * theta.cos(), r * theta.sin())
    }
}

/// Simulador de Canal Simétrico Binario (BSC).
///
/// Invierte bits (XOR) con una probabilidad de cruce `p`.
pub struct BscChannel {
    /// Probabilidad de error de bit ($p \in [0.0, 1.0]$).
    pub p: f32,
    prng: XorShift32,
}

impl BscChannel {
    /// Crea un nuevo canal BSC con probabilidad `p` y una semilla determinista.
    pub fn new(p: f32, seed: u32) -> Self {
        Self {
            p: p.clamp(0.0, 1.0),
            prng: XorShift32::new(seed),
        }
    }

    /// Inyecta ruido en una secuencia de bits, mutando el arreglo in-place.
    pub fn transmit(&mut self, data: &mut [u8]) {
        for bit in data.iter_mut() {
            if self.prng.next_f32() < self.p {
                *bit ^= 1; // Inyección de máscara de error XOR e_t
            }
        }
    }
}

/// Simulador de Canal AWGN con modulación BPSK y Decisión Dura.
pub struct AwgnChannel {
    /// Desviación estándar del ruido ($\sigma$).
    pub sigma: f32,
    prng: XorShift32,
    cached_gaussian: Option<f32>,
}

impl AwgnChannel {
    /// Configura el canal AWGN calculando la varianza a partir de la relación
    /// señal a ruido por bit ($E_b/N_0$) y la tasa del código ($R = k/n$).
    pub fn new(eb_n0_db: f32, code_rate: f32, seed: u32) -> Self {
        let snr = 10.0f32.powf(eb_n0_db / 10.0);
        let variance = 1.0 / (2.0 * code_rate * snr);
        
        Self {
            sigma: variance.sqrt(),
            prng: XorShift32::new(seed),
            cached_gaussian: None,
        }
    }

    /// Modula en BPSK, añade ruido gaussiano y demodula el buffer in-place (Decisión Dura).
    pub fn transmit_hard_decision(&mut self, data: &mut [u8]) {
        for bit in data.iter_mut() {
            // BPSK: 0 -> -1.0, 1 -> +1.0
            let tx_signal = if *bit == 1 { 1.0 } else { -1.0 };
            
            let noise = if let Some(n) = self.cached_gaussian.take() {
                n
            } else {
                let (n1, n2) = self.prng.next_gaussian_pair();
                self.cached_gaussian = Some(n2);
                n1
            };
            
            let rx_signal = tx_signal + noise * self.sigma;
            
            // Demodulación de Decisión Dura
            *bit = if rx_signal > 0.0 { 1 } else { 0 };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xorshift_determinism() {
        let mut rng1 = XorShift32::new(42);
        let mut rng2 = XorShift32::new(42);
        
        assert_eq!(rng1.next_u32(), rng2.next_u32());
        assert_eq!(rng1.next_f32(), rng2.next_f32());
    }

    #[test]
    fn test_bsc_extreme_probabilities() {
        let mut channel = BscChannel::new(1.0, 123);
        let mut data = [0, 1, 0, 1];
        channel.transmit(&mut data);
        assert_eq!(data, [1, 0, 1, 0], "p=1.0 debe invertir todos los bits");

        let mut channel_clean = BscChannel::new(0.0, 123);
        let mut clean_data = [1, 0, 1, 1];
        channel_clean.transmit(&mut clean_data);
        assert_eq!(clean_data, [1, 0, 1, 1], "p=0.0 no debe mutar los bits");
    }

    #[test]
    fn test_bsc_statistical_distribution() {
        let mut channel = BscChannel::new(0.25, 9999);
        let mut data = vec![0u8; 10_000];
        channel.transmit(&mut data);
        
        let errors = data.iter().map(|&b| b as usize).sum::<usize>();
        assert!(errors > 2300 && errors < 2700, "Errores esperados ~2500, obtenidos: {}", errors);
    }

    #[test]
    fn test_awgn_high_snr_preserves_data() {
        // Un SNR muy alto (20 dB) rara vez debería corromper los datos
        let mut channel = AwgnChannel::new(20.0, 0.5, 555);
        let mut data = vec![1, 0, 1, 1, 0, 0, 1];
        let original = data.clone();
        
        channel.transmit_hard_decision(&mut data);
        assert_eq!(data, original, "Con 20dB de SNR los datos deben mantenerse íntegros");
    }
}
