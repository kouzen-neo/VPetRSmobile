//! LinePutScript (`.lps`) parser and serializer compatible with VPet data files.

use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LpsError {
    #[error("failed to read LPS file `{path}`: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LpsSub {
    pub name: String,
    pub info: String,
}

impl LpsSub {
    #[must_use]
    pub fn new(name: impl Into<String>, info: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            info: info.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LpsLine {
    pub name: String,
    pub info: String,
    pub subs: Vec<LpsSub>,
}

impl LpsLine {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.subs
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(key))
            .map(|s| s.info.as_str())
    }

    #[must_use]
    pub fn get_f64(&self, key: &str) -> Option<f64> {
        self.get(key)?.trim().parse::<f64>().ok()
    }

    #[must_use]
    pub fn get_i32(&self, key: &str) -> Option<i32> {
        self.get(key)?.trim().parse::<i32>().ok()
    }

    #[must_use]
    pub fn get_u32(&self, key: &str) -> Option<u32> {
        self.get(key)?.trim().parse::<u32>().ok()
    }

    #[must_use]
    pub fn get_bool(&self, key: &str) -> bool {
        self.get(key).is_some_and(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "true" | "1" | "t" | "yes"
            )
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LpsDocument {
    pub lines: Vec<LpsLine>,
}

impl LpsDocument {
    pub fn parse_file(path: &Path) -> Result<Self, LpsError> {
        let bytes = fs::read(path).map_err(|source| LpsError::Io {
            path: path.display().to_string(),
            source,
        })?;
        // Tolerate minor non-UTF8 bytes in community mods via lossy conversion.
        let text = String::from_utf8_lossy(&bytes);
        Ok(Self::parse(&text))
    }

    #[must_use]
    pub fn parse(input: &str) -> Self {
        let stripped = input.strip_prefix('\u{feff}').unwrap_or(input);
        let mut lines = Vec::new();

        for raw_line in stripped.lines() {
            let trimmed = raw_line.trim_end_matches('\r').trim();
            if trimmed.is_empty() || trimmed.starts_with("///") {
                continue;
            }
            if let Some(parsed) = parse_single_line(trimmed) {
                lines.push(parsed);
            }
        }

        Self { lines }
    }

    #[must_use]
    pub fn find_line(&self, name: &str) -> Option<&LpsLine> {
        self.lines
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(name))
    }
}

fn parse_single_line(line: &str) -> Option<LpsLine> {
    let tokens = split_lps_tokens(line);
    let mut iter = tokens.into_iter().filter(|tok| {
        let t = tok.trim();
        !t.is_empty() && !t.starts_with("///")
    });

    let head = iter.next()?;
    let (name, info) = split_name_info(head);
    if name.is_empty() && info.is_empty() {
        return None;
    }

    let mut subs = Vec::new();
    for seg in iter {
        let (sub_name, sub_info) = split_name_info(seg);
        if !sub_name.is_empty() || !sub_info.is_empty() {
            subs.push(LpsSub {
                name: sub_name,
                info: sub_info,
            });
        }
    }

    Some(LpsLine { name, info, subs })
}

/// Split a single LPS line on unescaped `:|` terminators (`/:` and `/|` are escaped literals).
fn split_lps_tokens(line: &str) -> Vec<&str> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut start = 0_usize;
    let mut i = 0_usize;

    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b':' && bytes[i + 1] == b'|' {
            let escaped = i > 0 && bytes[i - 1] == b'/';
            if !escaped {
                out.push(&line[start..i]);
                i += 2;
                start = i;
                continue;
            }
        }
        i += 1;
    }

    if start < bytes.len() {
        let tail = &line[start..];
        if !tail.trim().is_empty() {
            out.push(tail);
        }
    }

    out
}

/// Split `token` on the first unescaped `#` (`/#` is an escaped `#`).
fn split_name_info(token: &str) -> (String, String) {
    let bytes = token.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'#' {
            let escaped = i > 0 && bytes[i - 1] == b'/';
            if !escaped {
                return (unescape_lps(&token[..i]), unescape_lps(&token[i + 1..]));
            }
        }
    }
    (unescape_lps(token), String::new())
}

#[must_use]
pub fn unescape_lps(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match (ch, chars.peek().copied()) {
            ('\\', Some('n')) => {
                chars.next();
                out.push('\n');
            }
            ('\\', Some('r')) => {
                chars.next();
                out.push('\n');
            }
            ('\\', Some('t')) => {
                chars.next();
                out.push('\t');
            }
            ('\\', Some('\\')) => {
                chars.next();
                out.push('\\');
            }
            ('/', Some('|')) => {
                chars.next();
                out.push('|');
            }
            ('/', Some('#')) => {
                chars.next();
                out.push('#');
            }
            ('/', Some(':')) => {
                chars.next();
                out.push(':');
            }
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn handles_bom_crlf_comments_escapes_and_duplicate_subs() {
        let raw = "\u{feff}/// header comment\r\n\
            food:|name#Apple/#1:|desc#Line1\\nLine2/:ok/|pipe:|tag#a:|tag#b:|///inline comment\r\n\
            key_only#value_with_colon::|\r\n";

        let doc = LpsDocument::parse(raw);
        assert_eq!(doc.lines.len(), 2);

        let food = &doc.lines[0];
        assert_eq!(food.name, "food");
        assert_eq!(food.info, "");
        assert_eq!(food.get("name"), Some("Apple#1"));
        assert_eq!(food.get("desc"), Some("Line1\nLine2:ok|pipe"));
        // Preserves duplicate sub keys in order.
        let tags: Vec<&str> = food
            .subs
            .iter()
            .filter(|s| s.name == "tag")
            .map(|s| s.info.as_str())
            .collect();
        assert_eq!(tags, vec!["a", "b"]);

        let second = &doc.lines[1];
        assert_eq!(second.name, "key_only");
        assert_eq!(second.info, "value_with_colon:");
    }

    #[test]
    fn golden_parses_real_vpet_0000_core_files_when_present() {
        let root = Path::new("/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core");
        if !root.exists() {
            return;
        }

        let info = LpsDocument::parse_file(&root.join("info.lps")).unwrap();
        assert_eq!(
            info.find_line("vupmod").map(|l| l.info.as_str()),
            Some("Core")
        );

        let vup = LpsDocument::parse_file(&root.join("pet/vup.lps")).unwrap();
        let head = vup.find_line("touchhead").unwrap();
        assert_eq!(head.get_i32("px"), Some(159));
        assert_eq!(head.get_i32("sw"), Some(189));

        let food = LpsDocument::parse_file(&root.join("food/food.lps")).unwrap();
        assert!(food.lines.len() >= 60);
        assert_eq!(food.lines[0].get("type"), Some("Drink"));

        let click = LpsDocument::parse_file(&root.join("text/ClickText.lps")).unwrap();
        assert!(click.lines.len() >= 80);

        let theme = LpsDocument::parse_file(&root.join("theme/default.lps")).unwrap();
        assert!(theme.find_line("Primary").is_some());

        let lang = LpsDocument::parse_file(&root.join("lang/en/Base2608.lps")).unwrap();
        assert!(!lang.lines.is_empty());
    }
}
