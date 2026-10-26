//! 2D CPU sprite compositor, animation decoder/cache, chat bubble renderer, and damage tracker.

pub mod cache;
pub mod canvas;
pub mod damage;
pub mod decode;
pub mod food;
pub mod loader;
pub mod text;

pub use cache::{DEFAULT_CACHE_MAX_BYTES, FrameCache};
pub use canvas::{
    RenderError, blit_pixmap_to_bgra, clear_bgra_rect, load_sprite_scaled,
    procedural_mascot_pixmap, rgba_image_to_pixmap, scale_pixmap,
};
pub use damage::DamageRect;
pub use decode::{
    DecodedClip, RenderedFrame, decode_animated_file, decode_animation_source, decode_png_sequence,
};
pub use food::compose_food_animation;
pub use loader::AsyncClipLoader;
pub use text::{BubbleRenderer, BubbleStyle, compose_pet_and_bubble};
pub use tiny_skia::Pixmap;
