//! Servidor wss del PC. Protocolo en docs/PROTOCOLO.md.

use crate::core::{Core, Device, PhoneCmd, HB_INTERVAL_MS, PORT};
use anyhow::{anyhow, bail, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use futures_util::{SinkExt, StreamExt};
use p256::ecdsa::{signature::Verifier, Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;
use rand::RngCore;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Runtime};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use tokio_rustls::{rustls, TlsAcceptor};
use tokio_tungstenite::tungstenite::{protocol::WebSocketConfig, Message};
use tokio_tungstenite::WebSocketStream;

type Ws = WebSocketStream<tokio_rustls::server::TlsStream<TcpStream>>;

const HANDSHAKE_STEP: Duration = Duration::from_secs(5);

pub async fn run<R: Runtime>(h: AppHandle<R>, core: Arc<Core>, tls: Arc<rustls::ServerConfig>) {
    let listener = match TcpListener::bind(("0.0.0.0", PORT)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("No se pudo abrir el puerto {PORT}: {e}");
            return;
        }
    };
    serve(h, core, TlsAcceptor::from(tls), listener).await;
}

async fn serve<R: Runtime>(h: AppHandle<R>, core: Arc<Core>, acceptor: TlsAcceptor, listener: TcpListener) {
    loop {
        let Ok((tcp, peer)) = listener.accept().await else { continue };
        // Sin plaza (demasiadas conexiones sin autenticar, u origen bloqueado por fallos) se corta antes de gastar TLS.
        let Some(permit) = core.limits.enter(peer.ip()) else { continue };
        let (h, core, acceptor) = (h.clone(), core.clone(), acceptor.clone());
        tokio::spawn(async move {
            let _ = session(&h, &core, tcp, acceptor, permit, peer.ip()).await;
        });
    }
}

/// Tamaño máximo de mensaje del canal (cabe un trozo de archivo de 1 MiB en base64 con holgura).
const MAX_MESSAGE: usize = 4 * 1024 * 1024;
/// Hasta autenticar solo se aceptan mensajes pequeños (hello, pair, auth).
const MAX_PRE_AUTH: usize = 16 * 1024;
/// Plazo total para completar la autenticación.
const HANDSHAKE_TOTAL: Duration = Duration::from_secs(12);

async fn send(ws: &mut Ws, v: Value) -> Result<()> {
    ws.send(Message::Text(v.to_string().into())).await?;
    Ok(())
}

fn key_id(der: &[u8]) -> String {
    hex::encode(Sha256::digest(der))
}

/// Handshake completo (pair opcional, hello, reto-respuesta). Devuelve el dispositivo autenticado o None.
async fn authenticate<R: Runtime>(
    h: &AppHandle<R>,
    core: &Arc<Core>,
    ws: &mut Ws,
    ip: std::net::IpAddr,
) -> Result<Option<Device>> {
    let mut msg = recv_small(ws, HANDSHAKE_STEP).await?;

    // Vinculación (solo la primera vez).
    if msg["t"] == "pair" {
        match pair(core, &msg) {
            Ok(()) => send(ws, json!({"t": "paired"})).await?,
            Err(e) => {
                core.limits.fail(ip);
                core.seclog(&format!("vinculación rechazada desde {ip}: {e}"));
                send(ws, json!({"t": "error", "msg": e.to_string()})).await?;
                return Ok(None);
            }
        }
        core.notify(h);
        msg = recv_small(ws, HANDSHAKE_STEP).await?;
    }

    if msg["t"] != "hello" {
        core.limits.fail(ip);
        send(ws, json!({"t": "error", "msg": "se esperaba hello"})).await?;
        return Ok(None);
    }
    let id = msg["id"].as_str().unwrap_or_default();
    let Some(device) = core.find_device(id) else {
        // Sin penalizar: un móvil que el usuario desvinculó (o un archivo restablecido) reintenta solo, y bloquearlo
        // retrasaría la nueva vinculación. Los límites de pendientes y de tiempo ya acotan este caso.
        send(ws, json!({"t": "error", "msg": "dispositivo no vinculado"})).await?;
        return Ok(None);
    };

    // Reto-respuesta: demuestra que el móvil tiene la clave privada.
    let mut nonce = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    send(ws, json!({"t": "challenge", "n": B64.encode(nonce)})).await?;
    let auth = recv_small(ws, HANDSHAKE_STEP).await?;
    if auth["t"] != "auth" || !verify(&device, &nonce, auth["sig"].as_str().unwrap_or_default()) {
        core.limits.fail(ip);
        core.seclog(&format!("firma inválida de {} desde {ip}", device.name));
        send(ws, json!({"t": "error", "msg": "firma inválida"})).await?;
        return Ok(None);
    }
    core.limits.success(ip);
    send(ws, json!({"t": "ok", "hb": HB_INTERVAL_MS})).await?;
    Ok(Some(device))
}

/// Como `recv` pero rechaza mensajes grandes: antes de autenticar nadie necesita enviar más que unos cientos de bytes.
async fn recv_small(ws: &mut Ws, wait: Duration) -> Result<Value> {
    let deadline = Instant::now() + wait;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match timeout(left, ws.next()).await {
            Err(_) => bail!("tiempo agotado"),
            Ok(None) | Ok(Some(Err(_))) => bail!("conexión cerrada"),
            Ok(Some(Ok(Message::Text(t)))) => {
                if t.len() > MAX_PRE_AUTH {
                    bail!("mensaje demasiado grande antes de autenticar");
                }
                return Ok(serde_json::from_str(t.as_str())?);
            }
            Ok(Some(Ok(Message::Close(_)))) => bail!("conexión cerrada"),
            Ok(Some(Ok(_))) => {}
        }
    }
}

async fn session<R: Runtime>(
    h: &AppHandle<R>,
    core: &Arc<Core>,
    tcp: TcpStream,
    acceptor: TlsAcceptor,
    permit: crate::limits::Permit,
    ip: std::net::IpAddr,
) -> Result<()> {
    let peer_ip = Some(ip.to_string());
    let tls = timeout(HANDSHAKE_STEP, acceptor.accept(tcp)).await??;
    let cfg = WebSocketConfig::default().max_message_size(Some(MAX_MESSAGE)).max_frame_size(Some(MAX_MESSAGE));
    let mut ws = timeout(HANDSHAKE_STEP, tokio_tungstenite::accept_async_with_config(tls, Some(cfg))).await??;

    let device = match timeout(HANDSHAKE_TOTAL, authenticate(h, core, &mut ws, ip)).await {
        Ok(r) => r?,
        Err(_) => {
            core.limits.fail(ip);
            return Ok(());
        }
    };
    let Some(device) = device else { return Ok(()) };
    drop(permit); // ya autenticado: la plaza de handshake se libera

    let sid = core.begin_session(h, device.name.clone());
    let (tx, mut rx) = mpsc::channel(32);
    core.attach_phone(sid, tx, peer_ip);
    if core.sync_notifs {
        // Lista inicial de notificaciones del móvil (sin ventanas emergentes).
        let (c, hh) = (core.clone(), h.clone());
        tokio::spawn(async move {
            if let Ok(r) = c.call("notifs", json!({})).await {
                c.set_notifs(&hh, &r);
            }
            // El PC le dice al móvil en qué modo está (el que se cambie después viaja en cualquiera de los dos sentidos).
            let _ = c.call("set_mode", json!({ "advanced": c.advanced() })).await;
            // Mientras haya móvil, cada 10 s se compara con lo que realmente tiene: si se quitó una notificación en el móvil
            // (o se perdió un aviso), el PC se corrige solo.
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                if c.phone_session() != Some(sid) {
                    break; // esta sesión terminó (otra posterior lleva su propia sincronización)
                }
                if let Ok(r) = c.call("notifs", json!({})).await {
                    c.resync_notifs(&hh, &r);
                }
            }
        });
    }
    let on_msg = |kind: &str, v: &Value| core.on_phone_event(h, kind, v);
    let result = session_loop(&mut ws, core.hb_timeout, &mut rx, &on_msg).await;
    core.detach_phone(sid);
    core.end_session(h, sid);
    result
}

fn pair(core: &Arc<Core>, msg: &Value) -> Result<()> {
    if !core.take_token(msg["tok"].as_str().unwrap_or_default()) {
        bail!("código de vinculación inválido o caducado");
    }
    let der = B64.decode(msg["pub"].as_str().unwrap_or_default())?;
    VerifyingKey::from_public_key_der(&der).map_err(|_| anyhow!("clave pública inválida"))?;
    core.add_device(Device {
        id: key_id(&der),
        name: msg["name"].as_str().unwrap_or("Móvil").chars().take(60).collect(),
        pubkey: B64.encode(&der),
    });
    Ok(())
}

fn verify(device: &Device, nonce: &[u8], sig_b64: &str) -> bool {
    let check = || -> Result<()> {
        let der = B64.decode(&device.pubkey)?;
        let key = VerifyingKey::from_public_key_der(&der)?;
        let sig = Signature::from_der(&B64.decode(sig_b64)?)?;
        let mut data = b"umbral-v1".to_vec();
        data.extend_from_slice(nonce);
        key.verify(&data, &sig)?;
        Ok(())
    };
    check().is_ok()
}

/// Atiende al móvil autenticado: latidos, y peticiones del PC con sus respuestas.
/// Termina cuando el móvil se cierra o deja de dar latidos durante `hb_timeout`.
async fn session_loop(
    ws: &mut Ws,
    hb_timeout: Duration,
    rx: &mut mpsc::Receiver<PhoneCmd>,
    on_msg: &(dyn Fn(&str, &Value) + Send + Sync),
) -> Result<()> {
    let mut last_hb = Instant::now();
    let mut pending: HashMap<u64, oneshot::Sender<Result<Value, String>>> = HashMap::new();
    let mut next_id = 1u64;
    loop {
        let left = hb_timeout.saturating_sub(last_hb.elapsed());
        if left.is_zero() {
            return Ok(());
        }
        tokio::select! {
            _ = tokio::time::sleep(left) => return Ok(()),
            cmd = rx.recv() => {
                let Some(cmd) = cmd else { return Ok(()) };
                let id = next_id;
                next_id += 1;
                let mut req = match cmd.args {
                    Value::Object(m) => Value::Object(m),
                    _ => json!({}),
                };
                req["t"] = json!("req");
                req["id"] = json!(id);
                req["op"] = json!(cmd.op);
                pending.insert(id, cmd.reply);
                send(ws, req).await?;
            }
            msg = ws.next() => match msg {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return Ok(()),
                Some(Ok(Message::Text(t))) => {
                    let Ok(v) = serde_json::from_str::<Value>(t.as_str()) else { continue };
                    match v["t"].as_str() {
                        // Solo un latido válido reinicia el plazo (los ping/pong del transporte no cuentan).
                        Some("hb") => {
                            last_hb = Instant::now();
                            send(ws, json!({"t": "ack", "seq": v["seq"]})).await?;
                        }
                        Some("res") => {
                            if let Some(reply) = v["id"].as_u64().and_then(|id| pending.remove(&id)) {
                                let _ = reply.send(if v["ok"] == true {
                                    Ok(v["data"].clone())
                                } else {
                                    Err(v["err"].as_str().unwrap_or("Error del móvil").to_string())
                                });
                            }
                        }
                        Some(
                            kind @ ("notif" | "notif_gone" | "status" | "mode" | "lock_pc" | "unlock_pc" | "share_text" | "share_begin" | "share_chunk"
                            | "share_end"),
                        ) => on_msg(kind, &v),
                        _ => {}
                    }
                }
                Some(Ok(_)) => {}
            },
        }
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
