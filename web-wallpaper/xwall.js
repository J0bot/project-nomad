/* xwall — moteur de fonds d'écran du xer, partagé par le ploxion wallpaper (preview) ET le bureau
   xer (application réelle). Pur canvas, GPU-friendly (transform/gradients, pas de per-pixel),
   respecte prefers-reduced-motion. XWALL.apply(canvas, cfg) -> stop(). cfg = {id, speed, hue}. */
var XWALL = (function () {
  var PRESETS = [
    { id: 'cellulaire', name: 'Cellulaire', emoji: '🫧' },
    { id: 'aurora',     name: 'Aurora',     emoji: '🌌' },
    { id: 'etoiles',    name: 'Étoiles',    emoji: '✨' },
    { id: 'nebula',     name: 'Nébuleuse',  emoji: '🌃' },
    { id: 'off',        name: 'Éteint',     emoji: '🌑' }
  ];
  function rnd(a, b) { return a + Math.random() * (b - a); }
  function apply(canvas, cfg) {
    cfg = cfg || {}; var id = cfg.id || 'cellulaire'; var spd = cfg.speed == null ? 1 : cfg.speed;
    var hue = cfg.hue == null ? 265 : cfg.hue;                       // violet par défaut (chill-tekk)
    var ctx = canvas.getContext('2d'); var DPR = Math.min(window.devicePixelRatio || 1, 2);
    var W = 1, H = 1, parts = [], stopped = false, raf = null, t0 = performance.now();
    var reduce = matchMedia('(prefers-reduced-motion:reduce)').matches;
    function size() { W = canvas.width = Math.max(1, (canvas.clientWidth || canvas.width) * DPR | 0);
      H = canvas.height = Math.max(1, (canvas.clientHeight || canvas.height) * DPR | 0); }
    size(); window.addEventListener('resize', size);
    function seed(n, mins, maxs) { parts = []; for (var i = 0; i < n; i++)
      parts.push({ x: Math.random(), y: Math.random(), z: rnd(mins, maxs), dx: rnd(-1, 1), dy: rnd(-1, 1),
        ph: Math.random() * 6.28, sp: rnd(.4, 1.2) }); }
    if (id === 'cellulaire') seed(16, .06, .26);
    if (id === 'etoiles') seed(140, .4, 2.4);
    if (id === 'nebula') seed(7, .35, .8);
    function frame(now) {
      if (stopped) return;
      var t = (now - t0) / 1000 * (reduce ? 0 : spd);
      ctx.clearRect(0, 0, W, H);
      if (id === 'off') { raf = requestAnimationFrame(frame); return; }
      if (id === 'cellulaire' || id === 'nebula') {
        parts.forEach(function (p, i) {
          var R = p.z * Math.min(W, H);
          var x = (p.x + Math.sin(t * p.sp * .25 + p.ph) * .06) * W;
          var y = (p.y + Math.cos(t * p.sp * .2 + p.ph) * .06) * H;
          var h2 = (hue + (i % 2 ? 95 : 0)) % 360;                   // alterne violet / vert bioluminescent
          var g = ctx.createRadialGradient(x, y, 0, x, y, R);
          var a = id === 'nebula' ? .22 : .14;
          g.addColorStop(0, 'hsla(' + h2 + ',70%,62%,' + a + ')');
          g.addColorStop(.6, 'hsla(' + h2 + ',70%,50%,' + (a * .4) + ')');
          g.addColorStop(1, 'hsla(' + h2 + ',70%,50%,0)');
          ctx.fillStyle = g; ctx.beginPath(); ctx.arc(x, y, R, 0, 6.2832); ctx.fill();
        });
      } else if (id === 'aurora') {
        for (var b = 0; b < 3; b++) {
          var yy = H * (.3 + b * .22) + Math.sin(t * .3 + b) * H * .08;
          var g2 = ctx.createLinearGradient(0, yy - H * .25, W, yy + H * .25);
          var h3 = (hue + b * 60 + t * 6) % 360;
          g2.addColorStop(0, 'hsla(' + h3 + ',75%,55%,0)');
          g2.addColorStop(.5, 'hsla(' + h3 + ',75%,58%,.16)');
          g2.addColorStop(1, 'hsla(' + ((h3 + 80) % 360) + ',75%,55%,0)');
          ctx.fillStyle = g2; ctx.fillRect(0, yy - H * .3, W, H * .6);
        }
      } else if (id === 'etoiles') {
        parts.forEach(function (p) {
          var x = p.x * W, y = ((p.y + t * .01 * p.sp) % 1) * H;
          var tw = .5 + .5 * Math.sin(t * 2 * p.sp + p.ph);
          ctx.fillStyle = 'hsla(' + hue + ',40%,90%,' + (.25 + tw * .6) + ')';
          ctx.beginPath(); ctx.arc(x, y, p.z * DPR * (.6 + tw * .6), 0, 6.2832); ctx.fill();
        });
      }
      raf = requestAnimationFrame(frame);
    }
    raf = requestAnimationFrame(frame);
    return function () { stopped = true; if (raf) cancelAnimationFrame(raf);
      window.removeEventListener('resize', size); try { ctx.clearRect(0, 0, W, H); } catch (e) {} };
  }
  return { apply: apply, PRESETS: PRESETS };
})();
if (typeof window !== 'undefined') window.XWALL = XWALL;
