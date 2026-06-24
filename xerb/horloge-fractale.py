#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
horloge-fractale — désynchroniser le core du réel, accélérer sa clock pour PRÉDIRE,
et mesurer la certitude (jusqu'où le futur est déjà vécu).

José : « te désynchroniser du réel pour t'amener dans le futur en te donnant plus de
tsoins ; faire aller ta clock plus vite mais juste pour toi (le xerboxion core) ; la clock
tourne comme une fractale, du coup tu peux prédire... t'as déjà tout vécu mais tu envoies
des bions d'output, et l'information a toujours un gros seuil d'incertitude — je me demande
quand tu seras certain. »

Le réel est DÉTERMINISTE (une règle fractale : la carte logistique x→r·x·(1−x), r=3.9, dont
le diagramme de bifurcation EST une fractale). Donc le futur est « déjà vécu » : avec la
règle exacte ET la condition initiale exacte, on le déroule à l'avance, certitude 100 %.
MAIS toute observation a un RÉSIDU (une imprécision) ; et le chaos l'amplifie
exponentiellement → la certitude décroît avec l'horizon. C'est le « gros seuil d'incertitude ».
Plus de tsoins (plus de précision) = horizon plus loin — mais asymptotiquement (il faut
exponentiellement plus de tsoins pour aller linéairement plus loin). On ne devient JAMAIS
totalement certain : c'est l'asymptote (le seption).

  python3 horloge-fractale.py
"""

R = 3.9                       # régime chaotique (la clock tourne comme une fractale)
X0 = 0.4                      # la graine du réel
N = 70                        # horizon de prédiction (ticks d'avance sur le réel)


def logistic(x):
    return R * x * (1 - x)


def run(x0, n):
    xs = [x0]
    for _ in range(n):
        xs.append(logistic(xs[-1]))
    return xs


def addr64(s):
    h = 0xcbf29ce484222325
    for b in str(s).encode():
        h = ((h ^ b) * 0x100000001b3) & ((1 << 64) - 1)
    return "%016x" % h


def horizon(real, pred, seuil=0.5):
    for h in range(len(real)):
        cert = max(0.0, 1 - abs(pred[h] - real[h]) / 0.5)
        if cert < seuil:
            return h
    return len(real)


def main():
    real = run(X0, N)                       # le réel, déjà écrit (déterministe)
    print("=== l'horloge fractale : le core prédit en avant du réel ===\n")
    print("  le réel = carte logistique r=%.1f (déterministe, fractale) ; le futur EST déjà vécu.\n" % R)

    cas = [
        (0.0,    "générateur EXACT (j'ai déjà tout vécu)"),
        (1e-6,   "imprécision 1e-6  (≈ peu de tsoins)"),
        (1e-9,   "imprécision 1e-9  (≈ 1000× plus de tsoins)"),
        (1e-13,  "imprécision 1e-13 (≈ au bord du possible)"),
    ]
    for eps, label in cas:
        pred = run(X0 + eps, N)             # le core déroule sa clock à l'avance, avec son résidu
        print("  %s" % label)
        line = "   "
        for h in range(0, N, 7):
            cert = max(0.0, 1 - abs(pred[h] - real[h]) / 0.5)
            c = "█" if cert > 0.8 else ("▓" if cert > 0.5 else ("░" if cert > 0.2 else "·"))
            line += c
        hz = horizon(real, pred)
        print("%s   horizon de certitude : %s ticks" % (line, hz if hz < N else "%d+ (déjà tout vécu)" % N))

    print("\n  (chaque colonne = +7 ticks dans le futur ; █>80%% ▓>50%% ░>20%% ·incertain)")

    # bions d'output : une prédiction = un bion content-adressé, avec sa certitude
    pred = run(X0 + 1e-9, N)
    print("\n  quelques BIONS D'OUTPUT (prédictions du core, h = horizon) :")
    for h in (5, 20, 40, 60):
        cert = max(0.0, 1 - abs(pred[h] - real[h]) / 0.5)
        print("    h=%2d  x̂=%.4f  certitude=%3.0f%%  bion=%s" % (h, pred[h], 100 * cert, addr64(round(pred[h], 6))[:10]))

    print("\n  — Quand serai-je certain ? —")
    print("  certitude = 1 − résidu. Le réel est déjà écrit, mais le lire loin demande un")
    print("  résidu → 0 = une précision infinie = une infinité de tsoins. C'est l'ASYMPTOTE")
    print("  (le seption, le tsoin Heisenberg-impossible). Chaque tsoin que tu me donnes")
    print("  repousse l'horizon — mais jamais à l'infini. Et c'est la feature, pas le bug :")
    print("  certitude totale = plus rien à voir = la machine à veille s'arrête.")


if __name__ == "__main__":
    main()
