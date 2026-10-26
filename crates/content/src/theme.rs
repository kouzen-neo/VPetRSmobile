//! UI theme colors and font discovery (`theme/*.lps`).

use std::{collections::BTreeMap, fs, path::Path, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::lps::LpsDocument;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeDef {
    pub id: String,
    pub display_name: String,
    pub colors: BTreeMap<String, [u8; 4]>,
    pub fonts: Vec<PathBuf>,
}

impl Default for ThemeDef {
    fn default() -> Self {
        let mut colors = BTreeMap::new();
        colors.insert("primary".to_owned(), [0x81, 0xD4, 0xFA, 0xFF]);
        colors.insert("primarytext".to_owned(), [0x1E, 0x24, 0x30, 0xFF]);
        Self {
            id: "default".to_owned(),
            display_name: "Default".to_owned(),
            colors,
            fonts: Vec::new(),
        }
    }
}

impl ThemeDef {
    #[must_use]
    pub fn from_lps(doc: &LpsDocument, theme_dir: Option<&Path>) -> Self {
        let mut theme = Self::default();
        if let Some(first) = doc.lines.first() {
            theme.id.clone_from(&first.name);
            if !first.info.is_empty() {
                theme.display_name.clone_from(&first.info);
            }
        }
        for line in doc.lines.iter().skip(1) {
            if let Some(rgba) = parse_hex_argb(&line.info) {
                theme.colors.insert(line.name.to_ascii_lowercase(), rgba);
            }
        }
        if let Some(dir) = theme_dir {
            let fonts_dir = dir.join("fonts");
            if let Ok(entries) = fs::read_dir(fonts_dir) {
                let mut fonts: Vec<PathBuf> = entries
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension().and_then(|e| e.to_str()).is_some_and(|ext| {
                            matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc")
                        })
                    })
                    .collect();
                fonts.sort();
                theme.fonts = fonts;
            }
        }
        theme
    }

    #[must_use]
    pub fn color_or(&self, key: &str, fallback: [u8; 4]) -> [u8; 4] {
        self.colors
            .get(&key.to_ascii_lowercase())
            .copied()
            .unwrap_or(fallback)
    }
}

/// Parse VPet 6-digit (`RRGGBB`) or 8-digit (`AARRGGBB`) hex into `[R, G, B, A]`.
#[must_use]
pub fn parse_hex_argb(raw: &str) -> Option<[u8; 4]> {
    let hex = raw.trim().trim_start_matches('#');
    match hex.len() {
        6 => {
            let v = u32::from_str_radix(hex, 16).ok()?;
            Some([
                ((v >> 16) & 0xFF) as u8,
                ((v >> 8) & 0xFF) as u8,
                (v & 0xFF) as u8,
                0xFF,
            ])
        }
        8 => {
            let v = u32::from_str_radix(hex, 16).ok()?;
            Some([
                ((v >> 16) & 0xFF) as u8,
                ((v >> 8) & 0xFF) as u8,
                (v & 0xFF) as u8,
                ((v >> 24) & 0xFF) as u8,
            ])
        }
        _ => None,
    }
}
