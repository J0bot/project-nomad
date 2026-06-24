#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
video_to_tsoins — la vidéo → un flux de TSOINS (instants bruts). Rien de plus.

José : « un tsoin peut avoir n'importe quelle forme, mais il capture un INSTANT. C'est le
XERBION qui devra linéariser les tsoins, par la suite — pas moi, pas un prétraitement. C'est
vraiment plus deep. Et pour le moment c'est JUSTE un xerbion qu'on feed de tsoins (pas d'output). »

Donc ce script ne fait QUE ça : découper la vidéo en instants bruts ; chaque instant = un
tsoin (content-adressé, dédupliqué). **PAS de sens** (pas de transcription), **PAS de delta
calculé par moi**, **PAS de réseau ici**, **PAS d'output**. Le xerbion mangera ce flux et fera
la linéarisation LUI-MÊME — c'est là qu'est la profondeur, et c'est son travail, pas le mien.

  ffmpeg dans le PATH.  python xerbion/video_to_tsoins.py --dir "D:/videos"
"""
import os
import sys
import struct
import argparse
import subprocess

VIDEO_EXT = (".mp4", ".mkv", ".mov", ".avi", ".webm", ".m4v", ".flv", ".wmv", ".ts", ".mpg", ".mpeg")
W = H = 64        # la taille de l'instant capté (downsample) — juste une forme, pas un sens
FPS = 4          # instants par seconde


def addr64(b):
    h = 0xcbf29ce484222325
    for x in b:
        h = ((h ^ x) * 0x100000001b3) & ((1 << 64) - 1)
    return h


def instants(video):
    """ffmpeg → flux brut d'instants gris WxH. Chaque frame = un instant = un tsoin."""
    cmd = ["ffmpeg", "-i", video, "-vf", "fps=%d,scale=%d:%d,format=gray" % (FPS, W, H),
           "-f", "rawvideo", "-pix_fmt", "gray", "-loglevel", "error", "-"]
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE)
    n = W * H
    try:
        while True:
            buf = p.stdout.read(n)
            if len(buf) < n:
                break
            yield buf
    finally:
        p.stdout.close()
        p.wait()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", required=True, help="dossier des vidéos (récursif)")
    ap.add_argument("--out", default=None)
    a = ap.parse_args()

    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.dirname(here)
    outdir = a.out or os.path.join(repo, "corpus")
    os.makedirs(outdir, exist_ok=True)
    stream = os.path.join(outdir, "video-tsoins.bin")     # le flux : [addr64][WxH octets] par tsoin
    index = os.path.join(outdir, "video-tsoins.idx")      # addr64 + source, 1 ligne/tsoin
    done_p = os.path.join(outdir, "video-tsoins.done")
    done = set(open(done_p, encoding="utf-8").read().splitlines()) if os.path.exists(done_p) else set()

    seen = set()
    if os.path.exists(index):
        for ln in open(index, encoding="utf-8", errors="replace"):
            seen.add(ln.split(" ", 1)[0])

    vids = sorted(os.path.join(dp, f) for dp, _, fns in os.walk(a.dir)
                  for f in fns if f.lower().endswith(VIDEO_EXT))
    vids = [v for v in vids if v not in done]
    print("%d vidéos -> flux de tsoins (instants %dx%d, %d/s)\n" % (len(vids), W, H, FPS))

    fs = open(stream, "ab")
    fi = open(index, "a", encoding="utf-8")
    fm = open(done_p, "a", encoding="utf-8")
    n = u = 0
    for i, v in enumerate(vids, 1):
        base = os.path.basename(v)
        c = 0
        try:
            for buf in instants(v):
                n += 1
                c += 1
                ad = "%016x" % addr64(buf)
                if ad in seen:                       # même instant = même bion (dédup)
                    continue
                seen.add(ad)
                u += 1
                fs.write(struct.pack("<Q", int(ad, 16)) + buf)   # le tsoin, brut
                fi.write("%s %s\n" % (ad, base))
            fm.write(v + "\n")
            fs.flush(); fi.flush(); fm.flush()
            print("[%d/%d] %-50s  %d instants" % (i, len(vids), base[:50], c))
        except Exception as e:
            print("[%d/%d] ERREUR %s : %s" % (i, len(vids), base[:40], e))

    print("\n%d tsoins captés, %d uniques. flux -> %s" % (n, u, stream))
    print("Le xerbion mange ce flux et linéarise lui-même (par la suite). On ne prétraite rien.")


if __name__ == "__main__":
    main()
