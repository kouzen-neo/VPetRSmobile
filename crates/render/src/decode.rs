//! Decoders for VPet PNG frame sequences, single PNGs, APNG, and GIF animations.

use std::{fs::File, io::BufReader, path::Path, sync::Arc};

use image::{
    AnimationDecoder, ImageDecoder,
    codecs::{gif::GifDecoder, png::PngDecoder},
};
use tiny_skia::Pixmap;
use vpet_content::{AnimationSource, FrameFile};

use crate::canvas::{RenderError, load_sprite_scaled, rgba_image_to_pixmap, scale_pixmap};

#[derive(Debug, Clone)]
pub struct RenderedFrame {
    pub pixmap: Arc<Pixmap>,
    pub duration_ms: u32,
}

impl RenderedFrame {
    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.pixmap.data().len()
    }
}

#[derive(Debug, Clone)]
pub struct DecodedClip {
    pub key: String,
    pub frames: Arc<[RenderedFrame]>,
    pub byte_size: usize,
}

impl DecodedClip {
    #[must_use]
    pub fn from_frames(key: impl Into<String>, frames: Vec<RenderedFrame>) -> Self {
        let byte_size = frames.iter().map(RenderedFrame::byte_len).sum();
        Self {
            key: key.into(),
            frames: frames.into(),
            byte_size,
        }
    }
}

pub fn decode_png_sequence(
    key: impl Into<String>,
    frames: &[FrameFile],
    target_size: u32,
) -> Result<DecodedClip, RenderError> {
    let mut out = Vec::with_capacity(frames.len());
    for ff in frames {
        let pm = load_sprite_scaled(&ff.path, target_size, target_size)?;
        out.push(RenderedFrame {
            pixmap: Arc::new(pm),
            duration_ms: ff.duration_ms.max(16),
        });
    }
    Ok(DecodedClip::from_frames(key, out))
}

pub fn decode_animated_file(
    key: impl Into<String>,
    path: &Path,
    target_size: u32,
) -> Result<DecodedClip, RenderError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    let file = File::open(path).map_err(|e| RenderError::ImageDecode {
        path: path.display().to_string(),
        source: image::ImageError::IoError(e),
    })?;
    let reader = BufReader::new(file);

    if ext == "gif" {
        let decoder = GifDecoder::new(reader).map_err(|source| RenderError::ImageDecode {
            path: path.display().to_string(),
            source,
        })?;
        return collect_anim_frames(key, path, decoder.into_frames(), target_size);
    }

    // Try APNG for `.apng` or `.png` with acTL chunk; fall back to static PNG.
    let png = PngDecoder::new(reader).map_err(|source| RenderError::ImageDecode {
        path: path.display().to_string(),
        source,
    })?;
    if png.is_apng().unwrap_or(false) {
        let apng = png.apng().map_err(|source| RenderError::ImageDecode {
            path: path.display().to_string(),
            source,
        })?;
        return collect_anim_frames(key, path, apng.into_frames(), target_size);
    }

    let mut rgba_buf = vec![0_u8; png.total_bytes() as usize];
    let (w, h) = png.dimensions();
    png.read_image(&mut rgba_buf)
        .map_err(|source| RenderError::ImageDecode {
            path: path.display().to_string(),
            source,
        })?;
    if let Some(rgba) = image::RgbaImage::from_raw(w, h, rgba_buf) {
        let raw = rgba_image_to_pixmap(&rgba)?;
        let scaled = scale_pixmap(&raw, target_size, target_size)?;
        return Ok(DecodedClip::from_frames(
            key,
            vec![RenderedFrame {
                pixmap: Arc::new(scaled),
                duration_ms: 125,
            }],
        ));
    }

    let pm = load_sprite_scaled(path, target_size, target_size)?;
    Ok(DecodedClip::from_frames(
        key,
        vec![RenderedFrame {
            pixmap: Arc::new(pm),
            duration_ms: 125,
        }],
    ))
}

fn collect_anim_frames(
    key: impl Into<String>,
    path: &Path,
    frames: image::Frames<'_>,
    target_size: u32,
) -> Result<DecodedClip, RenderError> {
    let mut out = Vec::new();
    for frame_res in frames {
        let frame = frame_res.map_err(|source| RenderError::ImageDecode {
            path: path.display().to_string(),
            source,
        })?;
        let (numer, denom) = frame.delay().numer_denom_ms();
        let ms = numer.checked_div(denom).map_or(100, |v| v.max(16));
        let raw = rgba_image_to_pixmap(frame.buffer())?;
        let scaled = scale_pixmap(&raw, target_size, target_size)?;
        out.push(RenderedFrame {
            pixmap: Arc::new(scaled),
            duration_ms: ms,
        });
    }
    Ok(DecodedClip::from_frames(key, out))
}

pub fn decode_animation_source(
    key: impl Into<String>,
    source: &AnimationSource,
    target_size: u32,
) -> Result<DecodedClip, RenderError> {
    match source {
        AnimationSource::PngSequence { frames, .. } => {
            decode_png_sequence(key, frames, target_size)
        }
        AnimationSource::SingleImage { file, duration_ms } => {
            let pm = load_sprite_scaled(file, target_size, target_size)?;
            Ok(DecodedClip::from_frames(
                key,
                vec![RenderedFrame {
                    pixmap: Arc::new(pm),
                    duration_ms: (*duration_ms).max(16),
                }],
            ))
        }
        AnimationSource::AnimatedImage { file, .. } => decode_animated_file(key, file, target_size),
        AnimationSource::Food { .. } => Ok(DecodedClip::from_frames(key, Vec::new())),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use image::{Delay, Frame, codecs::gif::GifEncoder};

    use super::*;

    #[test]
    fn decodes_real_vup_png_sequence_and_synthetic_gif() {
        let tmp = std::env::temp_dir().join(format!("vpet_rs_gif_test_{}.gif", std::process::id()));
        {
            let file = File::create(&tmp).unwrap();
            let mut enc = GifEncoder::new(file);
            for c in [80_u8, 200_u8] {
                let mut img = image::RgbaImage::new(16, 16);
                for px in img.pixels_mut() {
                    *px = image::Rgba([c, 120, 240, 255]);
                }
                enc.encode_frame(Frame::from_parts(
                    img,
                    0,
                    0,
                    Delay::from_numer_denom_ms(80, 1),
                ))
                .unwrap();
            }
        }

        let gif_clip = decode_animated_file("test_gif", &tmp, 32).unwrap();
        assert_eq!(gif_clip.frames.len(), 2);
        assert_eq!(gif_clip.frames[0].pixmap.width(), 32);
        let _ = std::fs::remove_file(&tmp);

        let vup_default = Path::new(
            "/home/kouzen/Documents/VPet/VPet-Simulator.Windows/mod/0000_core/pet/vup/Default/Nomal/1",
        );
        if vup_default.is_dir() {
            let files = [
                FrameFile {
                    path: vup_default.join("_000_250.png"),
                    duration_ms: 250,
                },
                FrameFile {
                    path: vup_default.join("_001_125.png"),
                    duration_ms: 125,
                },
            ];
            let start = std::time::Instant::now();
            let clip = decode_png_sequence("default_nomal_1", &files, 250).unwrap();
            assert_eq!(clip.frames.len(), 2);
            assert_eq!(clip.byte_size, 2 * 250 * 250 * 4);
            assert!(start.elapsed().as_millis() < 2_000);
        }
    }
}
