//! # Componente 1: Parametrización y Validación del Código Convolucional (`config.rs`)
//!
//! Define la estructura inmutable [`CodecConfig`] que valida las invariantes algebraicas y
//! topológicas del código convolucional $(n, k, K)$ antes de pre-asignar las tablas del Trellis.

use core::fmt;

/// Límites arquitectónicos para garantizar localidad en caché L1 y evitar explosión exponencial en RAM.
pub const MIN_K_INPUT: u8 = 1;
/// Máximo número de bits de entrada por ciclo de reloj ($k \le 4$).
pub const MAX_K_INPUT: u8 = 4;

/// Mínimo número de bits codificados de salida por ciclo de reloj ($n \ge 1$).
pub const MIN_N_OUTPUT: u8 = 1;
/// Máximo número de bits codificados de salida por ciclo de reloj ($n \le 8$, empaquetado en `u8`).
pub const MAX_N_OUTPUT: u8 = 8;

/// Mínimo factor de restricción ($K \ge 2$).
pub const MIN_CONSTRAINT_LEN: u8 = 2;
/// Máximo factor de restricción ($K \le 10$).
pub const MAX_CONSTRAINT_LEN: u8 = 10;

/// Límite estricto de longitud total del registro de desplazamiento ($L_{\text{reg}} = k \cdot K \le 16$).
///
/// Garantiza que la ventana deslizante $W_t$ quepa en un `u16`/`u32` y que el espacio de estados
/// $N_s = 2^{k(K-1)}$ nunca exceda $2^{15} = 32\,768$ estados.
pub const MAX_REGISTER_BITS: u8 = 16;

/// Errores de validación al construir un [`CodecConfig`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// El parámetro $k$ está fuera del rango permitido $[1, 4]$.
    InvalidInputBits {
        /// Valor de $k$ proporcionado.
        k: u8,
    },
    /// El parámetro $n$ está fuera del rango permitido $[1, 8]$.
    InvalidOutputBits {
        /// Valor de $n$ proporcionado.
        n: u8,
    },
    /// El factor de restricción $K$ está fuera del rango permitido $[2, 10]$.
    InvalidConstraintLength {
        /// Valor de $K$ proporcionado.
        constraint_len: u8,
    },
    /// La longitud total del registro $L_{\text{reg}} = k \cdot K$ supera el límite de 16 bits.
    RegisterOverflow {
        /// Bits de entrada $k$.
        k: u8,
        /// Factor de restricción $K$.
        constraint_len: u8,
        /// Producto resultante $k \cdot K$.
        total_bits: u16,
    },
    /// El número de polinomios generadores proporcionados no coincide con la salida $n$.
    GeneratorCountMismatch {
        /// Cantidad esperada ($n$).
        expected: usize,
        /// Cantidad recibida (`generators.len()`).
        actual: usize,
    },
    /// Un polinomio generador es nulo ($G_j = 0$, desconectado) o excede la máscara de $2^{k \cdot K} - 1$.
    InvalidGeneratorPolynomial {
        /// Índice $j \in [0, n-1]$ del polinomio inválido.
        index: usize,
        /// Valor del polinomio $G_j$.
        polynomial: u32,
        /// Valor máximo permitido ($2^{k \cdot K} - 1$).
        max_allowed: u32,
    },
    /// Error de sintaxis al parsear una cadena de polinomio en formato octal, binario o decimal.
    PolynomialParseError {
        /// Representación textual que falló al parsearse.
        token: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInputBits { k } => {
                write!(f, "Parámetro k inválido ({k}): debe pertenecer al rango [1, 4].")
            }
            Self::InvalidOutputBits { n } => {
                write!(f, "Parámetro n inválido ({n}): debe pertenecer al rango [1, 8].")
            }
            Self::InvalidConstraintLength { constraint_len } => {
                write!(
                    f,
                    "Factor de restricción K inválido ({constraint_len}): debe pertenecer al rango [2, 10]."
                )
            }
            Self::RegisterOverflow {
                k,
                constraint_len,
                total_bits,
            } => {
                write!(
                    f,
                    "Desbordamiento de registro: L_reg = k({k}) * K({constraint_len}) = {total_bits} bits (máximo permitido: {MAX_REGISTER_BITS})."
                )
            }
            Self::GeneratorCountMismatch { expected, actual } => {
                write!(
                    f,
                    "Discrepancia en cantidad de polinomios generadores: se esperaban n = {expected}, pero se recibieron {actual}."
                )
            }
            Self::InvalidGeneratorPolynomial {
                index,
                polynomial,
                max_allowed,
            } => {
                write!(
                    f,
                    "Polinomio generador G_{index} inválido (0x{polynomial:X} / {polynomial}): debe estar en el rango [1, 0x{max_allowed:X}]."
                )
            }
            Self::PolynomialParseError { token } => {
                write!(
                    f,
                    "No se pudo interpretar el polinomio generador '{token}'. Use prefijos '0b', '0o', '0x' o dígitos válidos."
                )
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// Configuración validada e inmutable de un código convolucional $(n, k, K)$.
///
/// Pre-calcula y almacena en caché todas las constantes de dimensión y máscaras de bits
/// requeridas en tiempo de ejecución $O(1)$ por el codificador, el constructor de topología
/// del Trellis y los motores de decodificación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecConfig {
    /// Bits de salida codificados por ciclo de reloj ($n \in [1, 8]$).
    n: u8,
    /// Bits de entrada por ciclo de reloj ($k \in [1, 4]$).
    k: u8,
    /// Factor de restricción en bloques de $k$ bits ($K \in [2, 10]$).
    constraint_len: u8,
    /// Vector de $n$ polinomios generadores en formato entero sin signo ($G_j \in [1, 2^{kK}-1]$).
    generators: Vec<u32>,
    /// Longitud total del registro de desplazamiento en bits: $L_{\text{reg}} = K \cdot k$.
    register_bits: u8,
    /// Bits de memoria de estado: $(K - 1) \cdot k$.
    state_bits: u8,
    /// Número total de estados en la Máquina de Estados Finitos: $N_s = 2^{k(K-1)}$.
    num_states: usize,
    /// Número de símbolos de entrada posibles por ciclo de reloj: $M = 2^k$.
    num_inputs: usize,
    /// Máscara de bits para extraer la ventana completa $W_t$: $2^{kK} - 1$.
    window_mask: u32,
    /// Máscara de bits para retener el nuevo estado $\sigma_t$: $2^{k(K-1)} - 1$.
    state_mask: u32,
    /// Máscara de bits para aislar un símbolo de entrada $u_t$: $2^k - 1$.
    input_mask: u8,
    /// Máscara de bits para aislar un símbolo codificado $v_t$: $2^n - 1$.
    output_mask: u8,
}

impl CodecConfig {
    /// Construye y valida una nueva instancia de [`CodecConfig`] a partir de polinomios ya convertidos a `u32`.
    ///
    /// # Reglas de Validación
    /// 1. $k \in [1, 4]$
    /// 2. $n \in [1, 8]$
    /// 3. $K \in [2, 10]$
    /// 4. $L_{\text{reg}} = k \cdot K \le 16$
    /// 5. `generators.len() == n as usize`
    /// 6. Para todo $j \in [0, n-1]$: $1 \le G_j \le 2^{kK} - 1$
    pub fn new(n: u8, k: u8, constraint_len: u8, generators: Vec<u32>) -> Result<Self, ConfigError> {
        if !(MIN_K_INPUT..=MAX_K_INPUT).contains(&k) {
            return Err(ConfigError::InvalidInputBits { k });
        }
        if !(MIN_N_OUTPUT..=MAX_N_OUTPUT).contains(&n) {
            return Err(ConfigError::InvalidOutputBits { n });
        }
        if !(MIN_CONSTRAINT_LEN..=MAX_CONSTRAINT_LEN).contains(&constraint_len) {
            return Err(ConfigError::InvalidConstraintLength { constraint_len });
        }

        let total_bits = (k as u16) * (constraint_len as u16);
        if total_bits > (MAX_REGISTER_BITS as u16) {
            return Err(ConfigError::RegisterOverflow {
                k,
                constraint_len,
                total_bits,
            });
        }

        let expected_gens = n as usize;
        if generators.len() != expected_gens {
            return Err(ConfigError::GeneratorCountMismatch {
                expected: expected_gens,
                actual: generators.len(),
            });
        }

        let register_bits = total_bits as u8;
        let state_bits = (constraint_len - 1) * k;
        let window_mask = (1u32 << register_bits) - 1;
        let state_mask = (1u32 << state_bits) - 1;

        for (index, &poly) in generators.iter().enumerate() {
            if poly == 0 || poly > window_mask {
                return Err(ConfigError::InvalidGeneratorPolynomial {
                    index,
                    polynomial: poly,
                    max_allowed: window_mask,
                });
            }
        }

        let num_states = 1usize << state_bits;
        let num_inputs = 1usize << k;
        let input_mask = ((1u16 << k) - 1) as u8;
        let output_mask = ((1u16 << n) - 1) as u8;

        Ok(Self {
            n,
            k,
            constraint_len,
            generators,
            register_bits,
            state_bits,
            num_states,
            num_inputs,
            window_mask,
            state_mask,
            input_mask,
            output_mask,
        })
    }

    /// Construye un [`CodecConfig`] a partir de polinomios expresados en notación **octal**
    /// (por ejemplo, `[0o171, 0o133]` o cadenas octales `["171", "133"]`).
    pub fn from_octal_strs(
        n: u8,
        k: u8,
        constraint_len: u8,
        octal_polys: &[&str],
    ) -> Result<Self, ConfigError> {
        let mut parsed = Vec::with_capacity(octal_polys.len());
        for &raw in octal_polys {
            let trimmed = raw.trim().trim_start_matches("0o").trim_start_matches("0O");
            if trimmed.is_empty() {
                return Err(ConfigError::PolynomialParseError {
                    token: raw.to_string(),
                });
            }
            let val = u32::from_str_radix(trimmed, 8).map_err(|_| {
                ConfigError::PolynomialParseError {
                    token: raw.to_string(),
                }
            })?;
            parsed.push(val);
        }
        Self::new(n, k, constraint_len, parsed)
    }

    /// Parsea una lista de especificaciones de polinomios con detección explícita de base:
    /// - `"0b..."` o `"0B..."` $\rightarrow$ Base 2 (Binario)
    /// - `"0o..."` o `"0O..."` $\rightarrow$ Base 8 (Octal)
    /// - `"0x..."` o `"0X..."` $\rightarrow$ Base 16 (Hexadecimal)
    /// - Sin prefijo $\rightarrow$ Base `default_radix` (ej. `8` para octal, `10` para decimal, `2` para binario).
    pub fn from_str_specs(
        n: u8,
        k: u8,
        constraint_len: u8,
        specs: &[&str],
        default_radix: u32,
    ) -> Result<Self, ConfigError> {
        if !(2..=36).contains(&default_radix) {
            return Err(ConfigError::PolynomialParseError {
                token: format!("radix={default_radix}"),
            });
        }

        let mut parsed = Vec::with_capacity(specs.len());
        for &raw in specs {
            let token = raw.trim();
            if token.is_empty() {
                return Err(ConfigError::PolynomialParseError {
                    token: raw.to_string(),
                });
            }

            let (digits, radix) = if let Some(rest) = token
                .strip_prefix("0b")
                .or_else(|| token.strip_prefix("0B"))
            {
                (rest, 2)
            } else if let Some(rest) = token
                .strip_prefix("0o")
                .or_else(|| token.strip_prefix("0O"))
            {
                (rest, 8)
            } else if let Some(rest) = token
                .strip_prefix("0x")
                .or_else(|| token.strip_prefix("0X"))
            {
                (rest, 16)
            } else {
                (token, default_radix)
            };

            let clean_digits: String = digits.chars().filter(|&c| c != '_').collect();
            if clean_digits.is_empty() {
                return Err(ConfigError::PolynomialParseError {
                    token: raw.to_string(),
                });
            }

            let val = u32::from_str_radix(&clean_digits, radix).map_err(|_| {
                ConfigError::PolynomialParseError {
                    token: raw.to_string(),
                }
            })?;
            parsed.push(val);
        }

        Self::new(n, k, constraint_len, parsed)
    }

    /// Retorna el número de bits de salida codificados por ciclo de reloj ($n$).
    #[inline(always)]
    pub const fn n(&self) -> u8 {
        self.n
    }

    /// Retorna el número de bits de entrada por ciclo de reloj ($k$).
    #[inline(always)]
    pub const fn k(&self) -> u8 {
        self.k
    }

    /// Retorna el factor de restricción ($K$).
    #[inline(always)]
    pub const fn constraint_len(&self) -> u8 {
        self.constraint_len
    }

    /// Retorna una tajada inmutable con los $n$ polinomios generadores $[G_0, G_1, \dots, G_{n-1}]$.
    #[inline(always)]
    pub fn generators(&self) -> &[u32] {
        &self.generators
    }

    /// Retorna la tasa nominal del código $R = k / n$ en punto flotante de doble precisión.
    #[inline(always)]
    pub fn code_rate(&self) -> f64 {
        (self.k as f64) / (self.n as f64)
    }

    /// Retorna la longitud total del registro de desplazamiento en bits ($L_{\text{reg}} = K \cdot k$).
    #[inline(always)]
    pub const fn register_bits(&self) -> u8 {
        self.register_bits
    }

    /// Retorna la cantidad de bits de estado en memoria ($(K - 1) \cdot k$).
    #[inline(always)]
    pub const fn state_bits(&self) -> u8 {
        self.state_bits
    }

    /// Retorna el número total de estados de la FSM ($N_s = 2^{k(K-1)}$).
    #[inline(always)]
    pub const fn num_states(&self) -> usize {
        self.num_states
    }

    /// Retorna el número de símbolos de entrada posibles por paso ($2^k$).
    #[inline(always)]
    pub const fn num_inputs(&self) -> usize {
        self.num_inputs
    }

    /// Retorna el número total de transiciones (ramas) por paso de reloj en el Trellis ($N_s \cdot 2^k = 2^{kK}$).
    #[inline(always)]
    pub const fn num_branches_per_step(&self) -> usize {
        self.num_states * self.num_inputs
    }

    /// Retorna la máscara de bits de la ventana deslizante completa $W_t$: $2^{kK} - 1$.
    #[inline(always)]
    pub const fn window_mask(&self) -> u32 {
        self.window_mask
    }

    /// Retorna la máscara de bits para aislar el estado $\sigma_t$: $2^{k(K-1)} - 1$.
    #[inline(always)]
    pub const fn state_mask(&self) -> u32 {
        self.state_mask
    }

    /// Retorna la máscara de bits para aislar el símbolo de entrada $u_t$: $2^k - 1$.
    #[inline(always)]
    pub const fn input_mask(&self) -> u8 {
        self.input_mask
    }

    /// Retorna la máscara de bits para aislar el símbolo codificado de salida $v_t$: $2^n - 1$.
    #[inline(always)]
    pub const fn output_mask(&self) -> u8 {
        self.output_mask
    }

    /// Retorna la profundidad recomendada del buffer circular de *traceback* ($L_{\text{depth}} = 5 \cdot K$).
    #[inline(always)]
    pub const fn default_traceback_depth(&self) -> usize {
        5 * (self.constraint_len as usize)
    }

    /// Retorna el número de bloques de $k$ bits en cero requeridos para terminar la trama (*zero-tailing*): $K - 1$.
    #[inline(always)]
    pub const fn tail_symbols_count(&self) -> usize {
        (self.constraint_len - 1) as usize
    }

    /// Evalúa en $O(1)$ la ventana deslizante $W_t = (\sigma_{t-1} \ll k) \mid u_t$.
    #[inline(always)]
    pub const fn compute_window(&self, prev_state: u32, input_symbol: u8) -> u32 {
        ((prev_state << self.k) | ((input_symbol & self.input_mask) as u32)) & self.window_mask
    }

    /// Evalúa en $O(1)$ el estado siguiente $\sigma_t = W_t \ \& \ (2^{k(K-1)} - 1)$ a partir de la ventana $W_t$.
    #[inline(always)]
    pub const fn next_state_from_window(&self, window: u32) -> u32 {
        window & self.state_mask
    }

    /// Evalúa en $O(1)$ el símbolo de salida empaquetado de $n$ bits en $GF(2)$ para una ventana $W_t$:
    ///
    /// $$v_{t, j} = \text{popcnt}(W_t \ \& \ G_j) \pmod 2$$
    /// $$v_t = \sum_{j=0}^{n-1} \left( v_{t, j} \ll (n - 1 - j) \right)$$
    #[inline(always)]
    pub fn eval_output_from_window(&self, window: u32) -> u8 {
        let mut out: u8 = 0;
        let n_len = self.n as usize;
        let mut j = 0;
        while j < n_len {
            let parity = ((window & self.generators[j]).count_ones() & 1) as u8;
            out = (out << 1) | parity;
            j += 1;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_nasa_voyager_k7_rate_half() {
        // Estándar NASA / CCSDS: (n=2, k=1, K=7), G = [171_8, 133_8] = [121, 91] = [0b1111001, 0b1011011]
        let cfg = CodecConfig::from_octal_strs(2, 1, 7, &["171", "133"])
            .expect("La configuración estándar (2, 1, 7) debe ser válida");

        assert_eq!(cfg.n(), 2);
        assert_eq!(cfg.k(), 1);
        assert_eq!(cfg.constraint_len(), 7);
        assert_eq!(cfg.register_bits(), 7);
        assert_eq!(cfg.state_bits(), 6);
        assert_eq!(cfg.num_states(), 64);
        assert_eq!(cfg.num_inputs(), 2);
        assert_eq!(cfg.num_branches_per_step(), 128);
        assert_eq!(cfg.window_mask(), 0b111_1111);
        assert_eq!(cfg.state_mask(), 0b011_1111);
        assert_eq!(cfg.input_mask(), 0b1);
        assert_eq!(cfg.output_mask(), 0b11);
        assert_eq!(cfg.generators(), &[0o171, 0o133]);
        assert_eq!(cfg.default_traceback_depth(), 35);
        assert_eq!(cfg.tail_symbols_count(), 6);
        assert!((cfg.code_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_classic_k3_gf2_window_and_output_evaluation() {
        // Código clásico de libro de texto: (n=2, k=1, K=3), G0 = 7_8 (111_2), G1 = 5_8 (101_2)
        let cfg = CodecConfig::new(2, 1, 3, vec![0b111, 0b101]).unwrap();

        assert_eq!(cfg.num_states(), 4);
        assert_eq!(cfg.window_mask(), 0b111);
        assert_eq!(cfg.state_mask(), 0b011);

        // Si estado anterior sigma_{t-1} = 0b00 y entra u_t = 1:
        // W_t = (0b00 << 1) | 1 = 0b001
        // sigma_t = 0b001 & 0b011 = 0b01
        // v_{t,0} = popcnt(0b001 & 0b111) & 1 = 1
        // v_{t,1} = popcnt(0b001 & 0b101) & 1 = 1 -> v_t = 0b11 (3)
        let w1 = cfg.compute_window(0b00, 1);
        assert_eq!(w1, 0b001);
        assert_eq!(cfg.next_state_from_window(w1), 0b01);
        assert_eq!(cfg.eval_output_from_window(w1), 0b11);

        // Siguiente ciclo: sigma_{t-1} = 0b01 y entra u_t = 0:
        // W_t = (0b01 << 1) | 0 = 0b010
        // sigma_t = 0b010 & 0b011 = 0b10
        // v_{t,0} = popcnt(0b010 & 0b111) & 1 = 1
        // v_{t,1} = popcnt(0b010 & 0b101) & 1 = 0 -> v_t = 0b10 (2)
        let w2 = cfg.compute_window(0b01, 0);
        assert_eq!(w2, 0b010);
        assert_eq!(cfg.next_state_from_window(w2), 0b10);
        assert_eq!(cfg.eval_output_from_window(w2), 0b10);
    }

    #[test]
    fn test_multi_bit_input_k2_configuration() {
        // Código de tasa R = 2/3 con k=2, n=3, K=3 -> L_reg = 6 bits, N_s = 2^(2*2) = 16 estados
        let cfg = CodecConfig::from_str_specs(3, 2, 3, &["0b111001", "0o53", "39"], 10)
            .expect("Debe soportar formatos mixtos (binario, octal y decimal)");

        assert_eq!(cfg.register_bits(), 6);
        assert_eq!(cfg.state_bits(), 4);
        assert_eq!(cfg.num_states(), 16);
        assert_eq!(cfg.num_inputs(), 4);
        assert_eq!(cfg.generators(), &[0b111001, 0o53, 39]);
    }

    #[test]
    fn test_validation_rejects_invalid_bounds_and_polynomials() {
        // 1. k fuera de rango
        assert_eq!(
            CodecConfig::new(2, 0, 3, vec![7, 5]),
            Err(ConfigError::InvalidInputBits { k: 0 })
        );
        assert_eq!(
            CodecConfig::new(2, 5, 3, vec![7, 5]),
            Err(ConfigError::InvalidInputBits { k: 5 })
        );

        // 2. n fuera de rango
        assert_eq!(
            CodecConfig::new(0, 1, 3, vec![]),
            Err(ConfigError::InvalidOutputBits { n: 0 })
        );
        assert_eq!(
            CodecConfig::new(9, 1, 3, vec![1; 9]),
            Err(ConfigError::InvalidOutputBits { n: 9 })
        );

        // 3. K fuera de rango
        assert_eq!(
            CodecConfig::new(2, 1, 1, vec![1, 1]),
            Err(ConfigError::InvalidConstraintLength { constraint_len: 1 })
        );

        // 4. Desbordamiento de registro L_reg = k * K > 16 (ej. k=3, K=6 -> 18 bits)
        assert_eq!(
            CodecConfig::new(2, 3, 6, vec![7, 5]),
            Err(ConfigError::RegisterOverflow {
                k: 3,
                constraint_len: 6,
                total_bits: 18
            })
        );

        // 5. Cantidad de polinomios distinta de n
        assert_eq!(
            CodecConfig::new(3, 1, 3, vec![7, 5]),
            Err(ConfigError::GeneratorCountMismatch {
                expected: 3,
                actual: 2
            })
        );

        // 6. Polinomio nulo (0) o que excede 2^(k*K) - 1 = 7
        assert_eq!(
            CodecConfig::new(2, 1, 3, vec![0, 5]),
            Err(ConfigError::InvalidGeneratorPolynomial {
                index: 0,
                polynomial: 0,
                max_allowed: 7
            })
        );
        assert_eq!(
            CodecConfig::new(2, 1, 3, vec![7, 8]),
            Err(ConfigError::InvalidGeneratorPolynomial {
                index: 1,
                polynomial: 8,
                max_allowed: 7
            })
        );
    }
}
