//! VPet 3-layer food animation compositor (`back_lay` + transformed item + `front_lay`).

use std::sync::Arc;

use tiny_skia::{BlendMode, FilterQuality, Pixmap, PixmapPaint, Transform};
use vpet_content::FoodKeyframe;

use crate::decode::{DecodedClip, RenderedFrame};

/// Composite a `FoodAnimation` clip at `target_size` from optional `back` / `front` layer clips
/// and the consumable `item_icon`.
#[must_use]
pub fn compose_food_animation(
    key: impl Into<String>,
    keyframes: &[FoodKeyframe],
    back_layer: Option<&DecodedClip>,
    front_layer: Option<&DecodedClip>,
    item_icon: Option<&Pixmap>,
    target_size: u32,
) -> DecodedClip {
    let scale = target_size as f32 / 500.0;

    let mut boundaries = std::collections::BTreeSet::new();
    boundaries.insert(0_u32);

    let mut total_t = 0_u32;
    if let Some(back) = back_layer {
        let mut t = 0_u32;
        for f in back.frames.iter() {
            t = t.saturating_add(f.duration_ms);
            boundaries.insert(t);
        }
        total_t = total_t.max(t);
    }
    if let Some(front) = front_layer {
        let mut t = 0_u32;
        for f in front.frames.iter() {
            t = t.saturating_add(f.duration_ms);
            boundaries.insert(t);
        }
        total_t = total_t.max(t);
    }
    let mut t = 0_u32;
    for kf in keyframes {
        t = t.saturating_add(kf.duration_ms);
        boundaries.insert(t);
    }
    total_t = total_t.max(t);

    if boundaries.len() <= 1 || total_t == 0 {
        if let Some(back) = back_layer {
            return back.clone();
        }
        return DecodedClip::from_frames(key, Vec::new());
    }

    let b_vec: Vec<u32> = boundaries.into_iter().collect();
    let mut out = Vec::with_capacity(b_vec.len());

    for i in 0..b_vec.len() - 1 {
        let t_curr = b_vec[i];
        let t_next = b_vec[i + 1];
        let duration_ms = t_next - t_curr;
        if duration_ms == 0 {
            continue;
        }

        let Some(mut frame_pm) = Pixmap::new(target_size.max(1), target_size.max(1)) else {
            continue;
        };

        if let Some(back) = back_layer.and_then(|c| sample_clip_at(c, t_curr)) {
            draw_layer(&mut frame_pm, &back.pixmap, 1.0, Transform::identity());
        }

        if let Some(kf) = sample_keyframe_at(keyframes, t_curr)
            && kf.visible
            && let Some(item) = item_icon
            && item.width() > 0
            && item.height() > 0
        {
            let desired_w = (kf.width * scale).max(1.0);
            let item_scale = desired_w / (item.width() as f32);
            let dst_x = kf.x * scale;
            let dst_y = kf.y * scale;
            let cx = dst_x + desired_w * 0.5;
            let cy = dst_y + (item.height() as f32) * item_scale * 0.5;

            let transform = Transform::from_scale(item_scale, item_scale)
                .post_translate(dst_x, dst_y)
                .post_rotate_at(kf.rotate_deg, cx, cy);
            draw_layer(&mut frame_pm, item, kf.opacity, transform);
        }

        if let Some(front) = front_layer.and_then(|c| sample_clip_at(c, t_curr)) {
            draw_layer(&mut frame_pm, &front.pixmap, 1.0, Transform::identity());
        }

        out.push(RenderedFrame {
            pixmap: Arc::new(frame_pm),
            duration_ms,
        });
    }

    DecodedClip::from_frames(key, out)
}

fn sample_keyframe_at(keyframes: &[FoodKeyframe], elapsed_ms: u32) -> Option<&FoodKeyframe> {
    if keyframes.is_empty() {
        return None;
    }
    let mut acc = 0_u32;
    for kf in keyframes {
        acc = acc.saturating_add(kf.duration_ms);
        if elapsed_ms < acc {
            return Some(kf);
        }
    }
    keyframes.last()
}

fn sample_clip_at(clip: &DecodedClip, elapsed_ms: u32) -> Option<&RenderedFrame> {
    if clip.frames.is_empty() {
        return None;
    }
    let mut acc = 0_u32;
    for f in clip.frames.iter() {
        acc = acc.saturating_add(f.duration_ms);
        if elapsed_ms < acc {
            return Some(f);
        }
    }
    clip.frames.last()
}

fn draw_layer(dst: &mut Pixmap, src: &Pixmap, opacity: f32, transform: Transform) {
    let paint = PixmapPaint {
        opacity: opacity.clamp(0.0, 1.0),
        blend_mode: BlendMode::SourceOver,
        quality: FilterQuality::Bilinear,
    };
    dst.draw_pixmap(0, 0, src.as_ref(), &paint, transform, None);
}
