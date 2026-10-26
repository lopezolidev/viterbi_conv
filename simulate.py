import viterbi_py
import matplotlib.pyplot as plt

# Estándar NASA/CCSDS: R=1/2, K=7
engine = viterbi_py.PyViterbiEngine(2, 1, 7, [0o171, 0o133])

# Relación Señal a Ruido (Eb/N0 en dB)
eb_n0_range = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]
bits_per_point = 1_000_000 
seed = 42

print("Ejecutando simulación de Monte Carlo en paralelo...")
ber_results = engine.simulate_ber(eb_n0_range, bits_per_point, seed)

plt.figure(figsize=(8, 6))
plt.semilogy(eb_n0_range, ber_results, 'b-o', linewidth=2, label="Hard Decision")
plt.grid(True, which="both", ls="--")
plt.xlabel('Eb/N0 (dB)')
plt.ylabel('Bit Error Rate (BER)')
plt.title('Rendimiento Viterbi - Código Convolucional NASA (K=7)')
plt.legend()
plt.savefig('viterbi_ber_curve.png')
plt.show()
