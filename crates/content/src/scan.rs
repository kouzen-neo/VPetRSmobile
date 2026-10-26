//! Content pack and mod directory scanner with `0000_core`-first override priority.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{
    anim::{AnimationIndex, PetConfig, natural_path_cmp},
    food::FoodDatabase,
    lang::{LangTable, clean_localized_text, contains_cjk},
    lps::LpsDocument,
    text::TextDatabase,
    theme::ThemeDef,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModMetadata {
    pub folder_name: String,
    pub name: String,
    pub author: String,
    pub intro: String,
    pub version: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct ContentDatabase {
    pub mods: Vec<ModMetadata>,
    pub pets: BTreeMap<String, PetConfig>,
    pub animations: BTreeMap<String, AnimationIndex>,
    pub foods: FoodDatabase,
    pub texts: TextDatabase,
    pub themes: BTreeMap<String, ThemeDef>,
    pub lang: LangTable,
    pub warnings: Vec<String>,
}

impl ContentDatabase {
    /// Load either a single mod root (such as `.../0000_core`) or a parent `mod/` directory
    /// containing `0000_core` plus additional mod folders.
    #[must_use]
    pub fn load_root(path: &Path) -> Self {
        let mut db = Self::default();
        db.merge_root(path);
        db
    }

    pub fn merge_root(&mut self, path: &Path) {
        if !path.exists() {
            self.record_warn(format!("content path does not exist: {}", path.display()));
            return;
        }
        if path.join("info.lps").is_file() || path.join("pet").is_dir() {
            self.load_single_mod(path);
            self.localize("en");
            return;
        }

        let Ok(entries) = fs::read_dir(path) else {
            self.record_warn(format!("cannot read mod container: {}", path.display()));
            return;
        };

        let mut dirs: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();

        // Ensure `0000_core` loads strictly first, followed by remaining mods sorted by folder name.
        dirs.sort_by(|a, b| {
            let a_core = a
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n == "0000_core");
            let b_core = b
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n == "0000_core");
            match (a_core, b_core) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => natural_path_cmp(a, b),
            }
        });

        for dir in dirs {
            self.load_single_mod(&dir);
        }
        self.localize("en");
    }

    /// Localize all loaded pets, foods, dialogues, themes, and mod descriptions into `locale`.
    pub fn localize(&mut self, locale: &str) {
        let enforce_non_cjk = locale.to_ascii_lowercase().starts_with("en");
        for m in &mut self.mods {
            let intro = clean_localized_text(self.lang.translate(locale, &m.intro));
            m.intro = if enforce_non_cjk && contains_cjk(&intro) {
                "Core content pack".to_owned()
            } else {
                intro
            };
        }
        for pet in self.pets.values_mut() {
            pet.localize(&self.lang, locale);
        }
        for theme in self.themes.values_mut() {
            let name = clean_localized_text(self.lang.translate(locale, &theme.display_name));
            theme.display_name = if enforce_non_cjk && contains_cjk(&name) {
                theme.id.clone()
            } else {
                name
            };
        }
        self.foods.localize(&self.lang, locale);
        self.texts.localize(&self.lang, locale);
    }

    pub fn load_single_mod(&mut self, mod_dir: &Path) {
        let folder_name = mod_dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_owned();

        let mut meta = ModMetadata {
            folder_name: folder_name.clone(),
            name: folder_name,
            author: String::new(),
            intro: String::new(),
            version: String::new(),
            path: mod_dir.to_path_buf(),
        };

        let info_path = mod_dir.join("info.lps");
        if info_path.is_file() {
            match LpsDocument::parse_file(&info_path) {
                Ok(doc) => {
                    if let Some(head) = doc.find_line("vupmod").or_else(|| doc.lines.first()) {
                        if !head.info.is_empty() {
                            meta.name.clone_from(&head.info);
                        }
                        if let Some(a) = head.get("author") {
                            meta.author = a.to_owned();
                        }
                        if let Some(v) = head.get("ver") {
                            meta.version = v.to_owned();
                        }
                    }
                    if let Some(intro) = doc.find_line("intro") {
                        meta.intro.clone_from(&intro.info);
                    }
                    self.lang.load_mod_info_doc(&doc);
                }
                Err(err) => self.record_warn(err.to_string()),
            }
        }
        self.mods.push(meta);

        // 1. Pets + animation folders
        let pet_dir = mod_dir.join("pet");
        if pet_dir.is_dir() {
            for lps_file in list_lps_files(&pet_dir) {
                match LpsDocument::parse_file(&lps_file) {
                    Ok(doc) => {
                        let cfg = PetConfig::from_lps(&doc);
                        let anim_root = pet_dir.join(&cfg.rel_path);
                        self.animations
                            .entry(cfg.id.clone())
                            .or_default()
                            .merge_pet_dir(&cfg.id, &anim_root);
                        self.pets.insert(cfg.id.clone(), cfg);
                    }
                    Err(err) => self.record_warn(err.to_string()),
                }
            }
        }

        // 2. Foods (override by food name)
        let food_dir = mod_dir.join("food");
        let image_food_dir = mod_dir.join("image").join("food");
        let img_opt = image_food_dir.is_dir().then_some(image_food_dir.as_path());
        if food_dir.is_dir() {
            for lps_file in list_lps_files(&food_dir) {
                match LpsDocument::parse_file(&lps_file) {
                    Ok(doc) => self.foods.load_lps_doc(&doc, img_opt),
                    Err(err) => self.record_warn(err.to_string()),
                }
            }
        }

        // 3. Texts (append dialogue pool)
        let text_dir = mod_dir.join("text");
        if text_dir.is_dir() {
            for lps_file in list_lps_files(&text_dir) {
                match LpsDocument::parse_file(&lps_file) {
                    Ok(doc) => self.texts.load_lps_doc(&doc),
                    Err(err) => self.record_warn(err.to_string()),
                }
            }
        }

        // 4. Themes (override by theme id)
        let theme_dir = mod_dir.join("theme");
        if theme_dir.is_dir() {
            for lps_file in list_lps_files(&theme_dir) {
                match LpsDocument::parse_file(&lps_file) {
                    Ok(doc) => {
                        let theme = ThemeDef::from_lps(&doc, Some(&theme_dir));
                        self.themes.insert(theme.id.to_ascii_lowercase(), theme);
                    }
                    Err(err) => self.record_warn(err.to_string()),
                }
            }
        }

        // 5. Localization (`lang/<locale>/*.lps`)
        let lang_dir = mod_dir.join("lang");
        if let Ok(locales) = fs::read_dir(&lang_dir) {
            let mut locale_dirs: Vec<PathBuf> = locales
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            locale_dirs.sort_by(|a, b| natural_path_cmp(a, b));
            for loc_dir in locale_dirs {
                let Some(locale) = loc_dir.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                for lps_file in list_lps_files(&loc_dir) {
                    match LpsDocument::parse_file(&lps_file) {
                        Ok(doc) => self.lang.load_locale_doc(locale, &doc),
                        Err(err) => self.record_warn(err.to_string()),
                    }
                }
            }
        }
    }

    fn record_warn(&mut self, msg: String) {
        warn!("{msg}");
        self.warnings.push(msg);
    }
}

fn list_lps_files(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("lps"))
        })
        .collect();
    out.sort_by(|a, b| natural_path_cmp(a, b));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_scanner_orders_0000_core_first_and_overrides_food() {
        let tmp = std::env::temp_dir().join(format!("vpet_rs_mod_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let core_food = tmp.join("0000_core").join("food");
        let mod_food = tmp.join("0010_custom").join("food");
        fs::create_dir_all(&core_food).unwrap();
        fs::create_dir_all(&mod_food).unwrap();

        fs::write(
            core_food.join("food.lps"),
            "food:|name#Apple:|type#Snack:|price#5:|StrengthFood#10:|\n",
        )
        .unwrap();
        fs::write(
            mod_food.join("override.lps"),
            "food:|name#Apple:|type#Snack:|price#12:|StrengthFood#25:|\n",
        )
        .unwrap();

        let db = ContentDatabase::load_root(&tmp);
        assert_eq!(db.mods.len(), 2);
        assert_eq!(db.mods[0].folder_name, "0000_core");
        assert_eq!(db.mods[1].folder_name, "0010_custom");

        let apple = db.foods.get("Apple").unwrap();
        assert_eq!(apple.price, 12.0);
        assert_eq!(apple.strength_food, 25.0);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn scans_real_0000_core_pack_and_translates_all_strings_to_english() {
        let root = Path::new("/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core");
        if !root.exists() {
            return;
        }

        let db = ContentDatabase::load_root(root);
        assert!(db.pets.contains_key("vup"));
        assert!(db.animations.get("vup").is_some_and(|a| a.len() > 150));
        assert!(db.foods.get("Fruit Juice").is_some());
        assert!(db.foods.get("Cola").is_some());
        assert!(!db.texts.click_texts().is_empty());
        assert!(db.themes.contains_key("default"));

        // Verify zero Chinese / CJK characters remain in localized English databases.
        for f in db.foods.all() {
            assert!(!contains_cjk(&f.name), "untranslated food name: {}", f.name);
            assert!(!contains_cjk(&f.desc), "untranslated food desc: {}", f.desc);
        }
        for t in db.texts.click_texts() {
            assert!(
                !contains_cjk(&t.text),
                "untranslated click text: {}",
                t.text
            );
        }
        for p in db.pets.values() {
            assert!(!contains_cjk(&p.pet_name));
            assert!(!contains_cjk(&p.intro));
        }
    }
}
