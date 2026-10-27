//! Food and consumable item database loaded from `food/*.lps` with English localization.

use std::{collections::BTreeMap, path::Path, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    lang::{LangTable, clean_localized_text, contains_cjk},
    lps::LpsDocument,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoodItem {
    pub name: String,
    pub category: String,
    pub exp: f64,
    pub strength: f64,
    pub strength_food: f64,
    pub strength_drink: f64,
    pub feeling: f64,
    pub health: f64,
    pub likability: f64,
    pub price: f64,
    pub graph: String,
    pub desc: String,
    pub buff: Option<String>,
    pub buff_time_mins: u32,
    pub image_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct FoodDatabase {
    items: BTreeMap<String, FoodItem>,
}

impl FoodDatabase {
    pub fn load_lps_doc(&mut self, doc: &LpsDocument, image_food_dir: Option<&Path>) {
        for line in &doc.lines {
            if !line.name.eq_ignore_ascii_case("food") {
                continue;
            }
            let Some(name) = line.get("name").map(str::trim).filter(|s| !s.is_empty()) else {
                continue;
            };
            let img_target = line.get("image").unwrap_or(name);
            let image_path = image_food_dir.and_then(|dir| {
                let p = dir.join(format!("{img_target}.png"));
                if p.is_file() {
                    Some(p)
                } else {
                    let p2 = dir.join(format!("{name}.png"));
                    p2.is_file().then_some(p2)
                }
            });

            let item = FoodItem {
                name: name.to_owned(),
                category: line.get("type").unwrap_or("Meal").to_owned(),
                exp: line.get_f64("Exp").unwrap_or(0.0),
                strength: line.get_f64("Strength").unwrap_or(0.0),
                strength_food: line.get_f64("StrengthFood").unwrap_or(0.0),
                strength_drink: line.get_f64("StrengthDrink").unwrap_or(0.0),
                feeling: line.get_f64("Feeling").unwrap_or(0.0),
                health: line.get_f64("Health").unwrap_or(0.0),
                likability: line.get_f64("Likability").unwrap_or(0.0),
                price: line.get_f64("price").unwrap_or(0.0),
                graph: line.get("graph").unwrap_or("eat").to_ascii_lowercase(),
                desc: line.get("desc").unwrap_or_default().to_owned(),
                buff: line.get("Buff").map(str::to_owned),
                buff_time_mins: line.get_u32("BuffTime").unwrap_or(0),
                image_path,
            };
            self.items.insert(name.to_lowercase(), item);
        }
    }

    /// Translate food names and descriptions using `lang` (`"en"` strips untranslated CJK items).
    pub fn localize(&mut self, lang: &LangTable, locale: &str) {
        let enforce_non_cjk = locale.to_ascii_lowercase().starts_with("en");
        let old_items = std::mem::take(&mut self.items);
        for (_, mut item) in old_items {
            let trans_name = if let Some(fb) = fallback_food_translation(&item.name) {
                fb.to_owned()
            } else {
                clean_localized_text(lang.translate(locale, &item.name))
            };
            let trans_desc = clean_localized_text(lang.translate(locale, &item.desc));

            if enforce_non_cjk && contains_cjk(&trans_name) {
                continue;
            }
            item.name = trans_name;
            item.desc = if enforce_non_cjk && contains_cjk(&trans_desc) {
                String::new()
            } else {
                trans_desc
            };
            self.items.insert(item.name.to_lowercase(), item);
        }
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&FoodItem> {
        let key = name.trim().to_lowercase();
        self.items.get(&key).or_else(|| {
            self.items
                .values()
                .find(|f| f.name.to_lowercase().contains(&key))
        })
    }

    #[must_use]
    pub fn all(&self) -> Vec<&FoodItem> {
        self.items.values().collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

fn fallback_food_translation(chinese_name: &str) -> Option<&'static str> {
    match chinese_name.trim() {
        "卖动" => Some("Mizone Drink"),
        "魔掌" => Some("Monster Energy"),
        "宫廷玉液酒" => Some("Royal Jade Wine"),
        "核子汽水" => Some("Nuka-Cola"),
        "美味棒" => Some("Umaibo Stick"),
        "野战口粮" => Some("Combat Ration"),
        "超级棒棒糖" => Some("Super Lollipop"),
        "恶魔果实" => Some("Devil Fruit"),
        "春秋肠" => Some("Eternal Sausage"),
        "野生狗奶" => Some("Energy Potion"),
        "绿豆糕" => Some("Mung Bean Cake"),
        "西湖醋鱼" => Some("Vinegar Fish"),
        "昏睡红茶" => Some("Sleepy Black Tea"),
        "不烧心" => Some("Heartburn Relief"),
        "豆汁" => Some("Fermented Bean Juice"),
        "毒马铃薯" => Some("Poisonous Potato"),
        "氪石" => Some("Kryptonite"),
        "网抑云播放器" => Some("Music Player"),
        "很好的麦克风" => Some("Great Microphone"),
        "非常好的麦克风" => Some("Studio Microphone"),
        _ => None,
    }
}
