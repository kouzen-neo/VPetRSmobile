//! Localization string lookup (`lang/<locale>/*.lps` and `info.lps` `lang#<locale>` lines).

use std::collections::BTreeMap;

use crate::lps::LpsDocument;

/// Return `true` if `text` contains any Han ideograph or CJK punctuation.
#[must_use]
pub fn contains_cjk(text: &str) -> bool {
    text.chars().any(|c| {
        ('\u{4E00}'..='\u{9FFF}').contains(&c)
            || ('\u{3400}'..='\u{4DBF}').contains(&c)
            || ('\u{3000}'..='\u{303F}').contains(&c)
    })
}

/// Sanitize VPet template placeholders (`{hostname}`, `/id`) for clean English display.
#[must_use]
pub fn clean_localized_text(raw: &str) -> String {
    let stripped = raw.trim().strip_prefix("/id").unwrap_or(raw.trim());
    stripped
        .replace("{hostname}", "Master")
        .replace("{0}", "Loris")
        .trim()
        .to_owned()
}

#[derive(Debug, Clone, Default)]
pub struct LangTable {
    locales: BTreeMap<String, BTreeMap<String, String>>,
}

impl LangTable {
    pub fn load_locale_doc(&mut self, locale: &str, doc: &LpsDocument) {
        let map = self.locales.entry(locale.to_ascii_lowercase()).or_default();
        for line in &doc.lines {
            insert_pair(map, &line.name, &line.info);
            for sub in &line.subs {
                insert_pair(map, &sub.name, &sub.info);
            }
        }
    }

    pub fn load_mod_info_doc(&mut self, doc: &LpsDocument) {
        for line in &doc.lines {
            if line.name.eq_ignore_ascii_case("lang") && !line.info.is_empty() {
                let map = self
                    .locales
                    .entry(line.info.to_ascii_lowercase())
                    .or_default();
                for sub in &line.subs {
                    insert_pair(map, &sub.name, &sub.info);
                }
            }
        }
    }

    #[must_use]
    pub fn translate<'a>(&'a self, locale: &str, key: &'a str) -> &'a str {
        let trimmed = key.trim();
        let loc = locale.to_ascii_lowercase();
        if let Some(table) = self.locales.get(&loc)
            && let Some(val) = table.get(key).or_else(|| table.get(trimmed))
        {
            return val.as_str();
        }
        if let Some((prefix, _)) = loc.split_once('-')
            && let Some(table) = self.locales.get(prefix)
            && let Some(val) = table.get(key).or_else(|| table.get(trimmed))
        {
            return val.as_str();
        }
        key
    }

    #[must_use]
    pub fn locale_count(&self, locale: &str) -> usize {
        self.locales
            .get(&locale.to_ascii_lowercase())
            .map_or(0, BTreeMap::len)
    }
}

fn insert_pair(map: &mut BTreeMap<String, String>, k: &str, v: &str) {
    let kt = k.trim();
    let vt = v.trim();
    if !kt.is_empty() && !vt.is_empty() {
        map.insert(k.to_owned(), vt.to_owned());
        if kt != k {
            map.insert(kt.to_owned(), vt.to_owned());
        }
    }
}
