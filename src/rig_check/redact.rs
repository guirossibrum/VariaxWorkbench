//! Personal data removal for reports (#249).
//!
//! The user name, host name and home folder are replaced by placeholders, and
//! device serial numbers by a short SHA-256 tag (the same serial gives the
//! same tag, so two reports from one rig can still be matched).

use super::record::sha256_hex;

#[derive(Clone, Debug, Default)]
pub struct Redactor {
    pub user: Option<String>,
    pub host: Option<String>,
    pub home: Option<String>,
    pub serials: Vec<String>,
}

impl Redactor {
    /// From the process environment and, on Unix, `/etc/hostname`. Never
    /// called by tests; they inject the values.
    pub fn from_environment() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let host = var("HOSTNAME").or_else(|| var("COMPUTERNAME")).or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty())
        });
        Self {
            user: var("USER").or_else(|| var("USERNAME")),
            host,
            home: var("HOME").or_else(|| var("USERPROFILE")),
            serials: Vec::new(),
        }
    }

    pub fn add_serial(&mut self, serial: &str) {
        let serial = serial.trim();
        if serial.len() >= 4 && !self.serials.iter().any(|s| s == serial) {
            self.serials.push(serial.to_owned());
        }
    }

    pub fn serial_tag(serial: &str) -> String {
        format!("serial-{}", &sha256_hex(serial.trim().as_bytes())[..8])
    }

    pub fn apply(&self, text: &str) -> String {
        let mut out = text.to_owned();
        // Longest, most specific first: home folder, then serials, host, user.
        if let Some(home) = self.home.as_deref().filter(|h| h.len() > 1) {
            out = out.replace(home, "~");
        }
        let mut serials: Vec<&String> = self.serials.iter().collect();
        serials.sort_by_key(|s| std::cmp::Reverse(s.len()));
        for serial in serials {
            out = out.replace(serial.as_str(), &Self::serial_tag(serial));
        }
        if let Some(host) = self.host.as_deref().filter(|h| h.len() > 1) {
            out = replace_word(&out, host, "<host>");
        }
        if let Some(user) = self.user.as_deref().filter(|u| u.len() > 1) {
            out = replace_word(&out, user, "<user>");
        }
        out
    }
}

/// Replace `needle` case-insensitively wherever it is not part of a longer
/// word, so a user called "pod" does not damage "PODxt" but `/home/pod/` and
/// `pod@studio` are still redacted.
fn replace_word(text: &str, needle: &str, with: &str) -> String {
    let lower = text.to_lowercase();
    let lower_needle = needle.to_lowercase();
    if lower.len() != text.len() || lower_needle.is_empty() {
        return text.replace(needle, with);
    }
    let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric());
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while let Some(found) = lower[index..].find(&lower_needle) {
        let start = index + found;
        let end = start + lower_needle.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        out.push_str(&text[index..start]);
        if word(before) || word(after) {
            out.push_str(&text[start..end]);
        } else {
            out.push_str(with);
        }
        index = end;
    }
    out.push_str(&text[index..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_paths_and_serials_are_removed() {
        let mut redactor = Redactor {
            user: Some("alice".into()),
            host: Some("Studio-PC".into()),
            home: Some("/home/alice".into()),
            serials: Vec::new(),
        };
        redactor.add_serial("AB12CD34");
        let text =
            "alice@studio-pc saved /home/alice/rig/report.txt; PODxt serial AB12CD34; malice stays";
        let out = redactor.apply(text);
        for secret in ["alice@", "studio-pc", "/home/alice", "AB12CD34"] {
            assert!(
                !out.to_lowercase().contains(&secret.to_lowercase()),
                "{out}"
            );
        }
        assert!(out.contains("~/rig/report.txt"));
        assert!(out.contains(&Redactor::serial_tag("AB12CD34")));
        assert!(out.contains("malice"), "parts of longer words stay: {out}");
        assert!(out.contains("<user>@<host>"));
    }
}
