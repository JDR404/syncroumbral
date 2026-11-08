const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const root = document.getElementById('toast-root');
const HIDE_AFTER_MS = 12000;
let timer = null;
let hovering = false;

function busy() {
  // No se cierra sola mientras el usuario escribe o apunta con el ratón.
  const typing = [...root.querySelectorAll('input')].some((i) => i.value || document.activeElement === i);
  // Una llamada no se oculta sola: hay que contestarla o rechazarla.
  const ringing = !!root.querySelector('.notif.incoming');
  return hovering || typing || ringing;
}

function arm() {
  clearTimeout(timer);
  timer = setTimeout(() => (busy() ? arm() : close()), HIDE_AFTER_MS);
}

function close() {
  clearTimeout(timer);
  invoke('toast_hide');
}

function fit() {
  if (!root.children.length) return close();
  invoke('toast_resize', { height: root.scrollHeight + 2 });
}

function add(n) {
  NotifUI.replaceCard(root, n, { onClose: () => { NotifUI.removeCard(root, n.key); fit(); } });
  while (root.children.length > 4) root.lastElementChild.remove();
  fit();
  arm();
}

// La ventana se ajusta sola a lo que haya dentro, también cuando aparece un mensaje de estado o de error debajo de los botones.
new ResizeObserver(() => { if (root.children.length) fit(); }).observe(root);

document.body.addEventListener('mouseenter', () => { hovering = true; });
document.body.addEventListener('mouseleave', () => { hovering = false; arm(); });

listen('notif', (e) => { if (e.payload.popup) add(e.payload.n); });
listen('notif_gone', (e) => { NotifUI.removeCard(root, e.payload.key); fit(); });
listen('notif_clear', () => close());

// La ventana se crea después del evento que la abrió: se piden las pendientes.
invoke('toast_items').then((items) => { for (const n of [...items].reverse()) add(n); }).catch(() => close());
