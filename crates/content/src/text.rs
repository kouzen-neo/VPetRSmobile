//! Dialogue and low-stat alert text databases (`text/*.lps`) with English localization.

use serde::{Deserialize, Serialize};
use vpet_core::ModeType;

use crate::{
    lang::{LangTable, clean_localized_text, contains_cjk},
    lps::LpsDocument,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClickTextEntry {
    pub text: String,
    pub state: String,
    pub working: Option<String>,
    pub like_min: f64,
    pub mode_mask: u8,
    pub feeling_delta: f64,
    pub likability_delta: f64,
    pub exp_delta: f64,
    pub money_delta: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LowTextEntry {
    pub text: String,
    pub mode: ModeType,
    pub strength_low: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TextDatabase {
    click_texts: Vec<ClickTextEntry>,
    low_texts: Vec<LowTextEntry>,
}

impl TextDatabase {
    pub fn load_lps_doc(&mut self, doc: &LpsDocument) {
        for line in &doc.lines {
            let tag = line.name.to_ascii_lowercase();
            if tag == "clicktext" {
                let Some(text) = line.get("Text").map(str::trim).filter(|s| !s.is_empty()) else {
                    continue;
                };
                self.click_texts.push(ClickTextEntry {
                    text: text.to_owned(),
                    state: line.get("State").unwrap_or("Nomal").to_owned(),
                    working: line.get("Working").map(str::to_owned),
                    like_min: line.get_f64("LikeMin").unwrap_or(0.0),
                    mode_mask: line.get_u32("Mode").unwrap_or(15) as u8,
                    feeling_delta: line.get_f64("Feeling").unwrap_or(0.0),
                    likability_delta: line.get_f64("Likability").unwrap_or(0.0),
                    exp_delta: line.get_f64("Exp").unwrap_or(0.0),
                    money_delta: line.get_f64("Money").unwrap_or(0.0),
                });
            } else if tag == "lowfoodtext" || tag == "lowdrinktext" {
                let Some(text) = line.get("Text").map(str::trim).filter(|s| !s.is_empty()) else {
                    continue;
                };
                let mode = line
                    .get("Mode")
                    .and_then(ModeType::parse_loose)
                    .unwrap_or(ModeType::PoorCondition);
                self.low_texts.push(LowTextEntry {
                    text: text.to_owned(),
                    mode,
                    strength_low: tag == "lowfoodtext",
                });
            }
        }
    }

    /// Translate dialogue and low-stat alert strings into `locale`, removing any untranslated
    /// CJK lines when targeting English (`"en"`).
    pub fn localize(&mut self, lang: &LangTable, locale: &str) {
        let enforce_non_cjk = locale.to_ascii_lowercase().starts_with("en");

        for entry in &mut self.click_texts {
            entry.text = clean_localized_text(lang.translate(locale, &entry.text));
            if let Some(w) = &entry.working {
                entry.working = Some(clean_localized_text(lang.translate(locale, w)));
            }
        }
        if enforce_non_cjk {
            self.click_texts
                .retain(|e| !e.text.is_empty() && !contains_cjk(&e.text));
        }

        for entry in &mut self.low_texts {
            entry.text = clean_localized_text(lang.translate(locale, &entry.text));
        }
        if enforce_non_cjk {
            self.low_texts
                .retain(|e| !e.text.is_empty() && !contains_cjk(&e.text));
        }
    }

    #[must_use]
    pub fn click_texts(&self) -> &[ClickTextEntry] {
        &self.click_texts
    }

    #[must_use]
    pub fn low_texts(&self) -> &[LowTextEntry] {
        &self.low_texts
    }

    /// Pick a deterministic dialogue line matching `mode`, `likability`, and idle/work `state`.
    #[must_use]
    pub fn pick_click_text(
        &self,
        mode: ModeType,
        likability: f64,
        state: &str,
        seed: u64,
    ) -> Option<&ClickTextEntry> {
        let candidates: Vec<&ClickTextEntry> = self
            .click_texts
            .iter()
            .filter(|e| {
                e.state.eq_ignore_ascii_case(state)
                    && likability >= e.like_min
                    && mode.matches_mask(e.mode_mask)
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        Some(candidates[(seed as usize) % candidates.len()])
    }
}
