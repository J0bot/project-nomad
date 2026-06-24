#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
sys-ram — le BION RAM : capture la RAM de l'hôte et la pousse (1) sur le bus xion (topic
`sys.ram`, pour le ploxion RAM du core / le xer connecté au xion) et (2) dans le storage du labo
(docker cp -> le labo sert /sys/ram.json, ploxion RAM côté labo.j0bot.ch SSO). Même pattern que
security-watch. José : « un ploxion sur bion RAM, pour voir sur labo.j0bot.ch avec SSO connecté. »

Boucle légère (toutes les ~20 s). Lancer comme service (sys-ram.service) ou en fond.
"""
import json, subprocess, time, urllib.request
from busauth import bus_headers  # auth bus (XION_BUS_TOKEN), rétrocompat

XION = "http://10.0.0.1:8730"
LABO_CONTAINER = "mw2-labo-app"
LABO_PATH = "/var/www/html/storage/app/sys-ram.json"
HOST_TMP = "/home/debian/.sys-ram.json"
INTERVAL = 20


def sh(args, timeout=10):
    try:
        return subprocess.run(args, capture_output=True, text=True, timeout=timeout).stdout
    except Exception:
        return ""


def capture():
    # free -m : total used free shared buff/cache available ; ligne Swap.
    out = sh(["free", "-m"])
    mem, swap = {}, {}
    for line in out.splitlines():
        p = line.split()
        if line.startswith("Mem:") and len(p) >= 7:
            mem = {"total": int(p[1]), "used": int(p[2]), "free": int(p[3]),
                   "buff_cache": int(p[5]), "avail": int(p[6])}
        elif line.startswith("Swap:") and len(p) >= 3:
            swap = {"total": int(p[1]), "used": int(p[2]), "free": int(p[3])}
    pct = round(100.0 * mem.get("used", 0) / mem.get("total", 1), 1) if mem.get("total") else 0
    pct_avail = round(100.0 * mem.get("avail", 0) / mem.get("total", 1), 1) if mem.get("total") else 0
    # top 5 RSS (qui mange la RAM) — utile à José (MC/flotte).
    top = []
    for ln in sh(["ps", "-eo", "rss,comm", "--sort=-rss"]).splitlines()[1:6]:
        q = ln.split(None, 1)
        if len(q) == 2:
            try:
                top.append({"mb": round(int(q[0]) / 1024), "name": q[1].strip()})
            except Exception:
                pass
    return {"mem": mem, "swap": swap, "pct_used": pct, "pct_avail": pct_avail, "top": top,
            "ts": time.strftime("%Y-%m-%dT%H:%M:%S")}


def push(snap):
    body = json.dumps(snap, ensure_ascii=False)
    # 1) bus xion
    try:
        data = json.dumps({"topic": "sys.ram", "payload": body}).encode()
        urllib.request.urlopen(urllib.request.Request(XION + "/emit", data=data,
                               headers=bus_headers()), timeout=6)
    except Exception:
        pass
    # 2) storage du labo (docker cp, comme security-watch)
    try:
        open(HOST_TMP, "w").write(body)
        subprocess.run(["sudo", "docker", "cp", HOST_TMP, f"{LABO_CONTAINER}:{LABO_PATH}"],
                       capture_output=True, timeout=15)
    except Exception:
        pass


def main():
    while True:
        try:
            push(capture())
        except Exception:
            pass
        time.sleep(INTERVAL)


if __name__ == "__main__":
    main()
