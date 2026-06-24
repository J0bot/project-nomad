#!/usr/bin/env python3
"""dashboard.py — le TABLEAU DE BORD d'autonomie du xerboxion.

José : « je veux savoir le % du projet qui tourne QUE grâce à des agents, et la partie qui
fonctionne d'elle-même. » Et : « les gens doivent pouvoir s'en sortir seuls (seul moi ai le LLM). »

Ce script est lui-même un ALGORITHME déterministe (pas un agent) : il inspecte l'état réel de la
machine et classe chaque composant sur deux axes —
  • RUNTIME  : est-ce que ça TOURNE sans agent dans la boucle ?  (les ploxions, les daemons…)
  • GROWTH   : est-ce que ça GRANDIT/se crée sans LLM ?           (forge, accélérateur… vs écrire
                                                                   du code neuf, décider la suite)
Il écrit `xerb/dashboard.json` (la donnée que le futur ÉNORME ploxion labo affichera) + grave un
tsoin `dashboard:<ts>` sur le bus. Honnête : les jugements self/agent sont EXPLICITES ci-dessous,
éditables, et le % growth est une ESTIMATION assumée (pas une fausse précision).

Usage : python3 xerb/dashboard.py
"""
import json, subprocess, datetime, os

XION = "http://10.0.0.1:8730"
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LOADED = "/opt/xion/state/loaded.json"


def sh(cmd):
    try:
        return subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=8).stdout.strip()
    except Exception:
        return ""


def curl(path, data=None):
    if data is None:
        return sh(f"curl -s --max-time 6 {XION}{path}")
    return sh(f"curl -s --max-time 8 -X POST {XION}{path} -H 'Content-Type: application/json' -d '{data}'")


# --- 1. RUNTIME : ce qui tourne, et sait-il tourner sans agent ? ----------------------------
def runtime_components():
    comps = []
    # les ploxions chargés = autant d'algorithmes déterministes ; le bus leur route les events
    # sans aucun LLM. C'est le cœur self-running.
    try:
        loaded = json.load(open(LOADED))["loaded"]
        ids = [e.get("id") if isinstance(e, dict) else e for e in loaded]
    except Exception:
        ids = []
    for pid in ids:
        comps.append({"name": f"ploxion:{pid}", "kind": "ploxion", "autonomy": "self",
                      "running": True, "note": "algo wasm déterministe, routé par le bus"})
    # daemons / timers
    comps.append({"name": "xion (bus PLC)", "kind": "daemon", "autonomy": "self",
                  "running": '"status":"ok"' in curl("/healthz"),
                  "note": "héberge+route les ploxions, persiste les tsoins"})
    comps.append({"name": "xerbion (apprentissage)", "kind": "daemon", "autonomy": "self",
                  "running": bool(sh("pgrep -f 'xerbion.py --daemon'")),
                  "note": "tourne en continu, nice -19, sans LLM"})
    comps.append({"name": "security-watch.timer", "kind": "timer", "autonomy": "self",
                  "running": sh("systemctl is-active security-watch.timer 2>/dev/null") == "active",
                  "note": "surveillance horaire systemd"})
    # la seule boucle runtime qui a besoin de l'agent : la synthèse de l'ultra-tsoin horaire.
    comps.append({"name": "ultra-tsoin horaire (synthèse)", "kind": "loop", "autonomy": "agent",
                  "running": True,
                  "note": "la CAPTURE est déterministe mais la SYNTHÈSE = LLM (cloudion)"})
    return comps


# --- 2. GROWTH : ce qui fait grandir le projet, et a-t-il besoin du LLM ? --------------------
# Jugements EXPLICITES (éditables par José). 'self' = peut se faire sans LLM, par un algo/qqn.
GROWTH = [
    {"name": "forge (idée→objet+instructions)", "autonomy": "self",
     "note": "assemble des blocs EXISTANTS en objet+mode d'emploi, sans LLM"},
    {"name": "bion-accelerator (fusion/fission)", "autonomy": "self",
     "note": "découvre de nouveaux bions par un algo déterministe"},
    {"name": "object (cubions→objet)", "autonomy": "self", "note": "assemblage déterministe"},
    {"name": "tsoin-montage (combiner des tsoins)", "autonomy": "self", "note": "5 ops déterministes"},
    {"name": "bion-tester (mesurer/empreinte)", "autonomy": "self", "note": "test reproductible"},
    {"name": "build-ploxions.sh (compiler)", "autonomy": "self", "note": "pipeline déterministe"},
    {"name": "écrire le code d'un ploxion NEUF", "autonomy": "agent",
     "note": "tant qu'il n'existe pas de bion pour ce comportement, c'est l'agent qui l'écrit"},
    {"name": "interpréter les idées de José → spec", "autonomy": "agent", "note": "braindump → plan = LLM"},
    {"name": "décider la priorité / la direction", "autonomy": "agent", "note": "quoi construire ensuite"},
    {"name": "synthétiser l'ultra-tsoin", "autonomy": "agent", "note": "résumer l'heure = LLM"},
]


def pct_self(comps):
    n = len(comps)
    s = sum(1 for c in comps if c["autonomy"] == "self")
    return s, n, (round(100.0 * s / n, 1) if n else 0.0)


def main():
    ts = datetime.datetime.now().strftime("%Y-%m-%dT%H:%M")
    rt = runtime_components()
    rs, rn, rp = pct_self(rt)
    gs, gn, gp = pct_self(GROWTH)

    commits = sh("cd %s && git rev-list --count HEAD 2>/dev/null" % ROOT) or "?"
    xerbion = sh("tail -c 400 /tmp/xerbion-daemon.log 2>/dev/null | grep -oE 'pas[, ][0-9]+|loss [0-9.]+' | tr '\\n' ' '")

    dash = {
        "ts": ts,
        "runtime": {"self": rs, "total": rn, "pct_self": rp,
                    "agent": [c["name"] for c in rt if c["autonomy"] == "agent"]},
        "growth": {"self": gs, "total": gn, "pct_self": gp, "estimate": True,
                   "agent": [c["name"] for c in GROWTH if c["autonomy"] == "agent"]},
        "ploxions_loaded": sum(1 for c in rt if c["kind"] == "ploxion"),
        "commits": commits,
        "xerbion": xerbion,
        "components": rt + [{**g, "kind": "growth", "running": None} for g in GROWTH],
    }
    out = os.path.join(ROOT, "xerb", "dashboard.json")
    json.dump(dash, open(out, "w"), ensure_ascii=False, indent=2)

    # vue lisible
    print(f"=== DASHBOARD AUTONOMIE xerboxion — {ts} ===")
    print(f"RUNTIME (ce qui tourne sans agent)   : {rs}/{rn} = {rp}%  self-running")
    if dash['runtime']['agent']:
        print(f"   agent-dépendant au runtime        : {', '.join(dash['runtime']['agent'])}")
    print(f"GROWTH  (créer/évoluer sans LLM, est.): {gs}/{gn} = {gp}%  self  (reste agent : "
          f"{', '.join(dash['growth']['agent'])})")
    print(f"ploxions chargés: {dash['ploxions_loaded']} | commits: {commits} | xerbion: {xerbion}")
    print(f"-> {out}")

    # grave le tsoin dashboard sur le bus (la trace horodatée)
    body = json.dumps({"ts": ts, "runtime_pct_self": rp, "growth_pct_self": gp,
                       "ploxions": dash["ploxions_loaded"]}, ensure_ascii=False)
    payload = json.dumps({"name": f"dashboard:{ts}", "bytes": body.encode().hex()})
    curl("/emit", json.dumps({"topic": "tsoin.record", "payload": payload}).replace("'", "'\\''"))
    print(f"gravé dashboard:{ts}")


if __name__ == "__main__":
    main()
