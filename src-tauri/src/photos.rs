use chrono::Utc;
use rusqlite::params;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

use crate::db::{with_db, with_db_mut};
use crate::models::{BatchRenameSummary, Photo, PhotoFilter, Workspace, WorkspaceFile};

static IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "tiff", "tif", "bmp", "heic", "heif", "raw", "cr2", "cr3",
    "nef", "arw", "orf", "rw2", "dng", "pef", "raf",
];

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn normalize_relative_path(root: &Path, full_path: &Path) -> Option<String> {
    let rel = full_path.strip_prefix(root).ok()?;
    let rel = rel.to_string_lossy().replace('\\', "/");
    if rel.is_empty() {
        None
    } else {
        Some(rel)
    }
}

fn metadata_signature(path: &Path) -> (Option<i64>, Option<String>) {
    let metadata = std::fs::metadata(path).ok();
    let file_size = metadata.as_ref().map(|m| m.len() as i64);
    let file_modified_at = metadata.and_then(|m| m.modified().ok()).map(|t| {
        let dt: chrono::DateTime<Utc> = t.into();
        dt.to_rfc3339()
    });
    (file_size, file_modified_at)
}

#[derive(Debug)]
struct PendingPhotoUpsert {
    relative_path: String,
    filename: String,
    file_size: Option<i64>,
    width: Option<i64>,
    height: Option<i64>,
    taken_at: Option<String>,
    camera_make: Option<String>,
    camera_model: Option<String>,
    lens_model: Option<String>,
    shutter_speed: Option<String>,
    aperture: Option<f64>,
    iso: Option<i64>,
    focal_length: Option<f64>,
    file_modified_at: Option<String>,
}

#[derive(Debug)]
struct PendingPhotoRevive {
    relative_path: String,
    file_size: Option<i64>,
    file_modified_at: Option<String>,
}

#[derive(Clone, Debug)]
struct ExistingPhotoState {
    file_modified_at: Option<String>,
    file_size: Option<i64>,
    is_missing: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ScanAction {
    Skip,
    Revive,
    Rescan,
}

#[derive(Clone, Debug)]
pub struct ScanProgress {
    pub phase: &'static str,
    pub done: usize,
    pub total: usize,
    pub current_path: Option<String>,
}

fn decide_scan_action(
    existing: Option<&ExistingPhotoState>,
    file_modified_at: &Option<String>,
    file_size: &Option<i64>,
) -> ScanAction {
    match existing {
        Some(state)
            if state.file_modified_at.as_deref() == file_modified_at.as_deref()
                && state.file_size == *file_size =>
        {
            if state.is_missing {
                ScanAction::Revive
            } else {
                ScanAction::Skip
            }
        }
        _ => ScanAction::Rescan,
    }
}

fn should_emit_progress(done: usize, total: usize) -> bool {
    done == 0 || done == total || done <= 5 || done % 25 == 0
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
            |r| {
                Ok(Workspace {
                    id: r.get(0)?,
                    path: r.get(1)?,
                    name: r.get(2)?,
                    last_opened_at: r.get(3)?,
                    settings_json: r.get::<_, String>(4).unwrap_or_default(),
                    photo_count: r.get(5)?,
                })
            },
        )?;
        Ok(ws)
    })
    .map_err(|e| e.to_string())
}

pub fn scan_workspace_with_progress<F>(
    workspace_id: i64,
    root_path: &str,
    mut on_progress: F,
) -> Result<usize, String>
where
    F: FnMut(ScanProgress),
{
    let root = PathBuf::from(root_path);
    let mut count = 0usize;
    let existing_states = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT relative_path, file_modified_at, file_size, is_missing
             FROM photos WHERE workspace_id=?1",
        )?;
        let mut map = HashMap::<String, ExistingPhotoState>::new();
        let rows = stmt.query_map(params![workspace_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                ExistingPhotoState {
                    file_modified_at: r.get(1)?,
                    file_size: r.get(2)?,
                    is_missing: r.get::<_, i64>(3)? != 0,
                },
            ))
        })?;

        for row in rows.filter_map(|v| v.ok()) {
            map.insert(row.0, row.1);
        }
        Ok(map)
    })
    .map_err(|e| e.to_string())?;

    let mut seen_paths = HashSet::<String>::new();
    let mut photos_to_upsert: Vec<PendingPhotoUpsert> = Vec::new();
    let mut photos_to_revive: Vec<PendingPhotoRevive> = Vec::new();
    let image_paths: Vec<PathBuf> = WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_image(e.path()))
        .map(|e| e.path().to_path_buf())
        .collect();

    let scan_total = image_paths.len();
    on_progress(ScanProgress {
        phase: "scan_files",
        done: 0,
        total: scan_total,
        current_path: None,
    });

    for full_path in image_paths {
        let relative = full_path
            .strip_prefix(&root)
            .unwrap_or(full_path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        let filename = full_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let (file_size, file_modified_at) = metadata_signature(full_path.as_path());

        seen_paths.insert(relative.clone());
        match decide_scan_action(
            existing_states.get(&relative),
            &file_modified_at,
            &file_size,
        ) {
            ScanAction::Skip => {
                count += 1;
                if should_emit_progress(count, scan_total) {
                    on_progress(ScanProgress {
                        phase: "scan_files",
                        done: count,
                        total: scan_total,
                        current_path: Some(relative),
                    });
                }
                continue;
            }
            ScanAction::Revive => {
                photos_to_revive.push(PendingPhotoRevive {
                    relative_path: relative,
                    file_size,
                    file_modified_at,
                });
                count += 1;
                if should_emit_progress(count, scan_total) {
                    on_progress(ScanProgress {
                        phase: "scan_files",
                        done: count,
                        total: scan_total,
                        current_path: Some(
                            photos_to_revive
                                .last()
                                .map(|p| p.relative_path.clone())
                                .unwrap_or_default(),
                        ),
                    });
                }
                continue;
            }
            ScanAction::Rescan => {}
        }

        // Read EXIF only when file is new or metadata changed.
        let (
            taken_at,
            camera_make,
            camera_model,
            lens_model,
            shutter_speed,
            aperture,
            iso,
            focal_length,
            width,
            height,
        ) = read_exif_data(full_path.as_path());

        photos_to_upsert.push(PendingPhotoUpsert {
            relative_path: relative,
            filename,
            file_size,
            width,
            height,
            taken_at,
            camera_make,
            camera_model,
            lens_model,
            shutter_speed,
            aperture,
            iso,
            focal_length,
            file_modified_at,
        });
        count += 1;
        if should_emit_progress(count, scan_total) {
            on_progress(ScanProgress {
                phase: "scan_files",
                done: count,
                total: scan_total,
                current_path: Some(
                    photos_to_upsert
                        .last()
                        .map(|p| p.relative_path.clone())
                        .unwrap_or_default(),
                ),
            });
        }
    }

    let upsert_total = photos_to_upsert.len() + photos_to_revive.len();
    if upsert_total > 0 {
        on_progress(ScanProgress {
            phase: "write_database",
            done: 0,
            total: upsert_total,
            current_path: None,
        });
        let mut upsert_done = 0usize;
        with_db_mut(|conn| {
            let tx = conn.transaction()?;
            for photo in &photos_to_upsert {
                tx.execute(
                    "INSERT OR IGNORE INTO photos
                     (workspace_id, relative_path, filename, file_size, width, height,
                      taken_at, camera_make, camera_model, lens_model, shutter_speed,
                      aperture, iso, focal_length, file_modified_at, is_missing)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,0)",
                    params![
                        workspace_id,
                        photo.relative_path,
                        photo.filename,
                        photo.file_size,
                        photo.width,
                        photo.height,
                        photo.taken_at,
                        photo.camera_make,
                        photo.camera_model,
                        photo.lens_model,
                        photo.shutter_speed,
                        photo.aperture,
                        photo.iso,
                        photo.focal_length,
                        photo.file_modified_at
                    ],
                )?;
                tx.execute(
                    "UPDATE photos
                     SET filename=?1, file_size=?2, width=?3, height=?4,
                         taken_at=?5, camera_make=?6, camera_model=?7, lens_model=?8,
                         shutter_speed=?9, aperture=?10, iso=?11, focal_length=?12,
                         file_modified_at=?13, is_missing=0
                     WHERE workspace_id=?14 AND relative_path=?15",
                    params![
                        photo.filename,
                        photo.file_size,
                        photo.width,
                        photo.height,
                        photo.taken_at,
                        photo.camera_make,
                        photo.camera_model,
                        photo.lens_model,
                        photo.shutter_speed,
                        photo.aperture,
                        photo.iso,
                        photo.focal_length,
                        photo.file_modified_at,
                        workspace_id,
                        photo.relative_path
                    ],
                )?;
                upsert_done += 1;
                if should_emit_progress(upsert_done, upsert_total) {
                    on_progress(ScanProgress {
                        phase: "write_database",
                        done: upsert_done,
                        total: upsert_total,
                        current_path: Some(photo.relative_path.clone()),
                    });
                }
            }

            for photo in &photos_to_revive {
                tx.execute(
                    "UPDATE photos
                     SET file_size=?1, file_modified_at=?2, is_missing=0
                     WHERE workspace_id=?3 AND relative_path=?4",
                    params![
                        photo.file_size,
                        photo.file_modified_at,
                        workspace_id,
                        photo.relative_path
                    ],
                )?;
                upsert_done += 1;
                if should_emit_progress(upsert_done, upsert_total) {
                    on_progress(ScanProgress {
                        phase: "write_database",
                        done: upsert_done,
                        total: upsert_total,
                        current_path: Some(photo.relative_path.clone()),
                    });
                }
            }
            tx.commit()?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    }

    let missing_paths: Vec<String> = existing_states
        .keys()
        .filter(|rel| !seen_paths.contains(*rel))
        .cloned()
        .collect();

    if !missing_paths.is_empty() {
        on_progress(ScanProgress {
            phase: "mark_missing",
            done: 0,
            total: missing_paths.len(),
            current_path: None,
        });
        let mut missing_done = 0usize;
        with_db_mut(|conn| {
            let tx = conn.transaction()?;
            for rel in &missing_paths {
                tx.execute(
                    "UPDATE photos SET is_missing=1 WHERE workspace_id=?1 AND relative_path=?2",
                    params![workspace_id, rel],
                )?;
                missing_done += 1;
                if should_emit_progress(missing_done, missing_paths.len()) {
                    on_progress(ScanProgress {
                        phase: "mark_missing",
                        done: missing_done,
                        total: missing_paths.len(),
                        current_path: Some(rel.clone()),
                    });
                }
            }
            tx.commit()?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    }

    Ok(count)
}

fn read_exif_data(
    path: &Path,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<f64>,
    Option<i64>,
    Option<f64>,
    Option<i64>,
    Option<i64>,
) {
    // Try to get image dimensions from the image crate first
    let (width, height) = image::image_dimensions(path)
        .map(|(w, h)| (Some(w as i64), Some(h as i64)))
        .unwrap_or((None, None));

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => {
            return (
                None, None, None, None, None, None, None, None, width, height,
            )
        }
    };
    let mut bufreader = std::io::BufReader::new(file);
    let exif_reader = exif::Reader::new();
    let exif = match exif_reader.read_from_container(&mut bufreader) {
        Ok(e) => e,
        Err(_) => {
            return (
                None, None, None, None, None, None, None, None, width, height,
            )
        }
    };

    let get_str = |tag: exif::Tag| -> Option<String> {
        exif.get_field(tag, exif::In::PRIMARY)
            .map(|f| f.display_value().to_string().trim_matches('"').to_string())
    };

    let get_f64 = |tag: exif::Tag| -> Option<f64> {
        exif.get_field(tag, exif::In::PRIMARY).and_then(|f| {
            if let exif::Value::Rational(ref v) = f.value {
                v.first().map(|r| r.to_f64())
            } else {
                None
            }
        })
    };

    let get_u32 = |tag: exif::Tag| -> Option<i64> {
        exif.get_field(tag, exif::In::PRIMARY).and_then(|f| {
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

    (
        taken_at,
        camera_make,
        camera_model,
        lens_model,
        shutter_speed,
        aperture,
        iso,
        focal_length,
        width,
        height,
    )
}

pub fn get_photos(workspace_id: i64, filter: &PhotoFilter) -> Result<Vec<Photo>, String> {
    with_db(|conn| {
        let mut conditions = vec!["p.workspace_id = ?1".to_string()];
        let mut param_idx = 2usize;

        if let Some(ref _subfolder) = filter.subfolder {
            conditions.push(format!("p.relative_path LIKE ?{}", param_idx));
            param_idx += 1;
        }
        if let Some(filename) = filter.filename_contains.as_ref().map(|v| v.trim()) {
            if !filename.is_empty() {
                conditions.push(format!("LOWER(p.filename) LIKE LOWER(?{})", param_idx));
                param_idx += 1;
            }
        }
        if filter.star_none == Some(true) {
            conditions.push("COALESCE(m.star_rating, 0) = 0".to_string());
        } else if let Some(min) = filter.star_min {
            if min > 0 {
                conditions.push(format!("COALESCE(m.star_rating, 0) = ?{}", param_idx));
                param_idx += 1;
            }
        }
        if let Some(ref labels) = filter.color_labels {
            if !labels.is_empty() {
                let placeholders: Vec<String> = (0..labels.len())
                    .map(|i| format!("?{}", param_idx + i))
                    .collect();
                if filter.color_none == Some(true) {
                    conditions.push(format!(
                        "(COALESCE(m.color_label, '') IN ({}) OR COALESCE(m.color_label, '') = '')",
                        placeholders.join(",")
                    ));
                } else {
                    conditions.push(format!(
                        "COALESCE(m.color_label, '') IN ({})",
                        placeholders.join(",")
                    ));
                }
            } else if filter.color_none == Some(true) {
                conditions.push("COALESCE(m.color_label, '') = ''".to_string());
            }
        } else if filter.color_none == Some(true) {
            conditions.push("COALESCE(m.color_label, '') = ''".to_string());
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
        let sort_dir = if filter.sort_desc == Some(true) {
            "DESC"
        } else {
            "ASC"
        };

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
        let filename_like = filter
            .filename_contains
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{}%", s));

        let photos = stmt
            .query_map(
                rusqlite::params_from_iter(build_params(
                    ws_id_val,
                    filter,
                    subfolder_like.as_deref(),
                    filename_like.as_deref(),
                )),
                |r| {
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
                },
            )?
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
    filename_like: Option<&str>,
) -> Vec<rusqlite::types::Value> {
    use rusqlite::types::Value;
    let mut params: Vec<Value> = vec![Value::Integer(workspace_id)];

    if let Some(s) = subfolder_like {
        params.push(Value::Text(s.to_string()));
    }
    if let Some(s) = filename_like {
        params.push(Value::Text(s.to_string()));
    }
    if filter.star_none != Some(true) {
        if let Some(min) = filter.star_min {
            if min > 0 {
                params.push(Value::Integer(min));
            }
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
    let mut folders: BTreeSet<String> = BTreeSet::new();

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_image(e.path()))
    {
        let rel = entry
            .path()
            .strip_prefix(&root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        let parts: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() <= 1 {
            continue;
        }
        let mut current = String::new();
        for segment in &parts[..parts.len() - 1] {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(segment);
            folders.insert(current.clone());
        }
    }

    Ok(folders.into_iter().collect())
}

pub fn get_workspace_files(root_path: &str) -> Result<Vec<WorkspaceFile>, String> {
    let root = PathBuf::from(root_path);
    let mut files: Vec<WorkspaceFile> = Vec::new();

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_image(e.path()))
    {
        let rel = entry
            .path()
            .strip_prefix(&root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        let filename = entry
            .path()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        files.push(WorkspaceFile {
            relative_path: rel,
            filename,
        });
    }

    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(files)
}

pub fn get_workspace_files_page(
    workspace_id: i64,
    offset: usize,
    limit: usize,
) -> Result<Vec<WorkspaceFile>, String> {
    if limit == 0 {
        return Ok(Vec::new());
    }

    let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);
    let offset_i64 = i64::try_from(offset).unwrap_or(i64::MAX);

    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT relative_path, filename
             FROM photos
             WHERE workspace_id = ?1 AND is_missing = 0
             ORDER BY relative_path ASC
             LIMIT ?2 OFFSET ?3",
        )?;
        let list = stmt
            .query_map(params![workspace_id, limit_i64, offset_i64], |r| {
                Ok(WorkspaceFile {
                    relative_path: r.get(0)?,
                    filename: r.get(1)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(list)
    })
    .map_err(|e| e.to_string())
}

pub fn update_photo_meta(
    photo_id: i64,
    star_rating: i64,
    color_label: &str,
    notes: &str,
) -> Result<(), String> {
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

fn filter_requires_meta_join(filter: &PhotoFilter) -> bool {
    filter.star_none == Some(true)
        || filter.star_min.unwrap_or(0) > 0
        || filter.color_none == Some(true)
        || filter
            .color_labels
            .as_ref()
            .map(|labels| !labels.is_empty())
            .unwrap_or(false)
        || filter.sort_by.as_deref() == Some("star_rating")
}

pub fn get_photos_basic(workspace_id: i64, filter: &PhotoFilter) -> Result<Vec<Photo>, String> {
    with_db(|conn| {
        let use_meta_join = filter_requires_meta_join(filter);
        let mut conditions = vec!["p.workspace_id = ?1".to_string()];
        let mut param_idx = 2usize;

        if let Some(ref _subfolder) = filter.subfolder {
            conditions.push(format!("p.relative_path LIKE ?{}", param_idx));
            param_idx += 1;
        }
        if let Some(filename) = filter.filename_contains.as_ref().map(|v| v.trim()) {
            if !filename.is_empty() {
                conditions.push(format!("LOWER(p.filename) LIKE LOWER(?{})", param_idx));
                param_idx += 1;
            }
        }
        if use_meta_join {
            if filter.star_none == Some(true) {
                conditions.push("COALESCE(m.star_rating, 0) = 0".to_string());
            } else if let Some(min) = filter.star_min {
                if min > 0 {
                    conditions.push(format!("COALESCE(m.star_rating, 0) = ?{}", param_idx));
                    param_idx += 1;
                }
            }
            if let Some(ref labels) = filter.color_labels {
                if !labels.is_empty() {
                    let placeholders: Vec<String> = (0..labels.len())
                        .map(|i| format!("?{}", param_idx + i))
                        .collect();
                    if filter.color_none == Some(true) {
                        conditions.push(format!(
                            "(COALESCE(m.color_label, '') IN ({}) OR COALESCE(m.color_label, '') = '')",
                            placeholders.join(",")
                        ));
                    } else {
                        conditions.push(format!("COALESCE(m.color_label, '') IN ({})", placeholders.join(",")));
                    }
                } else if filter.color_none == Some(true) {
                    conditions.push("COALESCE(m.color_label, '') = ''".to_string());
                }
            } else if filter.color_none == Some(true) {
                conditions.push("COALESCE(m.color_label, '') = ''".to_string());
            }
        }
        if filter.include_missing != Some(true) {
            conditions.push("p.is_missing = 0".to_string());
        }

        let sort_col = match filter.sort_by.as_deref() {
            Some("filename") => "p.filename",
            Some("file_size") => "p.file_size",
            Some("star_rating") if use_meta_join => "COALESCE(m.star_rating, 0)",
            _ => "COALESCE(p.taken_at, p.filename)",
        };
        let sort_dir = if filter.sort_desc == Some(true) { "DESC" } else { "ASC" };

        let where_clause = conditions.join(" AND ");
        let from_clause = if use_meta_join {
            "FROM photos p LEFT JOIN photo_meta m ON m.photo_id = p.id"
        } else {
            "FROM photos p"
        };
        let sql = format!(
            "SELECT p.id, p.workspace_id, p.relative_path, p.filename, p.file_size,
             p.width, p.height, p.taken_at, p.camera_make, p.camera_model, p.lens_model,
             p.shutter_speed, p.aperture, p.iso, p.focal_length, p.file_modified_at,
             p.is_missing
             {}
             WHERE {}
             ORDER BY {} {}",
            from_clause, where_clause, sort_col, sort_dir
        );

        let mut stmt = conn.prepare(&sql)?;
        let ws_id_val = workspace_id;
        let subfolder_like = filter.subfolder.as_ref().map(|s| format!("{}%", s));
        let filename_like = filter
            .filename_contains
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{}%", s));

        let photos = stmt.query_map(
            rusqlite::params_from_iter(build_params(
                ws_id_val,
                filter,
                subfolder_like.as_deref(),
                filename_like.as_deref(),
            )),
            |r| {
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
                    star_rating: 0,
                    color_label: String::new(),
                    notes: String::new(),
                })
            },
        )?
        .filter_map(|r| r.ok())
        .collect();

        Ok(photos)
    })
    .map_err(|e| e.to_string())
}

pub fn get_workspace_photo_meta(
    workspace_id: i64,
) -> Result<Vec<crate::models::PhotoMeta>, String> {
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT p.id, COALESCE(m.star_rating, 0), COALESCE(m.color_label, ''), COALESCE(m.notes, '')
             FROM photos p
             LEFT JOIN photo_meta m ON m.photo_id = p.id
             WHERE p.workspace_id = ?1",
        )?;

        let list = stmt
            .query_map(params![workspace_id], |r| {
                Ok(crate::models::PhotoMeta {
                    photo_id: r.get(0)?,
                    star_rating: r.get(1)?,
                    color_label: r.get::<_, String>(2).unwrap_or_default(),
                    notes: r.get::<_, String>(3).unwrap_or_default(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(list)
    })
    .map_err(|e| e.to_string())
}

pub fn get_workspace_present_photo_ids(workspace_id: i64) -> Result<Vec<i64>, String> {
    with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id
             FROM photos
             WHERE workspace_id = ?1 AND is_missing = 0",
        )?;

        let list = stmt
            .query_map(params![workspace_id], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(list)
    })
    .map_err(|e| e.to_string())
}

pub fn get_workspace_present_photo_count(workspace_id: i64) -> Result<usize, String> {
    with_db(|conn| {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*)
             FROM photos
             WHERE workspace_id = ?1 AND is_missing = 0",
            params![workspace_id],
            |r| r.get(0),
        )?;
        Ok(count.max(0) as usize)
    })
    .map_err(|e| e.to_string())
}

pub fn sync_created_files(
    workspace_id: i64,
    workspace_path: &str,
    paths: &[String],
) -> Result<usize, String> {
    let root = PathBuf::from(workspace_path);
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut dedup = HashSet::<String>::new();

    for raw_path in paths {
        let path = PathBuf::from(raw_path);
        if path.is_file() {
            if !is_image(&path) {
                continue;
            }
            let key = path.to_string_lossy().to_string();
            if dedup.insert(key) {
                candidates.push(path);
            }
            continue;
        }
        if path.is_dir() {
            for entry in WalkDir::new(&path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file() && is_image(e.path()))
            {
                let file = entry.path().to_path_buf();
                let key = file.to_string_lossy().to_string();
                if dedup.insert(key) {
                    candidates.push(file);
                }
            }
        }
    }

    if candidates.is_empty() {
        return Ok(0);
    }

    let mut synced = 0usize;
    with_db_mut(|conn| {
        let tx = conn.transaction()?;
        for full_path in &candidates {
            let Some(relative) = normalize_relative_path(&root, full_path) else {
                continue;
            };
            let filename = full_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let (file_size, file_modified_at) = metadata_signature(full_path);
            let (
                taken_at,
                camera_make,
                camera_model,
                lens_model,
                shutter_speed,
                aperture,
                iso,
                focal_length,
                width,
                height,
            ) = read_exif_data(full_path);

            tx.execute(
                "INSERT OR IGNORE INTO photos
                 (workspace_id, relative_path, filename, file_size, width, height,
                  taken_at, camera_make, camera_model, lens_model, shutter_speed,
                  aperture, iso, focal_length, file_modified_at, is_missing)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,0)",
                params![
                    workspace_id,
                    relative,
                    filename,
                    file_size,
                    width,
                    height,
                    taken_at,
                    camera_make,
                    camera_model,
                    lens_model,
                    shutter_speed,
                    aperture,
                    iso,
                    focal_length,
                    file_modified_at
                ],
            )?;
            tx.execute(
                "UPDATE photos
                 SET filename=?1, file_size=?2, width=?3, height=?4,
                     taken_at=?5, camera_make=?6, camera_model=?7, lens_model=?8,
                     shutter_speed=?9, aperture=?10, iso=?11, focal_length=?12,
                     file_modified_at=?13, is_missing=0
                 WHERE workspace_id=?14 AND relative_path=?15",
                params![
                    filename,
                    file_size,
                    width,
                    height,
                    taken_at,
                    camera_make,
                    camera_model,
                    lens_model,
                    shutter_speed,
                    aperture,
                    iso,
                    focal_length,
                    file_modified_at,
                    workspace_id,
                    relative
                ],
            )?;
            synced += 1;
        }
        tx.commit()?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    Ok(synced)
}

pub fn sync_removed_files(
    workspace_id: i64,
    workspace_path: &str,
    paths: &[String],
) -> Result<usize, String> {
    let root = PathBuf::from(workspace_path);
    let mut file_paths: HashSet<String> = HashSet::new();
    let mut folder_prefixes: HashSet<String> = HashSet::new();

    for raw_path in paths {
        let path = PathBuf::from(raw_path);
        let Some(relative) = normalize_relative_path(&root, &path) else {
            continue;
        };
        if is_image(&path) {
            file_paths.insert(relative);
        } else {
            folder_prefixes.insert(relative);
        }
    }

    if file_paths.is_empty() && folder_prefixes.is_empty() {
        return Ok(0);
    }

    let mut synced = 0usize;
    with_db_mut(|conn| {
        let tx = conn.transaction()?;
        for rel in &file_paths {
            let changed = tx.execute(
                "UPDATE photos SET is_missing=1 WHERE workspace_id=?1 AND relative_path=?2",
                params![workspace_id, rel],
            )?;
            synced += changed;
        }
        for folder in &folder_prefixes {
            let prefix = folder.trim_matches('/');
            if prefix.is_empty() {
                continue;
            }
            let like = format!("{prefix}/%");
            let changed = tx.execute(
                "UPDATE photos SET is_missing=1 WHERE workspace_id=?1 AND relative_path LIKE ?2",
                params![workspace_id, like],
            )?;
            synced += changed;
        }
        tx.commit()?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    Ok(synced)
}

#[derive(Debug)]
struct RenameCandidate {
    photo_id: i64,
    source_abs: PathBuf,
    target_relative_path: String,
    target_filename: String,
    target_abs: PathBuf,
}

#[derive(Debug)]
struct PreparedRename {
    candidate: RenameCandidate,
    temp_abs: PathBuf,
}

fn filename_extension_with_dot(filename: &str) -> &str {
    match filename.rfind('.') {
        Some(idx) if idx > 0 && idx + 1 < filename.len() => &filename[idx..],
        _ => "",
    }
}

fn join_relative_path(folder: &str, filename: &str) -> String {
    if folder.is_empty() {
        filename.to_string()
    } else {
        format!("{folder}/{filename}")
    }
}

fn format_batch_rename_filename(prefix: &str, sequence: usize, padding: usize, extension: &str) -> String {
    let width = padding.max(1);
    format!("{prefix}{sequence:0width$}{extension}", width = width)
}

fn make_temp_rename_path(source_abs: &Path, index: usize) -> PathBuf {
    let parent = source_abs.parent().unwrap_or_else(|| Path::new("."));
    let pid = std::process::id();
    let tick = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for attempt in 0..1024usize {
        let candidate = parent.join(format!(
            ".myphoto-rename-{pid}-{tick}-{index}-{attempt}.tmp"
        ));
        if !candidate.exists() {
            return candidate;
        }
    }

    parent.join(format!(".myphoto-rename-{pid}-{tick}-{index}.tmp"))
}

pub fn batch_rename_workspace_photos(
    workspace_id: i64,
    workspace_path: &str,
    prefix: &str,
    start_index: usize,
    padding: usize,
) -> Result<BatchRenameSummary, String> {
    let root = PathBuf::from(workspace_path);
    let prefix = prefix.trim();
    let sequence_start = start_index.max(1);
    let width = padding.clamp(1, 12);

    let records: Vec<(i64, String, String)> = with_db(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, relative_path, filename
             FROM photos
             WHERE workspace_id = ?1 AND is_missing = 0
             ORDER BY relative_path ASC",
        )?;
        let rows = stmt
            .query_map(params![workspace_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    })
    .map_err(|e| e.to_string())?;

    let total = records.len();
    if total == 0 {
        return Ok(BatchRenameSummary {
            total: 0,
            renamed: 0,
            skipped_conflict: 0,
            skipped_missing: 0,
            failed: 0,
        });
    }

    let source_relatives: HashSet<String> = records
        .iter()
        .map(|(_, relative_path, _)| relative_path.clone())
        .collect();

    let mut candidates: Vec<RenameCandidate> = Vec::new();
    let mut renamed = 0usize;
    let mut skipped_conflict = 0usize;
    let mut skipped_missing = 0usize;
    let mut failed = 0usize;
    let mut sequence = sequence_start;

    for (photo_id, relative_path, filename) in records {
        let normalized_relative = relative_path.replace('\\', "/");
        let source_abs = root.join(&normalized_relative);
        if !source_abs.is_file() {
            skipped_missing += 1;
            continue;
        }

        let folder = normalized_relative
            .rfind('/')
            .map(|idx| normalized_relative[..idx].to_string())
            .unwrap_or_default();
        let extension = filename_extension_with_dot(&filename);
        let target_filename = format_batch_rename_filename(prefix, sequence, width, extension);
        sequence += 1;
        let target_relative_path = join_relative_path(&folder, &target_filename);

        if target_relative_path == normalized_relative {
            renamed += 1;
            continue;
        }

        let target_abs = root.join(&target_relative_path);
        if target_abs.exists() && !source_relatives.contains(&target_relative_path) {
            skipped_conflict += 1;
            continue;
        }

        candidates.push(RenameCandidate {
            photo_id,
            source_abs,
            target_relative_path,
            target_filename,
            target_abs,
        });
    }

    let mut prepared: Vec<PreparedRename> = Vec::new();
    for (index, candidate) in candidates.into_iter().enumerate() {
        let temp_abs = make_temp_rename_path(&candidate.source_abs, index);
        if std::fs::rename(&candidate.source_abs, &temp_abs).is_err() {
            failed += 1;
            continue;
        }
        prepared.push(PreparedRename {
            candidate,
            temp_abs,
        });
    }

    let mut db_updates: Vec<(i64, String, String)> = Vec::new();
    for item in prepared {
        if std::fs::rename(&item.temp_abs, &item.candidate.target_abs).is_err() {
            let _ = std::fs::rename(&item.temp_abs, &item.candidate.source_abs);
            failed += 1;
            continue;
        }
        db_updates.push((
            item.candidate.photo_id,
            item.candidate.target_relative_path,
            item.candidate.target_filename,
        ));
        renamed += 1;
    }

    if !db_updates.is_empty() {
        with_db_mut(|conn| {
            let tx = conn.transaction()?;
            for (photo_id, relative_path, filename) in &db_updates {
                tx.execute(
                    "UPDATE photos
                     SET relative_path=?1, filename=?2, is_missing=0
                     WHERE id=?3 AND workspace_id=?4",
                    params![relative_path, filename, photo_id, workspace_id],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    }

    Ok(BatchRenameSummary {
        total,
        renamed,
        skipped_conflict,
        skipped_missing,
        failed,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        decide_scan_action, filename_extension_with_dot, format_batch_rename_filename,
        should_emit_progress, ExistingPhotoState, ScanAction,
    };

    fn existing(
        file_modified_at: Option<&str>,
        file_size: Option<i64>,
        is_missing: bool,
    ) -> ExistingPhotoState {
        ExistingPhotoState {
            file_modified_at: file_modified_at.map(|v| v.to_string()),
            file_size,
            is_missing,
        }
    }

    #[test]
    fn decide_scan_action_skips_unchanged_non_missing_file() {
        let current_modified = Some("2026-03-09T01:02:03Z".to_string());
        let current_size = Some(1234_i64);
        let state = existing(Some("2026-03-09T01:02:03Z"), Some(1234), false);

        let action = decide_scan_action(Some(&state), &current_modified, &current_size);

        assert_eq!(action, ScanAction::Skip);
    }

    #[test]
    fn decide_scan_action_revives_unchanged_missing_file() {
        let current_modified = Some("2026-03-09T01:02:03Z".to_string());
        let current_size = Some(1234_i64);
        let state = existing(Some("2026-03-09T01:02:03Z"), Some(1234), true);

        let action = decide_scan_action(Some(&state), &current_modified, &current_size);

        assert_eq!(action, ScanAction::Revive);
    }

    #[test]
    fn decide_scan_action_rescans_when_metadata_changed() {
        let current_modified = Some("2026-03-09T01:02:04Z".to_string());
        let current_size = Some(1234_i64);
        let state = existing(Some("2026-03-09T01:02:03Z"), Some(1234), false);

        let action = decide_scan_action(Some(&state), &current_modified, &current_size);

        assert_eq!(action, ScanAction::Rescan);
    }

    #[test]
    fn decide_scan_action_rescans_new_file() {
        let current_modified = Some("2026-03-09T01:02:03Z".to_string());
        let current_size = Some(1234_i64);

        let action = decide_scan_action(None, &current_modified, &current_size);

        assert_eq!(action, ScanAction::Rescan);
    }

    #[test]
    fn progress_emits_on_expected_boundaries() {
        assert!(should_emit_progress(0, 100));
        assert!(should_emit_progress(1, 100));
        assert!(should_emit_progress(5, 100));
        assert!(should_emit_progress(25, 100));
        assert!(should_emit_progress(100, 100));
        assert!(!should_emit_progress(26, 100));
    }

    #[test]
    fn extension_with_dot_keeps_suffix() {
        assert_eq!(filename_extension_with_dot("a.jpg"), ".jpg");
        assert_eq!(filename_extension_with_dot("archive.tar.gz"), ".gz");
        assert_eq!(filename_extension_with_dot(".gitignore"), "");
        assert_eq!(filename_extension_with_dot("noext"), "");
    }

    #[test]
    fn batch_rename_filename_formats_sequence_with_padding() {
        assert_eq!(
            format_batch_rename_filename("IMG_", 12, 4, ".jpg"),
            "IMG_0012.jpg"
        );
        assert_eq!(
            format_batch_rename_filename("", 3, 2, ".png"),
            "03.png"
        );
    }
}
