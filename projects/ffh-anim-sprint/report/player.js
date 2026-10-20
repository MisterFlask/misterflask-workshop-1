/* Sprite-sheet animation players for the sprint report.
   Data: window.SPRINT = {tile, arms: [{id, name, anims: {idle: {sheet, ms: [...], hit, loop, hold}}}]}
   Every <canvas data-arm data-anim> on the page becomes a player. Canvases inside the same [data-clock] group
   share one clock and one control bar ([data-ctl] buttons inside that group). */
(function () {
  const S = window.SPRINT;
  const ARM = Object.fromEntries(S.arms.map((a) => [a.id, a]));
  const FW = 160, VX = 16, VW = 128, VH = 128;            // frame width; view = canvas x 16..144 (the safe region)
  const TAIL = { idle: 0, strike: 600, flinch: 600, death: 1600 };
  const reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const cache = {};
  const img = (src) => { if (!cache[src]) { const i = new Image(); i.src = src; cache[src] = i; } return cache[src]; };
  const tile = img(S.tile);

  function timeline(arm, anim) {
    const A = ARM[arm].anims[anim];
    const seg = []; let t = 0;
    A.ms.forEach((m, i) => { seg.push({ sheet: A.sheet, i, a: t, b: t + m, hit: A.hit === i }); t += m; });
    const tail = TAIL[anim];
    if (tail) {
      const idle = ARM[arm].anims.idle;
      if (anim === 'death') seg.push({ sheet: A.sheet, i: A.ms.length - 1, a: t, b: t + tail, tail: true });
      else seg.push({ sheet: idle ? idle.sheet : A.sheet, i: 0, a: t, b: t + tail, tail: true });
      t += tail;
    }
    return { seg, total: t, n: A.ms.length };
  }

  function Cell(cv) {
    this.cv = cv; this.ctx = cv.getContext('2d');
    this.arm = cv.dataset.arm; this.anim = cv.dataset.anim;
    this.tl = timeline(this.arm, this.anim);
    this.step = 0; this.label = cv.parentElement.querySelector('[data-frame]');
  }
  Cell.prototype.draw = function (seg, zoom, showTile) {
    const c = this.cv, x = this.ctx;
    if (c.width !== VW * zoom) { c.width = VW * zoom; c.height = VH * zoom; c.style.width = VW * zoom + 'px'; c.style.height = VH * zoom + 'px'; }
    x.imageSmoothingEnabled = false;
    x.clearRect(0, 0, c.width, c.height);
    if (showTile && tile.complete && tile.naturalWidth) {
      x.drawImage(tile, (80 - 96 - VX) * zoom, (126 - 54) * zoom, tile.naturalWidth * zoom, tile.naturalHeight * zoom);
      x.fillStyle = 'rgba(10,20,10,0.28)';
      x.beginPath(); x.ellipse((80 - VX) * zoom, 124 * zoom, 24 * zoom, 7 * zoom, 0, 0, Math.PI * 2); x.fill();
    }
    const im = img(seg.sheet);
    if (im.complete && im.naturalWidth) x.drawImage(im, seg.i * FW + VX, 0, VW, VH, 0, 0, VW * zoom, VH * zoom);
    if (this.label) {
      const A = ARM[this.arm].anims[this.anim];
      this.label.textContent = seg.tail ? (this.anim === 'death' ? 'hold' : 'idle') :
        `frame ${seg.i}/${A.ms.length - 1} · ${A.ms[seg.i]} ms${seg.hit ? ' · hit' : ''}`;
    }
  };
  Cell.prototype.at = function (t) {
    const tt = t % this.tl.total;
    return this.tl.seg.find((s) => tt >= s.a && tt < s.b) || this.tl.seg[0];
  };

  function Group(root) {
    this.root = root;
    this.cells = Array.from(root.querySelectorAll('canvas[data-arm]')).map((c) => new Cell(c));
    this.zoom = +(root.dataset.zoom || 2); this.tile = root.dataset.tile !== 'off';
    this.playing = !reduce; this.t0 = performance.now(); this.tp = 0;
    root.querySelectorAll('[data-ctl]').forEach((b) => b.addEventListener('click', () => this.ctl(b)));
    this.sync();
  }
  Group.prototype.ctl = function (b) {
    const k = b.dataset.ctl;
    if (k === 'play') { this.playing = !this.playing; if (this.playing) this.t0 = performance.now() - this.tp; }
    if (k === 'prev' || k === 'next') {
      if (this.playing) { this.playing = false; this.cells.forEach((c) => { c.step = c.at(this.tp).i; }); }
      this.cells.forEach((c) => { const n = c.tl.n; c.step = (c.step + (k === 'next' ? 1 : n - 1)) % n; });
    }
    if (k === 'restart') { this.t0 = performance.now(); this.tp = 0; this.playing = true; }
    if (k === 'zoom') this.zoom = +b.dataset.v;
    if (k === 'tile') this.tile = !this.tile;
    this.sync();
  };
  Group.prototype.sync = function () {
    this.root.querySelectorAll('[data-ctl="play"]').forEach((b) => { b.textContent = this.playing ? 'Pause' : 'Play'; b.setAttribute('aria-pressed', String(!this.playing)); });
    this.root.querySelectorAll('[data-ctl="zoom"]').forEach((b) => b.setAttribute('aria-pressed', String(+b.dataset.v === this.zoom)));
    this.root.querySelectorAll('[data-ctl="tile"]').forEach((b) => b.setAttribute('aria-pressed', String(this.tile)));
  };
  Group.prototype.frame = function (now) {
    if (this.playing) this.tp = now - this.t0;
    for (const c of this.cells) {
      const seg = this.playing ? c.at(this.tp) : (c.tl.seg.find((s) => s.i === c.step && !s.tail) || c.tl.seg[0]);
      c.draw(seg, this.zoom, this.tile);
    }
  };

  const groups = Array.from(document.querySelectorAll('[data-clock]')).map((g) => new Group(g));
  function loop(now) { groups.forEach((g) => g.frame(now)); requestAnimationFrame(loop); }
  requestAnimationFrame(loop);
  window.SPRINT_GROUPS = groups;

  // blind "which is less stiff?" pairs
  document.querySelectorAll('[data-quiz]').forEach((q) => {
    const out = q.querySelector('[data-reveal]');
    q.querySelectorAll('[data-pick]').forEach((b) => b.addEventListener('click', () => {
      q.querySelectorAll('[data-pick]').forEach((x) => x.setAttribute('aria-pressed', String(x === b)));
      const left = q.dataset.left, right = q.dataset.right;
      const nm = (id) => (ARM[id] ? ARM[id].name : id);
      out.hidden = false;
      out.textContent = `Left was ${nm(left)}; right was ${nm(right)}. You picked ${b.dataset.pick === 'L' ? nm(left) : nm(right)}.`;
      q.classList.add('answered');
    }));
  });
})();
