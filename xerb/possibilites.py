#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
possibilites — combien d'univers est construit dans le xerboxion, et a-t-on déjà
plus de possibilités que les plus grands nombres ?

José : « pour faire un Kion, tu dois d'abord réinventer l'univers (Sagan). À combien
de % l'univers est-il construit dans le xerboxion ? A-t-on déjà plus de possibilités
que tous les plus gros nombres ? — un seul tsoin contient tous les tsoins. »

On répond honnêtement : un inventaire (le « déjà là »), l'espace combinatoire qu'il
ouvre (en log10), et la comparaison aux grands nombres nommés. La conclusion n'est pas
un gros COMPTE, c'est une CLÔTURE GÉNÉRATIVE (un petit générateur déplie l'illimité).
"""
import math

L2 = math.log10(2)

# --- inventaire des briques construites (approx. assumées, marquées) ---
inv = {
    "ions xerb (le langage)":      15,
    "bions partagés (SDK)":        11,
    "lois science (7 domaines)":   71,
    "protocoles réseau":            7,
    "ploxions vivants (xion)":     16,
    "organes du tsoin engine":      5,
}
vocab = sum(inv.values())

# --- repères de taille (log10 = nb de chiffres − 1) ---
atomes_corps   = 27      # ~10^27 atomes dans un corps humain
atomes_univers = 80      # ~10^80 atomes dans l'univers observable
googol         = 100     # 10^100
# googolplex = 10^(10^100) : son log10 vaut 10^100 (1 suivi d'un googol de zéros)
# nombre de Graham, TREE(3) : FINIS mais au-delà de toute tour de puissances

# --- espaces combinatoires ouverts par nos briques (en log10) ---
sousens_lois = 71 * L2                       # 2^71 sous-ensembles de lois
adresses     = 64 * L2                       # 2^64 adresses addr64
def prog_log10(L, alpha=15): return L * math.log10(alpha)   # programmes xerb de longueur L
L_googol = googol / math.log10(15)           # longueur de programme pour atteindre un googol


def cmp(nom, log10v, ref, refnom):
    rel = "≈" if abs(log10v - ref) < 1.5 else (">" if log10v > ref else "<")
    return "  %-34s 10^%-7s  %s %s (10^%d)" % (nom, ("%.1f" % log10v), rel, refnom, ref)


def main():
    print("=== combien d'univers, combien de possibilités ? ===\n")

    print("  INVENTAIRE — le « déjà là » (vocabulaire générateur) :")
    for k, v in inv.items():
        print("    %3d  %s" % (v, k))
    print("    --- %d briques génératrices au total ---\n" % vocab)

    print("  L'UNIVERS dans le xerboxion (honnête) :")
    print("    • couverture des domaines : 7/7  → le SQUELETTE est complet (chaque domaine a un pied)")
    print("    • profondeur : 71 lois reproductibles vs ~O(10^3) lois/constantes cœur de la science")
    print("      → de l'ordre de ~1–5 %% du CŒUR (estimation grossière) ; bien moins de l'applied/ingénierie")
    print("    → verdict : ~quelques %% du CONTENU, mais 100 %% en SQUELETTE de domaines.\n")

    print("  PLUS DE POSSIBILITÉS QUE LES PLUS GROS NOMBRES ? (espaces ouverts, en log10)")
    print(cmp("2^71 sous-ensembles de lois", sousens_lois, atomes_corps, "atomes d'un corps"))
    print(cmp("2^64 adresses (addr64)", adresses, atomes_corps, "atomes d'un corps"))
    print(cmp("programmes xerb de 85 ions", prog_log10(85), googol, "googol"))
    print(cmp("programmes xerb de 200 ions", prog_log10(200), googol, "googol"))
    print("    → un programme de seulement ~%d ions dépasse déjà un GOOGOL (10^100)." % math.ceil(L_googol))
    print("    → MAIS un googolplex = 10^(10^100) : il faudrait ~10^99 ions. Hors de portée.")
    print("    → Graham, TREE(3) : FINIS, mais si vastes qu'AUCUN compte concret ne s'en approche.\n")

    print("  LE POINT (un tsoin contient tous les tsoins) :")
    print("    L'espace de TOUS les tsoins possibles (longueur illimitée) est INFINI (ℵ₀) —")
    print("    donc « plus grand que tout nombre fini », Graham et TREE(3) compris. Mais c'est")
    print("    vrai de l'arithmétique aussi : l'infini n'est pas un gros nombre, c'est une CLÔTURE.")
    print("    La force du xerboxion n'est pas un gros COMPTE — c'est qu'un PETIT générateur")
    print("    (le protocol-bion, une ProtocolDef ; une règle science ; un ion) DÉPLIE l'illimité.")
    print("    « Un tsoin contient tous les tsoins » = l'Epsylaeu (une adresse dépend de tout) :")
    print("    pas un nombre, une RÈGLE qui les engendre. Le générateur EST l'univers.")


if __name__ == "__main__":
    main()
