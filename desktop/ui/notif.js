// Tarjetas de notificación, compartidas por la ventana principal y la ventana emergente.
(function () {
  const { invoke } = window.__TAURI__.core;

  function ago(ms) {
    const s = Math.max(0, Math.round((Date.now() - ms) / 1000));
    if (s < 60) return 'ahora';
    if (s < 3600) return `hace ${Math.floor(s / 60)} min`;
    if (s < 86400) return `hace ${Math.floor(s / 3600)} h`;
    return `hace ${Math.floor(s / 86400)} d`;
  }

  // Inicio (hora local del PC) de cada llamada en curso, por clave de notificación.
  const callStart = new Map();
  function fmtDur(ms) {
    const s = Math.max(0, Math.floor(ms / 1000));
    const mm = String(Math.floor(s / 60)).padStart(2, '0');
    return s >= 3600 ? `${Math.floor(s / 3600)}:${mm}:${String(s % 60).padStart(2, '0')}` : `${mm}:${String(s % 60).padStart(2, '0')}`;
  }
  // Un solo temporizador para todos los cronómetros visibles (tarjetas y pantalla de llamada).
  setInterval(() => {
    for (const t of document.querySelectorAll('.n-timer')) {
      const start = callStart.get(t.dataset.key);
      if (start) t.textContent = fmtDur(Date.now() - start);
    }
  }, 1000);

  function el(tag, cls, text) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text; // siempre textContent: el contenido viene del móvil
    return e;
  }

  // n: notificación tal como la envía el móvil. opts.onClose: botón extra para cerrar (ventana emergente).
  function card(n, opts = {}) {
    const root = el('div', 'notif' + (n.call ? ' call' : '') + (n.incoming ? ' incoming' : ''));
    root.dataset.key = n.key;
    if (n.incoming) root.append(el('div', 'n-callbadge', 'Llamada entrante'));
    else if (n.call) root.append(el('div', 'n-callbadge ongoing', 'Llamada en curso'));

    const head = el('div', 'n-head');
    if (n.call && !n.incoming) {
      // Llamada en curso: cronómetro desde que el PC la vio en curso (lo actualiza el temporizador de abajo).
      if (!callStart.has(n.key)) callStart.set(n.key, Date.now());
      const t = el('span', 'n-time n-timer', fmtDur(Date.now() - callStart.get(n.key)));
      t.dataset.key = n.key;
      head.append(el('span', 'n-app', n.app || n.pkg), t);
    } else {
      callStart.delete(n.key);
      head.append(el('span', 'n-app', n.app || n.pkg), el('span', 'n-time', ago(n.when)));
    }
    if (opts.onClose) {
      const x = el('button', 'n-close small', 'Cerrar');
      x.onclick = opts.onClose;
      head.append(x);
    }
    root.append(head);
    if (n.title) root.append(el('div', 'n-title', n.title));
    if (n.text) root.append(el('div', 'n-text', n.text));

    const status = el('div', 'n-status');
    const setStatus = (t, bad = false) => { status.textContent = t; status.classList.toggle('bad', bad); };

    const actions = Array.isArray(n.actions) ? n.actions : [];
    const reply = actions.find((a) => a.reply);
    if (reply) {
      const row = el('div', 'n-reply');
      const input = document.createElement('input');
      input.type = 'text';
      input.placeholder = reply.hint || 'Responder';
      input.autocomplete = 'off';
      const send = el('button', 'primary');
      send.title = 'Enviar respuesta';
      send.setAttribute('aria-label', 'Enviar respuesta');
      send.innerHTML = '<svg class="ic" viewBox="0 0 24 24"><path d="M5 12h13M13 6l6 6-6 6" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>'; // constante: no lleva datos del móvil
      const go = async () => {
        const text = input.value.trim();
        if (!text) return;
        send.disabled = true;
        setStatus('Enviando');
        try {
          await invoke('notif_reply', { key: n.key, index: reply.index, text });
          input.value = '';
          setStatus('Enviado');
        } catch (e) {
          setStatus(String(e), true);
        } finally {
          send.disabled = false;
        }
      };
      send.onclick = go;
      input.onkeydown = (e) => { if (e.key === 'Enter') go(); };
      row.append(input, send);
      root.append(row);
    }

    // Llamadas: Contestar y Rechazar/Colgar van por la API de llamadas del móvil (funciona con llamadas normales y con las de
    // WhatsApp); los botones de la notificación con esos mismos nombres no se repiten.
    const ANSWER = /contest|respond|answer|accept|acept|atender/i;
    const END = /rechaz|declin|reject|colgar|hang|\bend\b|finaliz|cortar|terminar/i;
    if (n.call) {
      const row = el('div', 'n-callbtns');
      const run = async (cmd, ok) => {
        setStatus('Un momento');
        try { await invoke(cmd, { key: n.key }); setStatus(ok); }
        catch (e) { setStatus(String(e), true); }
      };
      if (n.incoming) {
        const yes = el('button', 'call-answer', 'Contestar');
        yes.onclick = () => run('call_answer', 'Contestada');
        row.append(yes);
      }
      const no = el('button', 'call-end', n.incoming ? 'Rechazar' : 'Colgar');
      no.onclick = () => run('call_end', n.incoming ? 'Rechazada' : 'Colgada');
      row.append(no);
      root.append(row);
    }

    const used = new Set();
    if (n.media) {
      // Reproductor: carátula, título y artista, y los controles de transporte como botones redondos con icono.
      root.classList.add('media');
      if (typeof n.art === 'string' && /^[A-Za-z0-9+/=]+$/.test(n.art)) {
        const img = document.createElement('img');
        img.className = 'n-art';
        img.alt = '';
        img.src = 'data:image/jpeg;base64,' + n.art;
        root.append(img);
      }
      const GLYPH = [
        [/anterior|previous|prev\b|atr[aá]s/i, '<path d="M7 6v12M18 6l-8 6 8 6z" fill="currentColor" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>', 'transport'],
        [/siguiente|next|adelante/i, '<path d="M17 6v12M6 6l8 6-8 6z" fill="currentColor" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>', 'transport'],
        [/pausa|pause/i, '<path d="M8 6v12M16 6v12" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>', 'main'],
        [/reproducir|play|reanudar/i, '<path d="M8 5.5v13l11-6.5z" fill="currentColor" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/>', 'main'],
        [/no me gusta|dislike|thumb.?down/i, '<path d="M7 11v9H4v-9zM7 11l4-7a2 2 0 0 1 2 2v3h5.5a1.5 1.5 0 0 1 1.5 1.7l-1 6.5a2 2 0 0 1-2 1.8H7" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round" transform="translate(0 24) scale(1 -1)"/>', 'extra'],
        [/me gusta|like|favorit|guardar|save/i, '<path d="M12 20s-7-4.4-7-10a4 4 0 0 1 7-2.5A4 4 0 0 1 19 10c0 5.6-7 10-7 10z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/>', 'extra'],
      ];
      const ctl = el('div', 'n-media-ctl');
      for (const a of actions) {
        if (a.reply || !a.title) continue;
        const g = GLYPH.find(([re]) => re.test(a.title));
        if (!g) continue;
        used.add(a.index);
        const b = el('button', 'mbtn ' + g[2]);
        b.title = a.title;
        b.setAttribute('aria-label', a.title);
        b.innerHTML = '<svg class="ic" viewBox="0 0 24 24">' + g[1] + '</svg>'; // constante: el contenido del móvil no entra aquí
        b.onclick = async () => {
          try { await invoke('notif_action', { key: n.key, index: a.index }); } catch (e) { setStatus(String(e), true); }
        };
        ctl.append(b);
      }
      if (ctl.children.length) root.append(ctl);
    }

    // Los botones sin icono conocido se muestran con su nombre, como en el resto de notificaciones.
    const bar = el('div', 'n-actions');
    for (const a of actions.filter((x) => !x.reply && x.title && !used.has(x.index) && !(n.call && (ANSWER.test(x.title) || END.test(x.title))))) {
      const b = el('button', 'small', a.title);
      b.onclick = async () => {
        try { await invoke('notif_action', { key: n.key, index: a.index }); setStatus('Hecho'); }
        catch (e) { setStatus(String(e), true); }
      };
      bar.append(b);
    }
    if (n.clearable) {
      const d = el('button', 'small', 'Descartar');
      d.onclick = async () => {
        try { await invoke('notif_dismiss', { key: n.key }); } catch (e) { setStatus(String(e), true); }
      };
      bar.append(d);
    }
    if (n.pkg === 'umbral.share') {
      // Contenido enviado desde el móvil con "Compartir": texto (copiar) o archivo (mostrar en el Explorador).
      if (n.kind === 'text') {
        const c = el('button', 'small primary', 'Copiar');
        c.onclick = async () => {
          try { await navigator.clipboard.writeText(n.text); setStatus('Copiado al portapapeles'); }
          catch (e) { setStatus('No se pudo copiar: ' + e, true); }
        };
        bar.prepend(c);
      } else if (n.kind === 'file' && n.path) {
        const o = el('button', 'small primary', 'Mostrar en carpeta');
        o.onclick = async () => {
          try { await invoke('show_received', { path: n.path }); } catch (e) { setStatus(String(e), true); }
        };
        bar.prepend(o);
      }
      const x = el('button', 'small', 'Quitar');
      x.onclick = async () => {
        try { await invoke('notif_remove_local', { key: n.key }); } catch (e) { setStatus(String(e), true); }
      };
      bar.append(x);
    } else if (!n.call) { // silenciar la app de teléfono no tiene sentido en una llamada
      const m = el('button', 'small', 'Silenciar app');
      m.title = 'No mostrar más notificaciones de esta app en el PC';
      m.onclick = async () => {
        try { await invoke('notif_mute', { pkg: n.pkg, app: n.app || n.pkg }); } catch (e) { setStatus(String(e), true); }
      };
      bar.append(m);
    }
    if (bar.children.length) root.append(bar);
    root.append(status);
    return root;
  }

  // Sustituye la tarjeta de `key` conservando lo que se estaba escribiendo y el foco.
  function replaceCard(container, n, opts) {
    const old = container.querySelector(`[data-key="${CSS.escape(n.key)}"]`);
    const fresh = card(n, opts);
    if (!old) { container.prepend(fresh); return fresh; }
    const oi = old.querySelector('input');
    const ni = fresh.querySelector('input');
    if (oi && ni) {
      ni.value = oi.value;
      const focused = document.activeElement === oi;
      old.replaceWith(fresh);
      if (focused) { ni.focus(); ni.setSelectionRange(ni.value.length, ni.value.length); }
    } else {
      old.replaceWith(fresh);
    }
    return fresh;
  }

  function removeCard(container, key) {
    callStart.delete(key);
    container.querySelector(`[data-key="${CSS.escape(key)}"]`)?.remove();
  }

  window.NotifUI = { card, replaceCard, removeCard, callStart, fmtDur };
})();
