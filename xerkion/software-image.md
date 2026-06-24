# Recette de l'image — le bion physique (Pi Zero 2 W, 512 Mo)

## 0. Travail logiciel AVANT le makelab (jalon D-soft)
Trois pièces **n'existent pas** dans le repo (grep) et sont sur le chemin critique :
1. **Route `/cub4ion`** dans `crates/xerboxion-host/src/serve.rs` (pattern `include_str!` déjà utilisé pour `/paper`) :
   ```rust
   const CUB4ION_HTML: &str = include_str!("../../../web-cub4ion/index.html");
   .route("/cub4ion", get(|| async { axum::response::Html(CUB4ION_HTML) }))
   ```
   Sans elle, `cog … /cub4ion` = **404**. Critère : `curl 127.0.0.1:8730/cub4ion` = 200.
2. **Patch `web-cub4ion/index.html`** (~20 lignes) :
   ```js
   let imuActive=false;
   const es=new EventSource('/events');
   es.onmessage=e=>{ const m=JSON.parse(e.data);
     if(m.topic==='sensor.tilt'){ imuActive=true; rZW=m.payload.rZW; rXW=m.payload.rXW||0; rYW=m.payload.rYW||0; }
     if(m.topic==='cub4ion.lever'){ toggle(); } };
   // dans loop(): if(imuActive){ vZW=0; vXW=0; }  // et NE PAS appliquer rXY+=... ni rZW+=0.0016
   ```
   Le flag `imuActive` **tue la dérive** (l.174) et l'inertie drag (l.173) — sinon la scène vagabonde.
3. **`xerkion-tilt`** (pont I2C, ~120 lignes Python/Rust) — §3.

## 1. OS de base
- **Raspberry Pi OS Lite 64-bit (Bookworm, aarch64, kernel 6.6)**, headless. ~1,9 Go, ~95 Mo RAM repos.
- `config.txt` :
  ```ini
  dtparam=spi=on
  dtparam=i2c_arm=on
  dtparam=i2s=on
  # ECRAN : vrai pilote DRM SPI (carte DRM + connecteur que cog -P drm cible)
  dtoverlay=mipi-dbi-spi,spi0-0,write-only
  dtparam=width=240,height=240,bgr,reset-gpio=27,dc-gpio=25,backlight=13
  # AUDIO : SD tié en dur a VDD -> overlay SANS sdmode-pin
  dtoverlay=max98357a
  dtparam=watchdog=on
  gpu_mem=64            # Canvas2D logiciel ; a confirmer au banc
  disable_splash=1
  boot_delay=0
  ```
- **wifi** : `wpa_supplicant.conf` (SSID/PSK de José) sur la partition boot.

## 2. Renderer
- **`cog -P drm`** (WPE WebKit) sur la carte DRM créée par `mipi-dbi-spi` → pile **cohérente DRM↔DRM** (correction du seam fbtft/Cog).
- **`MemoryMax=300M`** sur le kiosk (l'OOM tue Cog, pas le daemon).
- **Plan B mesuré (recommandé si RSS > ~200 Mo ou latence)** : **renderer natif cairo/fbdev** (~5 Mo) qui réimplémente les 183 lignes Canvas2D et **lit l'I2C dans sa boucle** (latence quasi nulle).

## 3. Boucle accel → cub4ion (HYBRIDE, 6 FACES)
```
LSM6DS3 (I2C 0x6A) ──read 60Hz──▶ xerkion-tilt
   ├─ face dominante = signe de l'axe |g| max -> rZW ∈ {0, 2π/3, 4π/3} (hysteresis)
   ├─ WebSocket localhost:8731 ──▶ page (set rZW direct)   [DIRECT, <16ms]
   └─ POST :8730/emit {topic:"sensor.tilt", payload:{rZW,rXW,rYW,face,ax,ay,az}} 10Hz  [BUS]
```
- **Pas de yaw** : un 6-axes ne l'observe pas → on mappe la **face**.
- Levier (Hall GPIO17) → `POST /emit {topic:"cub4ion.lever"}` → `toggle()`.
- **Événementiel** : envoyer seulement si la face change → page fige le canvas au repos (conso).

## 4. Audio
- **Défaut : `mpg123`** d'un `.ogg` embarqué (`/opt/xerkion/fallback.ogg`), sur `cub4ion.power on` → **levier ON = toujours un son** (offline, sans user-gesture/Premium).
- **Bonus : `librespot`** (`--name xerkion --backend alsa`) joue `5rZDHBR8tLni1xp8FlRMjW` si wifi+Premium. Niçé pour ne pas voler le cœur du compositeur.
- Son sort du **Pi** via I2S→MAX98357A ; l'iframe reste fallback visuel.

## 5. Arborescence gravée
```
/boot/firmware/  config.txt  wpa_supplicant.conf
/opt/xerkion/
  xerboxion-rt            # daemon aarch64 (~7,1 Mo) AVEC route /cub4ion
  ploxions/*.wasm         # ~2 Mo
  web/cub4ion.html        # (ou include_str! par le daemon)
  xerkion-tilt            # pont I2C -> WS 8731 + /emit sensor.tilt
  fallback.ogg            # audio offline par defaut
  cog-kiosk.sh            # cog -P drm http://127.0.0.1:8730/cub4ion
  state/
/etc/systemd/system/  xion.service  xerkion-tilt.service  audio.service  cog-kiosk.service
```

## 6. Units systemd (installeur dédié, PAS flash.sh nu)
> `flash.sh pi` ne livre que le binaire en `--addr 0.0.0.0` sans kiosk/tilt/audio : insuffisant. On force `--addr 127.0.0.1`.
```ini
# xion.service
[Unit] Description=xion daemon
After=network.target
[Service]
WorkingDirectory=/opt/xerkion
ExecStart=/opt/xerkion/xerboxion-rt serve ./ploxions --addr 127.0.0.1 --port 8730 --state-dir ./state
ExecStartPost=/bin/sh -c 'until curl -sf http://127.0.0.1:8730/healthz; do sleep 0.3; done'
Restart=always
RestartSec=2
[Install] WantedBy=multi-user.target
```
```ini
# cog-kiosk.service
[Unit] After=xion.service
Requires=xion.service
[Service]
Environment=COG_PLATFORM_DRM_RENDER_DEVICE=/dev/dri/card0
ExecStartPre=/bin/sh -c 'until curl -sf http://127.0.0.1:8730/cub4ion; do sleep 0.3; done'
ExecStart=/usr/bin/cog -P drm http://127.0.0.1:8730/cub4ion
MemoryMax=300M
Restart=always
[Install] WantedBy=multi-user.target
```
```ini
# xerkion-tilt.service
[Unit] After=xion.service
Requires=xion.service
[Service]
ExecStart=/opt/xerkion/xerkion-tilt --i2c 0x6A --ws 8731 --emit http://127.0.0.1:8730/emit
Restart=always
[Install] WantedBy=multi-user.target
```
(`audio.service` : mpg123 + librespot déclenchés par `cub4ion.power`.)

## 7. Stratégie RAM (512 Mo, à MESURER)
| Composant | RAM |
|---|---|
| kernel + RPi OS Lite | ~95 Mo |
| gpu_mem (VC4) | 64 Mo |
| xion + wasm | ~25 Mo |
| Cog/WPE + page (**MESURER**, plan B natif si >200) | ~150 Mo |
| xerkion-tilt | ~12 Mo |
| mpg123 / librespot | ~20 Mo |
| **Total** | **~366 Mo** (marge ~146) |
- **zram** lz4 512 Mo (coussin). Mesure obligatoire `free -m` + RSS `cog` avant de figer ; si >200 → natif cairo.

## 8. Vers UN bion ≤16 Go
Image complète ~1,9 Go. Core OS ~15 Mo. Suite : Buildroot/initramfs ~80–150 Mo bootant direct sur `cog + xion` → image ~200 Mo = xerkion quasi pur.
