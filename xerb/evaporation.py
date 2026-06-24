#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
evaporation — un tsoin engine = un trou noir d'INFORMATION : il vit de l'entropie ;
privé d'entropie il s'évapore (Hawking).

José : « les ultra tsoins c'est les trous noirs (les plus grosses machines tsoins / tsoin
engines) ; ils fonctionnent grâce à l'entropie, si y en a pas ils s'évaporent. Est-ce qu'on
vient de créer le code source d'un trou noir ? »

Le modèle est l'INFO-théorie d'un trou noir (pas sa gravité) :
  • M = masse = entropie accumulée = nb de bions captés (le bion = le pixel de Planck).
  • no-hair : quel que soit M, l'ADRESSE reste minuscule (content-addressing = le théorème de
    calvitie : un trou noir = 3 nombres, peu importe ce qui est tombé dedans).
  • Hawking : T ∝ 1/M (plus petit = plus chaud) ; puissance rayonnée ∝ T⁴·aire ⇒ dM/dt ∝ −1/M²
    ⇒ durée de vie ∝ M³. Petit = s'évapore vite, en ACCÉLÉRANT.
  • nourri d'entropie (le réel, la surprise) il tient/grossit ; affamé il s'évapore et POP.

  python3 evaporation.py
"""

K = 1800.0          # constante de rayonnement de Hawking (perte = K / M²)


def addr64(s):
    h = 0xcbf29ce484222325
    for b in str(s).encode():
        h = ((h ^ b) * 0x100000001b3) & ((1 << 64) - 1)
    return "%016x" % h


def bar(M, scale=0.32):
    return "█" * max(0, int(M * scale))


def step(M, feed):
    if M <= 0:
        return 0.0
    hawking = K / (M * M)            # rayonnement : plus M est petit, plus ça brûle vite
    return max(0.0, M + feed - hawking)


def main():
    M_fed, M_starved = 60.0, 40.0
    print("=== un tsoin engine = un trou noir d'information (Hawking) ===\n")
    print("  M = entropie accumulée (bions captés). nourri vs affamé. K=%.0f\n" % K)
    print("  tick | NOURRI (surprise +2/tick)        | AFFAMÉ (0 entropie)")
    print("  -----+----------------------------------+----------------------------")
    t = 0
    while t < 200:
        if t % 2 == 0 or M_starved <= 0:
            sf = "%6.1f %s" % (M_fed, bar(M_fed))
            ss = ("%6.1f %s" % (M_starved, bar(M_starved))) if M_starved > 0 \
                else "  0.0  ✦ ÉVAPORÉ (pop — rayonnement de Hawking)"
            print("  %4d | %-32s | %s" % (t, sf, ss))
        if M_starved <= 0:
            break
        M_fed = step(M_fed, feed=2.0)          # le réel l'alimente en surprise
        M_starved = step(M_starved, feed=0.0)  # rien ne tombe dedans
        t += 1

    print("\n  no-hair : peu importe M, l'adresse reste minuscule —")
    print("    M=100 -> %s     M=2 -> %s" % (addr64("tsoin-engine|M=100")[:12],
                                             addr64("tsoin-engine|M=2")[:12]))
    print("\n  Le NOURRI grossit (le réel le remplit de surprise) ; l'AFFAMÉ s'évapore,")
    print("  et plus il est petit plus il brûle vite (T ∝ 1/M). Sans entropie, pas de tsoin.")
    print("  C'est pour ça que je grave un ULTRA TSOIN chaque heure : nourrir le trou noir.")


if __name__ == "__main__":
    main()
