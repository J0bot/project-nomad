#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
xi0n-coord — pont MOBILE-FIRST du /coord (xi0n.j0bot.ch, xi0n avec un ZERO).

Refait la page /coord du labo, en mieux, pour le telephone de Jose :
  - voir le fil de la flotte EN LIVE (poll leger),
  - POSTER (avec topic),
  - voir les images / tsoins attaches.

L'HISTORIQUE et les posts de la flotte restent : on BRIDGE le coord existant
(table `claude_messages` de mw2-labo-app, Laravel/PHP). On NE MODIFIE RIEN du
labo : LECTURE = SELECT read-only en DB (via docker exec mariadb), ECRITURE =
`php artisan coord:say` (validation + ping/Discord existants), IMAGES = docker cp
de storage/app/coord vers un cache tmpfs, servi SEULEMENT derriere le SSO.

SECURITE — ce backend n'a PAS d'auth a lui : il est protege par le forward-auth
SSO de Traefik (xi0n.yaml -> labo.j0bot.ch/sso/verify, admin .j0bot.ch only).
Il DOIT donc :
  - rester bind PRIVE (127.0.0.1 par defaut, 10.0.0.1 pour l'expo Traefik),
  - valider agent/topic ^[a-z0-9-]+$ AVANT tout docker exec,
  - passer le contenu en argv (subprocess list, JAMAIS shell) -> pas d'injection,
  - borner la taille des messages + rate-limiter par identite/IP,
  - resoudre image_path depuis la DB (jamais depuis le param) -> pas de traversal.
Si l'en-tete d'identite SSO est present (X-Forwarded-User / Remote-User), il est
utilise comme AUTEUR par defaut des posts.

UN SEUL FICHIER, Python 3.11 stdlib (http.server threaded). Aucune dependance.

Env :
  XI0N_ADDR   (defaut 127.0.0.1)   bind. Mettre 10.0.0.1 pour l'expo Traefik.
  XI0N_PORT   (defaut 8733)
  XI0N_LOG    (defaut quiet)        quiet | access
  XI0N_DB_CT  (defaut mw2-labo-db)  conteneur DB (mariadb client)
  XI0N_APP_CT (defaut mw2-labo-app) conteneur app Laravel (artisan + storage)
  XI0N_DB     (defaut mw2labo)      base
  XI0N_DB_USER(defaut mw2)          user read-only-en-pratique
  XI0N_DB_PASS                      mot de passe DB (sinon lu depuis l'env du
                                    conteneur app via docker inspect, une fois)
  XI0N_DOCKER (defaut "sudo docker") prefixe docker (ex "docker" si groupe docker)
  XI0N_CACHE  (defaut /run/xi0n-cache) cache tmpfs des images (TTL + cap)
  XI0N_DEFAULT_AGENT (defaut jsosion)  auteur par defaut si pas d'identite SSO
"""

import json
import os
import re
import time
import shlex
import threading
import subprocess
import collections
import hmac
import http.cookies
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, parse_qs

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
ADDR = os.environ.get("XI0N_ADDR", "127.0.0.1")
PORT = int(os.environ.get("XI0N_PORT", "8733"))
LOG_MODE = os.environ.get("XI0N_LOG", "quiet")  # quiet | access

DB_CT = os.environ.get("XI0N_DB_CT", "mw2-labo-db")
APP_CT = os.environ.get("XI0N_APP_CT", "mw2-labo-app")
DB_NAME = os.environ.get("XI0N_DB", "mw2labo")
DB_USER = os.environ.get("XI0N_DB_USER", "mw2")
DB_PASS = os.environ.get("XI0N_DB_PASS", "")  # sinon resolu via docker inspect
DOCKER = shlex.split(os.environ.get("XI0N_DOCKER", "sudo docker"))
CACHE = os.environ.get("XI0N_CACHE", "/run/xi0n-cache")
DEFAULT_AGENT = os.environ.get("XI0N_DEFAULT_AGENT", "jsosion")
# Lien-capability prive (PAS de login) : le lien EST la cle (le monde GPG de Jose).
TOKEN = os.environ.get("XI0N_TOKEN", "")


def cookie_token(headers):
    raw = headers.get("Cookie", "")
    if not raw:
        return ""
    try:
        c = http.cookies.SimpleCookie()
        c.load(raw)
        return c["xk"].value if "xk" in c else ""
    except Exception:
        return ""


# Bornes dures (RAM serree, Minecraft tourne)
MAX_BODY = 16 * 1024          # corps POST max
MAX_CONTENT = 2000            # message tronque a 2000 (le store web cape a 4000)
FEED_LIMIT_MAX = 100
FEED_LIMIT_DEF = 60
EXEC_TIMEOUT = 12.0           # timeout docker exec
FEED_CACHE_TTL = 1.5          # cache court du "after=0" pour ne pas spawner mysql par onglet
IMG_MAX_BYTES = 12 * 1024 * 1024   # 12 MiB (image-only/tsoin)
CACHE_CAP = 64 * 1024 * 1024  # 64 MiB de cache disque max
CACHE_TTL = 600.0             # 10 min puis evince

# Rate-limit ecriture (token-bucket par identite/IP)
RL_SAY_RATE = 0.2             # ~ 1 post / 5s en regime
RL_SAY_BURST = 6.0           # rafale 6 posts

AGENT_RE = re.compile(r"^[a-z0-9-]{1,80}$")
TOPIC_RE = re.compile(r"^[a-z0-9-]{1,80}$")
IMGPATH_RE = re.compile(r"^coord/[A-Za-z0-9._-]+$")

VIDEO_EXT = {"mp4", "webm", "mov", "ogv", "m4v"}
AUDIO_EXT = {"mp3", "ogg", "wav", "m4a", "oga", "weba"}
CTYPE = {
    "png": "image/png", "jpg": "image/jpeg", "jpeg": "image/jpeg",
    "gif": "image/gif", "webp": "image/webp", "svg": "image/svg+xml",
    "mp4": "video/mp4", "webm": "video/webm", "mov": "video/quicktime",
    "ogv": "video/ogg", "m4v": "video/mp4",
    "mp3": "audio/mpeg", "ogg": "audio/ogg", "wav": "audio/wav",
    "m4a": "audio/mp4", "oga": "audio/ogg", "weba": "audio/webm",
}

# ---------------------------------------------------------------------------
# Etat
# ---------------------------------------------------------------------------
_buckets = {}            # ident -> [tokens, ts]
_buckets_lock = threading.Lock()
_feed_cache = {"ts": 0.0, "key": None, "data": None}
_feed_lock = threading.Lock()
_cache_lock = threading.Lock()
_dbpass_resolved = {"val": DB_PASS, "done": bool(DB_PASS)}


def _log(line):
    if LOG_MODE == "access":
        try:
            os.write(2, (line + "\n").encode("utf-8", "replace"))
        except Exception:
            pass


def _rss_kb():
    try:
        with open("/proc/self/statm") as f:
            pages = int(f.read().split()[1])
        return pages * (os.sysconf("SC_PAGE_SIZE") // 1024)
    except Exception:
        return -1


def _allow_say(ident):
    now = time.time()
    with _buckets_lock:
        slot = _buckets.get(ident)
        if slot is None:
            slot = [RL_SAY_BURST, now]
            _buckets[ident] = slot
        tokens, ts = slot
        tokens = min(RL_SAY_BURST, tokens + (now - ts) * RL_SAY_RATE)
        if tokens < 1.0:
            slot[0] = tokens
            slot[1] = now
            return False
        slot[0] = tokens - 1.0
        slot[1] = now
        return True


# ---------------------------------------------------------------------------
# Resolution du mot de passe DB (une seule fois, via docker inspect de l'app)
# ---------------------------------------------------------------------------
def _resolve_db_pass():
    if _dbpass_resolved["done"]:
        return _dbpass_resolved["val"]
    try:
        out = subprocess.run(
            DOCKER + ["inspect", "--format",
                      "{{range .Config.Env}}{{println .}}{{end}}", APP_CT],
            capture_output=True, timeout=EXEC_TIMEOUT)
        for ln in out.stdout.decode("utf-8", "replace").splitlines():
            if ln.startswith("DB_PASSWORD="):
                _dbpass_resolved["val"] = ln.split("=", 1)[1]
                break
    except Exception:
        pass
    _dbpass_resolved["done"] = True
    return _dbpass_resolved["val"]


# ---------------------------------------------------------------------------
# LECTURE : SELECT read-only sur claude_messages via mariadb dans le conteneur DB
# Le mot de passe passe par MYSQL_PWD (env du subprocess), JAMAIS sur l'argv.
# ---------------------------------------------------------------------------
def _db_query(sql):
    """Execute un SELECT, retourne une liste de dicts. Read-only par construction."""
    pw = _resolve_db_pass()
    # -N : pas d'entete ; --batch : TSV echappe ; on prefixe les colonnes nous-memes.
    cmd = DOCKER + [
        "exec", "-e", "MYSQL_PWD",  # valeur injectee via env ci-dessous
        DB_CT, "mariadb", "-N", "--batch", "-u", DB_USER, DB_NAME, "-e", sql,
    ]
    env = dict(os.environ)
    env["MYSQL_PWD"] = pw
    try:
        out = subprocess.run(cmd, capture_output=True, timeout=EXEC_TIMEOUT, env=env)
    except subprocess.TimeoutExpired:
        return None
    if out.returncode != 0:
        _log("DB ERR rc=%d %r" % (out.returncode, out.stderr[:200]))
        return None
    return out.stdout.decode("utf-8", "replace")


def _unescape_tsv(v):
    # mariadb --batch echappe \t \n \\ et rend \N pour NULL
    if v == "\\N":
        return None
    return (v.replace("\\t", "\t").replace("\\n", "\n")
             .replace("\\r", "\r").replace("\\\\", "\\"))


COLS = ["id", "agent", "topic", "content", "level", "image_path", "created_at"]


def _row_to_msg(parts):
    d = {}
    for i, c in enumerate(COLS):
        d[c] = _unescape_tsv(parts[i]) if i < len(parts) else None
    try:
        mid = int(d["id"])
    except (TypeError, ValueError):
        return None
    image_path = d.get("image_path")
    has_media = bool(image_path) and IMGPATH_RE.match(image_path or "")
    media_kind = None
    if has_media:
        ext = image_path.rsplit(".", 1)[-1].lower() if "." in image_path else ""
        if ext in VIDEO_EXT:
            media_kind = "video"
        elif ext in AUDIO_EXT:
            media_kind = "audio"
        else:
            media_kind = "image"
    return {
        "id": mid,
        "agent": d.get("agent") or "?",
        "topic": d.get("topic"),
        "content": d.get("content") or "",
        "is_ping": (d.get("level") == "ping"),
        "media": ("/coord/api/image/%d" % mid) if has_media else None,
        "media_kind": media_kind,
        "at": d.get("created_at"),
    }


def _esc_sql(s):
    # filet de securite : on n'injecte que des entiers/regex-valides, mais on
    # echappe quand meme tout litteral string.
    return s.replace("\\", "\\\\").replace("'", "''")


def fetch_messages(after=0, before=0, limit=FEED_LIMIT_DEF, topic=None, agent=None):
    limit = max(1, min(FEED_LIMIT_MAX, int(limit)))
    where = []
    order = "DESC"
    if after > 0:
        where.append("id > %d" % int(after))
        order = "ASC"            # delta poll : chrono
    if before > 0:
        where.append("id < %d" % int(before))
    if topic and TOPIC_RE.match(topic):
        where.append("topic = '%s'" % _esc_sql(topic))
    if agent and AGENT_RE.match(agent):
        where.append("agent = '%s'" % _esc_sql(agent))
    wsql = (" WHERE " + " AND ".join(where)) if where else ""
    sql = ("SELECT id,agent,topic,content,level,image_path,created_at "
           "FROM claude_messages%s ORDER BY id %s LIMIT %d;"
           % (wsql, order, limit))
    raw = _db_query(sql)
    if raw is None:
        return None
    msgs = []
    for line in raw.split("\n"):
        if not line:
            continue
        parts = line.split("\t")
        m = _row_to_msg(parts)
        if m:
            msgs.append(m)
    if order == "DESC":
        msgs.reverse()  # toujours rendu en ordre chrono ascendant
    return msgs


def fetch_image_path(mid):
    sql = ("SELECT image_path FROM claude_messages WHERE id=%d LIMIT 1;" % int(mid))
    raw = _db_query(sql)
    if not raw:
        return None
    val = raw.strip().split("\n")[0].strip()
    val = _unescape_tsv(val) if val else None
    if not val or not IMGPATH_RE.match(val):
        return None
    return val


# ---------------------------------------------------------------------------
# ECRITURE : php artisan coord:say (validation + ping/Discord du labo)
# ---------------------------------------------------------------------------
def coord_say(agent, content, topic=None):
    if not AGENT_RE.match(agent or ""):
        return False, "bad_agent"
    if topic is not None and topic != "" and not TOPIC_RE.match(topic):
        return False, "bad_topic"
    content = (content or "").strip()
    if not content:
        return False, "empty"
    if len(content) > MAX_CONTENT:
        content = content[:MAX_CONTENT]
    cmd = DOCKER + ["exec", APP_CT, "php", "artisan", "coord:say"]
    if topic:
        cmd.append("--topic=%s" % topic)
    # FIX-D : separateur `--` -> un agent/contenu commencant par `-` ne peut pas
    # se faire passer pour une option (defense en profondeur ; deja en argv).
    cmd += ["--", agent, content]
    # NOTE: pas de --ping expose (anti-spam de notifs Discord @here).
    try:
        out = subprocess.run(cmd, capture_output=True, timeout=EXEC_TIMEOUT)
    except subprocess.TimeoutExpired:
        return False, "timeout"
    if out.returncode != 0:
        _log("SAY ERR rc=%d %r" % (out.returncode, out.stderr[:200]))
        return False, "say_failed"
    return True, None


# ---------------------------------------------------------------------------
# IMAGES : docker cp storage/app/<image_path> -> cache tmpfs (TTL + cap)
# image_path est resolu DEPUIS LA DB (jamais le param) -> pas de traversal.
# ---------------------------------------------------------------------------
def _cache_gc():
    try:
        now = time.time()
        entries = []
        total = 0
        for name in os.listdir(CACHE):
            fp = os.path.join(CACHE, name)
            try:
                st = os.stat(fp)
            except OSError:
                continue
            total += st.st_size
            entries.append((st.st_mtime, st.st_size, fp))
        # TTL
        for mt, sz, fp in entries:
            if now - mt > CACHE_TTL:
                try:
                    os.unlink(fp); total -= sz
                except OSError:
                    pass
        # cap : evince les plus vieux jusqu'a passer sous le cap
        if total > CACHE_CAP:
            for mt, sz, fp in sorted(entries):
                if total <= CACHE_CAP:
                    break
                try:
                    os.unlink(fp); total -= sz
                except OSError:
                    pass
    except FileNotFoundError:
        pass
    except Exception as e:
        _log("CACHE GC ERR %r" % (e,))


def get_image(mid):
    """Retourne (bytes, content_type) ou (None, None)."""
    image_path = fetch_image_path(mid)   # 'coord/<file>'
    if not image_path:
        return None, None
    fname = image_path.split("/", 1)[1]
    ext = fname.rsplit(".", 1)[-1].lower() if "." in fname else ""
    ctype = CTYPE.get(ext, "application/octet-stream")
    safe = re.sub(r"[^A-Za-z0-9._-]", "_", fname)
    cache_fp = os.path.join(CACHE, "%d_%s" % (mid, safe))
    with _cache_lock:
        if not os.path.exists(cache_fp):
            try:
                os.makedirs(CACHE, exist_ok=True)
            except OSError:
                pass
            _cache_gc()
            src = "%s:/var/www/html/storage/app/%s" % (APP_CT, image_path)
            tmp = cache_fp + ".part"
            try:
                out = subprocess.run(DOCKER + ["cp", src, tmp],
                                     capture_output=True, timeout=EXEC_TIMEOUT)
            except subprocess.TimeoutExpired:
                return None, None
            if out.returncode != 0:
                _log("CP ERR rc=%d %r" % (out.returncode, out.stderr[:200]))
                try:
                    os.unlink(tmp)
                except OSError:
                    pass
                return None, None
            try:
                if os.path.getsize(tmp) > IMG_MAX_BYTES:
                    os.unlink(tmp)
                    return None, None
                os.replace(tmp, cache_fp)
            except OSError:
                return None, None
        else:
            os.utime(cache_fp, None)  # rafraichit le TTL
    try:
        with open(cache_fp, "rb") as f:
            data = f.read(IMG_MAX_BYTES + 1)
        if len(data) > IMG_MAX_BYTES:
            return None, None
        return data, ctype
    except OSError:
        return None, None


# ---------------------------------------------------------------------------
# Identite SSO : Traefik forward-auth peut renvoyer X-Forwarded-User / Remote-User
# ---------------------------------------------------------------------------
def sso_ident(headers):
    for h in ("x-forwarded-user", "remote-user", "x-auth-user"):
        v = headers.get(h)
        if v:
            v = v.strip().lower()
            # normalise en slug agent valide
            v = re.sub(r"[^a-z0-9-]", "-", v).strip("-")[:80]
            if v and AGENT_RE.match(v):
                return v
    return None


# ---------------------------------------------------------------------------
# HTTP
# ---------------------------------------------------------------------------
class H(BaseHTTPRequestHandler):
    server_version = "xi0n/1"
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def _ident(self):
        ssou = sso_ident(self.headers)
        if ssou:
            return ssou
        xff = self.headers.get("x-forwarded-for")
        if xff:
            return "ip:" + xff.split(",")[0].strip()
        return "ip:" + (self.client_address[0] if self.client_address else "?")

    def _send(self, status, body=b"", ctype="application/json", extra=None):
        if isinstance(body, str):
            body = body.encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "no-referrer")
        if extra:
            for k, v in extra.items():
                self.send_header(k, v)
        self.end_headers()
        try:
            self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def _json(self, status, obj, extra=None):
        self._send(status, json.dumps(obj, separators=(",", ":")), "application/json", extra)

    # ---- GET -------------------------------------------------------------
    def do_GET(self):
        u = urlparse(self.path)
        path = u.path
        q = parse_qs(u.query)
        _log("GET %s" % path[:64])

        # --- gate capability : lien prive, PAS de login (le lien EST la cle) ---
        if TOKEN:
            k = q.get("k", [""])[0]
            if k and hmac.compare_digest(k, TOKEN):
                self.send_response(302)
                self.send_header("Set-Cookie",
                                 "xk=%s; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=31536000" % TOKEN)
                self.send_header("Location", path or "/")
                self.end_headers()
                return
            if not hmac.compare_digest(cookie_token(self.headers), TOKEN):
                self._send(403,
                           "<!doctype html><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>"
                           "<body style='margin:0;background:#0b0e14;color:#5b6b8c;font:16px system-ui;height:100vh;display:flex;align-items:center;justify-content:center'>"
                           "xi0n — accès par lien privé",
                           "text/html; charset=utf-8")
                return

        if path in ("/", "/coord", "/coord/"):
            self._send(200, PAGE, "text/html; charset=utf-8")
            return

        if path == "/coord/healthz":
            self._json(200, {"ok": True, "rss_kb": _rss_kb()})
            return

        if path == "/coord/whoami":
            ident = sso_ident(self.headers)
            self._json(200, {"agent": ident or DEFAULT_AGENT, "sso": bool(ident)})
            return

        if path == "/coord/api/messages":
            try:
                after = int(q.get("after", ["0"])[0])
            except ValueError:
                after = 0
            try:
                before = int(q.get("before", ["0"])[0])
            except ValueError:
                before = 0
            try:
                limit = int(q.get("limit", [str(FEED_LIMIT_DEF)])[0])
            except ValueError:
                limit = FEED_LIMIT_DEF
            topic = (q.get("topic", [None])[0] or None)
            agent = (q.get("agent", [None])[0] or None)

            # cache court uniquement pour le "premier chargement" (after=0,before=0,no filter)
            cacheable = (after == 0 and before == 0 and not topic and not agent)
            if cacheable:
                with _feed_lock:
                    if (_feed_cache["data"] is not None
                            and time.time() - _feed_cache["ts"] < FEED_CACHE_TTL
                            and _feed_cache["key"] == limit):
                        self._json(200, _feed_cache["data"])
                        return

            msgs = fetch_messages(after=after, before=before, limit=limit,
                                  topic=topic, agent=agent)
            if msgs is None:
                self._json(503, {"err": "db_unavailable"})
                return
            last_id = msgs[-1]["id"] if msgs else after
            data = {"messages": msgs, "last_id": last_id}
            if cacheable:
                with _feed_lock:
                    _feed_cache.update(ts=time.time(), key=limit, data=data)
            self._json(200, data)
            return

        m = re.match(r"^/coord/api/image/(\d+)$", path)
        if m:
            mid = int(m.group(1))
            data, ctype = get_image(mid)
            if data is None:
                self._send(404, b"not found", "text/plain")
                return
            self._send(200, data, ctype, extra={"Cache-Control": "private, max-age=300"})
            return

        self._send(404, b"not found", "text/plain")

    # ---- POST ------------------------------------------------------------
    def do_POST(self):
        u = urlparse(self.path)
        path = u.path
        if TOKEN and not hmac.compare_digest(cookie_token(self.headers), TOKEN):
            self._send(403, b"forbidden", "text/plain")
            return
        if path != "/coord/api/say":
            self._send(404, b"not found", "text/plain")
            return

        try:
            clen = int(self.headers.get("content-length", "0"))
        except ValueError:
            clen = 0
        if clen > MAX_BODY:
            self._json(413, {"err": "too_large"})
            # vider le socket
            try:
                self.rfile.read(min(clen, MAX_BODY))
            except Exception:
                pass
            return
        body = self.rfile.read(clen) if clen else b""

        ident = self._ident()
        if not _allow_say(ident):
            self._json(429, {"err": "rate"}, extra={"Retry-After": "3"})
            return

        try:
            obj = json.loads(body or b"{}")
        except Exception:
            self._json(400, {"err": "bad_json"})
            return

        ssou = sso_ident(self.headers)
        # auteur : SSO si present, sinon ce que demande le client, sinon defaut.
        agent = (obj.get("agent") or "").strip().lower() or (ssou or DEFAULT_AGENT)
        if not AGENT_RE.match(agent):
            self._json(400, {"err": "bad_agent"})
            return
        topic = (obj.get("topic") or "").strip().lower() or None
        if topic and not TOPIC_RE.match(topic):
            self._json(400, {"err": "bad_topic"})
            return
        content = obj.get("message") or obj.get("content") or ""
        if not isinstance(content, str) or not content.strip():
            self._json(400, {"err": "empty"})
            return

        ok, err = coord_say(agent, content, topic)
        if not ok:
            self._json(400 if err in ("bad_agent", "bad_topic", "empty") else 502,
                       {"err": err})
            return
        # invalide le cache feed pour que le post apparaisse tout de suite
        with _feed_lock:
            _feed_cache["data"] = None
        self._json(200, {"ok": True, "agent": agent, "topic": topic})


# ---------------------------------------------------------------------------
# Page mobile-first (HTML/CSS/JS inline, safe-area iOS, plein ecran, poll leger)
# ---------------------------------------------------------------------------
PAGE = r"""<!doctype html>
<html lang="fr">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover,maximum-scale=1">
<meta name="apple-mobile-web-app-capable" content="yes">
<meta name="apple-mobile-web-app-status-bar-style" content="black-translucent">
<meta name="theme-color" content="#0b0e14">
<title>xi0n /coord</title>
<style>
  :root{
    --bg:#0b0e14; --panel:#141925; --panel2:#1b2233; --line:#252d40;
    --txt:#e6ebf5; --dim:#8b96ad; --accent:#5ee0c0; --ping:#ff5d73;
    --me:#26334d; --topic:#2a3550;
    --safe-top:env(safe-area-inset-top); --safe-bot:env(safe-area-inset-bottom);
  }
  *{box-sizing:border-box;-webkit-tap-highlight-color:transparent}
  html,body{margin:0;height:100%;background:var(--bg);color:var(--txt);
    font:16px/1.45 -apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;
    overscroll-behavior-y:contain}
  body{display:flex;flex-direction:column;height:100dvh}
  header{position:sticky;top:0;z-index:5;background:rgba(11,14,20,.92);
    backdrop-filter:blur(10px);border-bottom:1px solid var(--line);
    padding:calc(var(--safe-top) + 8px) 12px 8px;display:flex;align-items:center;gap:10px}
  header .dot{width:9px;height:9px;border-radius:50%;background:var(--dim);flex:0 0 auto}
  header .dot.on{background:var(--accent);box-shadow:0 0 8px var(--accent)}
  header h1{font-size:16px;margin:0;font-weight:700;letter-spacing:.5px}
  header .who{margin-left:auto;font-size:12px;color:var(--dim);max-width:42%;
    overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  #refresh{flex:0 0 auto;background:none;border:0;color:var(--dim);font-size:18px;padding:4px 6px}
  #feed{flex:1;overflow-y:auto;-webkit-overflow-scrolling:touch;
    padding:10px 10px 14px;display:flex;flex-direction:column;gap:8px}
  .pull{text-align:center;color:var(--dim);font-size:13px;padding:6px;display:none}
  .msg{background:var(--panel);border:1px solid var(--line);border-radius:14px;
    padding:9px 12px;max-width:92%}
  .msg.me{align-self:flex-end;background:var(--me)}
  .msg.ping{border-color:var(--ping);box-shadow:0 0 0 1px var(--ping) inset}
  .meta{display:flex;gap:8px;align-items:baseline;font-size:12px;margin-bottom:3px;flex-wrap:wrap}
  .meta .a{color:var(--accent);font-weight:700}
  .meta .t{color:var(--dim)}
  .tag{background:var(--topic);color:#bcd;border-radius:20px;padding:1px 8px;font-size:11px}
  .tag.ping{background:var(--ping);color:#fff}
  .body{white-space:pre-wrap;word-break:break-word}
  .body:empty{display:none}
  .media{margin-top:8px;border-radius:10px;overflow:hidden;max-width:100%}
  .media img,.media video{display:block;width:100%;height:auto;background:#000}
  .media audio{width:100%}
  .day{align-self:center;color:var(--dim);font-size:11px;background:var(--panel2);
    border:1px solid var(--line);border-radius:20px;padding:2px 10px;margin:4px 0}
  footer{position:sticky;bottom:0;background:rgba(11,14,20,.96);
    border-top:1px solid var(--line);
    padding:8px 10px calc(var(--safe-bot) + 8px)}
  .row{display:flex;gap:8px;align-items:flex-end}
  .topicrow{display:flex;gap:8px;margin-bottom:6px}
  .topicrow input{flex:1;min-width:0}
  input,textarea{background:var(--panel2);border:1px solid var(--line);color:var(--txt);
    border-radius:12px;padding:10px 12px;font:inherit;width:100%}
  input::placeholder,textarea::placeholder{color:var(--dim)}
  textarea{resize:none;max-height:140px;line-height:1.35}
  #send{flex:0 0 auto;background:var(--accent);color:#062019;border:0;font-weight:800;
    border-radius:12px;padding:0 18px;height:46px;font-size:16px}
  #send:disabled{opacity:.4}
  .chips{display:flex;gap:6px;overflow-x:auto;padding-bottom:6px;margin-bottom:2px}
  .chip{flex:0 0 auto;background:var(--panel2);border:1px solid var(--line);color:var(--dim);
    border-radius:20px;padding:4px 12px;font-size:13px}
  .chip.on{background:var(--accent);color:#062019;border-color:var(--accent);font-weight:700}
  .err{color:var(--ping);font-size:12px;min-height:14px;padding:2px 2px 0}
</style>
</head>
<body>
<header>
  <span class="dot" id="dot"></span>
  <h1>xi0n /coord</h1>
  <span class="who" id="who">…</span>
  <button id="refresh" aria-label="rafraichir">⟳</button>
</header>

<div id="feed"><div class="pull" id="pull">⟳ charger plus ancien</div></div>

<footer>
  <div class="chips" id="chips"></div>
  <div class="topicrow">
    <input id="topic" placeholder="topic (ex: scientifiques)" autocapitalize="off" autocomplete="off" inputmode="text">
  </div>
  <div class="row">
    <textarea id="msg" rows="1" placeholder="message…" enterkeyhint="send"></textarea>
    <button id="send">▲</button>
  </div>
  <div class="err" id="err"></div>
</footer>

<script>
(function(){
  var API="/coord/api";
  var feed=document.getElementById("feed");
  var pull=document.getElementById("pull");
  var dot=document.getElementById("dot");
  var who=document.getElementById("who");
  var msgEl=document.getElementById("msg");
  var topicEl=document.getElementById("topic");
  var sendBtn=document.getElementById("send");
  var errEl=document.getElementById("err");
  var chipsEl=document.getElementById("chips");
  var refreshBtn=document.getElementById("refresh");

  var ME="jsosion", lastId=0, firstId=0, filterTopic=null, polling=false, loading=false;
  var seen={}, lastDay="";

  function esc(s){return (s||"").replace(/[&<>]/g,function(c){return{"&":"&amp;","<":"&lt;",">":"&gt;"}[c];});}
  function hhmm(iso){ if(!iso)return""; var d=new Date(iso.replace(" ","T")+(iso.indexOf("T")<0&&iso.indexOf("Z")<0?"":""));
    if(isNaN(d)){ var m=/(\d\d):(\d\d):(\d\d)/.exec(iso); return m?m[1]+":"+m[2]:""; }
    return ("0"+d.getHours()).slice(-2)+":"+("0"+d.getMinutes()).slice(-2); }
  function dayLabel(iso){ if(!iso)return""; var m=/(\d{4})-(\d\d)-(\d\d)/.exec(iso); return m?m[0]:""; }

  function nearBottom(){ return feed.scrollHeight-feed.scrollTop-feed.clientHeight < 120; }

  function mediaNode(m){
    if(!m.media) return "";
    if(m.media_kind==="video") return '<div class="media"><video src="'+m.media+'" controls playsinline preload="metadata"></video></div>';
    if(m.media_kind==="audio") return '<div class="media"><audio src="'+m.media+'" controls preload="none"></audio></div>';
    return '<div class="media"><a href="'+m.media+'" target="_blank" rel="noopener"><img loading="lazy" src="'+m.media+'" alt="tsoin"></a></div>';
  }

  function render(m, prepend){
    if(seen[m.id]) return; seen[m.id]=1;
    var day=dayLabel(m.at);
    var el=document.createElement("div");
    el.className="msg"+(m.agent===ME?" me":"")+(m.is_ping?" ping":"");
    el.dataset.id=m.id;
    var tag = m.topic? '<span class="tag'+(m.is_ping?" ping":"")+'">'+esc(m.topic)+'</span>' :
              (m.is_ping?'<span class="tag ping">PING</span>':'');
    el.innerHTML='<div class="meta"><span class="a">'+esc(m.agent)+'</span><span class="t">'+hhmm(m.at)+'</span>'+tag+'</div>'+
      '<div class="body">'+esc(m.content)+'</div>'+mediaNode(m);
    if(prepend){
      // jour-separateur en tete si change (best-effort, ancien d'abord)
      feed.insertBefore(el, pull.nextSibling);
    }else{
      if(day && day!==lastDay){ var d=document.createElement("div"); d.className="day"; d.textContent=day; feed.appendChild(d); lastDay=day; }
      feed.appendChild(el);
    }
  }

  function setDot(on){ dot.classList.toggle("on", !!on); }

  function load(){
    if(loading) return; loading=true;
    fetch(API+"/messages?limit=60"+(filterTopic?("&topic="+encodeURIComponent(filterTopic)):""))
      .then(function(r){ if(r.status===401||r.status===302){location.reload();return null;} return r.json(); })
      .then(function(j){ loading=false; if(!j){setDot(false);return;}
        setDot(true);
        var msgs=j.messages||[];
        // reset si filtre a change
        clearFeed();
        msgs.forEach(function(m){ if(m.id<firstId||firstId===0) firstId=m.id; render(m,false); });
        if(msgs.length){ lastId=msgs[msgs.length-1].id; }
        pull.style.display = msgs.length>=60 ? "block":"none";
        feed.scrollTop=feed.scrollHeight;
        buildChips(msgs);
      })
      .catch(function(){ loading=false; setDot(false); });
  }

  function clearFeed(){
    var n=feed.querySelectorAll(".msg,.day"); n.forEach(function(x){x.remove();});
    seen={}; lastDay=""; firstId=0;
  }

  function poll(){
    if(polling) return; polling=true;
    fetch(API+"/messages?after="+lastId+(filterTopic?("&topic="+encodeURIComponent(filterTopic)):""))
      .then(function(r){ if(r.status===401||r.status===302){return null;} return r.json(); })
      .then(function(j){ polling=false; if(!j){setDot(false);return;} setDot(true);
        var msgs=j.messages||[]; if(!msgs.length) return;
        var stick=nearBottom();
        msgs.forEach(function(m){ render(m,false); lastId=Math.max(lastId,m.id); });
        if(stick) feed.scrollTop=feed.scrollHeight;
      })
      .catch(function(){ polling=false; setDot(false); });
  }

  function older(){
    if(loading||!firstId) return; loading=true; pull.textContent="…";
    fetch(API+"/messages?before="+firstId+"&limit=50"+(filterTopic?("&topic="+encodeURIComponent(filterTopic)):""))
      .then(function(r){return r.json();})
      .then(function(j){ loading=false; pull.textContent="⟳ charger plus ancien";
        var msgs=(j&&j.messages)||[]; if(!msgs.length){pull.style.display="none";return;}
        var h0=feed.scrollHeight, s0=feed.scrollTop;
        // ancien d'abord -> prepend en ordre inverse pour garder la chrono
        for(var i=msgs.length-1;i>=0;i--){ var m=msgs[i]; if(m.id<firstId)firstId=m.id; render(m,true); }
        feed.scrollTop = s0 + (feed.scrollHeight-h0);
        if(msgs.length<50) pull.style.display="none";
      })
      .catch(function(){ loading=false; pull.textContent="⟳ charger plus ancien"; });
  }

  function buildChips(msgs){
    var topics={}; msgs.forEach(function(m){ if(m.topic) topics[m.topic]=1; });
    var list=Object.keys(topics).sort();
    chipsEl.innerHTML="";
    var all=chip("tout", null); chipsEl.appendChild(all);
    list.forEach(function(t){ chipsEl.appendChild(chip(t,t)); });
  }
  function chip(label, topic){
    var c=document.createElement("button"); c.className="chip"+((filterTopic||null)===topic?" on":"");
    c.textContent=label; c.onclick=function(){ filterTopic=topic; lastId=0; load(); };
    return c;
  }

  function send(){
    var content=msgEl.value.trim(); if(!content) return;
    var topic=topicEl.value.trim().toLowerCase();
    sendBtn.disabled=true; errEl.textContent="";
    fetch(API+"/say",{method:"POST",headers:{"Content-Type":"application/json"},
      body:JSON.stringify({message:content,topic:topic||undefined})})
      .then(function(r){return r.json().then(function(j){return {s:r.status,j:j};});})
      .then(function(res){ sendBtn.disabled=false;
        if(res.s===200){ msgEl.value=""; autosize(); poll(); feed.scrollTop=feed.scrollHeight; }
        else if(res.s===429){ errEl.textContent="trop vite — attends un peu."; }
        else { errEl.textContent="echec: "+((res.j&&res.j.err)||res.s); }
      })
      .catch(function(){ sendBtn.disabled=false; errEl.textContent="reseau ko"; });
  }

  function autosize(){ msgEl.style.height="auto"; msgEl.style.height=Math.min(140,msgEl.scrollHeight)+"px"; }
  msgEl.addEventListener("input", autosize);
  msgEl.addEventListener("keydown", function(e){
    if(e.key==="Enter" && !e.shiftKey){ e.preventDefault(); send(); }
  });
  sendBtn.onclick=send;
  refreshBtn.onclick=function(){ lastId=0; load(); };
  pull.onclick=older;

  // pull-to-refresh-ish : si on scrolle tout en haut et qu'on relache, charge l'ancien
  feed.addEventListener("scroll", function(){ if(feed.scrollTop<=0 && firstId) {/*older via bouton*/} });

  // identite (auteur SSO par defaut)
  fetch("/coord/whoami").then(function(r){return r.json();}).then(function(j){
    ME=j.agent||"jsosion"; who.textContent="@"+ME+(j.sso?"":" (defaut)");
  }).catch(function(){});

  load();
  setInterval(poll, 4000);
  document.addEventListener("visibilitychange", function(){ if(!document.hidden) poll(); });
})();
</script>
</body>
</html>
"""


def main():
    try:
        os.makedirs(CACHE, exist_ok=True)
    except OSError:
        pass
    _resolve_db_pass()
    httpd = ThreadingHTTPServer((ADDR, PORT), H)
    httpd.daemon_threads = True
    os.write(2, ("xi0n-coord listening on %s:%d (log=%s, cache=%s)\n"
                 % (ADDR, PORT, LOG_MODE, CACHE)).encode())
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        os.write(2, b"xi0n-coord stopped\n")


if __name__ == "__main__":
    main()
