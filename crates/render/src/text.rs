//! Pure-Rust chat bubble shaping and rasterization via `cosmic-text` + `tiny-skia`.

use std::path::Path;

use cosmic_text::{
    Attrs, Buffer, Color as CtColor, Family, FontSystem, Metrics, Shaping, SwashCache,
};
use tiny_skia::{
    BlendMode, Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke,
    Transform,
};

#[derive(Debug, Clone, Copy)]
pub struct BubbleStyle {
    pub bg_rgba: [u8; 4],
    pub border_rgba: [u8; 4],
    pub text_rgba: [u8; 4],
    pub font_px: f32,
    pub padding_px: u32,
    pub corner_radius: f32,
}

impl Default for BubbleStyle {
    fn default() -> Self {
        Self {
            bg_rgba: [252, 253, 255, 240],
            border_rgba: [129, 212, 250, 255],
            text_rgba: [30, 36, 48, 255],
            font_px: 15.0,
            padding_px: 10,
            corner_radius: 10.0,
        }
    }
}

pub struct BubbleRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
}

impl Default for BubbleRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl BubbleRenderer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    pub fn load_font_file(&mut self, path: &Path) {
        let _ = self.font_system.db_mut().load_font_file(path);
    }

    /// Render `text` into a rounded speech-bubble `Pixmap` capped at `max_width` pixels.
    pub fn render_bubble(
        &mut self,
        text: &str,
        max_width: u32,
        style: &BubbleStyle,
    ) -> Option<Pixmap> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }

        let font_px = style.font_px.max(10.0);
        let line_h = (font_px * 1.35).ceil();
        let mut buffer = Buffer::new(&mut self.font_system, Metrics::new(font_px, line_h));
        let inner_max_w = max_width.saturating_sub(style.padding_px * 2).max(80) as f32;
        buffer.set_size(Some(inner_max_w), None);
        buffer.set_text(
            trimmed,
            &Attrs::new().family(Family::SansSerif),
            Shaping::Advanced,
            None,
        );
        buffer.shape_until_scroll(&mut self.font_system, false);

        let mut text_w = 0.0_f32;
        let mut text_h = 0.0_f32;
        for run in buffer.layout_runs() {
            text_w = text_w.max(run.line_w);
            text_h = text_h.max(run.line_top + run.line_height);
        }
        if text_w <= 0.5 || text_h <= 0.5 {
            text_w = font_px * 2.0;
            text_h = line_h;
        }

        let pad = style.padding_px as f32;
        let tail_h = 8.0_f32;
        let box_w = (text_w.ceil() + pad * 2.0).max(48.0);
        let box_h = (text_h.ceil() + pad * 2.0).max(line_h + pad * 2.0);
        let total_w = (box_w.ceil() as u32).max(1);
        let total_h = ((box_h + tail_h).ceil() as u32).max(1);

        let mut pixmap = Pixmap::new(total_w, total_h)?;

        if let Some(bubble_path) =
            build_rounded_bubble_path(box_w, box_h, style.corner_radius, tail_h)
        {
            let mut fill = Paint {
                anti_alias: true,
                ..Paint::default()
            };
            let [br, bg, bb, ba] = style.bg_rgba;
            fill.set_color(Color::from_rgba8(br, bg, bb, ba));
            pixmap.fill_path(
                &bubble_path,
                &fill,
                FillRule::Winding,
                Transform::identity(),
                None,
            );

            let mut border = Paint {
                anti_alias: true,
                ..Paint::default()
            };
            let [sr, sg, sb, sa] = style.border_rgba;
            border.set_color(Color::from_rgba8(sr, sg, sb, sa));
            let stroke = Stroke {
                width: 2.0,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            pixmap.stroke_path(&bubble_path, &border, &stroke, Transform::identity(), None);
        }

        let [tr, tg, tb, ta] = style.text_rgba;
        let ct_color = CtColor::rgba(tr, tg, tb, ta);
        let ox = style.padding_px as i32;
        let oy = style.padding_px as i32;
        let pw = pixmap.width() as i32;
        let ph = pixmap.height() as i32;
        let data = pixmap.data_mut();

        buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            ct_color,
            |gx, gy, gw, gh, c| {
                let alpha = c.a();
                if alpha == 0 {
                    return;
                }
                for dy in 0..gh as i32 {
                    let py = oy + gy + dy;
                    if py < 0 || py >= ph {
                        continue;
                    }
                    for dx in 0..gw as i32 {
                        let px = ox + gx + dx;
                        if px < 0 || px >= pw {
                            continue;
                        }
                        let idx = ((py as usize) * (pw as usize) + (px as usize)) * 4;
                        blend_premultiplied_over(
                            &mut data[idx..idx + 4],
                            [c.r(), c.g(), c.b(), alpha],
                        );
                    }
                }
            },
        );

        Some(pixmap)
    }
}

/// Composite `pet` with an optional speech `bubble` centered above `pet`, returning
/// `(combined_pixmap, top_left_delta_from_pet)`.
#[must_use]
pub fn compose_pet_and_bubble(pet: &Pixmap, bubble: Option<&Pixmap>) -> (Pixmap, (i32, i32)) {
    let Some(bub) = bubble else {
        return (pet.clone(), (0, 0));
    };

    let gap = 4_i32;
    let bub_rel_x = (pet.width() as i32 - bub.width() as i32) / 2;
    let bub_rel_y = -(bub.height() as i32) - gap;

    let min_x = 0_i32.min(bub_rel_x);
    let min_y = 0_i32.min(bub_rel_y);
    let max_x = (pet.width() as i32).max(bub_rel_x + bub.width() as i32);
    let max_y = (pet.height() as i32).max(bub_rel_y + bub.height() as i32);

    let total_w = (max_x - min_x).max(1) as u32;
    let total_h = (max_y - min_y).max(1) as u32;
    let Some(mut out) = Pixmap::new(total_w, total_h) else {
        return (pet.clone(), (0, 0));
    };

    let paint = PixmapPaint {
        opacity: 1.0,
        blend_mode: BlendMode::SourceOver,
        quality: tiny_skia::FilterQuality::Nearest,
    };
    out.draw_pixmap(
        -min_x,
        -min_y,
        pet.as_ref(),
        &paint,
        Transform::identity(),
        None,
    );
    out.draw_pixmap(
        bub_rel_x - min_x,
        bub_rel_y - min_y,
        bub.as_ref(),
        &paint,
        Transform::identity(),
        None,
    );

    (out, (min_x, min_y))
}

#[inline]
fn blend_premultiplied_over(dst: &mut [u8], src_straight: [u8; 4]) {
    let sa = u16::from(src_straight[3]);
    let inv_sa = 255 - sa;
    let sr = (u16::from(src_straight[0]) * sa + 127) / 255;
    let sg = (u16::from(src_straight[1]) * sa + 127) / 255;
    let sb = (u16::from(src_straight[2]) * sa + 127) / 255;

    dst[0] = (sr + (u16::from(dst[0]) * inv_sa + 127) / 255).min(255) as u8;
    dst[1] = (sg + (u16::from(dst[1]) * inv_sa + 127) / 255).min(255) as u8;
    dst[2] = (sb + (u16::from(dst[2]) * inv_sa + 127) / 255).min(255) as u8;
    dst[3] = (sa + (u16::from(dst[3]) * inv_sa + 127) / 255).min(255) as u8;
}

fn build_rounded_bubble_path(w: f32, h: f32, radius: f32, tail_h: f32) -> Option<tiny_skia::Path> {
    let inset = 1.0_f32;
    let x0 = inset;
    let y0 = inset;
    let x1 = (w - inset).max(x0 + 4.0);
    let y1 = (h - inset).max(y0 + 4.0);
    let r = radius.min((x1 - x0) * 0.4).min((y1 - y0) * 0.4);
    let mid_x = (x0 + x1) * 0.5;

    let mut pb = PathBuilder::new();
    pb.move_to(x0 + r, y0);
    pb.line_to(x1 - r, y0);
    pb.quad_to(x1, y0, x1, y0 + r);
    pb.line_to(x1, y1 - r);
    pb.quad_to(x1, y1, x1 - r, y1);
    pb.line_to(mid_x + 7.0, y1);
    pb.line_to(mid_x, y1 + tail_h - 1.0);
    pb.line_to(mid_x - 7.0, y1);
    pb.line_to(x0 + r, y1);
    pb.quad_to(x0, y1, x0, y1 - r);
    pb.line_to(x0, y0 + r);
    pb.quad_to(x0, y0, x0 + r, y0);
    pb.close();
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::procedural_mascot_pixmap;

    #[test]
    fn snapshot_renders_chat_bubble_and_composes_above_pet_deterministically() {
        let mut renderer = BubbleRenderer::new();
        let style = BubbleStyle::default();

        let b1 = renderer
            .render_bubble("Hello Master! ~", 220, &style)
            .expect("bubble rendered");
        let b2 = renderer
            .render_bubble("Hello Master! ~", 220, &style)
            .expect("second deterministic render");

        assert_eq!(b1.width(), b2.width());
        assert_eq!(b1.height(), b2.height());
        // Snapshot determinism: identical text & style yield bit-identical PNG output.
        let png1 = b1.encode_png().unwrap();
        let png2 = b2.encode_png().unwrap();
        assert_eq!(png1, png2);

        let pet = procedural_mascot_pixmap(128).unwrap();
        let t0 = std::time::Instant::now();
        let (scene, delta) = compose_pet_and_bubble(&pet, Some(&b1));
        assert!(t0.elapsed().as_millis() < 50);
        assert!(delta.1 < 0, "bubble shifts composite origin upward");
        assert!(scene.height() > pet.height());
    }
}
