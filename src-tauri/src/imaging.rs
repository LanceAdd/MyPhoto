use crate::db::with_db;
use crate::models::ExportOptions;
use chrono::Local;
use image::{imageops::FilterType, DynamicImage};
use rusqlite::params;
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Instant, UNIX_EPOCH};
use walkdir::WalkDir;

#[derive(Clone, Debug)]
pub struct WarmupProgress {
    pub done: usize,
    pub total: usize,
    pub succeeded: usize,
    pub current_file: Option<String>,
    pub finished: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PreviewCacheInfo {
    pub path: String,
    pub size_bytes: u64,
    pub profile_sizes: BTreeMap<String, u64>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ThumbnailPerfStats {
    pub generated: u64,
    pub cache_hits: u64,
    pub decode_ms_total: u64,
    pub resize_ms_total: u64,
    pub encode_ms_total: u64,
    pub io_ms_total: u64,
}

fn thumb_perf_stats() -> &'static Mutex<ThumbnailPerfStats> {
    static STATS: OnceLock<Mutex<ThumbnailPerfStats>> = OnceLock::new();
    STATS.get_or_init(|| Mutex::new(ThumbnailPerfStats::default()))
}

fn u128_to_u64_saturating(value: u128) -> u64 {
    if value > u64::MAX as u128 {
        u64::MAX
    } else {
        value as u64
    }
}

fn record_thumb_cache_hit(io_ms: u128) {
    if let Ok(mut stats) = thumb_perf_stats().lock() {
        stats.cache_hits = stats.cache_hits.saturating_add(1);
        stats.io_ms_total = stats
            .io_ms_total
            .saturating_add(u128_to_u64_saturating(io_ms));
    }
}

fn record_thumb_generated(decode_ms: u128, resize_ms: u128, encode_ms: u128, io_ms: u128) {
    if let Ok(mut stats) = thumb_perf_stats().lock() {
        stats.generated = stats.generated.saturating_add(1);
        stats.decode_ms_total = stats
            .decode_ms_total
            .saturating_add(u128_to_u64_saturating(decode_ms));
        stats.resize_ms_total = stats
            .resize_ms_total
            .saturating_add(u128_to_u64_saturating(resize_ms));
        stats.encode_ms_total = stats
            .encode_ms_total
            .saturating_add(u128_to_u64_saturating(encode_ms));
        stats.io_ms_total = stats
            .io_ms_total
            .saturating_add(u128_to_u64_saturating(io_ms));
    }
}

pub fn get_thumbnail_perf_stats() -> ThumbnailPerfStats {
    thumb_perf_stats()
        .lock()
        .map(|stats| stats.clone())
        .unwrap_or_default()
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

pub fn warmup_preview_cache_with_progress<F>(
    workspace_id: i64,
    workspace_path: &str,
    size: u32,
    profile: &str,
    quality: u8,
    offset: usize,
    limit: usize,
    concurrency: usize,
    mut on_progress: F,
) -> Result<usize, String>
where
    F: FnMut(WarmupProgress),
{
    if limit == 0 {
        on_progress(WarmupProgress {
            done: 0,
            total: 0,
            succeeded: 0,
            current_file: None,
            finished: true,
        });
        return Ok(0);
    }

    if workspace_id <= 0 {
        return Err("invalid workspace id for warmup".to_string());
    }

    let files = crate::photos::get_workspace_files_page(workspace_id, offset, limit)?;
    let total = files.len();
    on_progress(WarmupProgress {
        done: 0,
        total,
        succeeded: 0,
        current_file: None,
        finished: total == 0,
    });
    if total == 0 {
        return Ok(0);
    }

    let worker_count = normalize_warmup_concurrency(concurrency).min(total.max(1));
    let files = Arc::new(files);
    let next_idx = Arc::new(AtomicUsize::new(0));
    let workspace_path_owned = workspace_path.to_string();
    let profile_owned = profile.to_string();
    let (result_tx, result_rx) = std::sync::mpsc::channel::<(bool, String)>();

    for _ in 0..worker_count {
        let files_ref = Arc::clone(&files);
        let next_idx_ref = Arc::clone(&next_idx);
        let tx_ref = result_tx.clone();
        let workspace_path_ref = workspace_path_owned.clone();
        let profile_ref = profile_owned.clone();
        std::thread::spawn(move || {
            loop {
                let idx = next_idx_ref.fetch_add(1, Ordering::Relaxed);
                if idx >= files_ref.len() {
                    break;
                }
                let file = &files_ref[idx];
                let full = PathBuf::from(&workspace_path_ref).join(&file.relative_path);
                let ok =
                    ensure_preview_cache_path(full.to_string_lossy().as_ref(), size, &profile_ref, quality)
                        .is_ok();
                let _ = tx_ref.send((ok, file.filename.clone()));
            }
        });
    }
    drop(result_tx);

    let mut warmed = 0usize;
    let mut done = 0usize;
    for (ok, filename) in result_rx {
        if ok {
            warmed += 1;
        }
        done += 1;
        on_progress(WarmupProgress {
            done,
            total,
            succeeded: warmed,
            current_file: Some(filename),
            finished: done >= total,
        });
    }
    Ok(total)
}

pub fn normalize_warmup_concurrency(concurrency: usize) -> usize {
    concurrency.clamp(1, 8)
}

fn resolve_thumbnail_cache_path_v2(
    photo_path: &str,
    size: u32,
    profile: &str,
    quality: u8,
) -> Option<PathBuf> {
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
        let io_started = Instant::now();
        if let Some(bytes) = try_read_cached_thumbnail(path) {
            record_thumb_cache_hit(io_started.elapsed().as_millis());
            return Ok(bytes);
        }
    }

    let decode_started = Instant::now();
    let img = image::open(photo_path).map_err(|e| e.to_string())?;
    let decode_ms = decode_started.elapsed().as_millis();

    let resize_started = Instant::now();
    let thumb = img.thumbnail(size, size);
    let resize_ms = resize_started.elapsed().as_millis();

    let encode_started = Instant::now();
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, quality);
    thumb
        .write_with_encoder(encoder)
        .map_err(|e| e.to_string())?;
    let encode_ms = encode_started.elapsed().as_millis();

    let io_started = Instant::now();
    if let Some(path) = cache_path.as_ref() {
        write_cached_thumbnail(path, &buf);
    }
    let io_ms = io_started.elapsed().as_millis();

    record_thumb_generated(decode_ms, resize_ms, encode_ms, io_ms);

    Ok(buf)
}

fn normalize_profile(profile: &str) -> String {
    let trimmed = profile.trim();
    if trimmed.is_empty() {
        return "default".to_string();
    }
    if trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        trimmed.to_string()
    } else {
        "default".to_string()
    }
}

fn normalize_quality(quality: u8) -> u8 {
    quality.clamp(1, 100)
}

pub fn rebuild_preview_cache() -> Result<usize, String> {
    let root = thumbnail_cache_root().ok_or_else(|| "cache root unavailable".to_string())?;
    if !root.exists() {
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        return Ok(0);
    }

    let mut removed = 0usize;
    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            removed += 1;
        }
    }

    std::fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(removed)
}

pub fn get_preview_cache_info() -> Result<PreviewCacheInfo, String> {
    let root = thumbnail_cache_root().ok_or_else(|| "cache root unavailable".to_string())?;
    if !root.exists() {
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    }

    let profile_sizes = compute_profile_size_bytes(&root);
    let size_bytes = profile_sizes
        .values()
        .copied()
        .fold(0u64, |acc, v| acc.saturating_add(v));
    Ok(PreviewCacheInfo {
        path: root.to_string_lossy().to_string(),
        size_bytes,
        profile_sizes,
    })
}

fn compute_profile_size_bytes(root: &Path) -> BTreeMap<String, u64> {
    let mut profile_sizes = BTreeMap::<String, u64>::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let bytes = meta.len();
        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        let depth = rel.components().count();
        let key = if depth >= 3 {
            rel.components()
                .next()
                .and_then(|c| c.as_os_str().to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "legacy".to_string())
        } else {
            "legacy".to_string()
        };
        let current = profile_sizes.get(&key).copied().unwrap_or(0);
        profile_sizes.insert(key, current.saturating_add(bytes));
    }
    profile_sizes
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
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                },
            )
        })
        .map_err(|e| e.to_string())?;

        let src_path = PathBuf::from(workspace_path).join(&rel_path);

        if !src_path.exists() {
            let _ = progress_tx.send((i + 1, filename.clone()));
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

        let dest_file = resolve_conflict(
            &dest.join(&dest_name),
            &options.conflict,
            options.rename_prefix.as_deref(),
            options.rename_suffix_mode.as_deref(),
            taken_at.as_deref(),
        );
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
        let _ = progress_tx.send((i + 1, filename.clone()));
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

fn resolve_conflict(
    path: &Path,
    conflict: &str,
    rename_prefix: Option<&str>,
    rename_suffix_mode: Option<&str>,
    taken_at: Option<&str>,
) -> Option<PathBuf> {
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
            let prefix = sanitize_filename_fragment(rename_prefix.unwrap_or(""));
            let suffix_mode = rename_suffix_mode
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or("seq");
            for i in 1..=9999 {
                let new_name =
                    build_conflict_filename(stem, ext, &prefix, suffix_mode, i, taken_at);
                let candidate = parent.join(new_name);
                if !candidate.exists() {
                    return Some(candidate);
                }
            }
            None
        }
    }
}

fn sanitize_filename_fragment(raw: &str) -> String {
    raw.chars()
        .filter(|c| !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect::<String>()
}

fn current_date_stamp() -> String {
    Local::now().format("%Y%m%d").to_string()
}

fn current_timestamp_stamp() -> String {
    Local::now().format("%Y%m%d_%H%M%S").to_string()
}

fn build_conflict_suffix(mode: &str, index: usize, _taken_at: Option<&str>) -> String {
    match mode {
        "date_seq" => format!("_{}_{:03}", current_date_stamp(), index),
        "timestamp_seq" => format!("_{}_{:03}", current_timestamp_stamp(), index),
        _ => format!("_{:03}", index),
    }
}

fn build_conflict_filename(
    stem: &str,
    ext: &str,
    prefix: &str,
    suffix_mode: &str,
    index: usize,
    taken_at: Option<&str>,
) -> String {
    let suffix = build_conflict_suffix(suffix_mode, index, taken_at);
    if ext.is_empty() {
        format!("{prefix}{stem}{suffix}")
    } else {
        format!("{prefix}{stem}{suffix}.{ext}")
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
    use super::build_conflict_filename;
    use super::build_conflict_suffix;
    use super::build_cache_key_v2;
    use super::ensure_preview_cache_path;
    use super::get_thumbnail_perf_stats;
    use super::normalize_warmup_concurrency;
    use super::warmup_preview_cache_with_progress;
    use image::{ImageBuffer, Rgb};
    use rusqlite::params;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn cache_key_changes_when_profile_or_quality_changes() {
        let base = build_cache_key_v2("C:/photos/a.jpg", 1800, 1234, 42, "preview", 82);
        let by_profile = build_cache_key_v2("C:/photos/a.jpg", 1800, 1234, 42, "grid", 82);
        let by_quality = build_cache_key_v2("C:/photos/a.jpg", 1800, 1234, 42, "preview", 90);

        assert_ne!(base, by_profile);
        assert_ne!(base, by_quality);
    }

    #[test]
    fn thumbnail_perf_stats_is_serializable() {
        let stats = get_thumbnail_perf_stats();
        let json = serde_json::to_value(stats).expect("serialize perf stats");
        assert!(json.get("generated").is_some());
        assert!(json.get("cache_hits").is_some());
        assert!(json.get("decode_ms_total").is_some());
    }

    #[test]
    fn conflict_suffix_seq_mode_uses_zero_padded_counter() {
        let suffix = build_conflict_suffix("seq", 1, None);
        assert_eq!(suffix, "_001");
    }

    #[test]
    fn conflict_filename_applies_prefix_before_original_stem() {
        let file = build_conflict_filename("sunset", "jpg", "EXP_", "seq", 7, None);
        assert_eq!(file, "EXP_sunset_007.jpg");
    }

    #[test]
    fn warmup_concurrency_is_clamped_to_safe_bounds() {
        assert_eq!(normalize_warmup_concurrency(0), 1);
        assert_eq!(normalize_warmup_concurrency(3), 3);
        assert_eq!(normalize_warmup_concurrency(32), 8);
    }

    #[test]
    fn ensure_preview_cache_returns_existing_file_path() {
        let _guard = cache_env_lock().lock().expect("cache env lock");
        let cache_root = create_cache_root();
        std::env::set_var(
            "MYPHOTO_THUMB_CACHE_ROOT",
            cache_root.to_string_lossy().as_ref(),
        );
        let src = create_temp_test_image();

        let cache_path =
            ensure_preview_cache_path(src.to_string_lossy().as_ref(), 1600, "preview", 82)
                .expect("preview cache path should be generated");

        assert!(PathBuf::from(&cache_path).exists());

        let _ = std::fs::remove_file(src);
        let _ = std::fs::remove_file(cache_path);
        let _ = std::fs::remove_dir_all(cache_root);
        std::env::remove_var("MYPHOTO_THUMB_CACHE_ROOT");
    }

    #[test]
    fn warmup_limits_number_of_generated_items() {
        let _guard = cache_env_lock().lock().expect("cache env lock");
        let cache_root = create_cache_root();
        std::env::set_var(
            "MYPHOTO_THUMB_CACHE_ROOT",
            cache_root.to_string_lossy().as_ref(),
        );
        let workspace = create_temp_workspace_with_images(3);
        let workspace_id = prepare_workspace_for_warmup(&workspace);

        let warmed = warmup_preview_cache_with_progress(
            workspace_id,
            workspace.to_string_lossy().as_ref(),
            1200,
            "preview",
            82,
            0,
            2,
            2,
            |_progress| {},
        )
        .expect("warmup should run");

        assert_eq!(warmed, 2);

        let generated = count_cached_jpegs(&cache_root);
        assert_eq!(generated, 2);

        cleanup_workspace_from_db(workspace_id);
        let _ = std::fs::remove_dir_all(workspace);
        let _ = std::fs::remove_dir_all(cache_root);
        std::env::remove_var("MYPHOTO_THUMB_CACHE_ROOT");
    }

    #[test]
    fn warmup_reports_progress_events() {
        let _guard = cache_env_lock().lock().expect("cache env lock");
        let cache_root = create_cache_root();
        std::env::set_var(
            "MYPHOTO_THUMB_CACHE_ROOT",
            cache_root.to_string_lossy().as_ref(),
        );
        let workspace = create_temp_workspace_with_images(2);
        let workspace_id = prepare_workspace_for_warmup(&workspace);

        let mut steps: Vec<(usize, usize, bool)> = Vec::new();
        let warmed = warmup_preview_cache_with_progress(
            workspace_id,
            workspace.to_string_lossy().as_ref(),
            1200,
            "preview",
            82,
            0,
            2,
            2,
            |progress| {
                steps.push((progress.done, progress.total, progress.finished));
            },
        )
        .expect("warmup should run");

        assert_eq!(warmed, 2);
        assert!(!steps.is_empty());
        assert_eq!(steps.first().copied(), Some((0, 2, false)));
        assert_eq!(steps.last().copied(), Some((2, 2, true)));

        cleanup_workspace_from_db(workspace_id);
        let _ = std::fs::remove_dir_all(workspace);
        let _ = std::fs::remove_dir_all(cache_root);
        std::env::remove_var("MYPHOTO_THUMB_CACHE_ROOT");
    }

    #[test]
    #[ignore = "performance benchmark: run manually with --ignored --nocapture"]
    fn warmup_benchmark_logs_batch_timings() {
        let _guard = cache_env_lock().lock().expect("cache env lock");
        let workspace = create_temp_workspace_with_images(800);
        let workspace_id = prepare_workspace_for_warmup(&workspace);
        let batch = 32usize;

        let cache_root_paged = create_cache_root();
        std::env::set_var(
            "MYPHOTO_THUMB_CACHE_ROOT",
            cache_root_paged.to_string_lossy().as_ref(),
        );
        let paged = run_warmup_benchmark_paged(
            workspace_id,
            workspace.to_string_lossy().as_ref(),
            batch,
            1200,
            "preview",
            82,
        );

        let cache_root_legacy = create_cache_root();
        std::env::set_var(
            "MYPHOTO_THUMB_CACHE_ROOT",
            cache_root_legacy.to_string_lossy().as_ref(),
        );
        let legacy = run_warmup_benchmark_legacy(
            workspace.to_string_lossy().as_ref(),
            batch,
            1200,
            "preview",
            82,
        );

        eprintln!(
            "[warmup-bench] paged: processed={}, batches={}, total_ms={}, per_batch_ms={:?}",
            paged.processed,
            paged.batch_ms.len(),
            paged.total_ms,
            paged.batch_ms
        );
        eprintln!(
            "[warmup-bench] legacy-sim: processed={}, batches={}, total_ms={}, per_batch_ms={:?}",
            legacy.processed,
            legacy.batch_ms.len(),
            legacy.total_ms,
            legacy.batch_ms
        );
        if paged.total_ms > 0 {
            let ratio = (legacy.total_ms as f64) / (paged.total_ms as f64);
            eprintln!("[warmup-bench] speedup(legacy/paged)={ratio:.2}x");
        }

        assert_eq!(paged.processed, legacy.processed);
        assert_eq!(paged.processed, 800);

        cleanup_workspace_from_db(workspace_id);
        let _ = std::fs::remove_dir_all(workspace);
        let _ = std::fs::remove_dir_all(cache_root_paged);
        let _ = std::fs::remove_dir_all(cache_root_legacy);
        std::env::remove_var("MYPHOTO_THUMB_CACHE_ROOT");
    }

    fn cache_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn create_temp_test_image() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        path.push(format!("myphoto-imaging-test-{nanos}.jpg"));

        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_fn(8, 8, |_x, _y| Rgb([180, 120, 90]));
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

    fn create_temp_workspace_with_images(count: usize) -> PathBuf {
        let mut root = std::env::current_dir().expect("current dir");
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        root.push(format!("target/myphoto-test-workspace-{nanos}"));
        std::fs::create_dir_all(&root).expect("create workspace root");

        for i in 0..count {
            let file = root.join(format!("img-{i:02}.jpg"));
            let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
                ImageBuffer::from_fn(8, 8, |_x, _y| Rgb([180, 120, 90]));
            img.save(file).expect("write source image");
        }

        root
    }

    struct WarmupBenchResult {
        processed: usize,
        total_ms: u128,
        batch_ms: Vec<u128>,
    }

    fn run_warmup_benchmark_paged(
        workspace_id: i64,
        workspace_path: &str,
        batch: usize,
        size: u32,
        profile: &str,
        quality: u8,
    ) -> WarmupBenchResult {
        let mut offset = 0usize;
        let mut processed = 0usize;
        let mut batch_ms = Vec::new();
        let started = Instant::now();

        loop {
            let t0 = Instant::now();
            let done = warmup_preview_cache_with_progress(
                workspace_id,
                workspace_path,
                size,
                profile,
                quality,
                offset,
                batch,
                2,
                |_progress| {},
            )
            .expect("paged warmup run");
            batch_ms.push(t0.elapsed().as_millis());
            if done == 0 {
                break;
            }
            processed += done;
            offset += done;
            if done < batch {
                break;
            }
        }

        WarmupBenchResult {
            processed,
            total_ms: started.elapsed().as_millis(),
            batch_ms,
        }
    }

    fn run_warmup_benchmark_legacy(
        workspace_path: &str,
        batch: usize,
        size: u32,
        profile: &str,
        quality: u8,
    ) -> WarmupBenchResult {
        let mut offset = 0usize;
        let mut processed = 0usize;
        let mut batch_ms = Vec::new();
        let started = Instant::now();

        loop {
            let t0 = Instant::now();
            let files = crate::photos::get_workspace_files(workspace_path).expect("legacy file scan");
            let take = files.len().saturating_sub(offset).min(batch);
            if take == 0 {
                batch_ms.push(t0.elapsed().as_millis());
                break;
            }
            for file in files.into_iter().skip(offset).take(batch) {
                let full = PathBuf::from(workspace_path).join(file.relative_path);
                let _ = ensure_preview_cache_path(full.to_string_lossy().as_ref(), size, profile, quality);
            }
            batch_ms.push(t0.elapsed().as_millis());
            processed += take;
            offset += take;
            if take < batch {
                break;
            }
        }

        WarmupBenchResult {
            processed,
            total_ms: started.elapsed().as_millis(),
            batch_ms,
        }
    }

    fn prepare_workspace_for_warmup(workspace: &Path) -> i64 {
        let db_path = create_temp_db_path();
        std::env::set_var("MYPHOTO_DB_PATH", db_path.to_string_lossy().as_ref());
        crate::db::init_db().expect("init db");
        let ws = crate::photos::open_or_create_workspace(workspace.to_string_lossy().as_ref())
            .expect("open workspace");
        crate::photos::scan_workspace_with_progress(
            ws.id,
            workspace.to_string_lossy().as_ref(),
            |_progress| {},
        )
        .expect("scan workspace");
        ws.id
    }

    fn cleanup_workspace_from_db(workspace_id: i64) {
        let _ = crate::db::with_db(|conn| {
            conn.execute(
                "DELETE FROM photo_meta WHERE photo_id IN (SELECT id FROM photos WHERE workspace_id = ?1)",
                params![workspace_id],
            )?;
            conn.execute("DELETE FROM photos WHERE workspace_id = ?1", params![workspace_id])?;
            conn.execute("DELETE FROM workspaces WHERE id = ?1", params![workspace_id])?;
            Ok(())
        });
        if let Ok(db_path) = std::env::var("MYPHOTO_DB_PATH") {
            let _ = std::fs::remove_file(db_path);
        }
        std::env::remove_var("MYPHOTO_DB_PATH");
    }

    fn create_temp_db_path() -> PathBuf {
        let mut path = std::env::current_dir().expect("current dir");
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        path.push(format!("target/myphoto-test-db-{nanos}.sqlite3"));
        path
    }

    fn count_cached_jpegs(root: &Path) -> usize {
        let mut count = 0usize;
        if !root.exists() {
            return count;
        }
        for entry in walkdir::WalkDir::new(root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("jpg"))
            {
                count += 1;
            }
        }
        count
    }
}
