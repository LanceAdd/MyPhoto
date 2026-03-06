use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use chrono::Utc;
use rusqlite::params;

use crate::db::with_db;
use crate::models::{Photo, PhotoFilter, Workspace};

static IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "tiff", "tif",
    "bmp", "heic", "heif", "raw", "cr2", "cr3", "nef",
    "arw", "orf", "rw2", "dng", "pef", "raf",
];

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn open_or_create_workspace(path: &str) -> Result<Workspace, String> {
    let name = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_string();

    let now = Utc::now().to_rfc3339();

    with_db(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO workspaces (path, name, last_opened_at) VALUES (?1, ?2, ?3)",
            params![path, name, now],
        )?;
        conn.execute(
            "UPDATE workspaces SET last_opened_at = ?1 WHERE path = ?2",
            params![now, path],
        )?;
        let ws = conn.query_row(
            "SELECT id, path, name, last_opened_at, settings_json,
             (SELECT COUNT(*) FROM photos WHERE workspace_id = workspaces.id) as photo_count
             FROM workspaces WHERE path = ?1",
            params![path],
            |r| Ok(Workspace {
                id: r.get(0)?,
                path: r.get(1)?,
                name: r.get(2)?,
                last_opened_at: r.get(3)?,
                settings_json: r.get::<_, String>(4).unwrap_or_default(),
                photo_count: r.get(5)?,
            }),
        )?;
        Ok(ws)
    })
    .map_err(|e| e.to_string())
}

pub fn scan_workspace(workspace_id: i64, root_path: &str) -> Result<usize, String> {
    let root = PathBuf::from(root_path);
    let mut count = 0usize;

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_image(e.path()))
    {
        let full_path = entry.path();
        let relative = full_path
            .strip_prefix(&root)
            .unwrap_or(full_path)
            .to_string_lossy()
            .replace('\\', "/");
        let filename = full_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let metadata = std::fs::metadata(full_path).ok();
        let file_size = metadata.as_ref().map(|m| m.len() as i64);
        let file_modified_at = metadata
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dt: chrono::DateTime<Utc> = t.into();
                dt.to_rfc3339()
            });

        // Read EXIF
        let (taken_at, camera_make, camera_model, lens_model, shutter_speed, aperture, iso, focal_length, width, height) =
            read_exif_data(full_path);

        let _ = with_db(|conn| {
            conn.execute(
                "INSERT OR IGNORE INTO photos
                 (workspace_id, relative_path, filename, file_size, width, height,
                  taken_at, camera_make, camera_model, lens_model, shutter_speed,
                  aperture, iso, focal_length, file_modified_at, is_missing)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,0)",
                params![
                    workspace_id, relative, filename, file_size, width, height,
                    taken_at, camera_make, camera_model, lens_model, shutter_speed,
                    aperture, iso, focal_length, file_modified_at
                ],
            )?;
            // Update file_modified_at and clear is_missing for existing
            conn.execute(
                "UPDATE photos SET file_modified_at=?1, is_missing=0 WHERE workspace_id=?2 AND relative_path=?3",
                params![file_modified_at, workspace_id, relative],
            )?;
            Ok(())
        });
        count += 1;
    }

    // Mark missing files
    let _ = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, relative_path FROM photos WHERE workspace_id=?1 AND is_missing=0",
        )?;
        let rows: Vec<(i64, String)> = stmt.query_map(params![workspace_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

        for (id, rel) in rows {
            let full = root.join(&rel);
            if !full.exists() {
                conn.execute("UPDATE photos SET is_missing=1 WHERE id=?1", params![id])?;
            }
        }
        Ok(())
    });

    Ok(count)
}

fn read_exif_data(path: &Path) -> (
    Option<String>, Option<String>, Option<String>, Option<String>,
    Option<String>, Option<f64>, Option<i64>, Option<f64>,
    Option<i64>, Option<i64>
) {
    // Try to get image dimensions from the image crate first
    let (width, height) = image::image_dimensions(path)
        .map(|(w, h)| (Some(w as i64), Some(h as i64)))
        .unwrap_or((None, None));

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return (None, None, None, None, None, None, None, None, width, height),
    };
    let mut bufreader = std::io::BufReader::new(file);
    let exif_reader = exif::Reader::new();
    let exif = match exif_reader.read_from_container(&mut bufreader) {
        Ok(e) => e,
        Err(_) => return (None, None, None, None, None, None, None, None, width, height),
    };

    let get_str = |tag: exif::Tag| -> Option<String> {
        exif.get_field(tag, exif::In::PRIMARY)
            .map(|f| f.display_value().to_string().trim_matches('"').to_string())
    };

    let get_f64 = |tag: exif::Tag| -> Option<f64> {
        exif.get_field(tag, exif::In::PRIMARY)
            .and_then(|f| {
                if let exif::Value::Rational(ref v) = f.value {
                    v.first().map(|r| r.to_f64())
                } else {
                    None
                }
            })
    };

    let get_u32 = |tag: exif::Tag| -> Option<i64> {
        exif.get_field(tag, exif::In::PRIMARY)
            .and_then(|f| {
                if let exif::Value::Short(ref v) = f.value {
                    v.first().map(|&n| n as i64)
                } else if let exif::Value::Long(ref v) = f.value {
                    v.first().map(|&n| n as i64)
                } else {
                    None
                }
            })
    };

    let taken_at = get_str(exif::Tag::DateTimeOriginal);
    let camera_make = get_str(exif::Tag::Make);
    let camera_model = get_str(exif::Tag::Model);
    let lens_model = get_str(exif::Tag::LensModel);
    let aperture = get_f64(exif::Tag::FNumber);
    let focal_length = get_f64(exif::Tag::FocalLength);
    let iso = get_u32(exif::Tag::PhotographicSensitivity);
    let shutter_speed = get_str(exif::Tag::ExposureTime);

    // Try to get image dimensions from EXIF if not already got
    let (width, height) = if width.is_none() {
        let w = get_u32(exif::Tag::PixelXDimension);
        let h = get_u32(exif::Tag::PixelYDimension);
        (w, h)
    } else {
        (width, height)
    };

    (taken_at, camera_make, camera_model, lens_model, shutter_speed, aperture, iso, focal_length, width, height)
}

pub fn get_photos(workspace_id: i64, filter: &PhotoFilter) -> Result<Vec<Photo>, String> {
    with_db(|conn| {
        let mut conditions = vec!["p.workspace_id = ?1".to_string()];
        let mut param_idx = 2;

        if let Some(ref _subfolder) = filter.subfolder {
            conditions.push(format!("p.relative_path LIKE ?{}", param_idx));
            param_idx += 1;
        }
        if let Some(min) = filter.star_min {
            if min > 0 {
                conditions.push(format!("COALESCE(m.star_rating, 0) >= ?{}", param_idx));
                param_idx += 1;
            }
        }
        if let Some(ref labels) = filter.color_labels {
            if !labels.is_empty() {
                let placeholders: Vec<String> = (0..labels.len())
                    .map(|i| format!("?{}", param_idx + i))
                    .collect();
                conditions.push(format!("m.color_label IN ({})", placeholders.join(",")));
                let _ = param_idx;
            }
        }
        if filter.include_missing != Some(true) {
            conditions.push("p.is_missing = 0".to_string());
        }

        let sort_col = match filter.sort_by.as_deref() {
            Some("filename") => "p.filename",
            Some("file_size") => "p.file_size",
            Some("star_rating") => "COALESCE(m.star_rating, 0)",
            _ => "COALESCE(p.taken_at, p.filename)",
        };
        let sort_dir = if filter.sort_desc == Some(true) { "DESC" } else { "ASC" };

        let where_clause = conditions.join(" AND ");
        let sql = format!(
            "SELECT p.id, p.workspace_id, p.relative_path, p.filename, p.file_size,
             p.width, p.height, p.taken_at, p.camera_make, p.camera_model, p.lens_model,
             p.shutter_speed, p.aperture, p.iso, p.focal_length, p.file_modified_at,
             p.is_missing,
             COALESCE(m.star_rating, 0), COALESCE(m.color_label, ''), COALESCE(m.notes, '')
             FROM photos p
             LEFT JOIN photo_meta m ON m.photo_id = p.id
             WHERE {}
             ORDER BY {} {}",
            where_clause, sort_col, sort_dir
        );

        let mut stmt = conn.prepare(&sql)?;

        // Build params dynamically
        let ws_id_val = workspace_id;
        let subfolder_like = filter.subfolder.as_ref().map(|s| format!("{}%", s));

        let photos = stmt.query_map(rusqlite::params_from_iter(
            build_params(ws_id_val, filter, subfolder_like.as_deref())
        ), |r| {
            Ok(Photo {
                id: r.get(0)?,
                workspace_id: r.get(1)?,
                relative_path: r.get(2)?,
                filename: r.get(3)?,
                file_size: r.get(4)?,
                width: r.get(5)?,
                height: r.get(6)?,
                taken_at: r.get(7)?,
                camera_make: r.get(8)?,
                camera_model: r.get(9)?,
                lens_model: r.get(10)?,
                shutter_speed: r.get(11)?,
                aperture: r.get(12)?,
                iso: r.get(13)?,
                focal_length: r.get(14)?,
                file_modified_at: r.get(15)?,
                is_missing: r.get::<_, i64>(16)? != 0,
                star_rating: r.get(17)?,
                color_label: r.get::<_, String>(18).unwrap_or_default(),
                notes: r.get::<_, String>(19).unwrap_or_default(),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

        Ok(photos)
    })
    .map_err(|e| e.to_string())
}

fn build_params(
    workspace_id: i64,
    filter: &PhotoFilter,
    subfolder_like: Option<&str>,
) -> Vec<rusqlite::types::Value> {
    use rusqlite::types::Value;
    let mut params: Vec<Value> = vec![Value::Integer(workspace_id)];

    if let Some(s) = subfolder_like {
        params.push(Value::Text(s.to_string()));
    }
    if let Some(min) = filter.star_min {
        if min > 0 {
            params.push(Value::Integer(min));
        }
    }
    if let Some(ref labels) = filter.color_labels {
        for label in labels {
            params.push(Value::Text(label.clone()));
        }
    }
    params
}

pub fn get_subfolders(_workspace_id: i64, root_path: &str) -> Result<Vec<String>, String> {
    let root = PathBuf::from(root_path);
    let mut folders = vec![];

    for entry in WalkDir::new(&root)
        .max_depth(10)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_dir() && e.depth() > 0)
    {
        let rel = entry.path()
            .strip_prefix(&root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        folders.push(rel);
    }

    Ok(folders)
}

pub fn update_photo_meta(photo_id: i64, star_rating: i64, color_label: &str, notes: &str) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    with_db(|conn| {
        conn.execute(
            "INSERT INTO photo_meta (photo_id, star_rating, color_label, notes, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(photo_id) DO UPDATE SET
               star_rating = excluded.star_rating,
               color_label = excluded.color_label,
               notes = excluded.notes,
               updated_at = excluded.updated_at",
            params![photo_id, star_rating, color_label, notes, now],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

pub fn get_recent_workspaces() -> Result<Vec<Workspace>, String> {
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, path, name, last_opened_at, settings_json,
             (SELECT COUNT(*) FROM photos WHERE workspace_id = workspaces.id AND is_missing=0) as photo_count
             FROM workspaces ORDER BY last_opened_at DESC LIMIT 10"
        )?;
        let list = stmt.query_map([], |r| Ok(Workspace {
            id: r.get(0)?,
            path: r.get(1)?,
            name: r.get(2)?,
            last_opened_at: r.get(3)?,
            settings_json: r.get::<_, String>(4).unwrap_or_default(),
            photo_count: r.get(5)?,
        }))?
        .filter_map(|r| r.ok())
        .collect();
        Ok(list)
    })
    .map_err(|e| e.to_string())
}
