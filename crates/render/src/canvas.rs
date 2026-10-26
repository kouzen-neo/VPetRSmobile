//! CPU sprite loading, scaling, and premultiplied RGBA -> Wayland `Argb8888` (BGRA) blitting.

use std::path::Path;

use thiserror::Error;
use tiny_skia::{
    Color, FillRule, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, Transform,
};

use crate::damage::DamageRect;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("failed to decode image `{path}`: {source}")]
    ImageDecode {
        path: String,
        #[source]
        source: image::ImageError,
    },
    #[error("invalid sprite dimensions {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },
}

/// Convert straight-alpha `image::RgbaImage` into a premultiplied `tiny_skia::Pixmap`.
pub fn rgba_image_to_pixmap(img: &image::RgbaImage) -> Result<Pixmap, RenderError> {
    let (width, height) = img.dimensions();
    let mut pixmap =
        Pixmap::new(width, height).ok_or(RenderError::InvalidDimensions { width, height })?;
    let dst = pixmap.data_mut();
    let (src_chunks, _) = img.as_raw().as_chunks::<4>();
    let (dst_chunks, _) = dst.as_chunks_mut::<4>();
    for (src_px, dst_px) in src_chunks.iter().zip(dst_chunks) {
        let a = u16::from(src_px[3]);
        // Premultiply with rounding: (c * a + 127) / 255.
        *dst_px = [
            ((u16::from(src_px[0]) * a + 127) / 255) as u8,
            ((u16::from(src_px[1]) * a + 127) / 255) as u8,
            ((u16::from(src_px[2]) * a + 127) / 255) as u8,
            src_px[3],
        ];
    }
    Ok(pixmap)
}

/// Scale a `Pixmap` to `(target_w, target_h)` using bilinear filtering.
pub fn scale_pixmap(src: &Pixmap, target_w: u32, target_h: u32) -> Result<Pixmap, RenderError> {
    if src.width() == target_w && src.height() == target_h {
        return Ok(src.clone());
    }
    let mut dst = Pixmap::new(target_w, target_h).ok_or(RenderError::InvalidDimensions {
        width: target_w,
        height: target_h,
    })?;
    let sx = target_w as f32 / src.width() as f32;
    let sy = target_h as f32 / src.height() as f32;
    let paint = PixmapPaint {
        opacity: 1.0,
        blend_mode: tiny_skia::BlendMode::Source,
        quality: FilterQuality::Bilinear,
    };
    dst.draw_pixmap(
        0,
        0,
        src.as_ref(),
        &paint,
        Transform::from_scale(sx, sy),
        None,
    );
    Ok(dst)
}

/// Decode a PNG/WebP/GIF first frame from disk and scale to `(target_w, target_h)`.
pub fn load_sprite_scaled(
    path: &Path,
    target_w: u32,
    target_h: u32,
) -> Result<Pixmap, RenderError> {
    let decoded = image::open(path)
        .map_err(|source| RenderError::ImageDecode {
            path: path.display().to_string(),
            source,
        })?
        .into_rgba8();
    let raw = rgba_image_to_pixmap(&decoded)?;
    scale_pixmap(&raw, target_w, target_h)
}

/// Procedural mascot silhouette used when no external asset path is configured.
pub fn procedural_mascot_pixmap(size: u32) -> Result<Pixmap, RenderError> {
    let mut pixmap = Pixmap::new(size, size).ok_or(RenderError::InvalidDimensions {
        width: size,
        height: size,
    })?;
    let s = size as f32;

    let mut body_paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    body_paint.set_color(Color::from_rgba8(129, 212, 250, 255));

    let mut pb = PathBuilder::new();
    // Left & right cat ears + round head/body silhouette so alpha input region is non-rectangular.
    pb.move_to(s * 0.22, s * 0.42);
    pb.line_to(s * 0.16, s * 0.12);
    pb.line_to(s * 0.40, s * 0.26);
    pb.close();
    pb.move_to(s * 0.78, s * 0.42);
    pb.line_to(s * 0.84, s * 0.12);
    pb.line_to(s * 0.60, s * 0.26);
    pb.close();
    if let Some(ears) = pb.finish() {
        pixmap.fill_path(
            &ears,
            &body_paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    if let Some(circle) = PathBuilder::from_circle(s * 0.50, s * 0.56, s * 0.34) {
        pixmap.fill_path(
            &circle,
            &body_paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    let mut eye_paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    eye_paint.set_color(Color::from_rgba8(35, 45, 65, 255));
    for cx in [0.39_f32, 0.61_f32] {
        if let Some(eye) = PathBuilder::from_circle(s * cx, s * 0.54, (s * 0.045).max(1.5)) {
            pixmap.fill_path(
                &eye,
                &eye_paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    Ok(pixmap)
}

/// Zero only `rect` inside a tightly-packed `buf_w * buf_h * 4` BGRA buffer.
pub fn clear_bgra_rect(canvas: &mut [u8], buf_w: u32, buf_h: u32, rect: DamageRect) {
    let Some(clamped) = rect.clamp_to(buf_w, buf_h) else {
        return;
    };
    let stride = (buf_w as usize) * 4;
    let x_byte = (clamped.x as usize) * 4;
    let w_bytes = (clamped.w as usize) * 4;
    for row in clamped.y as usize..(clamped.y + clamped.h) as usize {
        let start = row * stride + x_byte;
        canvas[start..start + w_bytes].fill(0);
    }
}

/// Blit a premultiplied RGBA `Pixmap` into a Wayland `Argb8888` (little-endian BGRA) buffer,
/// clipping cleanly against buffer edges and returning the drawn buffer-space `DamageRect`.
pub fn blit_pixmap_to_bgra(
    canvas: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    pixmap: &Pixmap,
    dst_x: i32,
    dst_y: i32,
) -> Option<DamageRect> {
    let pw = i32::try_from(pixmap.width()).ok()?;
    let ph = i32::try_from(pixmap.height()).ok()?;
    let target = DamageRect::new(dst_x, dst_y, pw, ph).clamp_to(buf_w, buf_h)?;

    let src_x0 = (target.x - dst_x) as usize;
    let src_y0 = (target.y - dst_y) as usize;
    let copy_w = target.w as usize;
    let copy_h = target.h as usize;

    let src_stride = (pixmap.width() as usize) * 4;
    let dst_stride = (buf_w as usize) * 4;
    let src_data = pixmap.data();

    for row in 0..copy_h {
        let s_off = (src_y0 + row) * src_stride + src_x0 * 4;
        let d_off = ((target.y as usize) + row) * dst_stride + (target.x as usize) * 4;
        let src_row = &src_data[s_off..s_off + copy_w * 4];
        let dst_row = &mut canvas[d_off..d_off + copy_w * 4];
        let (src_px, _) = src_row.as_chunks::<4>();
        let (dst_px, _) = dst_row.as_chunks_mut::<4>();
        for (sp, dp) in src_px.iter().zip(dst_px) {
            *dp = [sp[2], sp[1], sp[0], sp[3]]; // BGRA
        }
    }

    Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swizzles_rgba_to_bgra_and_clears_patch() {
        let mut img = image::RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([255, 64, 16, 255]));
        img.put_pixel(1, 1, image::Rgba([0, 200, 100, 128]));
        let pm = rgba_image_to_pixmap(&img).unwrap();

        let mut buf = vec![0_u8; 4 * 4 * 4];
        let drawn = blit_pixmap_to_bgra(&mut buf, 4, 4, &pm, 1, 1).unwrap();
        assert_eq!(drawn, DamageRect::new(1, 1, 2, 2));

        // Pixel (1, 1) in buffer corresponds to src (0, 0): [B=16, G=64, R=255, A=255].
        let off = (4 + 1) * 4;
        assert_eq!(&buf[off..off + 4], &[16, 64, 255, 255]);

        clear_bgra_rect(&mut buf, 4, 4, drawn);
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn clips_negative_and_right_bottom_overflow() {
        let pm = procedural_mascot_pixmap(32).unwrap();
        let mut buf = vec![0_u8; 20 * 20 * 4];
        let drawn = blit_pixmap_to_bgra(&mut buf, 20, 20, &pm, -10, 8).unwrap();
        assert_eq!(drawn, DamageRect::new(0, 8, 20, 12));
    }
}
