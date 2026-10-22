use crate::{db::Result, engine::Engine};
use base64::Engine as _;
use image::{ImageDecoder, ImageReader};
use sha2::{Digest, Sha256};
use std::{fs, io::Cursor, sync::atomic::Ordering};

pub fn thumbnail(engine: &Engine, id: i64, large: bool) -> Result<String> {
    thumbnail_with(engine, id, large, video_thumbnail)
}

fn thumbnail_with(
    engine: &Engine,
    id: i64,
    large: bool,
    video: impl FnOnce(&std::path::Path, u32) -> Result<image::DynamicImage>,
) -> Result<String> {
    // Serial decode provides a hard bound on simultaneous image allocations.
    let _gate = engine.0.thumbnail_gate.lock().map_err(|e| e.to_string())?;
    let (record, path) = engine.checked_file(id)?;
    if !matches!(record.category.as_str(), "images" | "videos") {
        return Err("No thumbnail available".into());
    }
    if record.category == "images" && record.size > 80 * 1024 * 1024 {
        return Err(
            "This image is too large for an inline preview. Open the original instead.".into(),
        );
    }
    crate::engine::check_identity(&record, &path)?;
    let side = if large { 1400 } else { 384 };
    let key = format!(
        "{:x}",
        Sha256::digest(
            format!(
                "v2:{}:{}:{}:{}",
                record.path, record.size, record.modified_token, side
            )
            .as_bytes()
        )
    );
    let cached = engine.0.cache.join(format!("{key}.jpg"));
    let bytes = if cached.is_file() {
        fs::read(cached).map_err(|e| e.to_string())?
    } else {
        let decoded = if record.category == "videos" {
            video(&path, side)?
        } else {
            let mut reader = ImageReader::open(&path)
                .map_err(|e| e.to_string())?
                .with_guessed_format()
                .map_err(|e| e.to_string())?;
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(128 * 1024 * 1024);
            limits.max_image_width = Some(20000);
            limits.max_image_height = Some(20000);
            reader.limits(limits);
            let mut decoder = reader.into_decoder().map_err(|_| {
                "Preview format is unsupported or exceeds memory limits. Open the original."
                    .to_owned()
            })?;
            let orientation = decoder
                .orientation()
                .unwrap_or(image::metadata::Orientation::NoTransforms);
            let mut decoded = image::DynamicImage::from_decoder(decoder).map_err(|_| {
                "This image could not be decoded within preview memory limits.".to_owned()
            })?;
            decoded.apply_orientation(orientation);
            decoded
        };
        let thumb = decoded.thumbnail(side, side).to_rgba8();
        // Composite transparency before JPEG encoding; dropping alpha makes
        // transparent artwork and screenshots turn black in the library.
        let image = image::RgbImage::from_fn(thumb.width(), thumb.height(), |x, y| {
            let pixel = thumb.get_pixel(x, y).0;
            let alpha = pixel[3] as u32;
            image::Rgb(std::array::from_fn(|i| {
                ((pixel[i] as u32 * alpha + 255 * (255 - alpha) + 127) / 255) as u8
            }))
        });
        let mut output = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut output, image::ImageFormat::Jpeg)
            .map_err(|e| e.to_string())?;
        let bytes = output.into_inner();
        // A changed, removed, redirected or newly excluded input must never
        // publish a stale thumbnail, even when extraction finished successfully.
        revalidate(engine, id, &record, &path)?;
        fs::write(cached, &bytes).map_err(|e| e.to_string())?;
        let size = engine
            .0
            .cache_bytes
            .fetch_add(bytes.len() as u64, Ordering::Relaxed)
            + bytes.len() as u64;
        if size > 256 * 1024 * 1024 {
            prune_cache(engine);
        }
        bytes
    };
    revalidate(engine, id, &record, &path)?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn revalidate(
    engine: &Engine,
    id: i64,
    record: &crate::model::FileRecord,
    path: &std::path::Path,
) -> Result<()> {
    let (_, current_path) = engine.checked_file(id)?;
    if current_path != path {
        return Err(
            "File location changed during thumbnail extraction. Refresh your library.".into(),
        );
    }
    crate::engine::check_identity(record, path)
}

#[cfg(windows)]
fn video_thumbnail(path: &std::path::Path, side: u32) -> Result<image::DynamicImage> {
    windows_video::extract(path, side)
}

#[cfg(not(windows))]
fn video_thumbnail(_: &std::path::Path, _: u32) -> Result<image::DynamicImage> {
    Err("Video thumbnails are currently available on Windows only.".into())
}

#[cfg(windows)]
#[path = "windows_video.rs"]
mod windows_video;
pub fn prune_cache(engine: &Engine) {
    let Ok(entries) = fs::read_dir(&engine.0.cache) else {
        return;
    };
    let mut files = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            if !m.is_file() {
                return None;
            }
            Some((e.path(), m.len(), m.modified().ok()))
        })
        .collect::<Vec<_>>();
    let mut size: u64 = files.iter().map(|f| f.1).sum();
    files.sort_by_key(|f| f.2);
    let target = if size > 256 * 1024 * 1024 {
        192 * 1024 * 1024
    } else {
        size
    };
    for (path, length, _) in files {
        if size <= target {
            break;
        }
        if fs::remove_file(path).is_ok() {
            size = size.saturating_sub(length);
        }
    }
    engine.0.cache_bytes.store(size, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, model::Query};
    use std::path::{Path, PathBuf};

    fn fixture() -> (tempfile::TempDir, Engine, PathBuf, i64) {
        let temp =
            tempfile::tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target")).unwrap();
        let root = temp.path().join("selected");
        fs::create_dir(&root).unwrap();
        let path = root.join("video ' & $ sample.mp4");
        fs::write(&path, include_bytes!("../tests/fixtures/thumbnail-red.mp4")).unwrap();
        let engine = Engine::new(&temp.path().join("data")).unwrap();
        engine.add_root(&root).unwrap();
        engine.scan_mode(false).unwrap();
        let id = db::query(&db::connect(&engine.0.db).unwrap(), Query::default())
            .unwrap()
            .files[0]
            .id;
        (temp, engine, path, id)
    }

    fn frame() -> image::DynamicImage {
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            160,
            90,
            image::Rgb([255, 0, 0]),
        ))
    }

    #[test]
    fn video_cache_reuses_frames_and_refuses_changed_or_removed_inputs() {
        let (_temp, engine, path, id) = fixture();
        let first = thumbnail_with(&engine, id, false, |actual, side| {
            assert_eq!(actual, fs::canonicalize(&path).unwrap());
            assert_eq!(side, 384);
            Ok(frame())
        })
        .unwrap();
        assert!(first.starts_with("data:image/jpeg;base64,"));
        assert_eq!(
            thumbnail_with(&engine, id, false, |_, _| panic!(
                "cached frame must not decode again"
            ))
            .unwrap(),
            first
        );
        fs::write(&path, b"changed input").unwrap();
        assert!(thumbnail_with(&engine, id, false, |_, _| panic!(
            "changed input must not decode"
        ))
        .is_err());
        engine.scan_mode(false).unwrap();
        let fresh = thumbnail_with(&engine, id, false, |_, _| Ok(frame())).unwrap();
        assert_eq!(first, fresh);
        fs::remove_file(&path).unwrap();
        assert!(thumbnail_with(&engine, id, false, |_, _| panic!(
            "removed input must not decode"
        ))
        .is_err());
    }

    #[test]
    fn video_changes_during_extraction_never_publish_a_cache_entry() {
        let (_temp, engine, path, id) = fixture();
        assert!(thumbnail_with(&engine, id, false, |_, _| {
            fs::write(&path, b"replacement while extracting").unwrap();
            Ok(frame())
        })
        .is_err());
        assert_eq!(fs::read_dir(&engine.0.cache).unwrap().count(), 0);
    }

    #[test]
    fn video_failures_are_not_cached_and_removed_roots_cannot_use_cached_frames() {
        let (_temp, engine, _path, id) = fixture();
        assert!(
            thumbnail_with(&engine, id, false, |_, _| Err("unsupported codec".into())).is_err()
        );
        assert_eq!(fs::read_dir(&engine.0.cache).unwrap().count(), 0);
        thumbnail_with(&engine, id, false, |_, _| Ok(frame())).unwrap();
        db::connect(&engine.0.db)
            .unwrap()
            .execute("DELETE FROM roots", [])
            .unwrap();
        assert!(thumbnail_with(&engine, id, false, |_, _| panic!(
            "removed root must not decode"
        ))
        .is_err());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Windows Shell/installed-codec integration smoke; run npm run native:video explicitly"]
    fn windows_extracts_a_real_video_frame_with_correct_color_and_bounds() {
        let (_temp, engine, _path, id) = fixture();
        for large in [false, true] {
            let data = thumbnail(&engine, id, large).unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data.split_once(',').unwrap().1)
                .unwrap();
            let image = image::load_from_memory(&bytes).unwrap().to_rgb8();
            assert!(image.width() > 0 && image.width() <= if large { 1400 } else { 384 });
            assert!(image.height() > 0 && image.height() <= if large { 1400 } else { 384 });
            let center = image.get_pixel(image.width() / 2, image.height() / 2).0;
            assert!(
                center[0] > 200 && center[1] < 40 && center[2] < 40,
                "expected real red video frame, got {center:?}"
            );
        }
    }
}
