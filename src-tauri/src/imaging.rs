use std::path::{Path, PathBuf};
use std::io::BufWriter;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::time::UNIX_EPOCH;
use image::{DynamicImage, imageops::FilterType};
use crate::models::ExportOptions;
use crate::db::with_db;
use rusqlite::params;

pub fn generate_thumbnail(photo_path: &str, size: u32) -> Result<Vec<u8>, String> {
    let cache_path = resolve_thumbnail_cache_path(photo_path, size);
    if let Some(path) = cache_path.as_ref() {
        if let Some(bytes) = try_read_cached_thumbnail(path) {
            return Ok(bytes);
        }
    }

    let img = image::open(photo_path).map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(size, size);
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    thumb.write_to(&mut cursor, image::ImageFormat::Jpeg)
        .map_err(|e| e.to_string())?;

    if let Some(path) = cache_path.as_ref() {
        write_cached_thumbnail(path, &buf);
    }

    Ok(buf)
}

pub fn ensure_preview_cache_path(
    photo_path: &str,
    size: u32,
    profile: &str,
    quality: u8,
) -> Result<String, String> {
    let profile = normalize_profile(profile);
    let quality = normalize_quality(quality);
    let cache_path = resolve_thumbnail_cache_path_v2(photo_path, size, &profile, quality)
        .ok_or_else(|| "failed to resolve preview cache path".to_string())?;

    if !cache_path.exists() {
        let bytes = generate_thumbnail_with_profile(photo_path, size, &profile, quality)?;
        if !cache_path.exists() {
            if let Some(parent) = cache_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&cache_path, bytes).map_err(|e| e.to_string())?;
        }
    }

    if !cache_path.exists() {
        return Err("failed to persist preview cache".to_string());
    }

    Ok(cache_path.to_string_lossy().to_string())
}

fn resolve_thumbnail_cache_path(photo_path: &str, size: u32) -> Option<PathBuf> {
    let root = thumbnail_cache_root()?;
    let metadata = std::fs::metadata(photo_path).ok()?;
    let file_len = metadata.len();
    let modified_ns = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let key = build_cache_key(photo_path, size, file_len, modified_ns);
    let shard = &key[0..2];
    Some(root.join(shard).join(format!("{key}.jpg")))
}

fn resolve_thumbnail_cache_path_v2(photo_path: &str, size: u32, profile: &str, quality: u8) -> Option<PathBuf> {
    let root = thumbnail_cache_root()?;
    let metadata = std::fs::metadata(photo_path).ok()?;
    let file_len = metadata.len();
    let modified_ns = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let key = build_cache_key_v2(photo_path, size, file_len, modified_ns, profile, quality);
    let shard = &key[0..2];
    Some(root.join(profile).join(shard).join(format!("{key}.jpg")))
}

fn thumbnail_cache_root() -> Option<PathBuf> {
    if let Ok(override_root) = std::env::var("MYPHOTO_THUMB_CACHE_ROOT") {
        let p = override_root.trim();
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    dirs_next::data_local_dir().map(|d| d.join("myphoto").join("thumb_cache"))
}

fn try_read_cached_thumbnail(cache_path: &Path) -> Option<Vec<u8>> {
    let bytes = std::fs::read(cache_path).ok()?;
    if bytes.is_empty() {
        None
    } else {
        Some(bytes)
    }
}

fn write_cached_thumbnail(cache_path: &Path, bytes: &[u8]) {
    if let Some(parent) = cache_path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let _ = std::fs::write(cache_path, bytes);
}

fn build_cache_key(photo_path: &str, size: u32, file_len: u64, modified_ns: u128) -> String {
    let mut hasher = DefaultHasher::new();
    photo_path.hash(&mut hasher);
    size.hash(&mut hasher);
    file_len.hash(&mut hasher);
    modified_ns.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn build_cache_key_v2(
    photo_path: &str,
    size: u32,
    file_len: u64,
    modified_ns: u128,
    profile: &str,
    quality: u8,
) -> String {
    let mut hasher = DefaultHasher::new();
    photo_path.hash(&mut hasher);
    size.hash(&mut hasher);
    file_len.hash(&mut hasher);
    modified_ns.hash(&mut hasher);
    profile.hash(&mut hasher);
    quality.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn generate_thumbnail_with_profile(
    photo_path: &str,
    size: u32,
    profile: &str,
    quality: u8,
) -> Result<Vec<u8>, String> {
    let cache_path = resolve_thumbnail_cache_path_v2(photo_path, size, profile, quality);
    if let Some(path) = cache_path.as_ref() {
        if let Some(bytes) = try_read_cached_thumbnail(path) {
            return Ok(bytes);
        }
    }

    let img = image::open(photo_path).map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(size, size);
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
    thumb
        .write_with_encoder(encoder)
        .map_err(|e| e.to_string())?;

    if let Some(path) = cache_path.as_ref() {
        write_cached_thumbnail(path, &buf);
    }

    Ok(buf)
}

fn normalize_profile(profile: &str) -> String {
    let trimmed = profile.trim();
    if trimmed.is_empty() {
        return "default".to_string();
    }
    if trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        trimmed.to_string()
    } else {
        "default".to_string()
    }
}

fn normalize_quality(quality: u8) -> u8 {
    quality.clamp(1, 100)
}

pub fn export_photos(
    workspace_path: &str,
    options: &ExportOptions,
    progress_tx: std::sync::mpsc::Sender<(usize, String)>,
) -> Result<Vec<String>, String> {
    let dest = PathBuf::from(&options.dest_folder);
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;

    let total = options.photo_ids.len();
    let mut results = vec![];
    let mut seq = 1usize;

    for (i, &photo_id) in options.photo_ids.iter().enumerate() {
        // Get photo info from db
        let (rel_path, filename, taken_at) = with_db(|conn| {
            conn.query_row(
                "SELECT relative_path, filename, taken_at FROM photos WHERE id=?1",
                params![photo_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)),
            )
        })
        .map_err(|e| e.to_string())?;

        let src_path = PathBuf::from(workspace_path).join(&rel_path);
        let _ = progress_tx.send((i, filename.clone()));

        if !src_path.exists() {
            continue;
        }

        let dest_name = match options.naming_rule.as_str() {
            "date_seq" => {
                let date_prefix = taken_at
                    .as_deref()
                    .and_then(|t| t.split('T').next())
                    .unwrap_or("unknown")
                    .replace(':', "-");
                let ext = get_output_ext(&options.format, &filename);
                format!("{}_{:04}.{}", date_prefix, seq, ext)
            }
            _ => {
                let ext = get_output_ext(&options.format, &filename);
                let stem = Path::new(&filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&filename);
                format!("{}.{}", stem, ext)
            }
        };
        seq += 1;

        let dest_file = resolve_conflict(&dest.join(&dest_name), &options.conflict);
        if dest_file.is_none() {
            continue; // skip
        }
        let dest_file = dest_file.unwrap();

        if options.format == "original" && options.max_dimension.is_none() {
            std::fs::copy(&src_path, &dest_file).map_err(|e| e.to_string())?;
        } else {
            let img = image::open(&src_path).map_err(|e| e.to_string())?;
            let img = apply_max_dimension(img, options.max_dimension);
            save_image(&img, &dest_file, &options.format, options.quality)?;
        }

        results.push(dest_file.to_string_lossy().to_string());
    }

    let _ = progress_tx.send((total, "done".to_string()));
    Ok(results)
}

fn get_output_ext(format: &str, original_filename: &str) -> String {
    match format {
        "jpeg" => "jpg".to_string(),
        "png" => "png".to_string(),
        "webp" => "webp".to_string(),
        _ => Path::new(original_filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg")
            .to_lowercase(),
    }
}

fn resolve_conflict(path: &Path, conflict: &str) -> Option<PathBuf> {
    if !path.exists() {
        return Some(path.to_path_buf());
    }
    match conflict {
        "skip" => None,
        "overwrite" => Some(path.to_path_buf()),
        _ => {
            // rename: add _1, _2, ...
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let parent = path.parent().unwrap_or(Path::new("."));
            for i in 1..=9999 {
                let new_name = if ext.is_empty() {
                    format!("{}_{}", stem, i)
                } else {
                    format!("{}_{}.{}", stem, i, ext)
                };
                let candidate = parent.join(new_name);
                if !candidate.exists() {
                    return Some(candidate);
                }
            }
            None
        }
    }
}

fn apply_max_dimension(img: DynamicImage, max_dim: Option<u32>) -> DynamicImage {
    match max_dim {
        None => img,
        Some(max) => {
            let (w, h) = (img.width(), img.height());
            if w <= max && h <= max {
                img
            } else {
                img.resize(max, max, FilterType::Lanczos3)
            }
        }
    }
}

fn save_image(img: &DynamicImage, path: &Path, format: &str, quality: u8) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut writer = BufWriter::new(file);
    match format {
        "png" => img.write_to(&mut writer, image::ImageFormat::Png),
        "webp" => img.write_to(&mut writer, image::ImageFormat::WebP),
        _ => {
            // jpeg
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);
            img.write_with_encoder(encoder)
        }
    }
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::build_cache_key;
    use super::build_cache_key_v2;
    use super::ensure_preview_cache_path;
    use image::{ImageBuffer, Rgb};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn build_cache_key_is_stable_for_same_signature() {
        let k1 = build_cache_key("C:/photos/a.jpg", 1600, 1234, 42);
        let k2 = build_cache_key("C:/photos/a.jpg", 1600, 1234, 42);
        assert_eq!(k1, k2);
    }

    #[test]
    fn build_cache_key_changes_when_signature_changes() {
        let base = build_cache_key("C:/photos/a.jpg", 1600, 1234, 42);
        let by_size = build_cache_key("C:/photos/a.jpg", 1800, 1234, 42);
        let by_len = build_cache_key("C:/photos/a.jpg", 1600, 9999, 42);
        let by_mtime = build_cache_key("C:/photos/a.jpg", 1600, 1234, 43);

        assert_ne!(base, by_size);
        assert_ne!(base, by_len);
        assert_ne!(base, by_mtime);
    }

    #[test]
    fn cache_key_changes_when_profile_or_quality_changes() {
        let base = build_cache_key_v2("C:/photos/a.jpg", 1800, 1234, 42, "preview", 82);
        let by_profile = build_cache_key_v2("C:/photos/a.jpg", 1800, 1234, 42, "grid", 82);
        let by_quality = build_cache_key_v2("C:/photos/a.jpg", 1800, 1234, 42, "preview", 90);

        assert_ne!(base, by_profile);
        assert_ne!(base, by_quality);
    }

    #[test]
    fn ensure_preview_cache_returns_existing_file_path() {
        let cache_root = create_cache_root();
        std::env::set_var("MYPHOTO_THUMB_CACHE_ROOT", cache_root.to_string_lossy().as_ref());
        let src = create_temp_test_image();

        let cache_path = ensure_preview_cache_path(
            src.to_string_lossy().as_ref(),
            1600,
            "preview",
            82,
        )
        .expect("preview cache path should be generated");

        assert!(PathBuf::from(&cache_path).exists());

        let _ = std::fs::remove_file(src);
        let _ = std::fs::remove_file(cache_path);
        let _ = std::fs::remove_dir_all(cache_root);
        std::env::remove_var("MYPHOTO_THUMB_CACHE_ROOT");
    }

    fn create_temp_test_image() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        path.push(format!("myphoto-imaging-test-{nanos}.jpg"));

        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(8, 8, |_x, _y| Rgb([180, 120, 90]));
        img.save(&path).expect("write source image");
        path
    }

    fn create_cache_root() -> PathBuf {
        let mut path = std::env::current_dir().expect("current dir");
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        path.push(format!("target/myphoto-test-cache-{nanos}"));
        std::fs::create_dir_all(&path).expect("create cache root");
        path
    }
}
