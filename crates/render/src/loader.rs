//! Off-thread background animation decoder and startup preloader.
//!
//! Ensures disk I/O and PNG/GIF/APNG decompression never block Wayland `frame` callbacks.

use std::{
    collections::HashSet,
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
};

use tracing::warn;
use vpet_content::AnimationSource;

use crate::{
    cache::FrameCache,
    decode::{DecodedClip, decode_animation_source},
};

struct DecodeJob {
    key: String,
    source: AnimationSource,
    target_size: u32,
}

enum DecodeOutcome {
    Ok(DecodedClip),
    Err { key: String, message: String },
}

pub struct AsyncClipLoader {
    tx: Option<Sender<DecodeJob>>,
    rx: Receiver<DecodeOutcome>,
    in_flight: HashSet<String>,
    worker: Option<JoinHandle<()>>,
}

impl Default for AsyncClipLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncClipLoader {
    #[must_use]
    pub fn new() -> Self {
        let (job_tx, job_rx) = mpsc::channel::<DecodeJob>();
        let (res_tx, res_rx) = mpsc::channel::<DecodeOutcome>();

        let worker = thread::Builder::new()
            .name("vpet-decode".to_owned())
            .spawn(move || {
                while let Ok(job) = job_rx.recv() {
                    let outcome =
                        match decode_animation_source(&job.key, &job.source, job.target_size) {
                            Ok(clip) => DecodeOutcome::Ok(clip),
                            Err(err) => DecodeOutcome::Err {
                                key: job.key,
                                message: err.to_string(),
                            },
                        };
                    if res_tx.send(outcome).is_err() {
                        break;
                    }
                }
            })
            .expect("spawn vpet-decode worker thread");

        Self {
            tx: Some(job_tx),
            rx: res_rx,
            in_flight: HashSet::new(),
            worker: Some(worker),
        }
    }

    #[must_use]
    pub fn is_in_flight(&self, key: &str) -> bool {
        self.in_flight.contains(key)
    }

    pub fn request(&mut self, key: String, source: AnimationSource, target_size: u32) -> bool {
        if self.in_flight.contains(&key) {
            return false;
        }
        let Some(tx) = &self.tx else {
            return false;
        };
        if tx
            .send(DecodeJob {
                key: key.clone(),
                source,
                target_size,
            })
            .is_ok()
        {
            self.in_flight.insert(key);
            true
        } else {
            false
        }
    }

    pub fn preload<I>(&mut self, items: I, cache: &FrameCache, target_size: u32)
    where
        I: IntoIterator<Item = (String, AnimationSource)>,
    {
        for (key, source) in items {
            if !cache.contains(&key) {
                self.request(key, source, target_size);
            }
        }
    }

    /// Drain completed background decodes into `cache`, returning the number of newly cached clips.
    pub fn poll_into(&mut self, cache: &mut FrameCache) -> usize {
        let mut count = 0;
        while let Ok(outcome) = self.rx.try_recv() {
            match outcome {
                DecodeOutcome::Ok(clip) => {
                    self.in_flight.remove(&clip.key);
                    cache.insert(clip);
                    count += 1;
                }
                DecodeOutcome::Err { key, message } => {
                    self.in_flight.remove(&key);
                    warn!(key = %key, error = %message, "background animation decode failed");
                }
            }
        }
        count
    }
}

impl Drop for AsyncClipLoader {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn background_worker_decodes_and_populates_cache() {
        let tmp = std::env::temp_dir().join(format!("vpet_rs_async_{}.png", std::process::id()));
        let mut img = image::RgbaImage::new(32, 32);
        for px in img.pixels_mut() {
            *px = image::Rgba([80, 180, 250, 255]);
        }
        img.save(&tmp).unwrap();

        let mut loader = AsyncClipLoader::new();
        let mut cache = FrameCache::new(4 * 1024 * 1024);
        let src = AnimationSource::SingleImage {
            file: tmp.clone(),
            duration_ms: 200,
        };
        assert!(loader.request("single_test".to_owned(), src.clone(), 64));
        // Duplicate request while in-flight is deduplicated.
        assert!(!loader.request("single_test".to_owned(), src, 64));

        let deadline = Instant::now() + Duration::from_secs(2);
        while !cache.contains("single_test") && Instant::now() < deadline {
            loader.poll_into(&mut cache);
            thread::sleep(Duration::from_millis(5));
        }

        let _ = std::fs::remove_file(&tmp);
        assert!(cache.contains("single_test"));
    }
}
