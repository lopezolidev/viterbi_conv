
# Viterbi FEC Engine & Trellis Observability

**[Español](https://www.google.com/search?q=%23espa%C3%B1ol) | [English**](https://www.google.com/search?q=%23english)

---

# 🇪🇸 Documentación en Español

Motor matemático de bajo nivel y alta eficiencia para **Codificación Convolucional** y **Decodificación por Algoritmo de Viterbi**, con soporte para simulación de canales ruidosos (BSC/AWGN), exportación a **WebAssembly (WASM)** para ejecución en el navegador y paralelización masiva en **Python (PyO3/Rayon)** para análisis científico.

---

## 🏗️ Arquitectura del Workspace (Cargo Workspace)

El proyecto está estructurado bajo un diseño modular estricto que prohíbe las asignaciones dinámicas de memoria en el *fast-path* y maximiza la localidad de caché L1:

```text
.
├── Cargo.toml                  # Configuración global del workspace
├── crates/
│   ├── viterbi-core/           # Núcleo matemático independiente de la plataforma
│   │   └── src/
│   │       ├── config.rs       # Componente 1: Validación de k, n, K y parseo de polinomios
│   │       ├── fsm.rs          # Componente 2: Precomputación de tablas Trellis 1D O(1)
│   │       ├── encoder.rs      # Componente 3: Codificador y telemetría de ventana W_t
│   │       ├── decoder/        # Componentes 4 y 5: Motor ACS de doble búfer y Traceback
│   │       └── channel.rs      # Componente 7: Simuladores de canal BSC y AWGN
│   ├── viterbi-wasm/           # Componente 8: Bindings FFI para WebAssembly (Navegador)
│   └── viterbi-py/             # Componente 9: Bindings PyO3 y paralelismo con Rayon (Python)
└── README.md

```

---

## 🚀 Características Principales

1. **Cero Asignaciones Dinámicas (Zero-Heap-Allocations):** Ningún ciclo de reloj en el codificador o en la unidad Add-Compare-Select (ACS) del decodificador realiza llamadas a la pila dinámica (`Vec::push`, `Box::new`). Toda la memoria se preasigna en la inicialización.
2. **Localidad de Caché:** Las matrices bidimensionales del Trellis se aplanan en arreglos unidimensionales indexados aritméticamente mediante $f(r, c) = r \cdot C + c$.
3. **Operaciones a Nivel de Bits $O(1)$:** Uso de máscaras estáticas y la instrucción intrínseca `.count_ones() & 1` para evaluar polinomios generadores en $GF(2)$.
4. **Multiplataforma:** Compila simultáneamente hacia nodos nativos de alto rendimiento, extensiones de Python y módulos interactivos en WebAssembly.

---

# 🇬🇧 English Documentation

High-performance, low-level mathematical engine for **Convolutional Encoding** and **Viterbi Algorithm Decoding**, featuring noisy channel simulations (BSC/AWGN), **WebAssembly (WASM)** export for browser execution, and massive parallelization in **Python (PyO3/Rayon)** for scientific benchmarking.

---

## 🏗️ Workspace Architecture

The project follows a strict modular design avoiding dynamic memory allocations on the fast-path and maximizing L1 cache locality:

```text
.
├── Cargo.toml                  # Global workspace configuration
├── crates/
│   ├── viterbi-core/           # Platform-agnostic mathematical core
│   │   └── src/
│   │       ├── config.rs       # Component 1: k, n, K validation & polynomial parsing
│   │       ├── fsm.rs          # Component 2: O(1) 1D Trellis precomputation tables
│   │       ├── encoder.rs      # Component 3: Encoder & W_t sliding window telemetry
│   │       ├── decoder/        # Components 4 & 5: Dual-buffer ACS unit & Traceback
│   │       └── channel.rs      # Component 7: BSC and AWGN channel simulators
│   ├── viterbi-wasm/           # Component 8: WebAssembly FFI bindings (Browser)
│   └── viterbi-py/             # Component 9: PyO3 bindings & Rayon parallelism (Python)
└── README.md

```

---

## 🚀 Key Features

1. **Zero-Heap-Allocations:** No dynamic memory allocations (`Vec::push`, `Box::new`) occur within the encoder clock cycle or the Viterbi Add-Compare-Select (ACS) fast-path. All memory is pre-allocated at startup.
2. **Cache Locality:** Two-dimensional Trellis matrices are flattened into 1D arrays indexed arithmetically via $f(r, c) = r \cdot C + c$.
3. **$O(1)$ Bit-Level Operations:** Utilizes static bitmasks and the `.count_ones() & 1` intrinsic instruction to evaluate generator polynomials in $GF(2)$.
4. **Cross-Platform:** Compiles seamlessly into high-performance native binaries, Python extensions, and interactive WebAssembly modules.

---

## 🛠️ Compilation & Testing / Compilación y Pruebas

Para ejecutar las pruebas unitarias del núcleo en Rust:
To run the core unit tests in Rust:

```bash
cargo test

```
