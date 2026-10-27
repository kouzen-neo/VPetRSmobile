//! LPS parser, animation index, and content databases.

pub mod anim;
pub mod food;
pub mod lang;
pub mod lps;
pub mod scan;
pub mod text;
pub mod theme;

pub use anim::{
    AnimatType, AnimationEntry, AnimationIndex, AnimationSource, FoodKeyframe, FrameFile,
    GraphType, HitRect, MoveDef, PerMode, PetConfig, WorkDef, WorkKind, natural_cmp,
    natural_path_cmp, parse_frame_duration_ms,
};
pub use food::{FoodDatabase, FoodItem};
pub use lang::{LangTable, clean_localized_text, contains_cjk};
pub use lps::{LpsDocument, LpsError, LpsLine, LpsSub, unescape_lps};
pub use scan::{ContentDatabase, ModMetadata};
pub use text::{ClickTextEntry, LowTextEntry, TextDatabase};
pub use theme::{ThemeDef, parse_hex_argb};
pub use vpet_core::ModeType;
