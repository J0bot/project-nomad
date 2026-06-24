#!/usr/bin/env python3
"""ploxion record — l'enregistreur de tsoins.

v0 : capture **écran + micro** (MediaRecorder, client) + un **journal d'events horodaté**
(clic / touche / navigation) → un **tsoin** = média (.webm) + piste d'events (.json) + méta.
Stocké côté serveur (volume privé /data), listé, **rejouable** (vidéo + overlay events).
Privé : derrière SSO (forward-auth Traefik). Le montage = le [[cubion-edit]] (plus tard).

API : GET / (recorder, liste embarquée) · POST /upload (multipart: video, events, meta)
      · GET /list (json) · GET /media/<id> · GET /play/<id> (rejoueur) · GET /health
"""
import os, json, re, time, cgi
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

DATA = os.environ.get("REC_DATA", "/data")
os.makedirs(DATA, exist_ok=True)

def safe_id(s):
    return re.sub(r"[^0-9a-zA-Z_-]", "", s or "")[:40]

def recordings():
    out = []
    for f in sorted(os.listdir(DATA), reverse=True):
        if f.endswith(".meta.json"):
            try:
                m = json.load(open(os.path.join(DATA, f)))
            except Exception:
                m = {}
            rid = f[:-len(".meta.json")]
            vid = os.path.join(DATA, rid + ".webm")
            m["id"] = rid
            m["size"] = os.path.getsize(vid) if os.path.exists(vid) else 0
            out.append(m)
    return out

PAGE = """<!doctype html><html lang=fr><head><meta charset=utf-8>
<meta name=viewport content="width=device-width,initial-scale=1"><meta name=robots content="noindex,nofollow">
<title>record — ploxion</title>
<style>
 *{box-sizing:border-box}body{margin:0;background:#0c0f17;color:#cdd6f4;font:14px system-ui,sans-serif}
 header{padding:12px 16px;border-bottom:1px solid #222a3d;display:flex;gap:10px;align-items:center;flex-wrap:wrap}
 h1{margin:0;font-size:15px}a{color:#9b5de5;text-decoration:none}a:hover{text-decoration:underline}
 #body{padding:16px;max-width:900px}
 button{background:#9b5de5;color:#fff;border:none;border-radius:8px;padding:9px 16px;cursor:pointer;font:600 14px system-ui}
 button.stop{background:#f38ba8}button:disabled{opacity:.4;cursor:default}
 .sh{background:#1b2440;color:#cdd6f4;border:1px solid #2a3550;padding:5px 11px;font-size:.85rem;border-radius:8px}
 .sh.on{background:#9b5de5;color:#fff;border-color:#9b5de5}
 .row{display:flex;gap:8px;align-items:center;flex-wrap:wrap;margin:8px 0}
 label.opt{color:#7f8cb0;font-size:.85rem}
 #st{color:#a6e3a1;font-size:.85rem}#timer{font:700 16px ui-monospace;color:#ffb86b}
 video{width:100%;max-height:42vh;background:#000;border:1px solid #222a3d;border-radius:10px;margin-top:8px}
 .rec{margin:8px 0;padding:8px 10px;border-left:2px solid #2a3550;display:flex;gap:10px;align-items:center}
 .rec .meta{color:#7f8cb0;font-size:.78rem}.dim{color:#7f8cb0;font-size:.8rem}
</style></head><body>
<header><h1>⏺ ploxion <b>record</b> <span class=dim>— enregistre un tsoin (écran + micro + events)</span></h1>
<span style=flex:1></span><a href="https://tsoin.j0bot.ch/">🕳 tsoin</a><a href="https://labo.j0bot.ch/">⚡ labo</a></header>
<div id=body>
  <div class=row>
    <button id=start>⏺ Enregistrer un tsoin</button>
    <button id=stop class=stop disabled>⏹ Stop</button>
    <span id=timer>00:00</span><span id=st>prêt</span>
  </div>
  <div class=row>
    <label class=opt><input type=checkbox id=cam> caméra (incrustée)</label>
    <label class=opt><input type=checkbox id=mic checked> micro</label>
    <span class=dim>l'écran + le son système sont demandés au partage.</span>
  </div>
  <div class=row>
    <span class=opt>partage (partagion) :</span>
    <button class=sh data-sh="O">O · rien</button>
    <button class=sh data-sh="I">I · moi</button>
    <button class=sh data-sh="N">N · tout</button>
    <span class=dim id=shnote>I = privé (toi seul)</span>
  </div>
  <video id=prev muted autoplay playsinline></video>
  <h3 class=dim>Tes tsoins enregistrés</h3>
  <div id=list>__LIST__</div>
</div>
<script>
(function(){
 var start=document.getElementById('start'),stop=document.getElementById('stop'),st=document.getElementById('st'),
     prev=document.getElementById('prev'),timer=document.getElementById('timer');
 var rec,chunks=[],events=[],t0=0,tick=null,stream=null,share='I',screenStream=null,camStream=null,rafId=null;
 var shNote=document.getElementById('shnote'),shTxt={O:'O = rien (brouillon privé)',I:'I = privé (toi seul)',N:'N = tout (partagé)'};
 function setShare(v){share=v;document.querySelectorAll('.sh').forEach(function(b){b.classList.toggle('on',b.dataset.sh===v);});shNote.textContent=shTxt[v];}
 document.querySelectorAll('.sh').forEach(function(b){b.onclick=function(){setShare(b.dataset.sh);};}); setShare('I');
 function log(type,d){ events.push(Object.assign({t:Math.round(performance.now()-t0),type:type},d||{})); }
 function fmt(ms){var s=Math.floor(ms/1000);return ('0'+Math.floor(s/60)).slice(-2)+':'+('0'+(s%60)).slice(-2);}
 var lastMove=0;
 function onMove(e){var n=performance.now();if(n-lastMove<200)return;lastMove=n;log('move',{x:e.clientX,y:e.clientY});}
 function bind(on){var m=on?'addEventListener':'removeEventListener';
   document[m]('click',onClick);document[m]('keydown',onKey);document[m]('mousemove',onMove);}
 function onClick(e){log('click',{x:e.clientX,y:e.clientY});}
 function onKey(e){log('key',{k:e.key});}
 start.onclick=async function(){
   try{
     stream=await navigator.mediaDevices.getDisplayMedia({video:true,audio:true});
     if(document.getElementById('mic').checked){
       try{var mic=await navigator.mediaDevices.getUserMedia({audio:true});mic.getAudioTracks().forEach(function(t){stream.addTrack(t);});}catch(e){}
     }
   }catch(e){ st.textContent='partage annulé'; return; }
   prev.srcObject=stream; chunks=[]; events=[]; t0=performance.now();
   rec=new MediaRecorder(stream,{mimeType:'video/webm'});
   rec.ondataavailable=function(e){ if(e.data.size) chunks.push(e.data); };
   rec.onstop=upload;
   rec.start(1000); bind(true);
   start.disabled=true; stop.disabled=false; st.textContent='● enregistrement…'; st.style.color='#f38ba8';
   tick=setInterval(function(){ timer.textContent=fmt(performance.now()-t0); },500);
   stream.getVideoTracks()[0].addEventListener('ended',function(){ if(!stop.disabled) stop.click(); });
 };
 stop.onclick=function(){ if(rec&&rec.state!=='inactive') rec.stop(); bind(false);
   if(stream) stream.getTracks().forEach(function(t){t.stop();});
   clearInterval(tick); stop.disabled=true; st.textContent='envoi…'; st.style.color='#ffb86b'; };
 function upload(){
   var blob=new Blob(chunks,{type:'video/webm'}), id='tsoin-'+Date.now();
   var meta={id:id,ms:Math.round(performance.now()-t0),events:events.length,at:new Date().toISOString(),share:share};
   var fd=new FormData();
   fd.append('id',id); fd.append('meta',JSON.stringify(meta));
   fd.append('events',new Blob([JSON.stringify(events)],{type:'application/json'}),'events.json');
   fd.append('video',blob,'video.webm');
   fetch('/upload',{method:'POST',body:fd,credentials:'same-origin'})
     .then(function(r){ if(!r.ok) throw 0; st.textContent='✓ tsoin enregistré ('+fmt(meta.ms)+', '+events.length+' events)'; st.style.color='#a6e3a1'; setTimeout(function(){location.reload();},900); })
     .catch(function(){ st.textContent='⚠ échec upload (recommence / vérifie le login)'; st.style.color='#f38ba8'; });
   start.disabled=false;
 }
})();
</script></body></html>"""

PLAY = """<!doctype html><html lang=fr><head><meta charset=utf-8><meta name=robots content="noindex">
<title>tsoin __ID__</title><style>body{margin:0;background:#0c0f17;color:#cdd6f4;font:14px system-ui}
header{padding:12px 16px;border-bottom:1px solid #222a3d}a{color:#9b5de5}video{width:100%;max-height:62vh;background:#000;display:block}
#strip{position:relative;height:22px;background:#11162a;border-bottom:1px solid #222a3d;cursor:pointer}
#strip .mk{position:absolute;top:3px;width:2px;height:16px;background:#9b5de5;cursor:pointer}
#strip .mk:hover{background:#ffb86b;width:3px}
#ev{padding:8px 16px;color:#7f8cb0;font:12px ui-monospace;max-height:22vh;overflow:auto}
.evrow{cursor:pointer;padding:1px 4px;border-radius:4px}.evrow:hover{background:#1b2440;color:#cdd6f4}
.hint{color:#a6e3a1;font-size:.8rem;padding:4px 16px}</style></head><body>
<header><a href="/">← record</a> &nbsp; <b>__ID__</b> · <span id=info></span></header>
<video id=v src="/media/__ID__" controls autoplay></video>
<div id=strip title="clique pour revenir à ce moment (revenir au réel)"></div>
<div class=hint>↑ clique la barre ou un event pour <b>revenir à ce moment</b> du tsoin.</div>
<div id=ev>chargement events…</div>
<script>
var v=document.getElementById('v'),strip=document.getElementById('strip'),evbox=document.getElementById('ev');
function seek(t){ v.currentTime=t/1000; v.play(); }
fetch('/events/__ID__',{credentials:'same-origin'}).then(function(r){return r.json();}).then(function(evs){
 document.getElementById('info').textContent=evs.length+' events';
 evbox.textContent=''; strip.innerHTML='';
 var maxT=evs.length?evs[evs.length-1].t:1;
 var shown=evs.slice(0,500);
 shown.forEach(function(e){
   var d=document.createElement('div'); d.className='evrow';
   d.textContent=(e.t/1000).toFixed(1)+'s · '+e.type+' '+(e.k||(e.x!=null?e.x+','+e.y:''));
   d.onclick=function(){ seek(e.t); }; evbox.appendChild(d);
   var m=document.createElement('i'); m.className='mk'; m.style.left=(e.t/maxT*100)+'%';
   m.title=(e.t/1000).toFixed(1)+'s '+e.type; m.onclick=function(ev){ ev.stopPropagation(); seek(e.t); };
   strip.appendChild(m);
 });
 v.addEventListener('loadedmetadata',function(){
   var dur=v.duration*1000; if(!isFinite(dur)||dur<=0) return;
   var mks=strip.querySelectorAll('.mk');
   shown.forEach(function(e,i){ if(mks[i]) mks[i].style.left=(Math.min(e.t,dur)/dur*100)+'%'; });
 });
}).catch(function(){ evbox.textContent='(events indisponibles)'; });
strip.addEventListener('click',function(ev){ if(ev.target!==strip) return; var r=strip.getBoundingClientRect();
  if(isFinite(v.duration)&&v.duration>0){ v.currentTime=(ev.clientX-r.left)/r.width*v.duration; v.play(); } });
</script></body></html>"""

class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def _send(self, code, ctype, body, extra=None):
        b = body.encode() if isinstance(body, str) else body
        self.send_response(code); self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(b)))
        for k, v in (extra or {}).items(): self.send_header(k, v)
        self.end_headers(); self.wfile.write(b)
    def _file(self, path, ctype):
        if not os.path.exists(path): return self._send(404, "text/plain", "not found")
        b = open(path, "rb").read(); self._send(200, ctype, b)
    def do_GET(self):
        p = urlparse(self.path).path
        if p == "/health": return self._send(200, "application/json", json.dumps({"status":"ok","ploxion":"record","count":len(recordings())}))
        if p in ("/list", "/feed"): return self._send(200, "application/json", json.dumps({"recordings":recordings(), "tsoins":recordings()}))
        if p in ("/", ""):
            recs = recordings()
            if recs:
                rows = "".join('<div class=rec><a href="/play/{id}">▶ {id}</a><span class=meta>[{sh}] {ms}s · {ev} events · {kb} Ko · {at}</span></div>'
                    .format(id=r["id"], sh=r.get("share","?"), ms=round(r.get("ms",0)/1000), ev=r.get("events",0), kb=round(r.get("size",0)/1024), at=r.get("at","")[:19]) for r in recs)
            else:
                rows = '<p class=dim>aucun tsoin enregistré pour l\'instant.</p>'
            return self._send(200, "text/html; charset=utf-8", PAGE.replace("__LIST__", rows))
        if p.startswith("/play/"):
            rid = safe_id(p[len("/play/"):]); return self._send(200, "text/html; charset=utf-8", PLAY.replace("__ID__", rid))
        if p.startswith("/media/"):
            rid = safe_id(p[len("/media/"):]); return self._file(os.path.join(DATA, rid + ".webm"), "video/webm")
        if p.startswith("/events/"):
            rid = safe_id(p[len("/events/"):]); return self._file(os.path.join(DATA, rid + ".events.json"), "application/json")
        return self._send(404, "text/plain", "not found")
    def do_POST(self):
        if urlparse(self.path).path != "/upload": return self._send(404, "text/plain", "not found")
        ctype = self.headers.get("Content-Type", "")
        if "multipart/form-data" not in ctype: return self._send(400, "text/plain", "multipart attendu")
        form = cgi.FieldStorage(fp=self.rfile, headers=self.headers,
                                environ={"REQUEST_METHOD": "POST", "CONTENT_TYPE": ctype})
        rid = safe_id(form.getfirst("id") or ("tsoin-" + str(int(time.time()))))
        meta = form.getfirst("meta") or "{}"
        open(os.path.join(DATA, rid + ".meta.json"), "w").write(meta)
        if "events" in form and form["events"].file:
            open(os.path.join(DATA, rid + ".events.json"), "wb").write(form["events"].file.read())
        if "video" in form and form["video"].file:
            open(os.path.join(DATA, rid + ".webm"), "wb").write(form["video"].file.read())
        return self._send(200, "application/json", json.dumps({"ok": True, "id": rid}))

if __name__ == "__main__":
    host = os.environ.get("REC_HOST", "127.0.0.1"); port = int(os.environ.get("REC_PORT", "3012"))
    print(f"ploxion record → http://{host}:{port}  (data={DATA})")
    ThreadingHTTPServer((host, port), H).serve_forever()
