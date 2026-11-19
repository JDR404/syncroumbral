//! Pruebas de punta a punta: un cliente que se comporta como la app Android
//! (huella fijada, vinculación, firma del reto, latidos) contra el servidor real.

use super::*;
use p256::ecdsa::{signature::Signer, SigningKey};
use p256::pkcs8::EncodePublicKey;
use tokio_rustls::rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use tokio_rustls::rustls::{DigitallySignedStruct, Error as TlsError, SignatureScheme};
use tokio_rustls::TlsConnector;
use tokio_tungstenite::tungstenite::Message;

type ClientWs = WebSocketStream<tokio_rustls::client::TlsStream<TcpStream>>;

/// Igual que PinnedTls.kt: acepta solo el certificado con la huella esperada.
#[derive(Debug)]
struct Pin {
    fp: String,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for Pin {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        if hex::encode(Sha256::digest(end_entity.as_ref())) == self.fp {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(TlsError::General("huella distinta".into()))
        }
    }
    fn verify_tls12_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(m, c, d, &self.provider.signature_verification_algorithms)
    }
    fn verify_tls13_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(m, c, d, &self.provider.signature_verification_algorithms)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

struct Harness {
    core: Arc<Core>,
    port: u16,
    fp: String,
    _app: tauri::App<tauri::test::MockRuntime>,
}

async fn harness(hb_timeout: Duration) -> Harness {
    harness_with(hb_timeout, false).await
}

async fn harness_with(hb_timeout: Duration, sync_notifs: bool) -> Harness {
    let ck = rcgen::generate_simple_self_signed(vec!["umbral.local".to_string()]).unwrap();
    let (cert, key) = (ck.cert.der().to_vec(), ck.key_pair.serialize_der());
    let fp = hex::encode(Sha256::digest(&cert));
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![CertificateDer::from(cert)], PrivatePkcs8KeyDer::from(key).into())
        .unwrap();

    let dir = std::env::temp_dir().join(format!("umbral-test-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut core = Core::load(dir, fp.clone());
    core.hb_timeout = hb_timeout;
    core.sync_notifs = sync_notifs;
    let core = Arc::new(core);

    let app = tauri::test::mock_app();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(serve(app.handle().clone(), core.clone(), TlsAcceptor::from(Arc::new(cfg)), listener));
    Harness { core, port, fp, _app: app }
}

async fn connect(port: u16, fp: &str) -> anyhow::Result<ClientWs> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let cfg = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pin { fp: fp.to_string(), provider }))
        .with_no_client_auth();
    let tcp = TcpStream::connect(("127.0.0.1", port)).await?;
    let tls = TlsConnector::from(Arc::new(cfg)).connect(ServerName::try_from("umbral.local")?, tcp).await?;
    let (ws, _) = tokio_tungstenite::client_async("wss://umbral.local/", tls).await?;
    Ok(ws)
}

async fn say(ws: &mut ClientWs, v: Value) {
    ws.send(Message::Text(v.to_string().into())).await.unwrap();
}

async fn hear(ws: &mut ClientWs) -> Value {
    let m = timeout(Duration::from_secs(3), ws.next()).await.expect("sin respuesta").unwrap().unwrap();
    match m {
        Message::Text(t) => serde_json::from_str(t.as_str()).unwrap(),
        other => panic!("mensaje inesperado: {other:?}"),
    }
}

fn spki(key: &SigningKey) -> Vec<u8> {
    key.verifying_key().to_public_key_der().unwrap().as_bytes().to_vec()
}

/// hello → challenge → auth firmado con `key`. Devuelve la respuesta final del PC.
async fn login(ws: &mut ClientWs, key: &SigningKey) -> Value {
    say(ws, json!({"t": "hello", "id": key_id(&spki(key))})).await;
    let ch = hear(ws).await;
    if ch["t"] != "challenge" {
        return ch;
    }
    let mut data = b"umbral-v1".to_vec();
    data.extend(B64.decode(ch["n"].as_str().unwrap()).unwrap());
    let sig: p256::ecdsa::Signature = key.sign(&data);
    say(ws, json!({"t": "auth", "sig": B64.encode(sig.to_der().as_bytes())})).await;
    hear(ws).await
}

async fn pair_device(h: &Harness, key: &SigningKey) {
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    say(&mut ws, json!({"t": "pair", "tok": h.core.new_token(), "pub": B64.encode(spki(key)), "name": "Pixel de prueba"})).await;
    assert_eq!(hear(&mut ws).await["t"], "paired");
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(250)).await;
}

#[tokio::test]
async fn pair_auth_heartbeat_and_lock_on_disconnect() {
    let h = harness(Duration::from_secs(10)).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    assert!(h.core.snapshot().locked, "debe arrancar bloqueado");

    let mut ws = connect(h.port, &h.fp).await.unwrap();
    say(&mut ws, json!({"t": "pair", "tok": h.core.new_token(), "pub": B64.encode(spki(&key)), "name": "Pixel de prueba"})).await;
    assert_eq!(hear(&mut ws).await["t"], "paired");
    let ok = login(&mut ws, &key).await;
    assert_eq!(ok["t"], "ok");
    assert_eq!(ok["hb"], 2000);

    settle().await;
    let s = h.core.snapshot();
    assert!(s.present && !s.locked, "con el móvil presente debe estar desbloqueado");
    assert_eq!(s.device.as_deref(), Some("Pixel de prueba"));

    say(&mut ws, json!({"t": "hb", "seq": 7})).await;
    let ack = hear(&mut ws).await;
    assert_eq!((ack["t"].as_str(), ack["seq"].as_u64()), (Some("ack"), Some(7)));

    drop(ws); // el móvil desaparece
    settle().await;
    let s = h.core.snapshot();
    assert!(!s.present && s.locked, "al irse el móvil debe bloquearse");
}

#[tokio::test]
async fn silent_phone_is_locked_after_heartbeat_timeout() {
    let h = harness(Duration::from_millis(700)).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;

    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    settle().await;
    assert!(!h.core.snapshot().locked);

    // El socket sigue abierto pero no hay latidos (red caída sin cierre limpio).
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(h.core.snapshot().locked, "sin latidos debe bloquearse aunque el socket siga abierto");
}

#[tokio::test]
async fn heartbeats_keep_it_unlocked() {
    let h = harness(Duration::from_millis(700)).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    for seq in 0..6 {
        tokio::time::sleep(Duration::from_millis(300)).await;
        say(&mut ws, json!({"t": "hb", "seq": seq})).await;
        assert_eq!(hear(&mut ws).await["t"], "ack");
    }
    assert!(!h.core.snapshot().locked, "1.8 s con latidos cada 0.3 s no debe bloquear (plazo 0.7 s)");
}

#[tokio::test]
async fn rejects_bad_token_reuse_wrong_key_and_unknown_device() {
    let h = harness(Duration::from_secs(10)).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    let pubkey = B64.encode(spki(&key));

    // Token incorrecto.
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    h.core.new_token();
    say(&mut ws, json!({"t": "pair", "tok": "00", "pub": pubkey, "name": "x"})).await;
    assert_eq!(hear(&mut ws).await["t"], "error");

    // Token de un solo uso.
    let tok = h.core.new_token();
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    say(&mut ws, json!({"t": "pair", "tok": tok, "pub": pubkey, "name": "x"})).await;
    assert_eq!(hear(&mut ws).await["t"], "paired");
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    say(&mut ws, json!({"t": "pair", "tok": tok, "pub": pubkey, "name": "x"})).await;
    assert_eq!(hear(&mut ws).await["t"], "error", "el token no puede reutilizarse");

    // Dispositivo vinculado pero firmando con otra clave (suplantación).
    let attacker = SigningKey::random(&mut rand::rngs::OsRng);
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    say(&mut ws, json!({"t": "hello", "id": key_id(&spki(&key))})).await;
    let ch = hear(&mut ws).await;
    let mut data = b"umbral-v1".to_vec();
    data.extend(B64.decode(ch["n"].as_str().unwrap()).unwrap());
    let sig: p256::ecdsa::Signature = attacker.sign(&data);
    say(&mut ws, json!({"t": "auth", "sig": B64.encode(sig.to_der().as_bytes())})).await;
    assert_eq!(hear(&mut ws).await["msg"], "firma inválida");

    // Firma válida pero de un reto anterior (repetición).
    let mut ws1 = connect(h.port, &h.fp).await.unwrap();
    say(&mut ws1, json!({"t": "hello", "id": key_id(&spki(&key))})).await;
    let old = hear(&mut ws1).await;
    let mut old_data = b"umbral-v1".to_vec();
    old_data.extend(B64.decode(old["n"].as_str().unwrap()).unwrap());
    let old_sig: p256::ecdsa::Signature = key.sign(&old_data);
    let mut ws2 = connect(h.port, &h.fp).await.unwrap();
    say(&mut ws2, json!({"t": "hello", "id": key_id(&spki(&key))})).await;
    let _fresh = hear(&mut ws2).await;
    say(&mut ws2, json!({"t": "auth", "sig": B64.encode(old_sig.to_der().as_bytes())})).await;
    assert_eq!(hear(&mut ws2).await["msg"], "firma inválida", "una firma vieja no sirve con un reto nuevo");

    // Dispositivo desconocido.
    let stranger = SigningKey::random(&mut rand::rngs::OsRng);
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &stranger).await["msg"], "dispositivo no vinculado");

    settle().await;
    assert!(h.core.snapshot().locked, "ningún intento fallido debe desbloquear");
}

/// Error real visto en el Pixel: crear la clave con el PC sin móvil dejaba la sesión "abierta con clave"
/// y al irse el móvil después ya no se bloqueaba.
#[tokio::test]
async fn phone_leaving_locks_even_if_session_was_opened_with_password() {
    let h = harness(Duration::from_secs(10)).await;
    h.core.set_password("secreto-123").unwrap();
    assert!(!h.core.is_locked(), "recién creada la clave, sin móvil, la sesión está abierta");

    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    settle().await;
    assert!(!h.core.is_locked());

    drop(ws);
    settle().await;
    assert!(h.core.is_locked(), "al irse el móvil debe bloquearse aunque antes se abriera con clave");

    // Y sin móvil, la clave sigue abriendo.
    h.core.unlock_with_password("secreto-123").unwrap();
    assert!(!h.core.is_locked());
}

#[tokio::test]
async fn pc_can_query_the_phone_over_the_authenticated_session() {
    let h = harness(Duration::from_secs(10)).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    settle().await;

    // Respuesta correcta.
    let core = h.core.clone();
    let task = tokio::spawn(async move { core.call("ls", json!({"path": "DCIM"})).await });
    let req = hear(&mut ws).await;
    assert_eq!((req["t"].as_str(), req["op"].as_str(), req["path"].as_str()), (Some("req"), Some("ls"), Some("DCIM")));
    say(&mut ws, json!({"t": "res", "id": req["id"], "ok": true, "data": {"items": []}})).await;
    assert_eq!(task.await.unwrap().unwrap()["items"], json!([]));

    // Error devuelto por el móvil.
    let core = h.core.clone();
    let task = tokio::spawn(async move { core.call("read", json!({"path": "x"})).await });
    let req = hear(&mut ws).await;
    say(&mut ws, json!({"t": "res", "id": req["id"], "ok": false, "err": "sin permiso"})).await;
    assert_eq!(task.await.unwrap().unwrap_err(), "sin permiso");

    // Una respuesta con un id que el PC no pidió se ignora (no rompe la sesión).
    say(&mut ws, json!({"t": "res", "id": 9999, "ok": true, "data": {}})).await;

    // Bloqueada: el PC no debe poder consultar el móvil.
    h.core.lock_now();
    let e = h.core.call("ls", json!({"path": ""})).await.unwrap_err();
    assert!(e.contains("bloqueada"), "{e}");
    h.core.unlock_with_password("x").ok();
}

#[tokio::test]
async fn request_in_flight_fails_cleanly_when_phone_disconnects() {
    let h = harness(Duration::from_secs(10)).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    settle().await;

    let core = h.core.clone();
    let task = tokio::spawn(async move { core.call("ls", json!({"path": ""})).await });
    let _ = hear(&mut ws).await;
    drop(ws);
    let e = task.await.unwrap().unwrap_err();
    assert!(e.contains("desconect"), "{e}");

    let e = h.core.call("ls", json!({"path": ""})).await.unwrap_err();
    assert!(e.contains("bloqueada") || e.contains("no está conectado"), "{e}");
}

fn notif(key: &str, text: &str, quiet: bool) -> Value {
    json!({"t": "notif", "key": key, "pkg": "com.whatsapp", "app": "WhatsApp", "title": "Ana", "text": text,
           "quiet": quiet, "clearable": true, "when": 1,
           "actions": [{"index": 0, "title": "Responder", "reply": true, "hint": "Responder"}]})
}

fn counts(h: &Harness) -> (usize, usize) {
    (h.core.notifs_state()["items"].as_array().unwrap().len(), h.core.popup_items().len())
}

#[tokio::test]
async fn notifications_arrive_update_and_disappear() {
    let h = harness(Duration::from_secs(10)).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    settle().await;

    say(&mut ws, notif("k1", "hola", false)).await;
    settle().await;
    assert_eq!(counts(&h), (1, 1), "una notificación normal va a la lista y a la ventana emergente");

    say(&mut ws, notif("k1", "hola, como estas?", false)).await;
    settle().await;
    assert_eq!(counts(&h), (1, 1), "la misma clave se actualiza, no se duplica");
    assert_eq!(h.core.notifs_state()["items"][0]["text"], "hola, como estas?");

    say(&mut ws, notif("k2", "promo", true)).await;
    settle().await;
    assert_eq!(counts(&h), (2, 1), "una silenciosa va a la lista pero no a la ventana emergente");

    say(&mut ws, json!({"t": "notif_gone", "key": "k1"})).await;
    settle().await;
    assert_eq!(counts(&h), (1, 0));

    say(&mut ws, json!({"t": "notif", "title": "sin clave"})).await; // mal formada: se ignora
    settle().await;
    assert_eq!(counts(&h), (1, 0));

    drop(ws);
    settle().await;
    assert_eq!(counts(&h), (0, 0), "al irse el móvil se borran las notificaciones");
}

#[tokio::test]
async fn locked_app_ignores_notifications() {
    let h = harness(Duration::from_secs(10)).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    settle().await;
    say(&mut ws, notif("k1", "visible", false)).await;
    settle().await;
    assert_eq!(counts(&h), (1, 1));

    h.core.lock_now(); // el usuario bloquea con el móvil presente (el comando `lock` también notifica)
    h.core.notify(h._app.handle());
    settle().await;
    assert_eq!(counts(&h), (0, 0), "al bloquear se borran las notificaciones");
    say(&mut ws, notif("k2", "secreta", false)).await;
    settle().await;
    assert_eq!(counts(&h), (0, 0), "bloqueada: no se guarda ni se muestra nada");
}

#[tokio::test]
async fn initial_notification_list_is_synced_on_connect() {
    let h = harness_with(Duration::from_secs(10), true).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");

    let req = hear(&mut ws).await;
    assert_eq!((req["t"].as_str(), req["op"].as_str()), (Some("req"), Some("notifs")));
    let mut a = notif("a", "uno", false);
    a.as_object_mut().unwrap().remove("t");
    say(&mut ws, json!({"t": "res", "id": req["id"], "ok": true, "data": {"granted": true, "items": [a]}})).await;
    settle().await;
    assert_eq!(counts(&h), (1, 0), "la lista inicial no abre ventanas emergentes");
    assert_eq!(h.core.notifs_state()["granted"], true);
}

async fn connected(h: &Harness) -> (ClientWs, SigningKey) {
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(h, &key).await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    settle().await;
    (ws, key)
}

fn shared_items(h: &Harness) -> Vec<Value> {
    h.core.notifs_state()["items"].as_array().unwrap().iter().filter(|n| n["pkg"] == "umbral.share").cloned().collect()
}

#[tokio::test]
async fn shared_text_arrives_as_an_item_with_copy_support() {
    let h = harness(Duration::from_secs(10)).await;
    let (mut ws, _k) = connected(&h).await;
    say(&mut ws, json!({"t": "share_text", "text": "https://ejemplo.com/enlace"})).await;
    settle().await;
    let items = shared_items(&h);
    assert_eq!(items.len(), 1);
    assert_eq!((items[0]["kind"].as_str(), items[0]["text"].as_str()), (Some("text"), Some("https://ejemplo.com/enlace")));
    assert_eq!(h.core.popup_items().len(), 1, "lo compartido abre la ventana emergente");
    say(&mut ws, json!({"t": "share_text", "text": "   "})).await; // vacío: se ignora
    settle().await;
    assert_eq!(shared_items(&h).len(), 1);
}

#[tokio::test]
async fn shared_files_are_saved_complete_and_never_left_half_written() {
    let inbox = std::env::temp_dir().join(format!("umbral-inbox-{}", rand::random::<u64>()));
    std::env::set_var("UMBRAL_INBOX_DIR", &inbox);
    let h = harness(Duration::from_secs(10)).await;
    let (mut ws, _k) = connected(&h).await;
    let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();

    // Archivo completo en varios trozos; el nombre intenta salir de la carpeta.
    say(&mut ws, json!({"t": "share_begin", "id": 1, "name": "..\\..\\foto?.jpg", "size": data.len()})).await;
    for c in data.chunks(100_000) {
        say(&mut ws, json!({"t": "share_chunk", "id": 1, "data": B64.encode(c)})).await;
    }
    say(&mut ws, json!({"t": "share_end", "id": 1})).await;
    settle().await;
    let saved: Vec<_> = std::fs::read_dir(&inbox).unwrap().flatten().collect();
    assert_eq!(saved.len(), 1, "un único archivo, sin temporales");
    let p = saved[0].path();
    assert!(p.starts_with(&inbox), "el nombre no puede sacar el archivo de la carpeta");
    assert_eq!(std::fs::read(&p).unwrap(), data, "contenido idéntico");
    assert!(!p.to_string_lossy().contains(".umbral-part"));

    // Incompleto: se anuncian 10 bytes y llegan 5.
    say(&mut ws, json!({"t": "share_begin", "id": 2, "name": "corto.bin", "size": 10})).await;
    say(&mut ws, json!({"t": "share_chunk", "id": 2, "data": B64.encode([1u8; 5])})).await;
    say(&mut ws, json!({"t": "share_end", "id": 2})).await;
    settle().await;
    assert!(!inbox.join("corto.bin").exists(), "un archivo incompleto no se guarda con su nombre");
    assert_eq!(std::fs::read_dir(&inbox).unwrap().count(), 1, "y no deja temporales");
    assert!(shared_items(&h).iter().any(|n| n["text"].as_str().unwrap_or("").contains("No se recibió completo")));

    // Más datos de los anunciados: se aborta.
    say(&mut ws, json!({"t": "share_begin", "id": 3, "name": "exceso.bin", "size": 4})).await;
    say(&mut ws, json!({"t": "share_chunk", "id": 3, "data": B64.encode([1u8; 9])})).await;
    say(&mut ws, json!({"t": "share_end", "id": 3})).await;
    settle().await;
    assert!(!inbox.join("exceso.bin").exists());
    assert_eq!(std::fs::read_dir(&inbox).unwrap().count(), 1);

    // Mismo nombre otra vez: no se pisa.
    say(&mut ws, json!({"t": "share_begin", "id": 4, "name": "x.txt", "size": 1})).await;
    say(&mut ws, json!({"t": "share_chunk", "id": 4, "data": B64.encode([7u8])})).await;
    say(&mut ws, json!({"t": "share_end", "id": 4})).await;
    say(&mut ws, json!({"t": "share_begin", "id": 5, "name": "x.txt", "size": 1})).await;
    say(&mut ws, json!({"t": "share_chunk", "id": 5, "data": B64.encode([8u8])})).await;
    say(&mut ws, json!({"t": "share_end", "id": 5})).await;
    settle().await;
    assert!(inbox.join("x.txt").exists() && inbox.join("x (1).txt").exists());

    // Un ejecutable recibido no conserva su extensión (no se lanza por descuido) y todo lo recibido lleva Mark of the Web.
    say(&mut ws, json!({"t": "share_begin", "id": 6, "name": "instalador.exe", "size": 2})).await;
    say(&mut ws, json!({"t": "share_chunk", "id": 6, "data": B64.encode([1u8, 2])})).await;
    say(&mut ws, json!({"t": "share_end", "id": 6})).await;
    settle().await;
    assert!(!inbox.join("instalador.exe").exists(), "el .exe no se deja con su extensión");
    assert!(inbox.join("instalador.exe.recibido").exists());
    #[cfg(windows)]
    {
        let ads = format!("{}:Zone.Identifier", inbox.join("x.txt").display());
        let zone = std::fs::read_to_string(ads).expect("flujo Zone.Identifier");
        assert!(zone.contains("ZoneId=3"), "Mark of the Web: {zone}");
    }

    // Un archivo por encima del límite ni siquiera empieza a recibirse.
    let before = std::fs::read_dir(&inbox).unwrap().count();
    say(&mut ws, json!({"t": "share_begin", "id": 7, "name": "enorme.bin", "size": 3u64 * 1024 * 1024 * 1024})).await;
    settle().await;
    assert_eq!(std::fs::read_dir(&inbox).unwrap().count(), before, "no se crea ni un temporal");
    assert!(shared_items(&h).iter().any(|n| n["title"] == "Archivo demasiado grande"));
    let _ = std::fs::remove_dir_all(&inbox);
}

#[tokio::test]
async fn wrong_code_does_not_burn_the_real_pairing_code() {
    let h = harness(Duration::from_secs(10)).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    let tok = h.core.new_token();
    // Alguien en la red prueba un código falso justo antes de que el usuario vincule.
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    say(&mut ws, json!({"t": "pair", "tok": "00".repeat(16), "pub": B64.encode(spki(&key)), "name": "intruso"})).await;
    assert_eq!(hear(&mut ws).await["t"], "error");
    // El código legítimo sigue valiendo.
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    say(&mut ws, json!({"t": "pair", "tok": tok, "pub": B64.encode(spki(&key)), "name": "Pixel"})).await;
    assert_eq!(hear(&mut ws).await["t"], "paired", "un intento erróneo no debe gastar el código bueno");
}

#[tokio::test]
async fn big_messages_before_authentication_are_refused() {
    let h = harness(Duration::from_secs(10)).await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    let pad = "a".repeat(200_000);
    ws.send(Message::Text(json!({"t": "hello", "id": "x", "pad": pad}).to_string().into())).await.unwrap();
    // Se cierra sin procesarlo: ni respuesta de error con contenido, ni sesión.
    let r = timeout(Duration::from_secs(3), ws.next()).await.expect("el servidor debe reaccionar");
    match r {
        None | Some(Err(_)) | Some(Ok(Message::Close(_))) => {}
        Some(Ok(m)) => panic!("no debía contestar a un mensaje grande antes de autenticar: {m:?}"),
    }
    assert!(h.core.snapshot().locked);
}

#[tokio::test]
async fn repeated_failures_block_the_source_for_a_while() {
    let h = harness(Duration::from_secs(10)).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;

    // Un móvil desvinculado (id desconocido) que reintenta no es un ataque: nunca bloquea el origen.
    let stranger = SigningKey::random(&mut rand::rngs::OsRng);
    for _ in 0..10 {
        let mut ws = connect(h.port, &h.fp).await.unwrap();
        assert_eq!(login(&mut ws, &stranger).await["msg"], "dispositivo no vinculado");
    }

    // Firmar con otra clave haciéndose pasar por el móvil vinculado sí cuenta: tras 8 fallos el origen se bloquea.
    let attacker = SigningKey::random(&mut rand::rngs::OsRng);
    for _ in 0..8 {
        let mut ws = connect(h.port, &h.fp).await.unwrap();
        say(&mut ws, json!({"t": "hello", "id": key_id(&spki(&key))})).await;
        let ch = hear(&mut ws).await;
        let mut data = b"umbral-v1".to_vec();
        data.extend(B64.decode(ch["n"].as_str().unwrap()).unwrap());
        let sig: p256::ecdsa::Signature = attacker.sign(&data);
        say(&mut ws, json!({"t": "auth", "sig": B64.encode(sig.to_der().as_bytes())})).await;
        assert_eq!(hear(&mut ws).await["msg"], "firma inválida");
    }
    // El origen quedó bloqueado: ni siquiera completa la conexión TLS.
    assert!(connect(h.port, &h.fp).await.is_err(), "el origen bloqueado no debe poder conectar");
}

#[tokio::test]
async fn mute_on_call_setting_defaults_on_persists_and_resists_tampering() {
    let dir = std::env::temp_dir().join(format!("umbral-mute-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&dir).unwrap();
    assert!(Core::load(dir.clone(), "fp".into()).mute_on_call(), "activado por defecto");
    Core::load(dir.clone(), "fp".into()).set_mute_on_call(false);
    assert!(!Core::load(dir.clone(), "fp".into()).mute_on_call(), "el ajuste se conserva al reiniciar");
    // Editado a mano (sin sello válido): se ignora y vuelve al valor seguro por defecto.
    std::fs::write(dir.join("settings.json"), r#"{ "mute_on_call": false, "otro": 1 }"#).unwrap();
    assert!(Core::load(dir.clone(), "fp".into()).mute_on_call());
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn tampered_config_files_are_ignored_and_never_open_the_app() {
    let dir = std::env::temp_dir().join(format!("umbral-tamper-{}", rand::random::<u64>()));
    std::fs::create_dir_all(&dir).unwrap();
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    {
        let c = Core::load(dir.clone(), "fp".into());
        c.add_device(Device { id: key_id(&spki(&key)), name: "Pixel".into(), pubkey: B64.encode(spki(&key)) });
        c.set_password("clave-segura-1").unwrap();
    }
    // Reinicio normal: todo sigue ahí.
    let c = Core::load(dir.clone(), "fp".into());
    assert!(c.find_device(&key_id(&spki(&key))).is_some());
    assert!(c.snapshot().has_password);
    drop(c);

    // Un atacante con acceso a los archivos añade su propio móvil y cambia la clave por un hash que él conoce.
    let attacker = SigningKey::random(&mut rand::rngs::OsRng);
    let forged = json!([{"id": key_id(&spki(&attacker)), "name": "intruso", "pubkey": B64.encode(spki(&attacker))}]);
    std::fs::write(dir.join("devices.json"), forged.to_string()).unwrap();
    let salt = argon2::password_hash::SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let phc = argon2::PasswordHasher::hash_password(&argon2::Argon2::default(), b"la-del-atacante", &salt).unwrap().to_string();
    std::fs::write(dir.join("password.phc"), phc).unwrap();

    let c = Core::load(dir.clone(), "fp".into());
    assert!(c.find_device(&key_id(&spki(&attacker))).is_none(), "el móvil del atacante no se acepta");
    assert!(c.find_device(&key_id(&spki(&key))).is_none(), "y ninguno queda autorizado hasta revincular");
    assert!(c.snapshot().has_password, "con la clave manipulada no se ofrece crear una nueva");
    assert!(c.unlock_with_password("la-del-atacante").is_err(), "la clave del atacante no abre");
    assert!(c.unlock_with_password("clave-segura-1").is_err(), "ni siquiera la original: hay que restablecer");
    assert!(c.snapshot().locked);
    let log = std::fs::read_to_string(dir.join("security.log")).unwrap();
    assert!(log.contains("MANIPULADO devices.json") && log.contains("MANIPULADO password.phc"), "{log}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn battery_status_is_shown_and_cleared_when_the_phone_leaves() {
    let h = harness(Duration::from_secs(10)).await;
    let (mut ws, _k) = connected(&h).await;
    say(&mut ws, json!({"t": "status", "battery": 87, "charging": true})).await;
    settle().await;
    let s = h.core.snapshot();
    assert_eq!((s.battery, s.charging), (Some(87), true));
    say(&mut ws, json!({"t": "status", "battery": 250, "charging": false})).await; // valor absurdo: se acota
    settle().await;
    assert_eq!(h.core.snapshot().battery, Some(100));
    drop(ws);
    settle().await;
    assert_eq!(h.core.snapshot().battery, None);
}

#[tokio::test]
async fn muted_apps_are_dropped_now_and_later_and_remembered() {
    let h = harness(Duration::from_secs(10)).await;
    let (mut ws, _k) = connected(&h).await;
    say(&mut ws, notif("a", "uno", false)).await;
    say(&mut ws, json!({"t": "notif", "key": "b", "pkg": "com.otra", "app": "Otra", "title": "t", "text": "x", "quiet": false, "actions": []})).await;
    settle().await;
    assert_eq!(counts(&h).0, 2);

    h.core.mute_app(h._app.handle(), "com.whatsapp", "WhatsApp");
    assert_eq!(counts(&h), (1, 1), "se quitan las notificaciones actuales de esa app y solo esas");

    say(&mut ws, notif("c", "nuevo", false)).await;
    settle().await;
    assert_eq!(counts(&h), (1, 1), "las futuras se ignoran");

    // Persistencia: un Core nuevo con el mismo directorio sigue silenciando.
    let again = Core::load(h.core.dir.clone(), "x".into());
    assert_eq!(again.muted_list().len(), 1);

    h.core.unmute_app("com.whatsapp");
    say(&mut ws, notif("d", "otra vez", false)).await;
    settle().await;
    assert_eq!(counts(&h).0, 2, "al activarla de nuevo vuelven");
}

#[tokio::test]
async fn incoming_call_rings_until_answered_and_never_hides_itself() {
    let h = harness(Duration::from_secs(10)).await;
    let (mut ws, _k) = connected(&h).await;
    let call = json!({"t": "notif", "key": "call1", "pkg": "com.google.android.dialer", "app": "Teléfono", "title": "Mamá",
        "text": "Llamada entrante", "quiet": false, "call": true, "incoming": true, "clearable": false,
        "actions": [{"index": 0, "title": "Contestar", "reply": false}, {"index": 1, "title": "Rechazar", "reply": false}]});
    say(&mut ws, call).await;
    settle().await;
    assert!(h.core.is_ringing(), "una llamada entrante hace sonar el timbre");
    assert_eq!(h.core.popup_items().len(), 1, "y abre la ventana emergente");

    // La llamada pasa a "en curso" (contestada): el timbre se corta.
    say(&mut ws, json!({"t": "notif", "key": "call1", "pkg": "com.google.android.dialer", "app": "Teléfono", "title": "Mamá",
        "text": "En curso", "quiet": false, "call": true, "incoming": false, "actions": []})).await;
    settle().await;
    assert!(!h.core.is_ringing(), "contestada: deja de sonar");

    // Otra llamada que se corta antes de contestar.
    say(&mut ws, json!({"t": "notif", "key": "call2", "pkg": "p", "app": "T", "title": "x", "text": "y", "quiet": false,
        "call": true, "incoming": true, "actions": []})).await;
    settle().await;
    assert!(h.core.is_ringing());
    say(&mut ws, json!({"t": "notif_gone", "key": "call2"})).await;
    settle().await;
    assert!(!h.core.is_ringing(), "colgada: deja de sonar");
}

/// Caso real: el móvil parpadea (se corta y se reconecta) mientras una llamada sigue sonando. Al cortarse se cierran la ventana
/// y el timbre; al reconectar, la lista inicial trae la llamada todavía sonando y tiene que volver a mostrarse y sonar.
#[tokio::test]
async fn a_call_still_ringing_is_shown_again_when_the_phone_reconnects() {
    let h = harness_with(Duration::from_secs(10), true).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;
    let call = json!({"key": "call1", "pkg": "com.google.android.dialer", "app": "Teléfono", "title": "Mio 2",
        "text": "Llamada entrante", "when": 1, "quiet": false, "call": true, "incoming": true, "clearable": false,
        "actions": [{"index": 0, "title": "Contestar", "reply": false}]});

    // Primera conexión: la lista inicial ya trae la llamada sonando.
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    let req = hear(&mut ws).await;
    say(&mut ws, json!({"t": "res", "id": req["id"], "ok": true, "data": {"granted": true, "items": [call.clone()]}})).await;
    settle().await;
    assert!(h.core.is_ringing(), "una llamada que ya sonaba al conectar debe sonar");
    assert_eq!(h.core.popup_items().len(), 1, "y abrir la ventana emergente");

    // El móvil se corta: se limpia todo, incluido el timbre.
    drop(ws);
    settle().await;
    assert!(!h.core.is_ringing());
    assert_eq!(h.core.popup_items().len(), 0);

    // Reconecta y la llamada sigue sonando: vuelve a aparecer y a sonar.
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    let req = hear(&mut ws).await;
    say(&mut ws, json!({"t": "res", "id": req["id"], "ok": true, "data": {"granted": true, "items": [call]}})).await;
    settle().await;
    assert!(h.core.is_ringing(), "tras reconectar, la llamada que sigue sonando vuelve a sonar");
    assert_eq!(h.core.popup_items().len(), 1, "y vuelve a mostrarse");
    let log = std::fs::read_to_string(h.core.dir.join("security.log")).unwrap();
    assert!(log.contains("sesión del móvil iniciada") && log.contains("sesión del móvil terminada"), "{log}");
    assert!(!log.contains("Mio 2"), "el nombre de quien llama no se escribe en el registro");
}

#[tokio::test]
async fn shared_items_can_be_removed_individually_and_all_at_once() {
    let h = harness(Duration::from_secs(10)).await;
    let (mut ws, _k) = connected(&h).await;
    say(&mut ws, json!({"t": "share_text", "text": "uno"})).await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    say(&mut ws, json!({"t": "share_text", "text": "dos"})).await;
    say(&mut ws, notif("w", "mensaje", false)).await;
    settle().await;
    assert_eq!(shared_items(&h).len(), 2);
    let key = shared_items(&h)[0]["key"].as_str().unwrap().to_string();
    h.core.remove_local(h._app.handle(), &key);
    assert_eq!(shared_items(&h).len(), 1, "se quita uno");
    h.core.remove_shared(h._app.handle());
    assert_eq!(shared_items(&h).len(), 0, "se quita todo lo compartido");
    assert_eq!(counts(&h).0, 1, "las notificaciones del móvil no se tocan");
}

#[tokio::test]
async fn wrong_certificate_fingerprint_is_refused() {
    let h = harness(Duration::from_secs(10)).await;
    assert!(connect(h.port, &"00".repeat(32)).await.is_err(), "un PC con otra huella no debe aceptarse");
}

#[tokio::test]
async fn password_unlock_forced_lock_and_attempt_limit() {
    let h = harness(Duration::from_secs(10)).await;
    assert!(h.core.set_password("123").is_err(), "clave demasiado corta");
    h.core.set_password("secreto-123").unwrap();
    h.core.lock_now();
    assert!(h.core.is_locked());

    assert!(h.core.unlock_with_password("mala").is_err());
    assert!(h.core.is_locked());
    h.core.unlock_with_password("secreto-123").unwrap();
    assert!(!h.core.is_locked(), "la clave abre aunque no haya móvil");

    // Bloqueo manual con el móvil presente: sigue bloqueado hasta usar la clave.
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    settle().await;
    h.core.lock_now();
    say(&mut ws, json!({"t": "hb", "seq": 1})).await;
    hear(&mut ws).await;
    assert!(h.core.is_locked(), "bloquear a mano gana sobre la presencia");
    h.core.unlock_with_password("secreto-123").unwrap();
    assert!(!h.core.is_locked());

    // Bloqueo manual y el móvil se reconecta (parpadeo de red): sigue bloqueado, solo la clave abre.
    h.core.lock_now();
    drop(ws);
    settle().await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    settle().await;
    assert!(h.core.is_locked(), "reconectar no debe levantar un bloqueo pedido por el usuario");
    h.core.unlock_with_password("secreto-123").unwrap();
    assert!(!h.core.is_locked());

    // Límite de intentos.
    h.core.lock_now();
    for _ in 0..5 {
        let _ = h.core.unlock_with_password("mala");
    }
    let e = h.core.unlock_with_password("secreto-123").unwrap_err();
    assert!(e.contains("Espera"), "tras 5 fallos debe haber espera, incluso con la clave buena: {e}");
}

#[tokio::test]
async fn phone_can_lock_and_unlock_the_pc_from_its_notification() {
    let h = harness(Duration::from_secs(10)).await;
    h.core.set_password("secreto-123").unwrap();
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    settle().await;
    assert!(!h.core.is_locked(), "con el móvil presente está abierta");

    say(&mut ws, json!({"t": "lock_pc"})).await;
    settle().await;
    assert!(h.core.is_locked(), "el botón Bloquear del móvil bloquea");

    say(&mut ws, json!({"t": "unlock_pc"})).await;
    settle().await;
    assert!(!h.core.is_locked(), "el botón Desbloquear del móvil abre sin la clave");

    // Un desbloqueo sin sesión autenticada no existe: el mensaje solo se atiende dentro de una sesión.
    h.core.lock_now();
    h.core.unlock_from_phone(); // con el móvil presente (sesión abierta) levanta el bloqueo
    assert!(!h.core.is_locked());
    drop(ws);
    settle().await;
    h.core.lock_now();
    h.core.unlock_from_phone();
    assert!(h.core.is_locked(), "sin móvil presente no se levanta nada");
}

#[tokio::test]
async fn removing_a_device_cuts_its_live_session() {
    let h = harness(Duration::from_secs(10)).await;
    let key = SigningKey::random(&mut rand::rngs::OsRng);
    pair_device(&h, &key).await;
    let mut ws = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut ws, &key).await["t"], "ok");
    say(&mut ws, json!({"t": "hb", "seq": 1})).await;
    assert_eq!(hear(&mut ws).await["t"], "ack");
    assert!(!h.core.snapshot().locked, "conectado y vinculado: abierto");

    // El usuario lo quita de la lista: en el siguiente latido la sesión se corta y el PC se bloquea.
    h.core.remove_device(&key_id(&spki(&key)));
    say(&mut ws, json!({"t": "hb", "seq": 2})).await;
    let m = hear(&mut ws).await;
    assert_eq!(m["t"], "error");
    assert_eq!(m["msg"], "dispositivo no vinculado");
    settle().await;
    assert!(h.core.snapshot().locked, "sin móvil vinculado, el PC queda bloqueado");

    // Y no puede volver a entrar con la misma clave.
    let mut again = connect(h.port, &h.fp).await.unwrap();
    assert_eq!(login(&mut again, &key).await["msg"], "dispositivo no vinculado");
}
