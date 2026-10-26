//! VPet animation folder indexer (`PetLoader` + `GraphInfo` parity) and `vup.lps` metadata.

use std::{
    cmp::Ordering,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tracing::warn;
use vpet_core::ModeType;

use crate::{
    lang::{LangTable, clean_localized_text, contains_cjk},
    lps::{LpsDocument, LpsLine},
};

/// Animation categories in exact VPet `GraphInfo.GraphType` priority order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GraphType {
    Common,
    RaisedDynamic,
    RaisedStatic,
    Move,
    Default,
    TouchHead,
    TouchBody,
    Idel,
    Sleep,
    Say,
    StateOne,
    StateTwo,
    StartUp,
    Shutdown,
    Work,
    SwitchUp,
    SwitchDown,
    SwitchThirsty,
    SwitchHunger,
    SideHideLeftMain,
    SideHideLeftRise,
    SideHideRightMain,
    SideHideRightRise,
}

impl GraphType {
    pub const ALL: [Self; 23] = [
        Self::Common,
        Self::RaisedDynamic,
        Self::RaisedStatic,
        Self::Move,
        Self::Default,
        Self::TouchHead,
        Self::TouchBody,
        Self::Idel,
        Self::Sleep,
        Self::Say,
        Self::StateOne,
        Self::StateTwo,
        Self::StartUp,
        Self::Shutdown,
        Self::Work,
        Self::SwitchUp,
        Self::SwitchDown,
        Self::SwitchThirsty,
        Self::SwitchHunger,
        Self::SideHideLeftMain,
        Self::SideHideLeftRise,
        Self::SideHideRightMain,
        Self::SideHideRightRise,
    ];

    #[must_use]
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::RaisedDynamic => "raised_dynamic",
            Self::RaisedStatic => "raised_static",
            Self::Move => "move",
            Self::Default => "default",
            Self::TouchHead => "touch_head",
            Self::TouchBody => "touch_body",
            Self::Idel => "idel",
            Self::Sleep => "sleep",
            Self::Say => "say",
            Self::StateOne => "stateone",
            Self::StateTwo => "statetwo",
            Self::StartUp => "startup",
            Self::Shutdown => "shutdown",
            Self::Work => "work",
            Self::SwitchUp => "switch_up",
            Self::SwitchDown => "switch_down",
            Self::SwitchThirsty => "switch_thirsty",
            Self::SwitchHunger => "switch_hunger",
            Self::SideHideLeftMain => "sidehide_left_main",
            Self::SideHideLeftRise => "sidehide_left_rise",
            Self::SideHideRightMain => "sidehide_right_main",
            Self::SideHideRightRise => "sidehide_right_rise",
        }
    }

    #[must_use]
    pub const fn token_pattern(self) -> &'static [&'static str] {
        match self {
            Self::Common => &["common"],
            Self::RaisedDynamic => &["raised", "dynamic"],
            Self::RaisedStatic => &["raised", "static"],
            Self::Move => &["move"],
            Self::Default => &["default"],
            Self::TouchHead => &["touch", "head"],
            Self::TouchBody => &["touch", "body"],
            Self::Idel => &["idel"],
            Self::Sleep => &["sleep"],
            Self::Say => &["say"],
            Self::StateOne => &["stateone"],
            Self::StateTwo => &["statetwo"],
            Self::StartUp => &["startup"],
            Self::Shutdown => &["shutdown"],
            Self::Work => &["work"],
            Self::SwitchUp => &["switch", "up"],
            Self::SwitchDown => &["switch", "down"],
            Self::SwitchThirsty => &["switch", "thirsty"],
            Self::SwitchHunger => &["switch", "hunger"],
            Self::SideHideLeftMain => &["sidehide", "left", "main"],
            Self::SideHideLeftRise => &["sidehide", "left", "rise"],
            Self::SideHideRightMain => &["sidehide", "right", "main"],
            Self::SideHideRightRise => &["sidehide", "right", "rise"],
        }
    }

    #[must_use]
    pub fn parse_loose(raw: &str) -> Option<Self> {
        let lower = raw.trim().to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|g| g.canonical_name() == lower || g.canonical_name().replace('_', "") == lower)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum AnimatType {
    #[default]
    Single,
    AStart,
    BLoop,
    CEnd,
}

impl AnimatType {
    #[must_use]
    pub fn parse_loose(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "single" => Some(Self::Single),
            "a" | "start" | "a_start" | "astart" => Some(Self::AStart),
            "b" | "loop" | "b_loop" | "bloop" => Some(Self::BLoop),
            "c" | "end" | "c_end" | "cend" => Some(Self::CEnd),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameFile {
    pub path: PathBuf,
    pub duration_ms: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoodKeyframe {
    pub duration_ms: u32,
    pub visible: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub rotate_deg: f32,
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AnimationSource {
    PngSequence {
        dir: PathBuf,
        frames: Vec<FrameFile>,
        loop_hint: bool,
    },
    SingleImage {
        file: PathBuf,
        duration_ms: u32,
    },
    AnimatedImage {
        file: PathBuf,
        loop_hint: bool,
    },
    Food {
        dir: PathBuf,
        front_lay: Option<String>,
        back_lay: Option<String>,
        keyframes: Vec<FoodKeyframe>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationEntry {
    pub pet_id: String,
    pub name: String,
    pub graph_type: GraphType,
    pub animat: AnimatType,
    pub mode: ModeType,
    pub source_folder: String,
    pub source: AnimationSource,
}

/// Logical hit rectangle in VPet's `500x500` coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HitRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl HitRect {
    #[must_use]
    pub const fn contains(self, lx: i32, ly: i32) -> bool {
        self.w > 0
            && self.h > 0
            && lx >= self.x
            && lx < self.x + self.w
            && ly >= self.y
            && ly < self.y + self.h
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PerMode<T> {
    pub happy: T,
    pub nomal: T,
    pub poor_condition: T,
    pub ill: T,
}

impl<T: Copy> PerMode<T> {
    #[must_use]
    pub const fn get(&self, mode: ModeType) -> T {
        match mode {
            ModeType::Happy => self.happy,
            ModeType::Nomal => self.nomal,
            ModeType::PoorCondition => self.poor_condition,
            ModeType::Ill => self.ill,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkKind {
    Work,
    Study,
    Play,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkDef {
    pub kind: WorkKind,
    pub name: String,
    pub graph: String,
    pub money_base: f64,
    pub strength_food: f64,
    pub strength_drink: f64,
    pub feeling: f64,
    pub time_mins: u32,
    pub finish_bonus: f64,
    pub level_limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveDef {
    pub graph: String,
    pub speed_x: i32,
    pub speed_y: i32,
    pub distance: u32,
    pub mode_mask: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PetConfig {
    pub id: String,
    pub pet_name: String,
    pub intro: String,
    pub rel_path: String,
    pub tags: Vec<String>,
    pub touch_head: HitRect,
    pub touch_body: HitRect,
    pub pinch: HitRect,
    pub touch_raised: PerMode<HitRect>,
    pub raise_point: PerMode<(i32, i32)>,
    pub works: Vec<WorkDef>,
    pub moves: Vec<MoveDef>,
}

impl Default for PetConfig {
    fn default() -> Self {
        Self {
            id: "vup".to_owned(),
            pet_name: "Loris".to_owned(),
            intro: String::new(),
            rel_path: "vup".to_owned(),
            tags: vec!["vup".to_owned()],
            touch_head: HitRect {
                x: 159,
                y: 16,
                w: 189,
                h: 178,
            },
            touch_body: HitRect {
                x: 166,
                y: 206,
                w: 163,
                h: 136,
            },
            pinch: HitRect {
                x: 149,
                y: 128,
                w: 56,
                h: 59,
            },
            touch_raised: PerMode {
                happy: HitRect {
                    x: 0,
                    y: 50,
                    w: 500,
                    h: 200,
                },
                nomal: HitRect {
                    x: 0,
                    y: 50,
                    w: 500,
                    h: 200,
                },
                poor_condition: HitRect {
                    x: 0,
                    y: 50,
                    w: 500,
                    h: 200,
                },
                ill: HitRect {
                    x: 0,
                    y: 200,
                    w: 500,
                    h: 300,
                },
            },
            raise_point: PerMode {
                happy: (290, 128),
                nomal: (290, 128),
                poor_condition: (290, 128),
                ill: (225, 115),
            },
            works: Vec::new(),
            moves: Vec::new(),
        }
    }
}

impl PetConfig {
    #[must_use]
    pub fn from_lps(doc: &LpsDocument) -> Self {
        let mut cfg = Self::default();
        if let Some(first) = doc.find_line("pet").or_else(|| doc.lines.first()) {
            if !first.info.is_empty() {
                cfg.id.clone_from(&first.info);
            }
            if let Some(v) = first.get("petname") {
                cfg.pet_name = v.to_owned();
            }
            if let Some(v) = first.get("intor") {
                cfg.intro = v.to_owned();
            }
            if let Some(v) = first.get("path") {
                cfg.rel_path = v.replace('\\', "/");
            }
        }
        if let Some(tag_line) = doc.find_line("tag") {
            cfg.tags = tag_line
                .info
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
        }
        if let Some(th) = doc.find_line("touchhead") {
            cfg.touch_head = parse_rect(th, "", cfg.touch_head);
        }
        if let Some(tb) = doc.find_line("touchbody") {
            cfg.touch_body = parse_rect(tb, "", cfg.touch_body);
        }
        if let Some(p) = doc.find_line("pinch") {
            cfg.pinch = parse_rect(p, "", cfg.pinch);
        }
        if let Some(tr) = doc.find_line("touchraised") {
            cfg.touch_raised = PerMode {
                happy: parse_rect(tr, "happy_", cfg.touch_raised.happy),
                nomal: parse_rect(tr, "nomal_", cfg.touch_raised.nomal),
                poor_condition: parse_rect(tr, "poorcondition_", cfg.touch_raised.poor_condition),
                ill: parse_rect(tr, "ill_", cfg.touch_raised.ill),
            };
        }
        if let Some(rp) = doc.find_line("raisepoint") {
            cfg.raise_point = PerMode {
                happy: (
                    rp.get_i32("happy_x").unwrap_or(cfg.raise_point.happy.0),
                    rp.get_i32("happy_y").unwrap_or(cfg.raise_point.happy.1),
                ),
                nomal: (
                    rp.get_i32("nomal_x").unwrap_or(cfg.raise_point.nomal.0),
                    rp.get_i32("nomal_y").unwrap_or(cfg.raise_point.nomal.1),
                ),
                poor_condition: (
                    rp.get_i32("poorcondition_x")
                        .unwrap_or(cfg.raise_point.poor_condition.0),
                    rp.get_i32("poorcondition_y")
                        .unwrap_or(cfg.raise_point.poor_condition.1),
                ),
                ill: (
                    rp.get_i32("ill_x").unwrap_or(cfg.raise_point.ill.0),
                    rp.get_i32("ill_y").unwrap_or(cfg.raise_point.ill.1),
                ),
            };
        }

        for line in &doc.lines {
            if line.name.eq_ignore_ascii_case("work") {
                let kind = match line
                    .get("Type")
                    .unwrap_or("Work")
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "study" => WorkKind::Study,
                    "play" => WorkKind::Play,
                    _ => WorkKind::Work,
                };
                cfg.works.push(WorkDef {
                    kind,
                    name: line.get("Name").unwrap_or_default().to_owned(),
                    graph: line.get("Graph").unwrap_or_default().to_ascii_lowercase(),
                    money_base: line.get_f64("MoneyBase").unwrap_or(0.0),
                    strength_food: line.get_f64("StrengthFood").unwrap_or(0.0),
                    strength_drink: line.get_f64("StrengthDrink").unwrap_or(0.0),
                    feeling: line.get_f64("Feeling").unwrap_or(0.0),
                    time_mins: line.get_u32("Time").unwrap_or(10),
                    finish_bonus: line.get_f64("FinishBonus").unwrap_or(0.0),
                    level_limit: line.get_u32("LevelLimit").unwrap_or(0),
                });
            } else if line.name.eq_ignore_ascii_case("move") {
                cfg.moves.push(MoveDef {
                    graph: line.get("graph").unwrap_or_default().to_ascii_lowercase(),
                    speed_x: line.get_i32("SpeedX").unwrap_or(0),
                    speed_y: line.get_i32("SpeedY").unwrap_or(0),
                    distance: line.get_u32("Distance").unwrap_or(5),
                    mode_mask: line.get_u32("ModeType").unwrap_or(15) as u8,
                });
            }
        }

        cfg
    }

    pub fn localize(&mut self, lang: &LangTable, locale: &str) {
        let enforce_non_cjk = locale.to_ascii_lowercase().starts_with("en");
        let name = clean_localized_text(lang.translate(locale, &self.pet_name));
        self.pet_name = if enforce_non_cjk && contains_cjk(&name) {
            "Loris".to_owned()
        } else {
            name
        };

        let intro = clean_localized_text(lang.translate(locale, &self.intro));
        self.intro = if enforce_non_cjk && contains_cjk(&intro) {
            "Default virtual desktop pet".to_owned()
        } else {
            intro
        };

        for work in &mut self.works {
            let wname = clean_localized_text(lang.translate(locale, &work.name));
            work.name = if enforce_non_cjk && contains_cjk(&wname) {
                work.graph.clone()
            } else {
                wname
            };
        }
    }
}

fn parse_rect(line: &LpsLine, prefix: &str, fallback: HitRect) -> HitRect {
    HitRect {
        x: line.get_i32(&format!("{prefix}px")).unwrap_or(fallback.x),
        y: line.get_i32(&format!("{prefix}py")).unwrap_or(fallback.y),
        w: line.get_i32(&format!("{prefix}sw")).unwrap_or(fallback.w),
        h: line.get_i32(&format!("{prefix}sh")).unwrap_or(fallback.h),
    }
}

#[derive(Debug, Clone, Default)]
pub struct AnimationIndex {
    entries: Vec<AnimationEntry>,
}

impl AnimationIndex {
    #[must_use]
    pub fn scan_pet_dir(pet_id: &str, startup_dir: &Path) -> Self {
        let mut index = Self::default();
        index.merge_pet_dir(pet_id, startup_dir);
        index
    }

    pub fn merge_pet_dir(&mut self, pet_id: &str, startup_dir: &Path) {
        if !startup_dir.is_dir() {
            return;
        }
        walk_graph_dir(pet_id, startup_dir, startup_dir, &mut self.entries);
    }

    #[must_use]
    pub fn entries(&self) -> &[AnimationEntry] {
        &self.entries
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Query matching animations with automatic mode fallback (`mode -> Nomal -> any`).
    #[must_use]
    pub fn query(
        &self,
        graph_type: GraphType,
        name: Option<&str>,
        animat: AnimatType,
        mode: ModeType,
    ) -> Vec<&AnimationEntry> {
        let match_name = |e: &&AnimationEntry| name.is_none_or(|n| e.name.eq_ignore_ascii_case(n));

        let exact: Vec<&AnimationEntry> = self
            .entries
            .iter()
            .filter(|e| e.graph_type == graph_type && e.animat == animat && e.mode == mode)
            .filter(match_name)
            .collect();
        if !exact.is_empty() {
            return exact;
        }

        if mode != ModeType::Nomal {
            let nomal: Vec<&AnimationEntry> = self
                .entries
                .iter()
                .filter(|e| {
                    e.graph_type == graph_type && e.animat == animat && e.mode == ModeType::Nomal
                })
                .filter(match_name)
                .collect();
            if !nomal.is_empty() {
                return nomal;
            }
        }

        self.entries
            .iter()
            .filter(|e| e.graph_type == graph_type && e.animat == animat)
            .filter(match_name)
            .collect()
    }

    /// Pick one deterministic variant using `seed`.
    #[must_use]
    pub fn pick(
        &self,
        graph_type: GraphType,
        name: Option<&str>,
        animat: AnimatType,
        mode: ModeType,
        seed: u64,
    ) -> Option<&AnimationEntry> {
        let candidates = self.query(graph_type, name, animat, mode);
        if candidates.is_empty() {
            None
        } else {
            Some(candidates[(seed as usize) % candidates.len()])
        }
    }
}

fn walk_graph_dir(pet_id: &str, dir: &Path, startup_dir: &Path, out: &mut Vec<AnimationEntry>) {
    let info_path = dir.join("info.lps");
    if info_path.is_file() {
        match LpsDocument::parse_file(&info_path) {
            Ok(doc) => {
                for line in &doc.lines {
                    load_from_info_line(pet_id, dir, startup_dir, line, out);
                }
            }
            Err(err) => warn!(%err, "failed to parse animation info.lps"),
        }
        return;
    }

    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };

    let mut subdirs = Vec::new();
    let mut files = Vec::new();
    for entry in read_dir.filter_map(Result::ok) {
        let p = entry.path();
        if p.is_dir() {
            subdirs.push(p);
        } else if p.is_file() {
            files.push(p);
        }
    }
    subdirs.sort_by(|a, b| natural_path_cmp(a, b));
    files.sort_by(|a, b| natural_path_cmp(a, b));

    if subdirs.is_empty() {
        load_leaf_files(pet_id, dir, startup_dir, &files, None, out);
    } else {
        for sub in subdirs {
            walk_graph_dir(pet_id, &sub, startup_dir, out);
        }
    }
}

fn load_from_info_line(
    pet_id: &str,
    dir: &Path,
    startup_dir: &Path,
    line: &LpsLine,
    out: &mut Vec<AnimationEntry>,
) {
    let kind = line.name.to_ascii_lowercase();
    let target = match line.get("path") {
        Some(rel) if !rel.is_empty() => dir.join(rel.replace('\\', "/")),
        _ => dir.to_path_buf(),
    };

    match kind.as_str() {
        "pnganimation" => {
            if target.is_dir() {
                let files = collect_files(&target);
                load_leaf_files(pet_id, &target, startup_dir, &files, Some(line), out);
            } else if target.is_file() {
                push_single_image(pet_id, &target, startup_dir, Some(line), out);
            }
        }
        "picture" => {
            if target.is_file() {
                push_single_image(pet_id, &target, startup_dir, Some(line), out);
            } else if target.is_dir() {
                let files = collect_files(&target);
                if let Some(first) = files.first() {
                    push_single_image(pet_id, first, startup_dir, Some(line), out);
                }
            }
        }
        "apnganimation" => {
            if target.is_file() {
                let meta = resolve_graph_meta(&target, false, startup_dir, Some(line));
                out.push(AnimationEntry {
                    pet_id: pet_id.to_owned(),
                    name: meta.name,
                    graph_type: meta.graph_type,
                    animat: meta.animat,
                    mode: meta.mode,
                    source_folder: meta.source_folder,
                    source: AnimationSource::AnimatedImage {
                        file: target,
                        loop_hint: line.get_bool("loop"),
                    },
                });
            }
        }
        "foodanimation" => {
            let meta = resolve_graph_meta(&target, true, startup_dir, Some(line));
            let mut keyframes = Vec::new();
            let mut idx = 0_u32;
            while let Some(spec) = line.get(&format!("a{idx}")) {
                if let Some(kf) = parse_food_keyframe(spec) {
                    keyframes.push(kf);
                }
                idx += 1;
            }
            out.push(AnimationEntry {
                pet_id: pet_id.to_owned(),
                name: meta.name,
                graph_type: meta.graph_type,
                animat: meta.animat,
                mode: meta.mode,
                source_folder: meta.source_folder,
                source: AnimationSource::Food {
                    dir: target,
                    front_lay: line.get("front_lay").map(str::to_owned),
                    back_lay: line.get("back_lay").map(str::to_owned),
                    keyframes,
                },
            });
        }
        _ => {}
    }
}

fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    files.sort_by(|a, b| natural_path_cmp(a, b));
    files
}

fn load_leaf_files(
    pet_id: &str,
    dir: &Path,
    startup_dir: &Path,
    files: &[PathBuf],
    info: Option<&LpsLine>,
    out: &mut Vec<AnimationEntry>,
) {
    let mut pngs = Vec::new();
    for file in files {
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        match ext.as_str() {
            "png" => pngs.push(file.clone()),
            "apng" | "gif" => {
                let meta = resolve_graph_meta(file, false, startup_dir, info);
                out.push(AnimationEntry {
                    pet_id: pet_id.to_owned(),
                    name: meta.name,
                    graph_type: meta.graph_type,
                    animat: meta.animat,
                    mode: meta.mode,
                    source_folder: meta.source_folder,
                    source: AnimationSource::AnimatedImage {
                        file: file.clone(),
                        loop_hint: info.is_some_and(|l| l.get_bool("loop")),
                    },
                });
            }
            _ => {}
        }
    }

    if pngs.len() == 1 {
        push_single_image(pet_id, &pngs[0], startup_dir, info, out);
    } else if pngs.len() > 1 {
        let meta = resolve_graph_meta(dir, true, startup_dir, info);
        let frames = pngs
            .into_iter()
            .map(|p| FrameFile {
                duration_ms: parse_frame_duration_ms(&p),
                path: p,
            })
            .collect();
        out.push(AnimationEntry {
            pet_id: pet_id.to_owned(),
            name: meta.name,
            graph_type: meta.graph_type,
            animat: meta.animat,
            mode: meta.mode,
            source_folder: meta.source_folder,
            source: AnimationSource::PngSequence {
                dir: dir.to_path_buf(),
                frames,
                loop_hint: info.is_some_and(|l| l.get_bool("loop")),
            },
        });
    }
}

fn push_single_image(
    pet_id: &str,
    file: &Path,
    startup_dir: &Path,
    info: Option<&LpsLine>,
    out: &mut Vec<AnimationEntry>,
) {
    let meta = resolve_graph_meta(file, false, startup_dir, info);
    let duration_ms = info
        .and_then(|l| l.get_u32("length"))
        .unwrap_or_else(|| parse_frame_duration_ms(file));
    out.push(AnimationEntry {
        pet_id: pet_id.to_owned(),
        name: meta.name,
        graph_type: meta.graph_type,
        animat: meta.animat,
        mode: meta.mode,
        source_folder: meta.source_folder,
        source: AnimationSource::SingleImage {
            file: file.to_path_buf(),
            duration_ms,
        },
    });
}

struct ResolvedMeta {
    name: String,
    graph_type: GraphType,
    animat: AnimatType,
    mode: ModeType,
    source_folder: String,
}

/// Exact port of `GraphInfo(FileSystemInfo path, ILine info)` token extraction.
fn resolve_graph_meta(
    path: &Path,
    is_dir: bool,
    startup_dir: &Path,
    info: Option<&LpsLine>,
) -> ResolvedMeta {
    let parent_dir = if is_dir {
        path
    } else {
        path.parent().unwrap_or(startup_dir)
    };
    let rel_folder = parent_dir
        .strip_prefix(startup_dir)
        .unwrap_or(parent_dir)
        .to_string_lossy()
        .replace('\\', "/");

    let rel_target = if is_dir {
        rel_folder.clone()
    } else {
        let rel = path.strip_prefix(startup_dir).unwrap_or(path);
        let s = rel.to_string_lossy().replace('\\', "/");
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) if s.len() > ext.len() + 1 => s[..s.len() - ext.len() - 1].to_owned(),
            _ => s,
        }
    };

    let mut tokens: Vec<String> = rel_target
        .to_ascii_lowercase()
        .split(['/', '_', '\\'])
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .collect();

    let mode = info
        .and_then(|l| l.get("mode"))
        .and_then(ModeType::parse_loose)
        .unwrap_or_else(|| {
            if remove_first_token(&mut tokens, "happy") {
                ModeType::Happy
            } else if remove_first_token(&mut tokens, "nomal")
                || remove_first_token(&mut tokens, "normal")
            {
                ModeType::Nomal
            } else if remove_first_token(&mut tokens, "poorcondition") {
                ModeType::PoorCondition
            } else if remove_first_token(&mut tokens, "ill") {
                ModeType::Ill
            } else {
                ModeType::Nomal
            }
        });

    let graph_type = info
        .and_then(|l| l.get("graph"))
        .and_then(GraphType::parse_loose)
        .unwrap_or_else(|| {
            for candidate in GraphType::ALL {
                let pat = candidate.token_pattern();
                if let Some(idx) = find_subslice(&tokens, pat) {
                    tokens.drain(idx..idx + pat.len());
                    return candidate;
                }
            }
            GraphType::Common
        });

    let animat = info
        .and_then(|l| l.get("animat"))
        .and_then(AnimatType::parse_loose)
        .unwrap_or_else(|| {
            if remove_first_token(&mut tokens, "a") || remove_first_token(&mut tokens, "start") {
                AnimatType::AStart
            } else if remove_first_token(&mut tokens, "b")
                || remove_first_token(&mut tokens, "loop")
            {
                AnimatType::BLoop
            } else if remove_first_token(&mut tokens, "c") || remove_first_token(&mut tokens, "end")
            {
                AnimatType::CEnd
            } else {
                let _ = remove_first_token(&mut tokens, "single");
                AnimatType::Single
            }
        });

    let mut name = info.map(|l| l.info.trim().to_owned()).unwrap_or_default();

    if name.is_empty() {
        while let Some(last) = tokens.last() {
            if last.parse::<f64>().is_ok() || last.starts_with('~') {
                tokens.pop();
            } else {
                break;
            }
        }
        if let Some(last) = tokens.last() {
            name.clone_from(last);
        }
    }
    if name.is_empty() {
        name = graph_type.canonical_name().to_owned();
    }

    ResolvedMeta {
        name,
        graph_type,
        animat,
        mode,
        source_folder: if rel_folder == "." {
            String::new()
        } else {
            rel_folder
        },
    }
}

fn remove_first_token(tokens: &mut Vec<String>, needle: &str) -> bool {
    if let Some(idx) = tokens.iter().position(|t| t == needle) {
        tokens.remove(idx);
        true
    } else {
        false
    }
}

fn find_subslice(haystack: &[String], needle: &[&str]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|win| win.iter().zip(needle.iter()).all(|(h, n)| h.as_str() == *n))
}

fn parse_food_keyframe(spec: &str) -> Option<FoodKeyframe> {
    let parts: Vec<&str> = spec.split(',').map(str::trim).collect();
    let duration_ms = parts.first()?.parse::<u32>().ok()?.max(16);
    if parts.len() < 4 {
        return Some(FoodKeyframe {
            duration_ms,
            visible: false,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            rotate_deg: 0.0,
            opacity: 0.0,
        });
    }
    let x = parts[1].parse::<f32>().unwrap_or(0.0);
    let y = parts[2].parse::<f32>().unwrap_or(0.0);
    let width = parts[3].parse::<f32>().unwrap_or(64.0);
    let rotate_deg = parts
        .get(4)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0);
    let opacity = parts
        .get(5)
        .and_then(|s| s.parse::<f32>().ok())
        .map_or(1.0, |v| (v / 100.0).clamp(0.0, 1.0));
    Some(FoodKeyframe {
        duration_ms,
        visible: true,
        x,
        y,
        width,
        rotate_deg,
        opacity,
    })
}

/// Extract frame duration in milliseconds from trailing `_<ms>` stem (default `125` ms).
#[must_use]
pub fn parse_frame_duration_ms(path: &Path) -> u32 {
    path.file_stem()
        .and_then(|s| s.to_str())
        .and_then(|stem| stem.rsplit_once('_'))
        .and_then(|(_, tail)| tail.parse::<u32>().ok())
        .unwrap_or(125)
        .max(16)
}

/// Natural filename comparison (`_2_` < `_10_`, `loopA_002` < `loopA_010`).
#[must_use]
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ca = a.chars().peekable();
    let mut cb = b.chars().peekable();

    while let (Some(&xa), Some(&xb)) = (ca.peek(), cb.peek()) {
        if xa.is_ascii_digit() && xb.is_ascii_digit() {
            let na = take_number(&mut ca);
            let nb = take_number(&mut cb);
            match na.cmp(&nb) {
                Ordering::Equal => continue,
                non_eq => return non_eq,
            }
        }
        let la = xa.to_ascii_lowercase();
        let lb = xb.to_ascii_lowercase();
        match la.cmp(&lb) {
            Ordering::Equal => {
                ca.next();
                cb.next();
            }
            non_eq => return non_eq,
        }
    }

    ca.count().cmp(&cb.count())
}

fn take_number(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> u64 {
    let mut val = 0_u64;
    while let Some(&c) = chars.peek() {
        if let Some(d) = c.to_digit(10) {
            val = val.saturating_mul(10).saturating_add(u64::from(d));
            chars.next();
        } else {
            break;
        }
    }
    val
}

pub fn natural_path_cmp(a: &Path, b: &Path) -> Ordering {
    let sa = a.file_name().and_then(|s| s.to_str()).unwrap_or_default();
    let sb = b.file_name().and_then(|s| s.to_str()).unwrap_or_default();
    natural_cmp(sa, sb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_sort_orders_numeric_frame_indices() {
        let mut names = vec!["loop_010_125.png", "loop_002_125.png", "loop_000_250.png"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            names,
            vec!["loop_000_250.png", "loop_002_125.png", "loop_010_125.png"]
        );
        assert_eq!(parse_frame_duration_ms(Path::new("_003_375.png")), 375);
        assert_eq!(parse_frame_duration_ms(Path::new("static.png")), 125);
    }

    #[test]
    fn indexes_real_vup_pet_animations_when_present() {
        let vup_dir =
            Path::new("/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core/pet/vup");
        let vup_lps = Path::new(
            "/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core/pet/vup.lps",
        );
        if !vup_dir.exists() {
            return;
        }

        let doc = LpsDocument::parse_file(vup_lps).unwrap();
        let cfg = PetConfig::from_lps(&doc);
        assert_eq!(cfg.id, "vup");
        assert!(!cfg.works.is_empty());
        assert!(!cfg.moves.is_empty());

        let idx = AnimationIndex::scan_pet_dir("vup", vup_dir);
        assert!(
            idx.len() > 150,
            "expected >150 animations, got {}",
            idx.len()
        );

        // Core MVP animations must all resolve cleanly.
        assert!(
            !idx.query(
                GraphType::Default,
                None,
                AnimatType::Single,
                ModeType::Nomal
            )
            .is_empty()
        );
        assert!(
            !idx.query(GraphType::Sleep, None, AnimatType::BLoop, ModeType::Nomal)
                .is_empty()
        );
        assert!(
            !idx.query(
                GraphType::TouchHead,
                None,
                AnimatType::AStart,
                ModeType::Nomal
            )
            .is_empty()
        );
        assert!(
            !idx.query(
                GraphType::RaisedDynamic,
                None,
                AnimatType::Single,
                ModeType::Nomal
            )
            .is_empty()
        );
        assert!(
            !idx.query(
                GraphType::Move,
                Some("walk.left"),
                AnimatType::BLoop,
                ModeType::Nomal
            )
            .is_empty()
        );
    }
}
