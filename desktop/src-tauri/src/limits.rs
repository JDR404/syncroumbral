//! Límites de conexiones antes de autenticar: protegen el puerto del PC de agotamiento y de pruebas de código por fuerza bruta.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Conexiones sin autenticar simultáneas en total y por origen.
const MAX_PENDING_TOTAL: usize = 16;
const MAX_PENDING_PER_IP: u32 = 3;
/// Fallos (código de vinculación o firma erróneos) tolerados antes de bloquear el origen.
const MAX_FAILS: u32 = 8;
const BLOCK_FOR: Duration = Duration::from_secs(60);
/// Si se acumulan demasiados orígenes se purgan los inactivos (memoria acotada).
const MAX_TRACKED: usize = 512;

#[derive(Default)]
struct Entry {
    pending: u32,
    fails: u32,
    blocked_until: Option<Instant>,
    last: Option<Instant>,
}

#[derive(Default)]
struct State {
    total: usize,
    by_ip: HashMap<IpAddr, Entry>,
}

#[derive(Default, Clone)]
pub struct Limiter(Arc<Mutex<State>>);

/// Reserva de una plaza de handshake; se libera al soltarse.
pub struct Permit {
    ip: IpAddr,
    limiter: Limiter,
}

impl Drop for Permit {
    fn drop(&mut self) {
        let mut s = self.limiter.0.lock().unwrap();
        s.total = s.total.saturating_sub(1);
        if let Some(e) = s.by_ip.get_mut(&self.ip) {
            e.pending = e.pending.saturating_sub(1);
        }
    }
}

impl Limiter {
    /// Reserva plaza para un handshake nuevo, o None si el origen está bloqueado o hay demasiadas conexiones pendientes.
    pub fn enter(&self, ip: IpAddr) -> Option<Permit> {
        let mut s = self.0.lock().unwrap();
        let now = Instant::now();
        if s.total >= MAX_PENDING_TOTAL {
            return None;
        }
        if s.by_ip.len() >= MAX_TRACKED {
            s.by_ip.retain(|_, e| e.pending > 0 || e.blocked_until.map_or(false, |t| t > now));
        }
        let e = s.by_ip.entry(ip).or_default();
        if e.blocked_until.map_or(false, |t| t > now) || e.pending >= MAX_PENDING_PER_IP {
            return None;
        }
        e.pending += 1;
        e.last = Some(now);
        s.total += 1;
        Some(Permit { ip, limiter: self.clone() })
    }

    /// Anota un fallo de autenticación o de vinculación; al pasar el umbral el origen queda bloqueado un rato.
    pub fn fail(&self, ip: IpAddr) {
        let mut s = self.0.lock().unwrap();
        let e = s.by_ip.entry(ip).or_default();
        e.fails += 1;
        if e.fails >= MAX_FAILS {
            e.fails = 0;
            e.blocked_until = Some(Instant::now() + BLOCK_FOR);
        }
    }

    /// Un inicio de sesión correcto limpia los fallos del origen.
    pub fn success(&self, ip: IpAddr) {
        if let Some(e) = self.0.lock().unwrap().by_ip.get_mut(&ip) {
            e.fails = 0;
            e.blocked_until = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(n: u8) -> IpAddr {
        IpAddr::from([10, 0, 0, n])
    }

    #[test]
    fn per_ip_pending_cap_and_release() {
        let l = Limiter::default();
        let a = l.enter(ip(1)).unwrap();
        let _b = l.enter(ip(1)).unwrap();
        let _c = l.enter(ip(1)).unwrap();
        assert!(l.enter(ip(1)).is_none(), "el cuarto del mismo origen se rechaza");
        assert!(l.enter(ip(2)).is_some(), "otro origen no se ve afectado");
        drop(a);
        assert!(l.enter(ip(1)).is_some(), "al soltar una plaza se puede entrar");
    }

    #[test]
    fn total_cap() {
        let l = Limiter::default();
        let mut keep = Vec::new();
        for n in 0..MAX_PENDING_TOTAL as u8 {
            keep.push(l.enter(ip(n)).unwrap());
        }
        assert!(l.enter(ip(200)).is_none());
    }

    #[test]
    fn failures_block_then_success_clears() {
        let l = Limiter::default();
        for _ in 0..MAX_FAILS {
            l.fail(ip(5));
        }
        assert!(l.enter(ip(5)).is_none(), "tras demasiados fallos el origen se bloquea");
        assert!(l.enter(ip(6)).is_some());
        l.success(ip(5));
        assert!(l.enter(ip(5)).is_some());
    }
}
