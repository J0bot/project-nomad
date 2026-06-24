#!/usr/bin/env python3
"""ploxion filesystem — service interne avec API.
Sert l'arbre FS d'un boxion + toutes les branches git, en JSON (CORS ouvert).
Endpoints: GET /health  GET /tree?path=&depth=  GET /branches  GET / (viewer HTML)
Bind 127.0.0.1 par défaut (interne) ; exposition = via wildcard/reverse-proxy (setup José)."""
import os, json, subprocess
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

ALLOW_ROOTS = ["/bi0ns", "/home/debian"]          # ce que le ploxion accepte d'explorer
SKIP = {'.git','node_modules','target','.gradle','build','dumps','.cache','.cargo','.rustup'}
REPO_DIRS = ["/home/debian/xerboxion-core","/home/debian/ploxi0ns",
             "/home/debian/labo/my_website2","/home/debian/research"] + \
            ["/home/debian/repos/"+d for d in (os.listdir("/home/debian/repos") if os.path.isdir("/home/debian/repos") else [])]

def safe(p):
    p = os.path.realpath(p)
    return any(p == r or p.startswith(r + "/") for r in ALLOW_ROOTS)

def walk(p, depth=0, maxd=2):
    node = {"n": os.path.basename(p) or p, "t": "d", "c": []}
    try: entries = sorted(os.listdir(p))
    except Exception: return node
    for e in entries:
        if e in SKIP or e.startswith('.'): continue
        fp = os.path.join(p, e)
        if os.path.isdir(fp):
            node["c"].append(walk(fp, depth+1, maxd) if depth < maxd else {"n": e+"/", "t":"d","c":[]})
        else:
            try: sz = os.path.getsize(fp)
            except Exception: sz = 0
            node["c"].append({"n": e, "t": "f", "s": sz})
    return node

def git(d, *a):
    try: return subprocess.run(["git","-C",d,"-c","safe.directory=*",*a], capture_output=True, text=True, timeout=10).stdout.strip()
    except Exception: return ""

def branches():
    out = []
    for d in REPO_DIRS:
        if not os.path.isdir(os.path.join(d, ".git")): continue
        cur = git(d, "rev-parse", "--abbrev-ref", "HEAD")
        raw = git(d, "for-each-ref", "--sort=-committerdate", "refs/heads",
                  "--format=%(refname:short)|%(objectname:short)|%(committerdate:relative)|%(contents:subject)")
        brs = []
        for line in raw.splitlines():
            p = line.split("|", 3)
            if len(p) == 4: brs.append({"b":p[0],"sha":p[1],"when":p[2],"msg":p[3][:80]})
        out.append({"repo": os.path.relpath(d, "/home/debian"), "cur": cur, "branches": brs})
    return out

def repo_abs(rel):
    cand = os.path.realpath(os.path.join("/home/debian", rel))
    return cand if cand in [os.path.realpath(d) for d in REPO_DIRS] else None

def gitlog(rel, n=20):
    d = repo_abs(rel)
    if not d: return None
    raw = git(d, "log", "-n", str(min(n,100)), "--all", "--format=%h|%an|%cr|%d|%s")
    out = []
    for line in raw.splitlines():
        p = line.split("|", 4)
        if len(p) == 5:
            out.append({"sha":p[0],"who":p[1],"when":p[2],"refs":p[3].strip(),"msg":p[4][:90]})
    return out

VIEWER = """<!doctype html><html lang=fr><meta charset=utf-8>
<meta name=viewport content="width=device-width,initial-scale=1"><title>ploxion filesystem</title>
<style>:root{--bg:#0c0f17;--pan:#141926;--fg:#cdd6f4;--dim:#7f8cb0;--acc:#3a86ff;--grn:#a6e3a1}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--fg);font:14px/1.5 ui-monospace,Menlo,monospace}
header{padding:12px 16px;border-bottom:1px solid #222a3d;display:flex;gap:10px;align-items:center}
h1{margin:0;font-size:15px}.tab{cursor:pointer;padding:3px 11px;border-radius:18px;border:1px solid #2a3550;color:var(--dim)}
.tab.on{background:#1b2440;color:#fff}#body{padding:14px 18px;max-width:1100px}
ul{list-style:none;margin:0;padding-left:15px}li{white-space:nowrap}
.d{cursor:pointer}.d::before{content:"▸ ";color:var(--acc)}.d.open::before{content:"▾ "}
.f{color:var(--dim)}.f::before{content:"· "}.sz{color:#445;font-size:.75rem}
.repo{margin-bottom:12px}.rn{color:var(--acc);font-weight:600}.br{padding:1px 0 1px 14px}
.cur{color:var(--grn)}.meta{color:var(--dim);font-size:.78rem}
.nav{color:var(--dim);text-decoration:none;margin:0 6px;font-size:.82rem}.nav:hover{color:var(--fg)}</style>
<header><h1>🗂 ploxion <b>filesystem</b></h1><a class=nav href="https://tsoin.j0bot.ch/">🕳 tsoin</a><a class=nav href="https://labo.j0bot.ch/">⚡ labo</a><span class=tab data-t=tree>arbre</span>
<span class=tab data-t=branches>branches</span><span style=flex:1></span><span class=meta>boxion_1 · live</span></header>
<div id=body>…</div>
<script>
// Données EMBARQUÉES (pas de XHR : le forward-auth SSO casserait un fetch sur 302 cross-origin).
var TREE=__TREE__, BR=__BRANCHES__;
function tn(n){var li=document.createElement('li');if(n.t==='d'){var s=document.createElement('span');s.className='d';s.textContent=n.n;
var ul=document.createElement('ul');ul.style.display='none';(n.c||[]).forEach(function(c){ul.appendChild(tn(c))});
s.onclick=function(){var o=ul.style.display==='none';ul.style.display=o?'':'none';s.classList.toggle('open',o)};li.append(s,ul);}
else{var f=document.createElement('span');f.className='f';f.textContent=n.n+' ';var z=document.createElement('span');z.className='sz';
z.textContent=n.s>1024?Math.round(n.s/1024)+'k':(n.s||0)+'b';li.append(f,z);}return li}
var B=document.getElementById('body');
function tree(){B.innerHTML='';var ul=document.createElement('ul');ul.appendChild(tn(TREE));B.appendChild(ul)}
function branches(){B.innerHTML='';BR.forEach(function(r){var x=document.createElement('div');x.className='repo';
x.innerHTML='<div class=rn>📦 '+r.repo+' <span class=cur>● '+r.cur+'</span></div>';(r.branches||[]).forEach(function(b){var y=document.createElement('div');y.className='br';
var c=b.b===r.cur;y.innerHTML='<span class="'+(c?'cur':'')+'">'+(c?'● ':'  ')+b.b+'</span> <span class=meta>'+b.sha+' · '+b.when+'</span>';x.appendChild(y)});B.appendChild(x)})}
document.querySelectorAll('.tab').forEach(function(t){t.onclick=function(){document.querySelectorAll('.tab').forEach(function(o){o.classList.remove('on')});t.classList.add('on');(t.dataset.t==='tree'?tree:branches)()}});
document.querySelector('.tab').classList.add('on');tree();
</script></html>"""

class H(BaseHTTPRequestHandler):
    def _html(self, body):
        b = body.encode()
        self.send_response(200); self.send_header("Content-Type","text/html; charset=utf-8")
        self.send_header("Content-Length",str(len(b))); self.end_headers(); self.wfile.write(b)
    def _j(self, code, obj):
        b = json.dumps(obj, ensure_ascii=False).encode()
        self.send_response(code)
        self.send_header("Content-Type","application/json; charset=utf-8")
        self.send_header("Access-Control-Allow-Origin","*")
        self.send_header("Content-Length",str(len(b)))
        self.end_headers(); self.wfile.write(b)
    def log_message(self, *a): pass
    def do_GET(self):
        u = urlparse(self.path); q = parse_qs(u.query)
        if u.path == "/health": return self._j(200, {"status":"ok","ploxion":"filesystem","roots":ALLOW_ROOTS})
        if u.path == "/branches": return self._j(200, {"repos": branches()})
        if u.path == "/tree":
            path = (q.get("path") or ["/bi0ns"])[0]
            depth = int((q.get("depth") or ["2"])[0])
            if not safe(path): return self._j(403, {"error":"path hors périmètre"})
            return self._j(200, walk(os.path.realpath(path), 0, min(depth,4)))
        if u.path == "/log":
            rel = (q.get("repo") or [""])[0]
            n = int((q.get("n") or ["20"])[0])
            lg = gitlog(rel, n)
            if lg is None: return self._j(403, {"error":"repo hors périmètre"})
            return self._j(200, {"repo": rel, "log": lg})
        if u.path == "/" or u.path == "":
            tree = json.dumps(walk("/bi0ns", 0, 2), ensure_ascii=False).replace("</", "<\\/")
            br = json.dumps(branches(), ensure_ascii=False).replace("</", "<\\/")
            return self._html(VIEWER.replace("__TREE__", tree).replace("__BRANCHES__", br))
        return self._j(404, {"error":"not found"})

if __name__ == "__main__":
    host = os.environ.get("FS_HOST","127.0.0.1"); port = int(os.environ.get("FS_PORT","3010"))
    print(f"ploxion filesystem service → http://{host}:{port}  (roots={ALLOW_ROOTS})")
    ThreadingHTTPServer((host, port), H).serve_forever()
