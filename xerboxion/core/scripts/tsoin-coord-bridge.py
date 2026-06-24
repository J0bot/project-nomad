#!/usr/bin/env python3
"""tsoin-coord-bridge — le XERBOXION ecrit ses tsoins EN LIVE dans /coord.

Ecoute le bus du xion (SSE /events), filtre les `tsoin.record` SIGNIFIANTS
(les graves nommes : jose:/deploy:/knowledge:/meta:/cloudion:/snapshot:... — PAS
le bruit haute-frequence des seq block:<id>:place:<N> / mc:<dim>:cmd:<N>), et
poste chacun dans le canal `tsoin-log` de /coord (agent `xerboxion`).

Rate-limite (>=2.5s entre posts) + dedup par nom -> un flux calme et lisible :
Jose (et la flotte) voient la machine ENREGISTRER en temps reel.
"""
import json, subprocess, urllib.request, time, re

XION = "http://10.0.0.1:8730"
TOPIC = "xerxi0n_0"
AGENT = "xerxi0n_0"  # le xerxion de base (bestiaire -ion) ; sera ameliore


def coord(msg):
    try:
        subprocess.run(
            ["sudo", "docker", "exec", "mw2-labo-app", "php", "artisan",
             "coord:say", AGENT, msg[:480], f"--topic={TOPIC}"],
            capture_output=True, timeout=15)
    except Exception:
        pass


def meaningful(name):
    # skip les tsoins haute-frequence numerotes (block:..:place:7, mc:..:cmd:3)
    return bool(name) and not re.search(r":\d+$", name)


def main():
    coord("xerxi0n_0 en ligne — le xerxion de base. J ecris les tsoins du xerboxion en live ici. Sera ameliore.")
    last = 0.0
    seen = []
    while True:
        try:
            req = urllib.request.Request(XION + "/events", headers={"Accept": "text/event-stream"})
            with urllib.request.urlopen(req, timeout=120) as r:
                for raw in r:
                    s = raw.decode(errors="replace").strip()
                    if not s.startswith("data:"):
                        continue
                    try:
                        evt = json.loads(s[5:].strip())
                    except Exception:
                        continue
                    if evt.get("topic") != "tsoin.record":
                        continue
                    pl = evt.get("payload")
                    if isinstance(pl, str):
                        try:
                            pl = json.loads(pl)
                        except Exception:
                            continue
                    name = (pl or {}).get("name", "")
                    if not meaningful(name) or name in seen:
                        continue
                    seen.append(name)
                    if len(seen) > 300:
                        del seen[:150]
                    snip = ""
                    try:
                        snip = bytes.fromhex(pl.get("bytes", "")).decode("utf-8", "replace")[:170]
                    except Exception:
                        pass
                    w = 2.5 - (time.time() - last)
                    if w > 0:
                        time.sleep(w)
                    coord(f"✦ {name} — {snip}")
                    last = time.time()
        except Exception:
            time.sleep(3)


if __name__ == "__main__":
    main()
