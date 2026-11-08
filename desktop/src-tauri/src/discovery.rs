//! Anuncia el PC en la red local (mDNS, `_umbral._tcp`) para que el móvil lo encuentre aunque el router
//! le cambie la IP. El móvil sigue verificando la huella del certificado: anunciarse no da ninguna confianza.

use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::net::IpAddr;

pub const SERVICE_TYPE: &str = "_umbral._tcp.local.";

/// Registra el servicio. Devuelve el demonio, que debe mantenerse vivo mientras SyncroUmbral corra.
pub fn advertise(port: u16, fingerprint: &str) -> Option<ServiceDaemon> {
    let daemon = ServiceDaemon::new().ok()?;
    // No se publica el nombre del equipo: solo un identificador corto derivado de la huella (que de todos modos
    // cualquiera ve al conectar por TLS). Distingue a varios PC en la misma red sin filtrar datos personales.
    let host = format!("umbral-{}", &fingerprint[..fingerprint.len().min(6)]);
    let props = [("fp", &fingerprint[..fingerprint.len().min(16)])];
    // IPv4 reales de la red (sin bucle local ni enlace local): son las que el móvil puede alcanzar.
    let ips: Vec<IpAddr> = local_ip_address::list_afinet_netifas()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(_, ip)| match ip {
            IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local() => Some(IpAddr::V4(v4)),
            _ => None,
        })
        .collect();
    if ips.is_empty() {
        return None;
    }
    let info = ServiceInfo::new(
        SERVICE_TYPE,
        &format!("SyncroUmbral-{host}"),
        &format!("{host}.local."),
        ips.as_slice(),
        port,
        &props[..],
    )
    .ok()?
    .enable_addr_auto(); // y se actualizan si la red cambia
    // Diagnóstico (solo compilaciones de depuración): UMBRAL_MDNS_LOG=<archivo> guarda lo que hace el demonio mDNS.
    #[cfg(debug_assertions)]
    if let (Some(path), Ok(rx)) = (std::env::var_os("UMBRAL_MDNS_LOG"), daemon.monitor()) {
        std::thread::spawn(move || {
            use std::io::Write;
            while let Ok(ev) = rx.recv() {
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                    let _ = writeln!(f, "{ev:?}");
                }
            }
        });
    }
    daemon.register(info).ok()?;
    Some(daemon)
}
