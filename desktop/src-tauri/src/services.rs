//! Servicios de mensajería web que se pueden abrir desde SyncroUmbral.

pub struct Service {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    /// Dominios por los que puede navegar la ventana (con sus subdominios).
    pub hosts: &'static [&'static str],
}

pub const SERVICES: &[Service] = &[
    Service { id: "whatsapp", name: "WhatsApp", url: "https://web.whatsapp.com/", hosts: &["whatsapp.com", "whatsapp.net", "fbcdn.net"] },
    Service { id: "instagram", name: "Instagram", url: "https://www.instagram.com/", hosts: &["instagram.com", "cdninstagram.com", "fbcdn.net", "facebook.com"] },
    Service { id: "facebook", name: "Facebook", url: "https://www.facebook.com/", hosts: &["facebook.com", "facebook.net", "fbcdn.net", "fbsbx.com"] },
    Service { id: "messenger", name: "Messenger", url: "https://www.messenger.com/", hosts: &["messenger.com", "facebook.com", "fbcdn.net"] },
    Service { id: "telegram", name: "Telegram", url: "https://web.telegram.org/a/", hosts: &["telegram.org", "t.me", "telegram.me"] },
];

pub fn find(id: &str) -> Option<&'static Service> {
    SERVICES.iter().find(|s| s.id == id)
}
