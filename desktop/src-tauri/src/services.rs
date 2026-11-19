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
    Service { id: "google-messages", name: "Mensajes de Google (SMS y RCS)", url: "https://messages.google.com/web/", hosts: &["google.com", "gstatic.com", "googleusercontent.com"] },
    Service { id: "discord", name: "Discord", url: "https://discord.com/app", hosts: &["discord.com", "discordapp.com", "discord.gg", "discordapp.net"] },
    Service { id: "slack", name: "Slack", url: "https://app.slack.com/client", hosts: &["slack.com", "slack-edge.com"] },
    Service { id: "x", name: "X (Twitter)", url: "https://x.com/messages", hosts: &["x.com", "twitter.com", "twimg.com"] },
    Service { id: "linkedin", name: "LinkedIn", url: "https://www.linkedin.com/messaging/", hosts: &["linkedin.com", "licdn.com"] },
    Service { id: "gmail", name: "Gmail", url: "https://mail.google.com/", hosts: &["google.com", "gstatic.com", "googleusercontent.com"] },
    Service { id: "outlook", name: "Outlook", url: "https://outlook.live.com/mail/", hosts: &["live.com", "office.com", "microsoft.com", "microsoftonline.com", "outlook.com"] },
    Service { id: "teams", name: "Microsoft Teams", url: "https://teams.microsoft.com/", hosts: &["microsoft.com", "microsoftonline.com", "office.com", "teams.microsoft.com"] },
];

pub fn find(id: &str) -> Option<&'static Service> {
    SERVICES.iter().find(|s| s.id == id)
}
