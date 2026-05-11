pub const DASHBOARD_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>ESP32 Security Cameras</title>
  <style>
    *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background: #0d0d0d;
      color: #ccc;
      font-family: 'Courier New', monospace;
      min-height: 100vh;
    }
    header {
      background: #1a1a1a;
      border-bottom: 1px solid #333;
      padding: 16px 24px;
      display: flex;
      align-items: center;
      gap: 12px;
    }
    header h1 { font-size: 1.1rem; color: #eee; letter-spacing: 0.05em; }
    #dot {
      width: 8px; height: 8px; border-radius: 50%;
      background: #555; flex-shrink: 0;
      transition: background 0.3s;
    }
    #dot.ok { background: #4ade80; box-shadow: 0 0 6px #4ade80; }
    main { padding: 24px; display: flex; flex-wrap: wrap; gap: 24px; align-items: flex-start; }
    #no-cameras { color: #555; font-size: 0.9rem; }
    .cam {
      background: #1a1a1a;
      border: 1px solid #2a2a2a;
      border-radius: 6px;
      overflow: hidden;
      width: 660px;
      max-width: 100%;
    }
    .cam-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 10px 14px;
      background: #1f1f1f;
      border-bottom: 1px solid #2a2a2a;
    }
    .cam-header h2 { font-size: 0.95rem; color: #ddd; }
    .cam-header h2 a { color: inherit; text-decoration: none; }
    .cam-header h2 a:hover { color: #fff; text-decoration: underline; }
    .badge {
      font-size: 0.7rem;
      font-weight: bold;
      letter-spacing: 0.08em;
      padding: 2px 8px;
      border-radius: 3px;
    }
    .badge.live   { background: #14532d; color: #4ade80; border: 1px solid #166534; }
    .badge.offline { background: #1f1f1f; color: #555; border: 1px solid #333; }
    .cam-img-wrap {
      position: relative;
      width: 100%;
      background: #111;
      line-height: 0;
    }
    .cam-img-wrap:not(.rot90):not(.rot270) img {
      width: 100%;
      height: auto;
      display: block;
    }
    .cam-footer {
      padding: 8px 14px;
      font-size: 0.78rem;
      color: #666;
      display: flex;
      flex-wrap: wrap;
      gap: 12px;
      border-top: 1px solid #222;
    }
    .cam-footer a { color: #555; text-decoration: none; }
    .cam-footer a:hover { color: #888; }
    .viewers { color: #888; }
    .viewer-ips { color: #555; word-break: break-all; }
    .img-ctrl-btn {
      background: none;
      border: 1px solid #444;
      color: #888;
      padding: 2px 8px;
      border-radius: 3px;
      cursor: pointer;
      font-family: 'Courier New', monospace;
      font-size: 0.75rem;
      line-height: 1.4;
    }
    .img-ctrl-btn:hover { color: #ccc; border-color: #666; }
    .img-ctrl-btn.active { color: #4ade80; border-color: #4ade80; }
    .img-ctrl-btn.danger { color: #f87171; border-color: #7f1d1d; }
    .img-ctrl-btn.danger:hover { color: #fca5a5; border-color: #f87171; }
    .cam-settings {
      background: #161616;
      border-bottom: 1px solid #2a2a2a;
      padding: 10px 14px;
      font-size: 0.78rem;
    }
    .setting-row {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-bottom: 6px;
      flex-wrap: wrap;
    }
    .setting-row:last-child { margin-bottom: 0; }
    .setting-row label { color: #888; min-width: 40px; }
    .name-input {
      background: #222;
      border: 1px solid #444;
      color: #ccc;
      padding: 2px 6px;
      border-radius: 3px;
      font-family: 'Courier New', monospace;
      font-size: 0.78rem;
      flex: 1;
      min-width: 100px;
    }
    .name-input:focus { outline: none; border-color: #666; }
    .num-input {
      background: #222;
      border: 1px solid #444;
      color: #ccc;
      padding: 2px 4px;
      border-radius: 3px;
      font-family: 'Courier New', monospace;
      font-size: 0.78rem;
      width: 60px;
    }
    .num-input:focus { outline: none; border-color: #666; }
    .cam-img-wrap {
      overflow: hidden;
    }
    .cam-img-wrap img {
      transition: transform 0.3s ease;
    }
    .cam-img-wrap.rot90, .cam-img-wrap.rot270 {
      aspect-ratio: 3/4;
      display: flex;
      align-items: center;
      justify-content: center;
    }
    .cam-img-wrap.rot90 img, .cam-img-wrap.rot270 img {
      width: auto;
      height: 100%;
      flex-shrink: 0;
    }
  </style>
</head>
<body>
  <header>
    <div id="dot"></div>
    <h1>ESP32 Security Cameras</h1>
  </header>
  <main id="main">
    <span id="no-cameras" style="display:none">No cameras connected yet.</span>
  </main>

  <script>
    const OFFLINE_SRC = `data:image/svg+xml,${encodeURIComponent(`
<svg xmlns="http://www.w3.org/2000/svg" width="640" height="480" viewBox="0 0 640 480">
  <rect width="640" height="480" fill="#111"/>
  <g transform="translate(320,240)" fill="none" stroke="#333" stroke-width="3" stroke-linecap="round">
    <rect x="-52" y="-36" width="104" height="80" rx="8"/>
    <circle cx="0" cy="-4" r="22"/>
    <rect x="30" y="-52" width="22" height="16" rx="4"/>
    <line x1="-70" y1="-70" x2="70" y2="70" stroke="#444" stroke-width="2"/>
  </g>
  <text x="320" y="310" text-anchor="middle" font-family="monospace" font-size="14" fill="#444">OFFLINE</text>
</svg>`)}`;

    const cards = {};

    async function fetchStatus() {
      try {
        const r = await fetch('/status.json');
        const data = await r.json();
        document.getElementById('dot').className = 'ok';
        updateUI(data.cameras);
      } catch (_) {
        document.getElementById('dot').className = '';
      }
    }

    function updateUI(cameras) {
      const main = document.getElementById('main');
      document.getElementById('no-cameras').style.display =
        cameras.length === 0 ? 'inline' : 'none';

      for (const cam of cameras) {
        if (!cards[cam.id]) {
          const card = document.createElement('div');
          card.className = 'cam';
          card.id = `cam-${cam.id}`;
          card.innerHTML = `
            <div class="cam-header">
              <h2 class="cam-title"><a id="title-${cam.id}" href="${fbUrl(cam.name || cam.id)}" target="_blank" rel="noopener" title="Open in filebrowser">${escHtml(cam.name || cam.id)}</a></h2>
              <div style="display:flex;gap:8px;align-items:center;">
                <button class="img-ctrl-btn" id="gear-${cam.id}" title="Settings">&#9881;</button>
                <span class="badge offline" id="badge-${cam.id}">OFFLINE</span>
              </div>
            </div>
            <div class="cam-img-wrap" id="wrap-${cam.id}">
              <img id="img-${cam.id}" src="${OFFLINE_SRC}" alt="${escHtml(cam.name || cam.id)}">
            </div>
            <div class="cam-settings" id="settings-${cam.id}" style="display:none">
              <div class="setting-row">
                <label>Name:</label>
                <input type="text" class="name-input" id="name-input-${cam.id}" value="${escHtml(cam.name || '')}" placeholder="Camera ID">
                <button class="img-ctrl-btn" onclick="saveName('${cam.id}')">Set</button>
              </div>
              <div class="setting-row">
                <span>Motion:</span>
                <button class="img-ctrl-btn" id="motion-btn-${cam.id}" onclick="toggleMotion('${cam.id}')">${cam.motion_enabled ? 'ON' : 'OFF'}</button>
                <span>Notify:</span>
                <button class="img-ctrl-btn" id="notify-btn-${cam.id}" onclick="toggleNotify('${cam.id}')">${cam.notifications_enabled ? 'ON' : 'OFF'}</button>
              </div>
              <div class="setting-row">
                <span>Rotate:</span>
                <button class="img-ctrl-btn" id="rot0-${cam.id}" onclick="setRotation('${cam.id}', 0)">0</button>
                <button class="img-ctrl-btn" id="rot90-${cam.id}" onclick="setRotation('${cam.id}', 90)">90</button>
                <button class="img-ctrl-btn" id="rot180-${cam.id}" onclick="setRotation('${cam.id}', 180)">180</button>
                <button class="img-ctrl-btn" id="rot270-${cam.id}" onclick="setRotation('${cam.id}', 270)">270</button>
                <button class="img-ctrl-btn${cam.mirror ? ' active' : ''}" id="mirror-btn-${cam.id}" onclick="toggleServerMirror('${cam.id}')">MIR</button>
              </div>
              <div class="setting-row">
                <label>Thresh:</label>
                <input type="number" class="num-input" id="thresh-${cam.id}" value="${cam.pixel_threshold}" min="0" max="255">
                <button class="img-ctrl-btn" onclick="setThreshold('${cam.id}')">Set</button>
                <label>%:</label>
                <input type="number" class="num-input" id="percent-${cam.id}" value="${cam.motion_percent}" min="0" step="0.1">
                <button class="img-ctrl-btn" onclick="setPercent('${cam.id}')">Set</button>
              </div>
              <div class="setting-row">
                <label>Timeout:</label>
                <input type="number" class="num-input" id="timeout-${cam.id}" value="${Math.round(cam.motion_timeout_ms / 1000)}" min="1">
                <button class="img-ctrl-btn" onclick="setTimeout('${cam.id}')">Set</button>
                <label>Every:</label>
                <input type="number" class="num-input" id="every-${cam.id}" value="${cam.motion_check_every}" min="1">
                <button class="img-ctrl-btn" onclick="setCheckEvery('${cam.id}')">Set</button>
              </div>
              <div class="setting-row">
                <button class="img-ctrl-btn danger" onclick="deleteCamera('${cam.id}')">Delete camera</button>
              </div>
            </div>
            <div class="cam-footer">
              <span class="viewers" id="viewers-${cam.id}">0 viewers</span>
              <span id="fps-${cam.id}"></span>
              <span class="viewer-ips" id="ips-${cam.id}"></span>
              <a href="/stream/${encodeURIComponent(cam.id)}">direct stream</a>
            </div>`;
          main.appendChild(card);
          cards[cam.id] = { active: null };
          applyServerTransform(cam.id, cam.rotation, cam.mirror);
          document.getElementById(`gear-${cam.id}`).addEventListener('click', () => toggleSettings(cam.id));
        }

        const img = document.getElementById(`img-${cam.id}`);
        const badge = document.getElementById(`badge-${cam.id}`);
        const viewersEl = document.getElementById(`viewers-${cam.id}`);
        const ipsEl = document.getElementById(`ips-${cam.id}`);
        const titleEl = document.getElementById(`title-${cam.id}`);
        if (titleEl) {
          titleEl.textContent = cam.name || cam.id;
          titleEl.href = fbUrl(cam.name || cam.id);
        }
        const nameInput = document.getElementById(`name-input-${cam.id}`);
        if (nameInput && !nameInput.matches(':focus')) nameInput.value = cam.name || '';
        const motionBtn = document.getElementById(`motion-btn-${cam.id}`);
        if (motionBtn) { motionBtn.textContent = cam.motion_enabled ? 'ON' : 'OFF'; motionBtn.classList.toggle('active', cam.motion_enabled); }
        const notifyBtn = document.getElementById(`notify-btn-${cam.id}`);
        if (notifyBtn) { notifyBtn.textContent = cam.notifications_enabled ? 'ON' : 'OFF'; notifyBtn.classList.toggle('active', cam.notifications_enabled); }
        const mirrorBtn = document.getElementById(`mirror-btn-${cam.id}`);
        if (mirrorBtn) mirrorBtn.classList.toggle('active', cam.mirror);
        [0, 90, 180, 270].forEach(r => {
          const btn = document.getElementById(`rot${r}-${cam.id}`);
          if (btn) btn.classList.toggle('active', cam.rotation === r);
        });
        applyServerTransform(cam.id, cam.rotation, cam.mirror);

        if (cam.active !== cards[cam.id].active) {
          if (cam.active) {
            startStream(cam.id);
            badge.textContent = 'LIVE';
            badge.className = 'badge live';
          } else {
            img.src = OFFLINE_SRC;
            badge.textContent = 'OFFLINE';
            badge.className = 'badge offline';
          }
          cards[cam.id].active = cam.active;
        }

        const vc = cam.viewer_count;
        viewersEl.textContent = `${vc} viewer${vc !== 1 ? 's' : ''}`;
        const fpsEl = document.getElementById(`fps-${cam.id}`);
        if (fpsEl) fpsEl.textContent = cam.active && cam.fps > 0 ? `${cam.fps.toFixed(1)} fps` : '';
        ipsEl.textContent = cam.viewers.length > 0 ? cam.viewers.join(', ') : '';
      }
    }

    function applyServerTransform(id, rotation, mirror) {
      const img = document.getElementById(`img-${id}`);
      const wrap = document.getElementById(`wrap-${id}`);
      if (!img || !wrap) return;
      img.style.transform = `rotate(${rotation}deg) scaleX(${mirror ? -1 : 1})`;
      wrap.classList.toggle('rot90', rotation === 90);
      wrap.classList.toggle('rot270', rotation === 270);
    }

    function streamUrl(id) {
      return `/stream/${encodeURIComponent(id)}?t=${Date.now()}`;
    }

    function startStream(id) {
      const img = document.getElementById(`img-${id}`);
      img.onerror = null;
      img.src = streamUrl(id);
      img.onerror = () => {
        if (cards[id]?.active) {
          setTimeout(() => { if (cards[id]?.active) startStream(id); }, 3000);
        }
      };
    }

    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState !== 'visible') return;
      for (const [id, state] of Object.entries(cards)) {
        if (state.active) startStream(id);
      }
    });

    function escHtml(s) {
      return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
    }

    function sanitizeDir(name) {
      const s = (name || '').replace(/[^\p{L}\p{N}._-]/gu, '_').replace(/^_+|_+$/g, '');
      return s || '_';
    }

    function fbUrl(name) {
      return `/fb/files/${encodeURIComponent(sanitizeDir(name))}/`;
    }

    function toggleSettings(id) {
      const el = document.getElementById(`settings-${id}`);
      if (el) el.style.display = el.style.display === 'none' ? 'block' : 'none';
    }

    async function apiCall(endpoint, method, body) {
      try {
        await fetch(endpoint, {
          method,
          headers: body ? {'Content-Type': 'application/json'} : undefined,
          body: body ? JSON.stringify(body) : undefined,
        });
      } catch (_) {}
    }

    function saveName(id) {
      const input = document.getElementById(`name-input-${id}`);
      if (!input) return;
      const name = input.value.trim();
      apiCall(`/api/camera/${encodeURIComponent(id)}/config`, 'PATCH', { name });
    }

    function toggleMotion(id) {
      const btn = document.getElementById(`motion-btn-${id}`);
      const isOn = btn && btn.textContent === 'ON';
      apiCall(`/api/camera/${encodeURIComponent(id)}/config`, 'PATCH', { motion_enabled: !isOn });
    }

    function toggleNotify(id) {
      const btn = document.getElementById(`notify-btn-${id}`);
      const isOn = btn && btn.textContent === 'ON';
      apiCall(`/api/camera/${encodeURIComponent(id)}/config`, 'PATCH', { notifications_enabled: !isOn });
    }

    function setRotation(id, rot) {
      apiCall(`/api/camera/${encodeURIComponent(id)}/config`, 'PATCH', { rotation: rot });
    }

    function toggleServerMirror(id) {
      const btn = document.getElementById(`mirror-btn-${id}`);
      const isMirror = btn && btn.classList.contains('active');
      apiCall(`/api/camera/${encodeURIComponent(id)}/config`, 'PATCH', { mirror: !isMirror });
    }

    function setThreshold(id) {
      const val = parseInt(document.getElementById(`thresh-${id}`).value);
      if (!isNaN(val)) apiCall(`/api/camera/${encodeURIComponent(id)}/config`, 'PATCH', { pixel_threshold: val });
    }

    function setPercent(id) {
      const val = parseFloat(document.getElementById(`percent-${id}`).value);
      if (!isNaN(val)) apiCall(`/api/camera/${encodeURIComponent(id)}/config`, 'PATCH', { motion_percent: val });
    }

    function setTimeout(id) {
      const val = parseInt(document.getElementById(`timeout-${id}`).value);
      if (!isNaN(val)) apiCall(`/api/camera/${encodeURIComponent(id)}/config`, 'PATCH', { motion_timeout_ms: val * 1000 });
    }

    function setCheckEvery(id) {
      const val = parseInt(document.getElementById(`every-${id}`).value);
      if (!isNaN(val)) apiCall(`/api/camera/${encodeURIComponent(id)}/config`, 'PATCH', { motion_check_every: val });
    }

    async function deleteCamera(id) {
      const label = document.getElementById(`title-${id}`)?.textContent || id;
      if (!confirm(`Delete camera "${label}"?\n\nThis removes its config from the database. Saved frames on disk are not touched.\nIf the device keeps posting, it will reappear with default settings.`)) return;
      try {
        const r = await fetch(`/api/camera/${encodeURIComponent(id)}`, { method: 'DELETE' });
        if (!r.ok) { alert(`Delete failed: ${r.status}`); return; }
        document.getElementById(`cam-${id}`)?.remove();
        delete cards[id];
      } catch (e) {
        alert(`Delete failed: ${e}`);
      }
    }

    setInterval(fetchStatus, 2000);
    fetchStatus();
  </script>
</body>
</html>"##;
