use std::path::{Path, PathBuf};
use std::io::BufWriter;
use image::{DynamicImage, imageops::FilterType};
use crate::models::ExportOptions;
use crate::db::with_db;
use rusqlite::params;

pub fn generate_thumbnail(photo_path: &str, size: u32) -> Result<Vec<u8>, String> {
    let img = image::open(photo_path).map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(size, size);
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    thumb.write_to(&mut cursor, image::ImageFormat::Jpeg)
        .map_err(|e| e.to_string())?;
    Ok(buf)
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
