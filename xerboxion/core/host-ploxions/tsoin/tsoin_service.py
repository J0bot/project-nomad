#!/usr/bin/env python3
"""ploxion tsoin v0 — la machine à tsoins (flux réel).

Chaque commit git de tous les repos du boxion = un TSOIN (un instant de l'évolution
du projet : qui, quand, quoi, quel repo, quelle branche). On agrège TOUS les repos en
un flux chronologique unifié = la première vraie machine à tsoins, avec de la donnée
réelle qui existe déjà (« chaque commit est un tsoin », CLAUDE.md).

API : GET /health · GET /feed?limit=&repo= · GET /tsoin?repo=&sha= (détail+stat) · GET / (viewer)
Bind 0.0.0.0 dans le conteneur ; exposition = tsoin.j0bot.ch via Traefik TLS + forward-auth SSO.
"""
import os, json, subprocess
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

BASE = "/home/debian"
CAND = [BASE+"/xerboxion-core", BASE+"/ploxi0ns", BASE+"/labo/my_website2", BASE+"/research"] + \
       [BASE+"/repos/"+d for d in (os.listdir(BASE+"/repos") if os.path.isdir(BASE+"/repos") else [])]
REPOS = [d for d in CAND if os.path.isdir(os.path.join(d, ".git"))]

def git(d, *a):
    try: return subprocess.run(["git","-C",d,"-c","safe.directory=*",*a],
                               capture_output=True, text=True, timeout=15).stdout
    except Exception: return ""

def feed(limit=80, repo=None):
    sel = [r for r in REPOS if not repo or os.path.relpath(r, BASE) == repo or os.path.basename(r) == repo]
    out = []
    for d in sel:
        name = os.path.relpath(d, BASE)
        raw = git(d, "log", "--all", "--no-merges", "-n", str(min(limit, 300)),
                  "--format=%H%x1f%h%x1f%an%x1f%cI%x1f%cr%x1f%D%x1f%s")
        for line in raw.splitlines():
            p = line.split("\x1f")
            if len(p) == 7:
                out.append({"repo": name, "full": p[0], "sha": p[1], "who": p[2],
                            "iso": p[3], "when": p[4], "refs": p[5].strip(), "subject": p[6][:140]})
    out.sort(key=lambda t: t["iso"], reverse=True)
    return out[:limit]

def detail(repo, sha):
    d = next((r for r in REPOS if os.path.relpath(r, BASE) == repo or os.path.basename(r) == repo), None)
    if not d or not sha.isalnum(): return None
    return {"repo": repo, "sha": sha,
            "stat": git(d, "show", "--stat", "-p", "--format=%an%n%cI%n%s%n", sha)[:14000]}

VIEWER = """<!doctype html><html lang=fr><meta charset=utf-8>
<meta name=viewport content="width=device-width,initial-scale=1"><title>ploxion tsoin — machine à tsoins</title>
<style>:root{--bg:#0c0f17;--pan:#141926;--fg:#cdd6f4;--dim:#7f8cb0;--acc:#9b5de5;--grn:#a6e3a1;--gold:#ffb86b}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.6 ui-monospace,Menlo,monospace}
header{padding:12px 16px;border-bottom:1px solid #222a3d;display:flex;gap:10px;align-items:center;flex-wrap:wrap}
h1{margin:0;font-size:15px}h1 small{color:var(--dim);font-weight:400}
select{background:#0b1020;color:var(--fg);border:1px solid #2a3550;border-radius:8px;padding:4px 8px}
.nav{color:var(--dim);text-decoration:none;margin-left:10px;font-size:.82rem}.nav:hover{color:var(--fg)}
#body{padding:10px 16px;max-width:980px}
.day{color:var(--gold);margin:16px 0 6px;font-size:12px;letter-spacing:.06em;text-transform:uppercase}
.ts{display:flex;gap:10px;padding:8px 10px;border-left:2px solid #2a3550;margin-left:6px}
.ts:hover{background:#11162400;border-left-color:var(--acc)}
.repo{color:var(--acc);font-size:.72rem;border:1px solid #2a3550;border-radius:10px;padding:1px 7px;white-space:nowrap;align-self:flex-start}
.sub{flex:1}.meta{color:var(--dim);font-size:.76rem}.sha{color:var(--grn);text-decoration:none}.sha:hover{text-decoration:underline}
.refs{color:var(--gold);font-size:.72rem}</style>
<header><h1>🕳 ploxion <b>tsoin</b> <small>— machine à tsoins · chaque commit = un instant</small></h1><a class=nav href="https://fs.j0bot.ch/">🗂 fs</a><a class=nav href="https://labo.j0bot.ch/">⚡ labo</a>
<span style=flex:1></span><label class=meta>repo <select id=repo><option value="">tous</option></select></label></header>
<div id=body>…</div>
<script>
// Données EMBARQUÉES dans la page (pas de XHR : le forward-auth SSO casserait un fetch
// cross-origin sur 302). La page passe le SSO en navigation, donc DATA est déjà authentifié.
var DATA = __TSOIN_DATA__;
var B=document.getElementById('body'),S=document.getElementById('repo');
function day(iso){return iso.slice(0,10)}
function render(list){B.innerHTML='';var cur='';
 if(!list.length){B.textContent='aucun tsoin';return;}
 list.forEach(function(t){
  var d=day(t.iso);if(d!==cur){cur=d;var h=document.createElement('div');h.className='day';h.textContent=d;B.appendChild(h);}
  var row=document.createElement('div');row.className='ts';
  row.innerHTML='<span class=repo>'+t.repo+'</span><span class=sub>'+t.subject.replace(/</g,'&lt;')
   +'<br><span class=meta><a class=sha href="/show?repo='+encodeURIComponent(t.repo)+'&sha='+t.sha+'">'+t.sha+'</a> · '+t.who+' · '+t.when
   +(t.refs?' · <span class=refs>'+t.refs.replace(/</g,'&lt;')+'</span>':'')+'</span></span>';
  B.appendChild(row);});
}
(function(){var repos={};DATA.forEach(function(t){repos[t.repo]=1});
 Object.keys(repos).sort().forEach(function(r){var o=document.createElement('option');o.value=r;o.textContent=r;S.appendChild(o);});})();
function apply(){render(S.value?DATA.filter(function(t){return t.repo===S.value;}):DATA);}
S.onchange=apply;apply();
</script></html>"""

SHOW = """<!doctype html><html lang=fr><meta charset=utf-8>
<meta name=viewport content="width=device-width,initial-scale=1"><title>tsoin · __SHA__</title>
<style>body{margin:0;background:#0c0f17;color:#cdd6f4;font:14px/1.6 ui-monospace,Menlo,monospace}
header{padding:12px 16px;border-bottom:1px solid #222a3d}a{color:#9b5de5;text-decoration:none}a:hover{text-decoration:underline}
pre{padding:14px 18px;white-space:pre-wrap;color:#a6e3a1;font-size:.84rem}.repo{color:#9b5de5}</style>
<header><a href="/">← flux des tsoins</a> &nbsp; <b>__REPO__</b> · <span style=color:#a6e3a1>__SHA__</span></header>
<pre>__STAT__</pre></html>"""

class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def _send(self, code, ctype, body):
        b = body.encode() if isinstance(body, str) else body
        self.send_response(code); self.send_header("Content-Type", ctype)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Content-Length", str(len(b))); self.end_headers(); self.wfile.write(b)
    def _j(self, code, obj): self._send(code, "application/json; charset=utf-8", json.dumps(obj, ensure_ascii=False))
    def do_GET(self):
        u = urlparse(self.path); q = parse_qs(u.query)
        if u.path == "/health": return self._j(200, {"status": "ok", "ploxion": "tsoin", "repos": len(REPOS)})
        if u.path == "/feed":
            return self._j(200, {"tsoins": feed(int((q.get("limit") or ["80"])[0]), (q.get("repo") or [None])[0])})
        if u.path == "/tsoin":
            d = detail((q.get("repo") or [""])[0], (q.get("sha") or [""])[0])
            return self._j(200 if d else 404, d or {"error": "introuvable"})
        if u.path == "/show":
            import html as _h
            d = detail((q.get("repo") or [""])[0], (q.get("sha") or [""])[0])
            if not d:
                return self._send(404, "text/html; charset=utf-8", "<p>tsoin introuvable — <a href='/'>retour</a></p>")
            page = (SHOW.replace("__REPO__", _h.escape(d["repo"]))
                        .replace("__SHA__", _h.escape(d["sha"]))
                        .replace("__STAT__", _h.escape(d["stat"] or "(vide)")))
            return self._send(200, "text/html; charset=utf-8", page)
        if u.path in ("/", ""):
            data = json.dumps(feed(200), ensure_ascii=False).replace("</", "<\\/")
            return self._send(200, "text/html; charset=utf-8", VIEWER.replace("__TSOIN_DATA__", data))
        return self._j(404, {"error": "not found"})

if __name__ == "__main__":
    host = os.environ.get("TSOIN_HOST", "127.0.0.1"); port = int(os.environ.get("TSOIN_PORT", "3011"))
    print(f"ploxion tsoin → http://{host}:{port}  ({len(REPOS)} repos)")
    ThreadingHTTPServer((host, port), H).serve_forever()
