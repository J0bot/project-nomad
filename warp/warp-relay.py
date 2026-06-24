#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
warp-relay — RELAIS AVEUGLE (blind relay) pour le /warp du coeur (web-heart).

Le coeur (web-heart/index.html) reste l'autorite : record/replay du giga-tsoin,
crypto (ECDH P-256 -> HKDF -> AES-GCM cote NAVIGATEUR), export .heart.
Ce relais ne fait QUE deplacer des octets OPAQUES entre deux navigateurs :
  - il ne dechiffre rien, ne connait aucune cle privee,
  - il ne loggue AUCUN contenu (ni `ct`, ni body),
  - tout vit en RAM, borne, avec TTL : oublieux par conception,
  - suppression du service = etat d'avant, zero residu.

UN SEUL FICHIER, Python 3.11 stdlib asyncio. Aucune dependance, aucun venv.
Bind par defaut 127.0.0.1:8732 (prive). Exposition uniquement via Traefik.

Non-negociables respectes :
  (1) E2E : le relais ne voit jamais le plaintext ; seul ciphertext + roomId opaque.
  (2) Seules des pubkeys (non secretes) transitent en clair (boite d'amorcage).
  (3) Le fragment # ne touche jamais le serveur (cote client) ; ici Referrer-Policy:no-referrer.
  (4) Offline-first : si ce relais tombe, le coeur reste 100% local + .heart.
  (5) RAM-light, content-blind, zero log de contenu, retirable sans rien casser.

Env :
  WARP_ADDR   (defaut 127.0.0.1)   adresse de bind. Mettre 10.0.0.1 pour l'expo Traefik.
  WARP_PORT   (defaut 8732)        port de bind.
  WARP_LOG    (defaut quiet)       quiet = rien sauf erreurs ; access = ligne par requete (jamais le contenu).
"""

import asyncio
import json
import os
import time
import re
import collections

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------
ADDR = os.environ.get("WARP_ADDR", "127.0.0.1")
PORT = int(os.environ.get("WARP_PORT", "8732"))
LOG_MODE = os.environ.get("WARP_LOG", "quiet")  # quiet | access

# Bornes dures (anti-OOM ; RAM serree, Minecraft tourne)
MAX_ROOMS = 2000
MAX_BOXES = 2000
ROOM_BUF = 200            # ring-buffer de messages par room
BOX_BUF = 20             # ring-buffer d'items par boite
MAX_BODY_ROOM = 64 * 1024  # 64 KiB
MAX_BODY_BOX = 8 * 1024   # 8 KiB
ROOM_TTL = 600.0          # 10 min sans activite -> GC (si pas de waiter)
BOX_TTL = 300.0          # 5 min -> item evince
GC_EVERY = 30.0
SSE_HEARTBEAT = 25.0      # ping anti-timeout proxy
LONGPOLL_MAX = 30.0

# Rate-limit (token-bucket par IP)
RL_SEND_RATE = 20.0       # req/s
RL_SEND_BURST = 40.0
RL_BOX_RATE = 5.0
RL_BOX_BURST = 10.0

ID_RE = re.compile(r"^[A-Za-z0-9_-]{16,64}$")

# ---------------------------------------------------------------------------
# Etat (RAM uniquement)
# ---------------------------------------------------------------------------


class Room:
    __slots__ = ("buf", "seq", "waiters", "last")

    def __init__(self):
        self.buf = collections.deque(maxlen=ROOM_BUF)  # {seq, ct, at}
        self.seq = 0
        self.waiters = set()   # asyncio.Future en attente (long-poll)
        self.last = time.time()


rooms = {}   # roomId -> Room
boxes = {}   # boxId  -> deque({ct, at})  (boite d'amorcage des pubkeys)

# Rate-limit : ip -> {"send": [tokens, ts], "box": [tokens, ts]}
_buckets = {}


def _allow(ip, kind):
    rate, burst = (RL_SEND_RATE, RL_SEND_BURST) if kind == "send" else (RL_BOX_RATE, RL_BOX_BURST)
    now = time.time()
    b = _buckets.get(ip)
    if b is None:
        b = {"send": [RL_SEND_BURST, now], "box": [RL_BOX_BURST, now]}
        _buckets[ip] = b
    slot = b[kind]
    tokens, ts = slot
    tokens = min(burst, tokens + (now - ts) * rate)
    if tokens < 1.0:
        slot[0] = tokens
        slot[1] = now
        return False
    slot[0] = tokens - 1.0
    slot[1] = now
    return True


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

# ---------------------------------------------------------------------------
# HTTP minimal sur asyncio.start_server
# ---------------------------------------------------------------------------

CORS = (
    "Access-Control-Allow-Origin: *\r\n"
    "Access-Control-Allow-Headers: content-type\r\n"
    "Access-Control-Allow-Methods: GET,POST,OPTIONS\r\n"
    "Cache-Control: no-store\r\n"
    "X-Content-Type-Options: nosniff\r\n"
    "Referrer-Policy: no-referrer\r\n"
)


async def _read_request(reader):
    """Lit une requete HTTP/1.1 minimale. Retourne (method, path, headers, body) ou None."""
    try:
        start = await reader.readuntil(b"\r\n\r\n")
    except (asyncio.IncompleteReadError, asyncio.LimitOverrunError):
        return None
    head = start.decode("latin1")
    lines = head.split("\r\n")
    if not lines or " " not in lines[0]:
        return None
    parts = lines[0].split(" ")
    if len(parts) < 2:
        return None
    method, target = parts[0], parts[1]
    headers = {}
    for ln in lines[1:]:
        if ":" in ln:
            k, v = ln.split(":", 1)
            headers[k.strip().lower()] = v.strip()
    body = b""
    clen = headers.get("content-length")
    if clen:
        try:
            n = int(clen)
        except ValueError:
            return None
        # garde dure avant de lire le corps
        if n > MAX_BODY_ROOM:
            # on lit et jette pour vider le socket proprement, mais on signalera 413
            n = min(n, MAX_BODY_ROOM + 1)
        try:
            body = await reader.readexactly(n)
        except asyncio.IncompleteReadError as e:
            body = e.partial
    return method, target, headers, body


def _resp(status, body=b"", ctype="application/json", extra=""):
    if isinstance(body, str):
        body = body.encode("utf-8")
    return (
        "HTTP/1.1 " + status + "\r\n"
        "Content-Type: " + ctype + "\r\n"
        "Content-Length: " + str(len(body)) + "\r\n"
        + CORS + extra +
        "Connection: close\r\n\r\n"
    ).encode("latin1") + body


def _json(status, obj):
    return _resp(status, json.dumps(obj, separators=(",", ":")))


def _client_ip(headers, peer):
    xff = headers.get("x-forwarded-for")
    if xff:
        return xff.split(",")[0].strip()
    return peer[0] if peer else "?"


def _split_path(target):
    """Retourne (path, query_dict)."""
    if "?" in target:
        path, qs = target.split("?", 1)
    else:
        path, qs = target, ""
    q = {}
    for pair in qs.split("&"):
        if not pair:
            continue
        if "=" in pair:
            k, v = pair.split("=", 1)
        else:
            k, v = pair, ""
        q[k] = v
    return path, q


async def handle(reader, writer):
    peer = writer.get_extra_info("peername")
    try:
        req = await _read_request(reader)
        if req is None:
            writer.write(_resp("400 Bad Request", b"bad request", "text/plain"))
            await writer.drain()
            return
        method, target, headers, body = req
        path, q = _split_path(target)

        if method == "OPTIONS":
            writer.write(_resp("204 No Content", b"", "text/plain"))
            await writer.drain()
            return

        ip = _client_ip(headers, peer)
        _log("%s %s" % (method, path[:48]))

        # --- healthz ---
        if method == "GET" and path == "/warp/healthz":
            writer.write(_json("200 OK", {
                "ok": True, "rooms": len(rooms), "boxes": len(boxes), "rss_kb": _rss_kb()
            }))
            await writer.drain()
            return

        # --- racine : ne sert pas de contenu, juste un ping neutre ---
        if method == "GET" and path == "/":
            writer.write(_json("200 OK", {"warp": "relay", "blind": True}))
            await writer.drain()
            return

        # --- boite d'amorcage : POST /warp/box/{boxId} ---
        m = re.match(r"^/warp/box/([^/]+)$", path)
        if m and method == "POST":
            box_id = m.group(1)
            if not ID_RE.match(box_id):
                writer.write(_json("400 Bad Request", {"err": "bad_box_id"}))
                await writer.drain(); return
            if len(body) > MAX_BODY_BOX:
                writer.write(_json("413 Payload Too Large", {"err": "too_large"}))
                await writer.drain(); return
            if not _allow(ip, "box"):
                writer.write(_resp("429 Too Many Requests",
                                   json.dumps({"err": "rate"}), "application/json",
                                   "Retry-After: 1\r\n"))
                await writer.drain(); return
            try:
                obj = json.loads(body or b"{}")
                ct = obj.get("ct")
            except Exception:
                ct = None
            if not isinstance(ct, str) or not ct:
                writer.write(_json("400 Bad Request", {"err": "no_ct"}))
                await writer.drain(); return
            dq = boxes.get(box_id)
            if dq is None:
                if len(boxes) >= MAX_BOXES:
                    writer.write(_json("503 Service Unavailable", {"err": "boxes_full"}))
                    await writer.drain(); return
                dq = collections.deque(maxlen=BOX_BUF)
                boxes[box_id] = dq
            dq.append({"ct": ct, "at": time.time()})
            writer.write(_json("200 OK", {"ok": True}))
            await writer.drain(); return

        # --- boite d'amorcage : GET /warp/box/{boxId}?after=N ---
        if m and method == "GET":
            box_id = m.group(1)
            if not ID_RE.match(box_id):
                writer.write(_json("400 Bad Request", {"err": "bad_box_id"}))
                await writer.drain(); return
            try:
                after = int(q.get("after", "0"))
            except ValueError:
                after = 0
            dq = boxes.get(box_id)
            items = []
            if dq:
                lst = list(dq)
                items = lst[after:] if after < len(lst) else []
            writer.write(_json("200 OK", {"items": items, "idx": (after if not dq else len(dq))}))
            await writer.drain(); return

        # --- send : POST /warp/{roomId}/send ---
        m = re.match(r"^/warp/([^/]+)/send$", path)
        if m and method == "POST":
            room_id = m.group(1)
            if not ID_RE.match(room_id):
                writer.write(_json("400 Bad Request", {"err": "bad_room_id"}))
                await writer.drain(); return
            if len(body) > MAX_BODY_ROOM:
                writer.write(_json("413 Payload Too Large", {"err": "too_large"}))
                await writer.drain(); return
            if not _allow(ip, "send"):
                writer.write(_resp("429 Too Many Requests",
                                   json.dumps({"err": "rate"}), "application/json",
                                   "Retry-After: 1\r\n"))
                await writer.drain(); return
            try:
                obj = json.loads(body or b"{}")
                ct = obj.get("ct")
            except Exception:
                ct = None
            if not isinstance(ct, str) or not ct:
                writer.write(_json("400 Bad Request", {"err": "no_ct"}))
                await writer.drain(); return
            room = rooms.get(room_id)
            if room is None:
                if len(rooms) >= MAX_ROOMS:
                    writer.write(_json("503 Service Unavailable", {"err": "rooms_full"}))
                    await writer.drain(); return
                room = Room()
                rooms[room_id] = room
            room.seq += 1
            msg = {"seq": room.seq, "ct": ct, "at": time.time()}
            room.buf.append(msg)
            room.last = time.time()
            # reveille les long-poll en attente
            for fut in list(room.waiters):
                if not fut.done():
                    fut.set_result(msg)
            writer.write(_json("200 OK", {"seq": room.seq}))
            await writer.drain(); return

        # --- long-poll : GET /warp/{roomId}/poll?after=N&wait=25 ---
        m = re.match(r"^/warp/([^/]+)/poll$", path)
        if m and method == "GET":
            room_id = m.group(1)
            if not ID_RE.match(room_id):
                writer.write(_json("400 Bad Request", {"err": "bad_room_id"}))
                await writer.drain(); return
            try:
                after = int(q.get("after", "0"))
            except ValueError:
                after = 0
            try:
                wait = min(LONGPOLL_MAX, max(0.0, float(q.get("wait", "25"))))
            except ValueError:
                wait = 25.0
            room = rooms.get(room_id)
            if room:
                pending = [x for x in room.buf if x["seq"] > after]
                if pending:
                    writer.write(_json("200 OK", {"msgs": pending, "seq": room.seq}))
                    await writer.drain(); return
            if room is None:
                # rien encore ; on cree la room pour pouvoir attendre dessus
                if len(rooms) >= MAX_ROOMS:
                    writer.write(_json("200 OK", {"msgs": [], "seq": after}))
                    await writer.drain(); return
                room = Room()
                rooms[room_id] = room
            fut = asyncio.get_event_loop().create_future()
            room.waiters.add(fut)
            room.last = time.time()
            try:
                await asyncio.wait_for(fut, timeout=wait)
            except asyncio.TimeoutError:
                pass
            finally:
                room.waiters.discard(fut)
            pending = [x for x in room.buf if x["seq"] > after]
            writer.write(_json("200 OK", {"msgs": pending, "seq": room.seq}))
            await writer.drain(); return

        # --- SSE : GET /warp/{roomId}/events?after=N ---
        m = re.match(r"^/warp/([^/]+)/events$", path)
        if m and method == "GET":
            room_id = m.group(1)
            if not ID_RE.match(room_id):
                writer.write(_json("400 Bad Request", {"err": "bad_room_id"}))
                await writer.drain(); return
            try:
                after = int(q.get("after", "0"))
            except ValueError:
                after = 0
            room = rooms.get(room_id)
            if room is None:
                if len(rooms) >= MAX_ROOMS:
                    writer.write(_json("503 Service Unavailable", {"err": "rooms_full"}))
                    await writer.drain(); return
                room = Room()
                rooms[room_id] = room
            # en-tete SSE
            writer.write((
                "HTTP/1.1 200 OK\r\n"
                "Content-Type: text/event-stream\r\n"
                "Cache-Control: no-store\r\n"
                + CORS +
                "Connection: keep-alive\r\n\r\n"
            ).encode("latin1"))
            await writer.drain()
            # backlog
            sent = after
            for x in list(room.buf):
                if x["seq"] > sent:
                    writer.write(("data: " + json.dumps(x, separators=(",", ":")) + "\n\n").encode("utf-8"))
                    sent = x["seq"]
            await writer.drain()
            last_ping = time.time()
            try:
                while True:
                    fut = asyncio.get_event_loop().create_future()
                    room.waiters.add(fut)
                    room.last = time.time()
                    try:
                        await asyncio.wait_for(fut, timeout=SSE_HEARTBEAT)
                    except asyncio.TimeoutError:
                        pass
                    finally:
                        room.waiters.discard(fut)
                    # pousse tout ce qui depasse `sent`
                    for x in list(room.buf):
                        if x["seq"] > sent:
                            writer.write(("data: " + json.dumps(x, separators=(",", ":")) + "\n\n").encode("utf-8"))
                            sent = x["seq"]
                    now = time.time()
                    if now - last_ping >= SSE_HEARTBEAT:
                        writer.write(b": ping\n\n")
                        last_ping = now
                    await writer.drain()
            except (ConnectionResetError, BrokenPipeError, asyncio.CancelledError):
                pass
            return

        # --- 404 ---
        writer.write(_json("404 Not Found", {"err": "not_found"}))
        await writer.drain()

    except (ConnectionResetError, BrokenPipeError):
        pass
    except Exception as e:
        _log("ERR %r" % (e,))
        try:
            writer.write(_json("500 Internal Server Error", {"err": "internal"}))
            await writer.drain()
        except Exception:
            pass
    finally:
        try:
            writer.close()
        except Exception:
            pass


# ---------------------------------------------------------------------------
# Garbage collector (TTL, RAM bornee)
# ---------------------------------------------------------------------------
async def gc_loop():
    while True:
        await asyncio.sleep(GC_EVERY)
        now = time.time()
        # rooms : supprime si inactive ET sans waiter
        for rid in list(rooms.keys()):
            r = rooms.get(rid)
            if r is None:
                continue
            if not r.waiters and (now - r.last) > ROOM_TTL:
                del rooms[rid]
        # boxes : evince items vieux puis boites vides
        for bid in list(boxes.keys()):
            dq = boxes.get(bid)
            if dq is None:
                continue
            while dq and (now - dq[0]["at"]) > BOX_TTL:
                dq.popleft()
            if not dq:
                del boxes[bid]
        # rate-limit : purge les buckets pleins et vieux
        for ip in list(_buckets.keys()):
            b = _buckets[ip]
            if (now - b["send"][1]) > 60 and (now - b["box"][1]) > 60:
                del _buckets[ip]


async def main():
    server = await asyncio.start_server(handle, ADDR, PORT)
    asyncio.ensure_future(gc_loop())
    os.write(2, ("warp-relay listening on %s:%d (blind, RAM-only, log=%s)\n"
                 % (ADDR, PORT, LOG_MODE)).encode())
    async with server:
        await server.serve_forever()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        os.write(2, b"warp-relay stopped (silence)\n")
