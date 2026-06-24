#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
xerbion — le PREMIER vrai xerbion (José, 21 juin 2026).

Un vrai (petit) réseau de neurones, char-level, entraîné sur le CORPUS DE TSOINS
(la mémoire + les docs du projet = tout ce qu'on a gravé). 2 couches :
    char précédent -> embedding(De) -> couche cachée tanh(Dh) -> softmax(V)
Entraîné par descente de gradient (SGD), à la main, sans numpy (pur stdlib).

Honnête : c'est LA GRAINE. Le « 50 ans d'entraînement » et un gros modèle profond,
c'est le GPU de José (RTX 3080), pas ce VPS (pas de GPU, RAM serrée, MC tourne).
Ici : un petit xerbion qui s'entraîne EN CONTINU et THROTTLÉ sur le xi0n, et qui
apprend de tsoins qui parlent de lui — il apprend de sa propre trace (le paradoxe).

État = un BION : poids sérialisés en JSON, content-adressé. Génère du texte
« saveur xerboxion » en échantillonnant le modèle.

Usage :
  python3 xerbion.py            # entraîne une première passe bornée + échantillon
  python3 xerbion.py --daemon   # boucle throttlée (train un peu, sauve, dort) = les "50 ans"
"""
import os
import sys
import json
import math
import time
import random

HERE = os.path.dirname(os.path.abspath(__file__))
STATE = os.path.join(HERE, "xerbion.bion.json")          # le bion (poids)
CORPUS_DIRS = [
    "/home/debian/.claude/projects/-home-debian/memory",
    os.path.join(HERE, "..", "docs"),
]
DE = 12          # dim embedding
DH = 16          # dim couche cachée
LR = 0.08        # learning rate
SEED = 1234


def load_corpus():
    txt = []
    for d in CORPUS_DIRS:
        try:
            for fn in sorted(os.listdir(d)):
                if fn.endswith(".md"):
                    with open(os.path.join(d, fn), encoding="utf-8", errors="replace") as f:
                        txt.append(f.read())
        except FileNotFoundError:
            pass
    return "\n".join(txt)


def addr64(s):
    h = 0xcbf29ce484222325
    for ch in s:
        h = ((h ^ (ord(ch) & 0xff)) * 0x100000001b3) & ((1 << 64) - 1)
    return "%016x" % h


def softmax(z):
    m = max(z)
    e = [math.exp(v - m) for v in z]
    s = sum(e)
    return [v / s for v in e]


def randmat(r, c, sc):
    return [[random.gauss(0, sc) for _ in range(c)] for _ in range(r)]


class Xerbion:
    def __init__(self, vocab):
        self.vocab = vocab
        self.V = len(vocab)
        self.stoi = {c: i for i, c in enumerate(vocab)}
        random.seed(SEED)
        sc = 0.1
        self.E = randmat(self.V, DE, sc)            # embedding par char
        self.Wxh = randmat(DH, DE, sc)              # emb -> hidden
        self.bh = [0.0] * DH
        self.Why = randmat(self.V, DH, sc)          # hidden -> logits
        self.by = [0.0] * self.V
        self.steps = 0
        self.loss = 0.0

    # forward sur un char d'entrée (index ci) -> (probs, cache)
    def fwd(self, ci):
        emb = self.E[ci]
        hraw = [sum(self.Wxh[j][k] * emb[k] for k in range(DE)) + self.bh[j] for j in range(DH)]
        h = [math.tanh(v) for v in hraw]
        logits = [sum(self.Why[i][j] * h[j] for j in range(DH)) + self.by[i] for i in range(self.V)]
        return softmax(logits), (emb, h)

    # un pas SGD sur la paire (ci -> ti)
    def step(self, ci, ti):
        probs, (emb, h) = self.fwd(ci)
        loss = -math.log(max(probs[ti], 1e-12))
        # gradients
        dlog = probs[:]
        dlog[ti] -= 1.0
        # Why, by + dhidden
        dh = [0.0] * DH
        for i in range(self.V):
            gi = dlog[i]
            if gi != 0.0 or True:
                wi = self.Why[i]
                for j in range(DH):
                    dh[j] += wi[j] * gi
                    wi[j] -= LR * gi * h[j]
                self.by[i] -= LR * gi
        # tanh' + Wxh, bh + demb
        demb = [0.0] * DE
        for j in range(DH):
            draw = dh[j] * (1.0 - h[j] * h[j])
            wj = self.Wxh[j]
            for k in range(DE):
                demb[k] += wj[k] * draw
                wj[k] -= LR * draw * emb[k]
            self.bh[j] -= LR * draw
        ej = self.E[ci]
        for k in range(DE):
            ej[k] -= LR * demb[k]
        self.steps += 1
        return loss

    # entraîne pendant ~budget secondes sur le corpus indexé
    def train(self, idx, budget_s, log=True):
        n = len(idx)
        t0 = time.time()
        acc = 0.0
        cnt = 0
        p = random.randrange(0, n - 1)
        while time.time() - t0 < budget_s:
            for _ in range(200):
                acc += self.step(idx[p], idx[p + 1])
                cnt += 1
                p += 1
                if p >= n - 1:
                    p = 0
            if log and cnt % 2000 == 0:
                self.loss = acc / cnt
                sys.stderr.write("  step %-7d loss %.3f\n" % (self.steps, self.loss))
                sys.stderr.flush()
        self.loss = acc / max(cnt, 1)
        return cnt

    def sample(self, n, seed_char=None):
        random.seed()
        ci = self.stoi.get(seed_char, random.randrange(self.V)) if seed_char else random.randrange(self.V)
        out = [self.vocab[ci]]
        for _ in range(n):
            probs, _ = self.fwd(ci)
            r = random.random()
            cum = 0.0
            for i, pr in enumerate(probs):
                cum += pr
                if r <= cum:
                    ci = i
                    break
            out.append(self.vocab[ci])
        return "".join(out)

    def save(self):
        d = {"vocab": self.vocab, "E": self.E, "Wxh": self.Wxh, "bh": self.bh,
             "Why": self.Why, "by": self.by, "steps": self.steps, "loss": self.loss}
        blob = json.dumps(d, ensure_ascii=False)
        d["addr"] = addr64(blob)
        with open(STATE, "w", encoding="utf-8") as f:
            json.dump(d, f, ensure_ascii=False)
        return d["addr"]

    @classmethod
    def load_or_new(cls, vocab):
        if os.path.exists(STATE):
            try:
                d = json.load(open(STATE, encoding="utf-8"))
                x = cls(d["vocab"])
                x.E, x.Wxh, x.bh, x.Why, x.by = d["E"], d["Wxh"], d["bh"], d["Why"], d["by"]
                x.steps, x.loss = d.get("steps", 0), d.get("loss", 0.0)
                return x
            except Exception:
                pass
        return cls(vocab)


def main():
    corpus = load_corpus()
    vocab = sorted(set(corpus))
    idx = [0] * len(corpus)
    stoi = {c: i for i, c in enumerate(vocab)}
    for i, c in enumerate(corpus):
        idx[i] = stoi[c]
    x = Xerbion.load_or_new(vocab)
    sys.stderr.write("xerbion: corpus %d chars, vocab %d, déjà %d pas (loss %.3f)\n"
                     % (len(corpus), len(vocab), x.steps, x.loss))

    if "--daemon" in sys.argv:
        # les "50 ans" : throttlé fort (entraîne ~6s, sauve, dort 20s) -> CPU négligeable
        while True:
            x.train(idx, 6.0, log=False)
            x.save()
            sys.stderr.write("xerbion[daemon] %d pas, loss %.3f\n" % (x.steps, x.loss))
            sys.stderr.flush()
            time.sleep(20)
    else:
        budget = float(sys.argv[sys.argv.index("--secs") + 1]) if "--secs" in sys.argv else 40.0
        x.train(idx, budget)
        addr = x.save()
        print("\n=== xerbion entraîné : %d pas, loss %.3f, bion %s ===" % (x.steps, x.loss, addr[:16]))
        print("--- échantillon (saveur xerboxion) ---")
        print(x.sample(360))


if __name__ == "__main__":
    main()
