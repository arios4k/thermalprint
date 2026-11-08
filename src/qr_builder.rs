//! Ready-made QR code contents: a website, Wi-Fi sign-in, contact card, email, or phone number.
//! Each builder returns the text that is stored in the QR code, or a short reason it cannot.
use anyhow::{Result, bail};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WifiSecurity {
    #[default]
    Wpa,
    Wep,
    Open,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contact {
    pub name: String,
    pub organization: String,
    pub phone: String,
    pub email: String,
    pub website: String,
    pub address: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QrContent {
    Website(String),
    Wifi {
        ssid: String,
        password: String,
        security: WifiSecurity,
        hidden: bool,
    },
    Contact(Contact),
    Email {
        to: String,
        subject: String,
        body: String,
    },
    Phone(String),
}

/// Percent-encode everything except unreserved characters (RFC 3986).
fn percent(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn wifi_escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if matches!(c, '\\' | ';' | ',' | ':' | '"') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn vcard_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace(';', "\\;")
        .replace(',', "\\,")
}

fn looks_like_email(text: &str) -> bool {
    let Some((user, host)) = text.split_once('@') else {
        return false;
    };
    !user.is_empty()
        && host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !text.chars().any(|c| c.is_whitespace() || c.is_control())
        && !host.contains('@')
}

fn phone_digits(text: &str) -> Result<String> {
    let cleaned: String = text
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '(' | ')' | '.'))
        .collect();
    let digits = cleaned.strip_prefix('+').unwrap_or(&cleaned);
    if digits.len() < 3 || digits.len() > 20 || !digits.chars().all(|c| c.is_ascii_digit()) {
        bail!("Enter a phone number with digits only, such as +1 555 0100");
    }
    Ok(cleaned)
}

impl QrContent {
    pub fn payload(&self) -> Result<String> {
        match self {
            QrContent::Website(address) => {
                let address = address.trim();
                if address.is_empty() || address.chars().any(|c| c.is_whitespace()) {
                    bail!("Enter a web address without spaces");
                }
                let lower = address.to_ascii_lowercase();
                if lower.starts_with("http://") || lower.starts_with("https://") {
                    Ok(address.to_string())
                } else if address.contains("://") {
                    bail!("Only http and https addresses are supported")
                } else {
                    Ok(format!("https://{address}"))
                }
            }
            QrContent::Wifi {
                ssid,
                password,
                security,
                hidden,
            } => {
                if ssid.is_empty() {
                    bail!("Enter the network name");
                }
                if *security != WifiSecurity::Open && password.is_empty() {
                    bail!("Enter the password, or choose an open network");
                }
                let kind = match security {
                    WifiSecurity::Wpa => "WPA",
                    WifiSecurity::Wep => "WEP",
                    WifiSecurity::Open => "nopass",
                };
                let mut out = format!("WIFI:T:{kind};S:{};", wifi_escape(ssid));
                if *security != WifiSecurity::Open {
                    out.push_str(&format!("P:{};", wifi_escape(password)));
                }
                if *hidden {
                    out.push_str("H:true;");
                }
                out.push(';');
                Ok(out)
            }
            QrContent::Contact(contact) => {
                if contact.name.trim().is_empty() {
                    bail!("Enter a name for the contact card");
                }
                if !contact.email.is_empty() && !looks_like_email(&contact.email) {
                    bail!("The email address does not look right");
                }
                let mut lines = vec![
                    "BEGIN:VCARD".to_string(),
                    "VERSION:3.0".to_string(),
                    format!("FN:{}", vcard_escape(contact.name.trim())),
                    format!("N:{};;;;", vcard_escape(contact.name.trim())),
                ];
                if !contact.organization.is_empty() {
                    lines.push(format!("ORG:{}", vcard_escape(&contact.organization)));
                }
                if !contact.phone.is_empty() {
                    lines.push(format!("TEL:{}", phone_digits(&contact.phone)?));
                }
                if !contact.email.is_empty() {
                    lines.push(format!("EMAIL:{}", contact.email));
                }
                if !contact.website.is_empty() {
                    lines.push(format!("URL:{}", vcard_escape(&contact.website)));
                }
                if !contact.address.is_empty() {
                    lines.push(format!("ADR:;;{};;;;", vcard_escape(&contact.address)));
                }
                lines.push("END:VCARD".into());
                Ok(lines.join("\n"))
            }
            QrContent::Email { to, subject, body } => {
                if !looks_like_email(to.trim()) {
                    bail!("Enter a valid email address");
                }
                let mut out = format!("mailto:{}", to.trim());
                let mut parts = Vec::new();
                if !subject.is_empty() {
                    parts.push(format!("subject={}", percent(subject)));
                }
                if !body.is_empty() {
                    parts.push(format!("body={}", percent(body)));
                }
                if !parts.is_empty() {
                    out.push('?');
                    out.push_str(&parts.join("&"));
                }
                Ok(out)
            }
            QrContent::Phone(number) => Ok(format!("tel:{}", phone_digits(number)?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websites_get_a_scheme_and_reject_spaces_and_odd_schemes() {
        let site = |s: &str| QrContent::Website(s.into()).payload();
        assert_eq!(
            site("example.com/shop").unwrap(),
            "https://example.com/shop"
        );
        assert_eq!(site("HTTP://Example.com").unwrap(), "HTTP://Example.com");
        assert!(site("").is_err() && site("not a site").is_err());
        assert!(site("javascript://alert(1)").is_err());
        assert!(site("ftp://files.example").is_err());
    }

    #[test]
    fn wifi_details_are_escaped_and_open_networks_have_no_password() {
        let wifi = |ssid: &str, password: &str, security, hidden| {
            QrContent::Wifi {
                ssid: ssid.into(),
                password: password.into(),
                security,
                hidden,
            }
            .payload()
        };
        assert_eq!(
            wifi("Cafe;Guest", "pa:ss,word\\", WifiSecurity::Wpa, false).unwrap(),
            r"WIFI:T:WPA;S:Cafe\;Guest;P:pa\:ss\,word\\;;"
        );
        assert_eq!(
            wifi("Open", "ignored", WifiSecurity::Open, true).unwrap(),
            "WIFI:T:nopass;S:Open;H:true;;"
        );
        assert!(wifi("", "x", WifiSecurity::Wpa, false).is_err());
        assert!(wifi("Home", "", WifiSecurity::Wpa, false).is_err());
    }

    #[test]
    fn contact_cards_escape_separators_and_validate_fields() {
        let card = Contact {
            name: "Ada Lovelace".into(),
            organization: "Analytical, Ltd".into(),
            phone: "+1 (555) 010-0100".into(),
            email: "ada@example.com".into(),
            website: "https://example.com".into(),
            address: "1 Main St;\nLondon".into(),
        };
        let text = QrContent::Contact(card.clone()).payload().unwrap();
        assert!(text.starts_with("BEGIN:VCARD\nVERSION:3.0\nFN:Ada Lovelace"));
        assert!(text.contains("ORG:Analytical\\, Ltd"));
        assert!(text.contains("TEL:+15550100100"));
        assert!(text.contains(r"ADR:;;1 Main St\;\nLondon;;;;"));
        assert!(text.ends_with("END:VCARD"));
        let mut bad = card.clone();
        bad.email = "ada@".into();
        assert!(QrContent::Contact(bad).payload().is_err());
        let mut nameless = card;
        nameless.name = " ".into();
        assert!(QrContent::Contact(nameless).payload().is_err());
    }

    #[test]
    fn email_and_phone_links_are_percent_encoded() {
        let mail = QrContent::Email {
            to: "help@example.com".into(),
            subject: "Order #12 & more".into(),
            body: "Hi there".into(),
        };
        assert_eq!(
            mail.payload().unwrap(),
            "mailto:help@example.com?subject=Order%20%2312%20%26%20more&body=Hi%20there"
        );
        let plain = QrContent::Email {
            to: "help@example.com".into(),
            subject: String::new(),
            body: String::new(),
        };
        assert_eq!(plain.payload().unwrap(), "mailto:help@example.com");
        assert!(
            QrContent::Email {
                to: "nope".into(),
                subject: String::new(),
                body: String::new()
            }
            .payload()
            .is_err()
        );
        assert_eq!(
            QrContent::Phone("555 0100".into()).payload().unwrap(),
            "tel:5550100"
        );
        assert!(QrContent::Phone("call me".into()).payload().is_err());
        assert!(QrContent::Phone("+".into()).payload().is_err());
    }
}
