#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
train_concept — entraîner le VRAI xerbion (concept-level) sur la RTX 3080.

José : « je veux que mon réseau de neurones ait le CONCEPT (pas le contexte), et le concept
c'est le tsoin. » Le xerbion-démo prédit des caractères (aucun concept) ; ici on entraîne un
petit Transformer au niveau du **token-concept** (mot du xerboxion), sur le corpus du repo
(docs/, la thèse, les papiers, le primer, les outils xerb). Le réseau apprend les
**embeddings** = la variété des concepts ; les poids DEVIENNENT le xerboxion (in-weights,
permanent), au lieu d'être dans une fenêtre de contexte (in-context, éphémère).

⚠️ Écrit pour TA machine (silentbeast + 3080). Il n'a PAS été exécuté côté VPS (ni torch ni
GPU là-bas). C'est du PyTorch standard — ton agent installe torch (build CUDA) et le lance ;
s'il y a un détail à ajuster, il itère.

  pip install torch --index-url https://download.pytorch.org/whl/cu121   (build CUDA)
  python xerbion/train_concept.py                 # entraîne (utilise la 3080 auto)
  python xerbion/train_concept.py --sample "le tsoin"   # génère en xerboxion
  python xerbion/train_concept.py --near temps          # concepts proches (le manifold appris)
"""
import os
import re
import glob
import argparse

import torch
import torch.nn as nn
from torch.nn import functional as F

# --- hyperparamètres (modestes : petit corpus, entraînement rapide sur 3080) ---
BLOCK = 64        # contexte (en concepts)
N_EMBD = 192      # dimension d'embedding = la taille du vecteur-concept
N_HEAD = 6
N_LAYER = 4
DROPOUT = 0.1
LR = 3e-4
STEPS = 3000
BATCH = 32
SEED = 1337

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
CKPT = os.path.join(HERE, "xerbion-concept.pt")
DEV = "cuda" if torch.cuda.is_available() else "cpu"


def load_corpus():
    parts = []
    for pat in (os.path.join(REPO, "**", "*.md"), os.path.join(REPO, "**", "*.py")):
        for p in glob.glob(pat, recursive=True):
            if any(x in p for x in ("target", "node_modules", ".git", "vendor")):
                continue
            try:
                parts.append(open(p, encoding="utf-8", errors="replace").read())
            except Exception:
                pass
    return "\n\n".join(parts)


def tokenize(s):
    # niveau CONCEPT : un mot xerboxion (ou un symbole) = un token
    return re.findall(r"[a-zà-ÿ0-9_\-]+|[^\sa-zà-ÿ0-9]", s.lower())


class Block(nn.Module):
    def __init__(self):
        super().__init__()
        self.ln1 = nn.LayerNorm(N_EMBD)
        self.ln2 = nn.LayerNorm(N_EMBD)
        self.attn = nn.MultiheadAttention(N_EMBD, N_HEAD, dropout=DROPOUT, batch_first=True)
        self.mlp = nn.Sequential(nn.Linear(N_EMBD, 4 * N_EMBD), nn.GELU(),
                                 nn.Linear(4 * N_EMBD, N_EMBD), nn.Dropout(DROPOUT))

    def forward(self, x, mask):
        h = self.ln1(x)
        a, _ = self.attn(h, h, h, attn_mask=mask, need_weights=False)
        x = x + a
        x = x + self.mlp(self.ln2(x))
        return x


class Xerbion(nn.Module):
    def __init__(self, vocab):
        super().__init__()
        self.tok = nn.Embedding(vocab, N_EMBD)        # <- les vecteurs-concepts
        self.pos = nn.Embedding(BLOCK, N_EMBD)
        self.blocks = nn.ModuleList([Block() for _ in range(N_LAYER)])
        self.ln = nn.LayerNorm(N_EMBD)
        self.head = nn.Linear(N_EMBD, vocab)

    def forward(self, idx, targets=None):
        T = idx.size(1)
        x = self.tok(idx) + self.pos(torch.arange(T, device=idx.device))
        mask = torch.triu(torch.full((T, T), float("-inf"), device=idx.device), 1)
        for b in self.blocks:
            x = b(x, mask)
        logits = self.head(self.ln(x))
        loss = None
        if targets is not None:
            loss = F.cross_entropy(logits.reshape(-1, logits.size(-1)), targets.reshape(-1))
        return logits, loss


def build():
    torch.manual_seed(SEED)
    toks = tokenize(load_corpus())
    vocab = sorted(set(toks))
    stoi = {w: i for i, w in enumerate(vocab)}
    itos = {i: w for w, i in stoi.items()}
    data = torch.tensor([stoi[w] for w in toks], dtype=torch.long)
    return data, vocab, stoi, itos


def get_batch(data):
    ix = torch.randint(len(data) - BLOCK - 1, (BATCH,))
    x = torch.stack([data[i:i + BLOCK] for i in ix])
    y = torch.stack([data[i + 1:i + BLOCK + 1] for i in ix])
    return x.to(DEV), y.to(DEV)


def train():
    data, vocab, stoi, itos = build()
    print("device=%s  tokens=%d  vocab(concepts)=%d" % (DEV, len(data), len(vocab)))
    model = Xerbion(len(vocab)).to(DEV)
    n_params = sum(p.numel() for p in model.parameters())
    print("xerbion concept-level : %.2fM paramètres (les poids = les concepts)\n" % (n_params / 1e6))
    opt = torch.optim.AdamW(model.parameters(), lr=LR)
    for step in range(1, STEPS + 1):
        x, y = get_batch(data)
        _, loss = model(x, y)
        opt.zero_grad(set_to_none=True)
        loss.backward()
        opt.step()
        if step % 200 == 0 or step == 1:
            print("step %4d/%d  loss %.3f" % (step, STEPS, loss.item()))
    torch.save({"model": model.state_dict(), "vocab": vocab,
                "cfg": dict(N_EMBD=N_EMBD, N_HEAD=N_HEAD, N_LAYER=N_LAYER, BLOCK=BLOCK)}, CKPT)
    print("\nxerbion sauvé -> %s  (les concepts sont DANS LES POIDS, permanents)" % CKPT)
    print("essaie :  python train_concept.py --near temps   |   --sample \"le tsoin\"")


def _load():
    ck = torch.load(CKPT, map_location=DEV)
    vocab = ck["vocab"]
    model = Xerbion(len(vocab)).to(DEV)
    model.load_state_dict(ck["model"])
    model.eval()
    return model, vocab, {w: i for i, w in enumerate(vocab)}


@torch.no_grad()
def near(word, k=10):
    model, vocab, stoi = _load()
    if word not in stoi:
        print("'%s' hors vocabulaire" % word); return
    E = F.normalize(model.tok.weight, dim=1)         # la variété des concepts
    sims = E @ E[stoi[word]]
    top = sims.topk(k + 1).indices.tolist()[1:]
    print("concepts proches de « %s » (dans la variété apprise) :" % word)
    for i in top:
        print("   %-18s  %.3f" % (vocab[i], sims[i].item()))


@torch.no_grad()
def sample(prompt, n=60):
    model, vocab, stoi = _load()
    ids = [stoi[w] for w in tokenize(prompt) if w in stoi] or [0]
    idx = torch.tensor([ids], device=DEV)
    for _ in range(n):
        logits, _ = model(idx[:, -BLOCK:])
        probs = F.softmax(logits[:, -1, :], dim=-1)
        nxt = torch.multinomial(probs, 1)
        idx = torch.cat([idx, nxt], dim=1)
    print(" ".join(vocab[i] for i in idx[0].tolist()))


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--near")
    ap.add_argument("--sample")
    a = ap.parse_args()
    if a.near:
        near(a.near)
    elif a.sample:
        sample(a.sample)
    else:
        train()
