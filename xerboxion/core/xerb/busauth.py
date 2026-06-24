# xerb/busauth.py — en-têtes des requêtes au bus xion (10.0.0.1:8730).
#
# Ajoute `Authorization: Bearer <XION_BUS_TOKEN>` si l'env est posé, sinon rien. RÉTROCOMPAT :
# tant que `XION_BUS_TOKEN` n'est pas défini, on n'envoie pas de token → marche avec le daemon
# non-enforçant. L'enforcement s'active quand l'env est posé PARTOUT (daemon `xion.service` +
# tous ces clients émetteurs) — voir le middleware require_bus_token (serve.rs) et
# vps_infra/docs/REMEDIATION-LOG.md. DRY pour tous les émetteurs host (sys-ram, ultra-tsoin, …).
import os


def bus_headers(extra=None):
    h = {"Content-Type": "application/json"}
    if extra:
        h.update(extra)
    t = os.environ.get("XION_BUS_TOKEN", "").strip()
    if t:
        h["Authorization"] = "Bearer " + t
    return h
