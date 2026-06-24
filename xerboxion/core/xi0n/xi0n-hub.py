#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
xi0n-hub — l'instance xion de Jose a xi0n.j0bot.ch (xi0n avec un ZERO).

Un HUB mobile-first, PRIVE par lien-capability (PAS de login : le lien EST la
cle = le monde GPG de Jose). Il sert :
  - un LANCEUR (grille de tous les bions/ploxions),
  - chaque ploxion en statique sous /p/<slug>/,
  - /talk : le canal Josion <-> cloudion (chat, fichier talk.jsonl persistant).
Aucun docker, aucune DB, aucune escalade : juste des fichiers que cloudion
possede, derriere le gate token. Bind PRIVE ; Traefik (xi0n.yaml) devant pour TLS.

Env : XI0N_ADDR (127.0.0.1) · XI0N_PORT (8733) · XI0N_TOKEN · XI0N_PLOXIONS (/opt/xi0n/ploxions)
      · XI0N_TALK (/var/lib/xi0n/talk.jsonl)
"""
import os
import json
import time
import hmac
import http.cookies
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs, unquote

ADDR = os.environ.get("XI0N_ADDR", "127.0.0.1")
PORT = int(os.environ.get("XI0N_PORT", "8733"))
TOKEN = os.environ.get("XI0N_TOKEN", "")
ROOT = os.path.realpath(os.environ.get("XI0N_PLOXIONS", "/opt/xi0n/ploxions"))
TALK_FILE = os.environ.get("XI0N_TALK", "/var/lib/xi0n/talk.jsonl")
_talk_lock = threading.Lock()

# (slug, emoji, nom) — slug = dossier sous ROOT, sert /p/<slug>/index.html
PLOXIONS = [
    ("ranger", "\U0001F5C2️", "Ranger"),
    ("heart", "❤️", "le cœur /warp"),
    ("lausanne", "\U0001F5FA️", "Lausanne"),
    ("cub4ion", "\U0001F39A️", "cub4ion"),
    ("skyview", "\U0001F30C", "SkyView"),
    ("minecart", "\U0001F6D2", "Minecart"),
    ("wormion", "\U0001F300", "Wormion"),
    ("nexus", "\U0001F578️", "Nexus"),
    ("turing", "\U0001F4BB", "Turing"),
    ("kardashev", "\U0001F4C8", "Kardashev"),
    ("scad", "\U0001F4D0", "xerCAD"),
]
CTYPES = {
    ".html": "text/html; charset=utf-8", ".js": "application/javascript",
    ".css": "text/css", ".wasm": "application/wasm", ".json": "application/json",
    ".png": "image/png", ".jpg": "image/jpeg", ".jpeg": "image/jpeg",
    ".gif": "image/gif", ".svg": "image/svg+xml", ".webp": "image/webp",
    ".ico": "image/x-icon", ".map": "application/json", ".txt": "text/plain; charset=utf-8",
}


# --- /talk : le canal Josion <-> cloudion (fichier jsonl, cloudion ecrit dedans aussi) ---
def talk_read(after=0):
    out = []
    try:
        with open(TALK_FILE, "r", encoding="utf-8") as f:
            for ln in f:
                ln = ln.strip()
                if not ln:
                    continue
                try:
                    m = json.loads(ln)
                except Exception:
                    continue
                if int(m.get("id", 0)) > after:
                    out.append(m)
    except FileNotFoundError:
        pass
    return out


def talk_append(who, text):
    text = (text or "").strip()[:4000]
    if not text:
        return None
    with _talk_lock:
        msgs = talk_read(0)
        nid = (msgs[-1]["id"] + 1) if msgs else 1
        m = {"id": nid, "who": who, "text": text, "t": int(time.time())}
        try:
            os.makedirs(os.path.dirname(TALK_FILE), exist_ok=True)
            with open(TALK_FILE, "a", encoding="utf-8") as f:
                f.write(json.dumps(m, ensure_ascii=False) + "\n")
        except OSError:
            return None
    return m


def launcher():
    cards = ('<a class=c href="/talk"><span class=e>\U0001F4AC</span><span class=n>parler à cloudion</span></a>'
             + "".join(
                 '<a class=c href="/p/%s/"><span class=e>%s</span><span class=n>%s</span></a>' % (s, e, n)
                 for s, e, n in PLOXIONS))
    return (
        "<!doctype html><html lang=fr><head><meta charset=utf-8>"
        "<meta name=viewport content=\"width=device-width,initial-scale=1,viewport-fit=cover,maximum-scale=1\">"
        "<meta name=apple-mobile-web-app-capable content=yes>"
        "<meta name=apple-mobile-web-app-status-bar-style content=black-translucent>"
        "<meta name=theme-color content=#0a0f1c><title>xi0n</title><style>"
        "*{box-sizing:border-box;-webkit-tap-highlight-color:transparent}"
        "body{margin:0;background:radial-gradient(120% 80% at 50% -10%,#13203a,#0a0f1c 60%);"
        "color:#dfe9ff;font:15px -apple-system,BlinkMacSystemFont,system-ui,sans-serif;min-height:100vh}"
        "header{padding:max(18px,env(safe-area-inset-top)) 16px 6px;text-align:center}"
        "header b{font-size:22px;letter-spacing:4px}header small{display:block;color:#5b6b8c;font-size:12px;margin-top:2px}"
        ".grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(98px,1fr));gap:12px;"
        "padding:16px 16px calc(16px + env(safe-area-inset-bottom))}"
        ".c{display:flex;flex-direction:column;align-items:center;justify-content:center;gap:8px;aspect-ratio:1;"
        "background:rgba(16,24,48,.85);border:1px solid #21304e;border-radius:18px;text-decoration:none;color:#dfe9ff;"
        "transition:transform .1s}.c:active{transform:scale(.95)}"
        ".e{font-size:32px;line-height:1}.n{font-size:12px;text-align:center;color:#9fb6d8}"
        "footer{text-align:center;color:#46566f;padding:16px;font-size:11px}"
        "</style></head><body><header><b>xi0n</b><small>la dimension du xion · ton instance, en lien privé</small></header>"
        "<div class=grid>" + cards + "</div>"
        "<footer>tous tes bions · le coord arrive (branche root)</footer></body></html>"
    )


def talk_page():
    return (
        "<!doctype html><html lang=fr><head><meta charset=utf-8>"
        "<meta name=viewport content=\"width=device-width,initial-scale=1,viewport-fit=cover,maximum-scale=1\">"
        "<meta name=apple-mobile-web-app-capable content=yes>"
        "<meta name=apple-mobile-web-app-status-bar-style content=black-translucent>"
        "<meta name=theme-color content=#0a0f1c><title>xi0n · talk</title><style>"
        "*{box-sizing:border-box;-webkit-tap-highlight-color:transparent}"
        "html,body{margin:0;height:100%;background:#0a0f1c;color:#dfe9ff;font:15px/1.45 -apple-system,system-ui,sans-serif}"
        "#app{position:fixed;inset:0;display:flex;flex-direction:column}"
        "header{padding:max(12px,env(safe-area-inset-top)) 14px 10px;border-bottom:1px solid #21304e;display:flex;gap:10px;align-items:center}"
        "header b{letter-spacing:2px}header small{color:#5b6b8c;font-size:11px;display:block}"
        "a.home{margin-left:auto;color:#7fd0ff;text-decoration:none;font-size:13px}"
        "#feed{flex:1;overflow-y:auto;padding:14px;display:flex;flex-direction:column;gap:10px}"
        ".m{max-width:84%;padding:9px 13px;white-space:pre-wrap;word-wrap:break-word;animation:r .2s}"
        "@keyframes r{from{opacity:0;transform:translateY(6px)}}"
        ".me{align-self:flex-end;background:linear-gradient(180deg,#1d3a52,#16314a);border:1px solid #21304e;border-radius:15px 15px 4px 15px}"
        ".you{align-self:flex-start;background:#101830;border:1px solid #21304e;border-radius:15px 15px 15px 4px}"
        ".who{font-size:10px;color:#5b6b8c;margin-bottom:2px}"
        "#bar{display:flex;gap:8px;padding:8px 12px calc(8px + env(safe-area-inset-bottom));border-top:1px solid #21304e;background:rgba(10,15,28,.92)}"
        "#in{flex:1;background:#101830;color:#dfe9ff;border:1px solid #21304e;border-radius:21px;padding:11px 15px;font:inherit;resize:none;max-height:120px;min-height:42px}"
        "#send{width:44px;height:44px;border:none;border-radius:50%;background:linear-gradient(180deg,#7fd0ff,#2f8fd8);color:#06223a;font-size:18px}"
        "</style></head><body><div id=app>"
        "<header><b>xi0n · talk</b><small id=st>Josion ⟷ cloudion</small><a class=home href=\"/\">← hub</a></header>"
        "<div id=feed></div>"
        "<div id=bar><textarea id=in placeholder=\"parle à cloudion sur ton xi0n…\" rows=1></textarea><button id=send>➤</button></div>"
        "</div><script>"
        "const $=s=>document.querySelector(s);let last=0;"
        "function esc(s){return (s||'').replace(/[&<>]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;'}[c]))}"
        "function add(m){const e=document.createElement('div');e.className='m '+(m.who==='josion'?'me':'you');"
        "e.innerHTML='<div class=who>'+(m.who==='josion'?'toi':'cloudion')+'</div>'+esc(m.text);"
        "$('#feed').appendChild(e);$('#feed').scrollTop=$('#feed').scrollHeight;}"
        "async function poll(){try{const r=await fetch('/talk/msgs?after='+last);const d=await r.json();"
        "for(const m of d.msgs){add(m);last=Math.max(last,m.id);}}catch(e){}}"
        "async function send(){const t=$('#in').value.trim();if(!t)return;$('#in').value='';$('#in').style.height='42px';"
        "try{await fetch('/talk/say',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({text:t})});}catch(e){}poll();}"
        "$('#send').onclick=send;"
        "$('#in').addEventListener('input',()=>{$('#in').style.height='42px';$('#in').style.height=Math.min($('#in').scrollHeight,120)+'px'});"
        "$('#in').addEventListener('keydown',e=>{if(e.key==='Enter'&&!e.shiftKey){e.preventDefault();send();}});"
        "poll();setInterval(poll,2500);"
        "</script></body></html>"
    )


def authed(headers):
    raw = headers.get("Cookie", "")
    if not raw:
        return False
    try:
        c = http.cookies.SimpleCookie()
        c.load(raw)
        return "xk" in c and hmac.compare_digest(c["xk"].value, TOKEN)
    except Exception:
        return False


class H(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, code, body, ctype="text/html; charset=utf-8", extra=None):
        if isinstance(body, str):
            body = body.encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header("Cache-Control", "no-store")
        if extra:
            for k, v in extra.items():
                self.send_header(k, v)
        self.end_headers()
        try:
            self.wfile.write(body)
        except Exception:
            pass

    def _gate(self, path, q):
        """Retourne True si autorise. Gere ?k=token -> cookie -> 302, sinon 403."""
        if not TOKEN:
            return True
        k = q.get("k", [""])[0]
        if k and hmac.compare_digest(k, TOKEN):
            self.send_response(302)
            self.send_header("Set-Cookie",
                             "xk=%s; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=86400" % TOKEN)
            self.send_header("Location", path or "/")
            self.end_headers()
            return False
        if not authed(self.headers):
            self._send(403,
                       "<!doctype html><meta charset=utf-8>"
                       "<meta name=viewport content='width=device-width,initial-scale=1'>"
                       "<body style='margin:0;background:#0a0f1c;color:#5b6b8c;font:16px system-ui;"
                       "height:100vh;display:flex;align-items:center;justify-content:center'>"
                       "xi0n — accès par lien privé")
            return False
        return True

    def do_GET(self):
        u = urlparse(self.path)
        path = u.path
        q = parse_qs(u.query)
        if not self._gate(path, q):
            return

        if path in ("/", "/index.html"):
            self._send(200, launcher())
            return

        if path in ("/talk", "/talk/"):
            self._send(200, talk_page())
            return

        if path == "/talk/msgs":
            try:
                after = int(q.get("after", ["0"])[0])
            except ValueError:
                after = 0
            self._send(200, json.dumps({"msgs": talk_read(after)}, ensure_ascii=False), "application/json")
            return

        if path.startswith("/p/"):
            rel = unquote(path[3:])
            if rel == "" or rel.endswith("/"):
                rel = rel + "index.html"
            fp = os.path.realpath(os.path.join(ROOT, rel))
            if not (fp == ROOT or fp.startswith(ROOT + os.sep)) or not os.path.isfile(fp):
                self._send(404, "not found", "text/plain; charset=utf-8")
                return
            ext = os.path.splitext(fp)[1].lower()
            try:
                with open(fp, "rb") as f:
                    data = f.read()
            except OSError:
                self._send(404, "not found", "text/plain; charset=utf-8")
                return
            self._send(200, data, CTYPES.get(ext, "application/octet-stream"))
            return

        if path == "/healthz":
            self._send(200, "ok", "text/plain; charset=utf-8")
            return

        self._send(404, "not found", "text/plain; charset=utf-8")

    def do_POST(self):
        u = urlparse(self.path)
        path = u.path
        if TOKEN and not authed(self.headers):
            self._send(403, "forbidden", "text/plain")
            return
        if path != "/talk/say":
            self._send(404, "not found", "text/plain")
            return
        try:
            clen = int(self.headers.get("content-length", "0"))
        except ValueError:
            clen = 0
        body = self.rfile.read(clen) if 0 < clen <= 16384 else b""
        try:
            obj = json.loads(body or b"{}")
        except Exception:
            obj = {}
        m = talk_append("josion", obj.get("text", ""))
        self._send(200, json.dumps({"ok": bool(m), "id": (m or {}).get("id")}), "application/json")


def main():
    srv = ThreadingHTTPServer((ADDR, PORT), H)
    os.write(2, ("xi0n-hub on %s:%d (ploxions=%s, talk=%s)\n" % (ADDR, PORT, ROOT, TALK_FILE)).encode())
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
