const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const $ = (id) => document.getElementById(id);
let state = null;
let pairTimer = null;

// ---------------------------------------------------------------- utilidades

const ICON_DIR = '<svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor"><path d="M3 6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>';
const ICON_FILE = '<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M7 3h7l5 5v13H7z"/><path d="M14 3v5h5"/></svg>';

// Robot de Android para los .apk
const ICON_APK = '<svg viewBox="0 0 24 24" width="18" height="18" fill="currentColor"><path d="M6 18a1 1 0 0 0 1 1h1v3a1.2 1.2 0 0 0 2.4 0v-3h3.2v3a1.2 1.2 0 0 0 2.4 0v-3h1a1 1 0 0 0 1-1V8H6zM3.2 8.2a1.2 1.2 0 0 0-1.2 1.2v5.4a1.2 1.2 0 0 0 2.4 0V9.4a1.2 1.2 0 0 0-1.2-1.2zm17.6 0a1.2 1.2 0 0 0-1.2 1.2v5.4a1.2 1.2 0 0 0 2.4 0V9.4a1.2 1.2 0 0 0-1.2-1.2zM15.5 2.8l1.1-1.7a.3.3 0 0 0-.5-.3L14.9 2.6a6 6 0 0 0-5.8 0L7.9.8a.3.3 0 0 0-.5.3l1.1 1.7A5.6 5.6 0 0 0 6 7h12a5.6 5.6 0 0 0-2.5-4.2zM9.5 5.5a.7.7 0 1 1 0-1.4.7.7 0 0 1 0 1.4zm5 0a.7.7 0 1 1 0-1.4.7.7 0 0 1 0 1.4z"/></svg>';

function fmtSize(n) {
  if (n < 1024) return n + ' B';
  const u = ['KB', 'MB', 'GB', 'TB'];
  let i = -1;
  do { n /= 1024; i++; } while (n >= 1024 && i < u.length - 1);
  return (n >= 100 ? n.toFixed(0) : n.toFixed(1)) + ' ' + u[i];
}

function fmtDate(ms) {
  if (!ms) return '';
  const d = new Date(ms);
  const p = (x) => String(x).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

// Diálogos propios: los del navegador no son fiables dentro de la app.
function dialog({ title, text = '', input = null, ok = 'Aceptar', cancel = true }) {
  return new Promise((resolve) => {
    $('modal-title').textContent = title;
    $('modal-text').textContent = text;
    $('modal-cancel').classList.toggle('hidden', !cancel);
    const field = $('modal-input');
    field.classList.toggle('hidden', input === null);
    field.value = input ?? '';
    $('modal-ok').textContent = ok;
    $('modal').classList.remove('hidden');
    if (input !== null) { field.focus(); field.select(); }
    const done = (v) => {
      $('modal').classList.add('hidden');
      $('modal-ok').onclick = $('modal-cancel').onclick = field.onkeydown = null;
      field.value = '';
      resolve(v);
    };
    $('modal-ok').onclick = () => done(input === null ? true : field.value.trim());
    $('modal-cancel').onclick = () => done(null);
    field.onkeydown = (e) => { if (e.key === 'Enter') $('modal-ok').click(); if (e.key === 'Escape') done(null); };
  });
}

// ---------------------------------------------------------------- paneles de archivos

const pcAdapter = {
  list: (p) => invoke('local_ls', { path: p }),
  join: (b, n) => (b === '' ? n + '\\' : b.endsWith('\\') ? b + n : b + '\\' + n),
  label: (p) => (p === '' ? 'Unidades' : p),
  foot: () => '',
};

const phoneAdapter = {
  list: async (p) => {
    const r = await invoke('phone_ls', { path: p });
    const parent = r.path === '' ? null : r.path.includes('/') ? r.path.slice(0, r.path.lastIndexOf('/')) : '';
    return { path: r.path, parent, items: r.items, extra: { free: r.free, total: r.total } };
  },
  join: (b, n) => (b ? b + '/' + n : n),
  label: (p) => (p ? 'Almacenamiento interno/' + p : 'Almacenamiento interno'),
  foot: (x) => (x ? `${fmtSize(x.free)} libres de ${fmtSize(x.total)}` : ''),
};

let showHidden = false;

class Pane {
  constructor(el, adapter) {
    this.el = el;
    this.a = adapter;
    this.listEl = el.querySelector('[data-role=list]');
    this.pathEl = el.querySelector('[data-role=path]');
    this.footEl = el.querySelector('[data-role=foot]');
    this.reset();
  }

  reset(msg = '') {
    this.seq = (this.seq || 0) + 1; // invalida cualquier carga en curso
    this.path = null;
    this.parent = null;
    this.items = [];
    this.extra = null;
    this.sel = new Set();
    this.err = msg;
  }

  async load(path) {
    const my = ++this.seq;
    try {
      const r = await this.a.list(path);
      if (my !== this.seq) return;
      this.path = r.path;
      this.parent = r.parent;
      this.items = r.items;
      this.extra = r.extra || null;
      this.sel = new Set();
      this.err = '';
    } catch (e) {
      if (my !== this.seq) return;
      this.err = String(e);
    }
    this.render();
  }

  clear(msg = '') {
    this.reset(msg);
    this.render();
  }

  refresh() { if (this.path !== null) return this.load(this.path); }
  up() { if (this.parent !== null && this.parent !== undefined) return this.load(this.parent); }

  selectedItems() {
    return this.items
      .filter((it) => this.sel.has(it.name))
      .map((it) => ({ path: this.a.join(this.path, it.name), name: it.name, dir: it.dir, size: it.size }));
  }

  // Los archivos que empiezan por punto son ocultos (índices y cachés de las apps): no se muestran salvo que se pida.
  visible() {
    return showHidden ? this.items : this.items.filter((it) => !it.name.startsWith('.'));
  }

  render() {
    const shown = this.visible();
    const hiddenCount = this.items.length - shown.length;
    this.pathEl.textContent = this.path === null ? '' : this.a.label(this.path);
    this.footEl.textContent = this.err || (this.path === null ? '' : `${shown.length} elementos${hiddenCount ? ` (${hiddenCount} ocultos)` : ''}  ${this.a.foot(this.extra)}`);
    this.footEl.style.color = this.err ? 'var(--bad)' : '';
    this.listEl.replaceChildren();
    if (!shown.length) {
      const li = document.createElement('li');
      li.className = 'empty';
      li.textContent = this.err ? '' : this.path === null ? 'Sin datos' : 'Carpeta vacía';
      this.listEl.append(li);
      return;
    }
    for (const it of shown) {
      const li = document.createElement('li');
      li.className = 'row' + (it.dir ? ' isdir' : '') + (this.sel.has(it.name) ? ' sel' : '');
      li.insertAdjacentHTML('afterbegin', it.dir ? ICON_DIR : /\.apk$/i.test(it.name) ? ICON_APK : ICON_FILE);
      const name = document.createElement('span');
      name.className = 'name';
      name.textContent = it.name;
      const meta = document.createElement('span');
      meta.className = 'meta';
      meta.textContent = [it.dir ? '' : fmtSize(it.size), fmtDate(it.mtime)].filter(Boolean).join('   ');
      li.append(name, meta);
      li.onclick = () => {
        if (this.sel.has(it.name)) this.sel.delete(it.name); else this.sel.add(it.name);
        li.classList.toggle('sel');
      };
      li.ondblclick = () => { if (it.dir) this.load(this.a.join(this.path, it.name)); };
      this.listEl.append(li);
    }
  }
}

const pcPane = new Pane($('pane-pc'), pcAdapter);
const phonePane = new Pane($('pane-phone'), phoneAdapter);

for (const [pane, root] of [[pcPane, $('pane-pc')], [phonePane, $('pane-phone')]]) {
  root.addEventListener('click', (e) => {
    const b = e.target.closest('button[data-act]');
    if (!b) return;
    if (b.dataset.act === 'up') pane.up();
    if (b.dataset.act === 'refresh') pane.refresh();
  });
}

// Acciones sobre el panel del móvil (columna central).
async function phoneAction(fn) {
  if (phonePane.path === null) return alertMsg('El móvil no está conectado.');
  try { await fn(); } catch (err) { alertMsg(String(err)); }
}

$('phone-mkdir').onclick = () => phoneAction(async () => {
  const name = await dialog({ title: 'Carpeta nueva en el móvil', input: '' });
  if (name) { await invoke('phone_mkdir', { path: phoneAdapter.join(phonePane.path, name) }); await phonePane.refresh(); }
});

$('phone-rename').onclick = () => phoneAction(async () => {
  const sel = phonePane.selectedItems();
  if (sel.length !== 1) return alertMsg('Selecciona un solo elemento del móvil para renombrar.');
  const name = await dialog({ title: 'Renombrar', input: sel[0].name });
  if (name && name !== sel[0].name) { await invoke('phone_rename', { path: sel[0].path, name }); await phonePane.refresh(); }
});

$('phone-delete').onclick = () => phoneAction(async () => {
  const sel = phonePane.selectedItems();
  if (!sel.length) return alertMsg('Selecciona qué eliminar del móvil.');
  const ok = await dialog({
    title: 'Eliminar del móvil',
    text: `Se eliminarán ${sel.length} elemento(s) del móvil. No se puede deshacer.`,
    ok: 'Eliminar',
  });
  if (ok) { await invoke('phone_delete', { paths: sel.map((s) => s.path) }); await phonePane.refresh(); }
});

function alertMsg(text) {
  return dialog({ title: 'SyncroUmbral', text, ok: 'Entendido', cancel: false });
}

// ---------------------------------------------------------------- transferencias

const jobTarget = new Map(); // job -> panel que hay que actualizar al terminar

$('to-phone').onclick = async () => {
  const items = pcPane.selectedItems();
  if (!items.length) return alertMsg('Selecciona archivos o carpetas del PC.');
  if (phonePane.path === null) return alertMsg('El móvil no está conectado.');
  try { jobTarget.set(await invoke('transfer_upload', { items, remoteDir: phonePane.path }), phonePane); }
  catch (e) { alertMsg(String(e)); }
};

$('to-pc').onclick = async () => {
  const items = phonePane.selectedItems();
  if (!items.length) return alertMsg('Selecciona archivos o carpetas del móvil.');
  if (pcPane.path === null || pcPane.path === '') return alertMsg('Abre primero una carpeta del PC como destino.');
  try { jobTarget.set(await invoke('transfer_download', { items, localDir: pcPane.path }), pcPane); }
  catch (e) { alertMsg(String(e)); }
};

function renderTransfer(p) {
  // Un evento tardío (por ejemplo "cancelado" justo al bloquear) no debe dejar nombres en pantalla.
  if (!state || state.locked) return;
  let el = document.getElementById('job-' + p.job);
  if (!el) {
    el = document.createElement('div');
    el.id = 'job-' + p.job;
    el.className = 'tr';
    el.innerHTML = '<div class="tr-top"><span class="tr-name"></span><span class="tr-info"></span><button class="small tr-cancel">Cancelar</button></div><div class="bar"><i></i></div>';
    el.querySelector('.tr-cancel').onclick = () => invoke('cancel_transfers');
    $('transfers').prepend(el);
  }
  el.querySelector('.tr-cancel').classList.toggle('hidden', p.state !== 'planning' && p.state !== 'running');
  el.className = 'tr ' + p.state;
  el.querySelector('.tr-name').textContent = p.label;
  const pct = p.total ? Math.min(100, (p.done / p.total) * 100) : p.state === 'done' ? 100 : 0;
  el.querySelector('.bar > i').style.width = pct + '%';
  const info = {
    planning: 'Preparando',
    running: `${fmtSize(p.done)} de ${fmtSize(p.total)}`,
    done: 'Completado',
    cancelled: 'Cancelado',
    error: 'Error: ' + p.msg,
  }[p.state];
  el.querySelector('.tr-info').textContent = info;
  if (p.state === 'done') {
    (jobTarget.get(p.job) || pcPane).refresh();
    jobTarget.delete(p.job);
  }
}

listen('transfer', (e) => renderTransfer(e.payload));

// Arrastrar archivos desde el Explorador de Windows y soltarlos sobre el panel del móvil.
function overPhonePane(pos) {
  if (!pos) return false;
  const r = $('pane-phone').getBoundingClientRect();
  const x = pos.x / window.devicePixelRatio, y = pos.y / window.devicePixelRatio;
  return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
}
listen('tauri://drag-over', (e) => $('pane-phone').classList.toggle('drop', overPhonePane(e.payload.position)));
listen('tauri://drag-leave', () => $('pane-phone').classList.remove('drop'));
listen('tauri://drag-drop', async (e) => {
  $('pane-phone').classList.remove('drop');
  if (!state || state.locked || $('tab-files').classList.contains('hidden')) return;
  if (!overPhonePane(e.payload.position) && !e.payload.__test) return;
  if (phonePane.path === null) return alertMsg('El móvil no está conectado.');
  try {
    const items = await invoke('local_stat', { paths: e.payload.paths });
    if (!items.length) return;
    jobTarget.set(await invoke('transfer_upload', { items, remoteDir: phonePane.path }), phonePane);
  } catch (err) {
    alertMsg(String(err));
  }
});

// ---------------------------------------------------------------- aplicaciones

let apps = [];
let appSel = null;
let appsLoaded = false;
let iconRun = 0;
const icons = new Map();

function appMsg(text, bad = false) {
  $('apps-msg').textContent = text;
  $('apps-msg').style.color = bad ? 'var(--bad)' : '';
}

async function loadApps() {
  appMsg('Cargando aplicaciones del móvil');
  try {
    const r = await invoke('apps_list');
    apps = r.apps;
    appsLoaded = true;
    appSel = null;
    appMsg('');
    renderApps();
    loadIcons();
  } catch (e) {
    appMsg(String(e), true);
  }
}

function renderApps() {
  const q = $('apps-search').value.trim().toLowerCase();
  const showSystem = $('apps-system').checked;
  const list = apps.filter((a) => (showSystem || !a.system) && (!q || a.label.toLowerCase().includes(q) || a.pkg.toLowerCase().includes(q)));
  $('apps-count').textContent = apps.length ? `${list.length} de ${apps.length}` : '';
  const ul = $('apps-list');
  ul.replaceChildren();
  if (!list.length) {
    const li = document.createElement('li');
    li.className = 'empty';
    li.textContent = apps.length ? 'Ninguna coincide' : 'Sin datos';
    ul.append(li);
    return;
  }
  for (const a of list) {
    const li = document.createElement('li');
    li.className = 'app-row' + (a.pkg === appSel ? ' sel' : '');
    li.dataset.pkg = a.pkg;
    const img = document.createElement('img');
    img.className = 'app-icon';
    img.alt = '';
    if (icons.has(a.pkg)) img.src = 'data:image/png;base64,' + icons.get(a.pkg);
    const main = document.createElement('div');
    main.className = 'app-main';
    const name = document.createElement('div');
    name.className = 'app-name';
    name.textContent = a.label;
    if (a.system) { const b = document.createElement('span'); b.className = 'badge'; b.textContent = 'Sistema'; name.append(b); }
    if (!a.enabled) { const b = document.createElement('span'); b.className = 'badge'; b.textContent = 'Desactivada'; name.append(b); }
    const pkg = document.createElement('div');
    pkg.className = 'app-pkg';
    pkg.textContent = a.pkg;
    main.append(name, pkg);
    const meta = document.createElement('div');
    meta.className = 'app-meta';
    meta.textContent = `${a.version || '-'}  ·  ${fmtSize(a.size)}  ·  act. ${fmtDate(a.updated).slice(0, 10)}`;
    li.append(img, main, meta);
    li.onclick = () => {
      appSel = a.pkg;
      for (const el of ul.children) el.classList.toggle('sel', el.dataset.pkg === appSel);
    };
    ul.append(li);
  }
}

// Los iconos se piden por lotes después de pintar la lista, primero las apps del usuario.
async function loadIcons() {
  const run = ++iconRun;
  const order = [...apps].sort((a, b) => Number(a.system) - Number(b.system)).map((a) => a.pkg).filter((p) => !icons.has(p));
  for (let i = 0; i < order.length; i += 40) {
    if (run !== iconRun || !state || state.locked) return;
    try {
      const r = await invoke('apps_icons', { pkgs: order.slice(i, i + 40) });
      if (run !== iconRun || !state || state.locked) return;
      for (const [pkg, b64] of Object.entries(r.icons)) {
        icons.set(pkg, b64);
        const img = $('apps-list').querySelector(`[data-pkg="${CSS.escape(pkg)}"] img`);
        if (img) img.src = 'data:image/png;base64,' + b64;
      }
    } catch {
      return;
    }
  }
}

// Marca con "En el PC" las apps que tienen ventana abierta.
let openApps = new Set();
async function refreshOpenApps() {
  if (!state || state.locked || $('tab-apps').classList.contains('hidden')) return;
  try {
    openApps = new Set(await invoke('app_windows'));
  } catch {
    return;
  }
  for (const row of $('apps-list').querySelectorAll('.app-row')) {
    const name = row.querySelector('.app-name');
    let badge = name.querySelector('.badge.pc');
    if (openApps.has(row.dataset.pkg) && !badge) {
      badge = document.createElement('span');
      badge.className = 'badge pc';
      badge.textContent = 'En el PC';
      name.append(badge);
    } else if (!openApps.has(row.dataset.pkg) && badge) {
      badge.remove();
    }
  }
}
setInterval(refreshOpenApps, 3000);

function clearApps() {
  openApps = new Set();
  iconRun++;
  apps = [];
  appSel = null;
  appsLoaded = false;
  icons.clear();
  $('apps-list').replaceChildren();
  $('apps-search').value = '';
  $('apps-count').textContent = '';
  appMsg('');
}

$('apps-search').oninput = renderApps;
$('apps-system').onchange = renderApps;
$('apps-refresh').onclick = loadApps;

for (const btn of document.querySelectorAll('[data-app]')) {
  btn.onclick = async () => {
    const app = apps.find((a) => a.pkg === appSel);
    if (!app) return appMsg('Selecciona una aplicación de la lista.', true);
    const action = btn.dataset.app;
    if (app.system && (action === 'uninstall' || action === 'clear')) {
      return appMsg('Por seguridad no se permite desinstalar ni borrar datos de apps del sistema desde aquí.', true);
    }
    if (action === 'uninstall' || action === 'clear') {
      const ok = await dialog({
        title: action === 'uninstall' ? 'Desinstalar aplicación' : 'Borrar datos de la aplicación',
        text: action === 'uninstall'
          ? `Se desinstalará "${app.label}" del móvil junto con sus datos.`
          : `Se borrarán todos los datos de "${app.label}" (cuentas, ajustes, archivos internos). No se puede deshacer.`,
        ok: action === 'uninstall' ? 'Desinstalar' : 'Borrar datos',
      });
      if (!ok) return;
    }
    let localDir = null;
    if (action === 'pull') {
      if (pcPane.path === null || pcPane.path === '') return appMsg('Abre primero una carpeta del PC en la pestaña Archivos: ahí se guardará el APK.', true);
      localDir = pcPane.path;
    }
    appMsg(action === 'pc' ? 'Abriendo la app en el PC (unos segundos)' : 'Trabajando');
    try {
      const msg = action === 'pc'
        ? await invoke('app_open_pc', { pkg: app.pkg, label: app.label })
        : await invoke('app_action', { pkg: app.pkg, action, localDir });
      if (action === 'pc') refreshOpenApps();
      appMsg(msg);
      if (action === 'uninstall') await loadApps();
    } catch (e) {
      appMsg(String(e), true);
    }
  };
}

// ---------------------------------------------------------------- mensajería web
// Cada servicio se muestra DENTRO de esta ventana: SyncroUmbral coloca un webview hijo sobre el rectángulo #web-stage.

let webCurrent = null;
let webServices = [];

function stageRect() {
  const r = $('web-stage').getBoundingClientRect();
  return { x: r.left, y: r.top, w: r.width, height: r.height };
}

async function placeWeb() {
  if (!webCurrent || $('tab-web').classList.contains('hidden') || !state || state.locked) return;
  try { await invoke('web_show', { id: webCurrent, ...stageRect() }); $('web-msg').textContent = ''; }
  catch (e) { $('web-msg').textContent = String(e); }
}

async function loadWeb() {
  if (!state || state.locked) return;
  try { webServices = await invoke('web_list'); } catch { return; }
  const tabs = $('web-tabs');
  tabs.replaceChildren();
  for (const s of webServices) {
    const b = document.createElement('button');
    b.className = 'small wtab' + (s.id === webCurrent ? ' active' : '');
    const dot = document.createElement('span');
    dot.className = 'wdot' + (s.open ? ' open' : s.has_session ? ' saved' : '');
    dot.title = s.open ? 'Cargado' : s.has_session ? 'Sesión guardada' : 'Sin iniciar sesión';
    b.append(dot, document.createTextNode(s.name));
    b.onclick = () => selectWeb(s.id);
    tabs.append(b);
  }
  const cur = webServices.find((s) => s.id === webCurrent);
  $('web-forget').classList.toggle('hidden', !(cur && cur.has_session));
}

async function selectWeb(id) {
  webCurrent = id;
  $('web-stage').classList.add('live');
  $('web-hint').classList.add('hidden');
  await placeWeb();
  loadWeb();
}

function resetWebUi() {
  webCurrent = null;
  $('web-tabs').replaceChildren();
  $('web-stage').classList.remove('live');
  $('web-hint').classList.remove('hidden');
  $('web-forget').classList.add('hidden');
  $('web-msg').textContent = '';
}

$('web-forget').onclick = async () => {
  const s = webServices.find((x) => x.id === webCurrent);
  if (!s) return;
  await invoke('web_hide'); // el diálogo de SyncroUmbral quedaría tapado por el servicio
  const ok = await dialog({
    title: `Olvidar la sesión de ${s.name}`,
    text: 'Se cerrará y se borrará la sesión guardada en este PC. La próxima vez tendrás que iniciar sesión de nuevo.',
    ok: 'Olvidar',
  });
  if (ok) {
    try { await invoke('web_forget', { id: s.id }); resetWebUi(); }
    catch (e) { $('web-msg').textContent = String(e); }
  }
  placeWeb();
  loadWeb();
};

new ResizeObserver(() => placeWeb()).observe($('web-stage'));
window.addEventListener('resize', placeWeb);
setInterval(() => { if (!$('tab-web').classList.contains('hidden')) loadWeb(); }, 3000);

// ---------------------------------------------------------------- notificaciones

const notifsList = $('notifs-list');
let notifGranted = null;

// Pantalla de llamada: aparece mientras haya una llamada en curso (la leemos de las tarjetas de notificación) y se puede ocultar.
const dismissedCalls = new Set();
let callShown = false;
function updateCallScreen() {
  const card = state && !state.locked ? notifsList.querySelector('.notif.call:not(.incoming)') : null;
  const key = card?.dataset.key;
  const show = !!card && !dismissedCalls.has(key);
  if (show) {
    $('call-app').textContent = card.querySelector('.n-app')?.textContent || '';
    $('call-name').textContent = card.querySelector('.n-title')?.textContent || 'Llamada en curso';
    $('call-timer').dataset.key = key;
    const start = NotifUI.callStart.get(key);
    $('call-timer').textContent = NotifUI.fmtDur(start ? Date.now() - start : 0);
    $('call-screen').dataset.key = key;
  }
  if (show === callShown) return;
  callShown = show;
  $('call-screen').classList.toggle('hidden', !show);
  $('call-msg').textContent = '';
  if (show) invoke('web_hide'); // un servicio web integrado taparía la pantalla
  else if (!$('tab-web').classList.contains('hidden')) placeWeb();
}
$('call-hang').onclick = async () => {
  $('call-msg').textContent = 'Un momento';
  try { await invoke('call_end', { key: $('call-screen').dataset.key }); $('call-msg').textContent = ''; }
  catch (e) { $('call-msg').textContent = String(e); }
};
$('call-hide').onclick = () => { dismissedCalls.add($('call-screen').dataset.key); updateCallScreen(); };

function notifRefresh() {
  updateCallScreen();
  const n = notifsList.children.length;
  $('notif-count').textContent = n;
  $('notif-count').classList.toggle('hidden', !n);
  $('notifs-hint').textContent = notifGranted === false
    ? 'El móvil no ha dado acceso a las notificaciones. En el móvil abre SyncroUmbral y pulsa "Permitir acceso a las notificaciones".'
    : n ? '' : 'No hay notificaciones.';
}

function setNotifs(items, granted) {
  notifGranted = granted;
  notifsList.replaceChildren(...items.map((n) => NotifUI.card(n)));
  notifRefresh();
}

async function loadNotifs() {
  try { const r = await invoke('notifs_list'); setNotifs(r.items, r.granted); } catch { /* bloqueada o sin móvil */ }
  loadMuted();
}

// Apps silenciadas: se pueden volver a activar desde aquí.
async function loadMuted() {
  const bar = $('muted-bar');
  let list = [];
  try { list = await invoke('notif_muted'); } catch { /* sin datos */ }
  bar.replaceChildren();
  bar.classList.toggle('hidden', !list.length);
  if (!list.length) return;
  bar.append(document.createTextNode('Silenciadas: '));
  for (const m of list) {
    const chip = document.createElement('span');
    chip.className = 'chip';
    chip.append(document.createTextNode(m.app));
    const b = document.createElement('button');
    b.textContent = 'Activar';
    b.onclick = async () => { await invoke('notif_unmute', { pkg: m.pkg }); loadMuted(); };
    chip.append(b);
    bar.append(chip);
  }
}

// Texto del PC al portapapeles del móvil.
async function sendClip() {
  const text = $('clip-text').value;
  if (!text.trim()) return;
  try {
    await invoke('phone_clip_set', { text });
    $('clip-msg').textContent = 'Copiado en el móvil';
    $('clip-text').value = '';
  } catch (e) {
    $('clip-msg').textContent = String(e);
  }
  setTimeout(() => { $('clip-msg').textContent = ''; }, 4000);
}
$('show-hidden').onchange = () => { showHidden = $('show-hidden').checked; pcPane.render(); phonePane.render(); };

$('notifs-clear').onclick = async () => {
  try {
    const n = await invoke('notif_clear_all');
    $('notifs-clear-msg').textContent = n ? `${n} descartadas en el móvil` : 'Nada que descartar';
  } catch (e) {
    $('notifs-clear-msg').textContent = String(e);
  }
  setTimeout(() => { $('notifs-clear-msg').textContent = ''; }, 4000);
};

$('clip-send').onclick = sendClip;
$('clip-text').onkeydown = (e) => { if (e.key === 'Enter') sendClip(); };

function clearNotifsUI() {
  notifGranted = null;
  notifsList.replaceChildren();
  $('notif-count').classList.add('hidden');
  $('notifs-hint').textContent = '';
  dismissedCalls.clear();
  updateCallScreen();
}

listen('notif', (e) => {
  if (!state || state.locked) return;
  NotifUI.replaceCard(notifsList, e.payload.n);
  notifRefresh();
});
listen('notif_gone', (e) => { NotifUI.removeCard(notifsList, e.payload.key); notifRefresh(); loadMuted(); });
listen('notif_sync', (e) => { if (state && !state.locked) setNotifs(e.payload.items, e.payload.granted); });
listen('notif_clear', clearNotifsUI);

// ---------------------------------------------------------------- estado y bloqueo

function hidePairing() {
  clearInterval(pairTimer);
  $('qr').innerHTML = '';
  $('payload').value = '';
  $('pair-box').classList.add('hidden');
}

// Borra de la pantalla todo lo que haya mostrado datos del usuario.
function wipeDom() {
  hidePairing();
  pcPane.clear();
  phonePane.clear();
  clearApps();
  clearNotifsUI();
  $('tools-list').replaceChildren();
  for (const id of ['tools-msg', 'wpair-msg']) $(id).textContent = '';
  for (const id of ['wpair-addr', 'wpair-code']) $(id).value = '';
  $('muted-bar').replaceChildren();
  $('clip-text').value = '';
  $('clip-msg').textContent = '';
  resetWebUi();
  $('transfers').replaceChildren();
  jobTarget.clear();
  $('modal').classList.add('hidden');
  $('modal-input').value = '';
  $('devices').innerHTML = '';
  $('places').replaceChildren();
  $('status-name').textContent = '';
  $('status-sub').textContent = '';
  $('cable-msg').textContent = '';
  for (const id of ['setup-pw', 'setup-pw2', 'lock-pw']) $(id).value = '';
  for (const id of ['setup-err', 'lock-err']) $(id).textContent = '';
}

function show(view) {
  for (const v of ['setup', 'lock', 'main']) $('view-' + v).classList.toggle('hidden', v !== view);
}

function render(s) {
  if (!s.has_password) return show('setup');
  if (s.locked) {
    show('lock');
    $('lock-hint').textContent = s.lockout_secs > 0
      ? `Demasiados intentos. Espera ${s.lockout_secs} s.`
      : 'Esperando a tu móvil. Se abrirá solo cuando esté en la red o conectado por cable.';
    return;
  }
  show('main');
  $('dot').classList.toggle('on', s.present);
  const batt = s.present && s.battery != null ? ` · ${s.battery}%${s.charging ? ' cargando' : ''}` : '';
  $('status-name').textContent = s.present ? s.device : 'Móvil ausente';
  $('status-sub').textContent = s.present ? `Conectado${batt}` : 'Sesión abierta con clave';
  $('status-warn').textContent = s.warn || '';
  $('status-warn').classList.toggle('hidden', !s.warn);
  const ul = $('devices');
  ul.innerHTML = '';
  for (const d of s.devices) {
    const li = document.createElement('li');
    const name = document.createElement('span');
    name.textContent = d.name;
    const btn = document.createElement('button');
    btn.textContent = 'Quitar';
    btn.onclick = async () => {
      try { await invoke('remove_device', { id: d.id }); } catch (e) { alertMsg(String(e)); }
    };
    li.append(name, btn);
    ul.append(li);
  }
  $('no-devices').classList.toggle('hidden', s.devices.length > 0);
}

// Carga/vacía los paneles según lo que haya cambiado entre dos estados.
function syncPanes(was, s) {
  const wasOpen = was && !was.locked;
  if (s.locked) return;
  if (!wasOpen) pcInit();
  if (!s.present) {
    phonePane.clear('El móvil no está conectado. Los archivos del móvil aparecerán al conectarlo.');
    clearApps();
    clearNotifsUI();
  } else if (!wasOpen || !was.present) {
    phonePane.load('');
    loadNotifs();
    if (!$('tab-apps').classList.contains('hidden')) loadApps();
  }
}

async function pcInit() {
  try { await pcPane.load(await invoke('local_home')); } catch { pcPane.load(''); }
  // Accesos directos a las carpetas de siempre (Windows resuelve OneDrive: el Escritorio real puede no ser C:\Users\...\Desktop).
  try {
    const places = await invoke('local_places');
    $('places').replaceChildren(...places.map((p) => {
      const b = document.createElement('button');
      b.className = 'small';
      b.textContent = p.name;
      b.title = p.path;
      b.onclick = () => pcPane.load(p.path);
      return b;
    }));
  } catch { /* bloqueada */ }
}

function applyState(s) {
  const was = state;
  state = s;
  if (was && s.devices.length > was.devices.length) hidePairing();
  // Al pasar a bloqueado se vacía la interfaz antes de pintar la pantalla de bloqueo.
  if (s.locked && !(was && was.locked)) wipeDom();
  render(s);
  syncPanes(was, s);
}

listen('state', (e) => applyState(e.payload));

$('setup-btn').onclick = async () => {
  const [a, b] = [$('setup-pw').value, $('setup-pw2').value];
  if (a !== b) return ($('setup-err').textContent = 'Las claves no coinciden');
  try { await invoke('set_password', { password: a }); } catch (e) { $('setup-err').textContent = e; }
};

async function tryUnlock() {
  try { await invoke('unlock', { password: $('lock-pw').value }); $('lock-pw').value = ''; $('lock-err').textContent = ''; }
  catch (e) { $('lock-err').textContent = e; $('lock-pw').value = ''; }
}
$('lock-btn').onclick = tryUnlock;
$('lock-pw').onkeydown = (e) => { if (e.key === 'Enter') tryUnlock(); };
$('setup-pw2').onkeydown = (e) => { if (e.key === 'Enter') $('setup-btn').click(); };

$('lock-now').onclick = () => invoke('lock');

// Pestañas
function tab(name) {
  for (const t of ['files', 'notifs', 'web', 'apps', 'settings']) {
    $('tab-' + t).classList.toggle('hidden', t !== name);
    $('tab-btn-' + t).classList.toggle('active', t === name);
  }
  if (name === 'web') { loadWeb(); placeWeb(); } else invoke('web_hide'); // el servicio no debe tapar las demás pestañas
  if (name === 'settings') { loadAutostart(); loadTools(); }
  if (name === 'apps' && !appsLoaded && state && state.present) loadApps();
}
$('tab-btn-files').onclick = () => tab('files');
$('tab-btn-notifs').onclick = () => tab('notifs');
$('tab-btn-web').onclick = () => tab('web');
$('tab-btn-apps').onclick = () => tab('apps');
$('tab-btn-settings').onclick = () => tab('settings');

$('pair-btn').onclick = async () => {
  try {
    const info = await invoke('pairing_info');
    $('qr').innerHTML = info.qr_svg;
    $('payload').value = info.payload;
    $('pair-fp').textContent = info.fp_short;
    $('pair-box').classList.remove('hidden');
    clearInterval(pairTimer);
    let left = info.expires_secs;
    pairTimer = setInterval(() => { if (--left <= 0) hidePairing(); }, 1000);
  } catch (e) { alertMsg(String(e)); }
};

// Modo de uso: el usuario lo elige (aquí o en la app del móvil; se sincronizan). Avanzado = usa adb/scrcpy si están listos.
async function loadMode() {
  let advanced = true;
  try { advanced = await invoke('get_advanced'); } catch { /* por defecto */ }
  let ready = false;
  if (advanced) { try { await invoke('adb_status'); ready = true; } catch { /* sin adb o sin depuración */ } }
  $('mode-basic').classList.toggle('active', !advanced);
  $('mode-full').classList.toggle('active', advanced);
  $('mode-now').textContent = !advanced
    ? 'Modo básico: todo funciona sin depuración. Las acciones que usan adb (abrir apps del móvil en el PC, detener, desinstalar) están apagadas.'
    : ready
      ? 'Modo avanzado: la depuración está activa. Desactívala en el móvil cuando termines.'
      : 'Modo avanzado activado, pero falta preparar adb y la depuración inalámbrica (tarjetas Herramientas y siguiente). Mientras tanto funciona como el básico.';
}
async function chooseMode(advanced) {
  try { await invoke('set_advanced', { on: advanced }); } catch (e) { alertMsg(String(e)); }
  await loadMode();
  if (advanced) $('card-wireless')?.scrollIntoView({ behavior: 'smooth', block: 'center' });
}
$('mode-basic').onclick = () => chooseMode(false);
$('mode-full').onclick = () => chooseMode(true);
// Si el usuario cambia el modo en el móvil, el PC se actualiza solo.
listen('mode', () => loadMode());

// Panel lateral plegable (se recuerda entre sesiones).
function setSide(collapsed) {
  document.querySelector('.side').classList.toggle('collapsed', collapsed);
  const b = $('side-toggle');
  b.title = b.ariaLabel = collapsed ? 'Mostrar el panel lateral' : 'Ocultar el panel lateral';
  try { localStorage.setItem('side-collapsed', collapsed ? '1' : '0'); } catch { /* sin almacenamiento */ }
  setTimeout(placeWeb, 60);
}
$('side-toggle').onclick = () => setSide(!document.querySelector('.side').classList.contains('collapsed'));
try { if (localStorage.getItem('side-collapsed') === '1') setSide(true); } catch { /* sin almacenamiento */ }

// Herramientas externas (adb, scrcpy): estado e instalación.
async function loadTools() {
  loadMode();
  let st;
  try { st = await invoke('tools_status'); } catch { return; }
  const items = [
    { id: 'adb', name: 'adb (Android Debug Bridge)', ok: !!st.adb, detail: st.adb || 'no encontrado', uses: 'abrir, detener, desinstalar y extraer APK' },
    { id: 'scrcpy', name: 'scrcpy', ok: st.scrcpy, detail: st.scrcpy ? 'instalado' : 'no encontrado', uses: 'abrir apps del móvil en ventanas del PC' },
  ];
  const ul = $('tools-list');
  ul.replaceChildren();
  for (const t of items) {
    const li = document.createElement('li');
    const left = document.createElement('div');
    const name = document.createElement('div');
    name.textContent = t.name;
    const sub = document.createElement('div');
    sub.className = t.ok ? 'tool-ok' : 'tool-no';
    sub.textContent = `${t.ok ? 'Disponible' : 'Falta'} - ${t.detail}. Sirve para ${t.uses}.`;
    left.append(name, sub);
    li.append(left);
    if (!t.ok) {
      const b = document.createElement('button');
      b.className = 'primary small';
      b.textContent = 'Instalar';
      b.onclick = async () => {
        b.disabled = true;
        $('tools-msg').textContent = `Descargando ${t.id}`;
        try { $('tools-msg').textContent = await invoke('tools_install', { which: t.id }); }
        catch (e) { $('tools-msg').textContent = String(e); }
        loadTools();
      };
      li.append(b);
    }
    ul.append(li);
  }
}

$('wpair-detect').onclick = async () => {
  $('wpair-msg').textContent = 'Buscando';
  try {
    const found = await invoke('adb_pairing_services');
    if (found.length) { $('wpair-addr').value = found[0]; $('wpair-msg').textContent = 'Encontrado. Escribe el código que muestra el móvil.'; }
    else $('wpair-msg').textContent = 'No se encontró. Abre "Vincular dispositivo con código" en el móvil y vuelve a intentarlo.';
  } catch (e) { $('wpair-msg').textContent = String(e); }
};
$('wpair-btn').onclick = async () => {
  $('wpair-msg').textContent = 'Emparejando';
  try {
    $('wpair-msg').textContent = await invoke('adb_pair', { addr: $('wpair-addr').value.trim(), code: $('wpair-code').value.trim() });
    $('wpair-code').value = '';
  } catch (e) { $('wpair-msg').textContent = String(e); }
};

async function loadAutostart() {
  try { $('autostart').checked = await invoke('autostart_get'); } catch { /* sin dato */ }
  try { $('mute-call').checked = await invoke('get_mute_on_call'); } catch { /* sin dato */ }
  loadCallPerms();
}
$('mute-call').onchange = async () => {
  const want = $('mute-call').checked;
  try { await invoke('set_mute_on_call', { on: want }); } catch (e) { $('mute-call').checked = !want; }
};

// Avisa si al móvil le faltan permisos para contestar llamadas.
async function loadCallPerms() {
  const box = $('call-perms');
  try {
    const p = await invoke('call_perms');
    const miss = [];
    if (!p.answerPhone) miss.push('"Permitir contestar llamadas" (llamadas normales)');
    if (p.overlayAvailable && !p.overlay) miss.push('"Permitir mostrar sobre otras apps" (llamadas de WhatsApp)');
    box.textContent = miss.length
      ? 'Para poder contestar desde el PC, en la app SyncroUmbral del móvil pulsa: ' + miss.join(' y ') + '.'
      : 'El móvil tiene todos los permisos para contestar llamadas.';
  } catch { box.textContent = 'Conecta el móvil para comprobar los permisos de llamadas.'; }
}
$('autostart').onchange = async () => {
  const want = $('autostart').checked;
  try { await invoke('autostart_set', { on: want }); $('autostart-msg').textContent = ''; }
  catch (e) { $('autostart').checked = !want; $('autostart-msg').textContent = String(e); }
};

$('cable-btn').onclick = async () => {
  try { $('cable-msg').textContent = await invoke('enable_cable'); }
  catch (e) { $('cable-msg').textContent = e; }
};

// Estado inicial y cuenta atrás del bloqueo por intentos.
invoke('get_state').then(applyState);
setInterval(async () => {
  if (state && state.locked && state.lockout_secs > 0) render(await invoke('get_state'));
}, 1000);
