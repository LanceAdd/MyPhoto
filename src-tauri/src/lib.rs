mod db;
mod imaging;
mod models;
mod photos;
mod watcher;

use crate::db::with_db;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use models::*;
use rusqlite::params;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

pub struct AppState {
    pub watcher_manager: Arc<watcher::WatcherManager>,
}

// ─── Workspace Commands ───────────────────────────────────────────────────────

#[tauri::command]
async fn open_workspace(
    path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Workspace, String> {
    let ws = photos::open_or_create_workspace(&path)?;
    let ws_id = ws.id;
    let path_clone = path.clone();

    let app_for_emit = app.clone();
    let scan_path = path.clone();
    tokio::spawn(async move {
        let app_for_progress = app_for_emit.clone();
        match photos::scan_workspace_with_progress(ws_id, &scan_path, |progress| {
            let _ = app_for_progress.emit(
                "scan-progress",
                serde_json::json!({
                    "workspace_id": ws_id,
                    "phase": progress.phase,
                    "done": progress.done,
                    "total": progress.total,
                    "current_path": progress.current_path,
                }),
            );
        }) {
            Ok(count) => {
                let _ = app_for_emit.emit(
                    "scan-complete",
                    serde_json::json!({
                        "workspace_id": ws_id,
                        "count": count
                    }),
                );
            }
            Err(e) => {
                let _ = app_for_emit.emit(
                    "scan-error",
                    serde_json::json!({
                        "workspace_id": ws_id,
                        "error": e
                    }),
                );
            }
        }
    });

    state
        .watcher_manager
        .watch_workspace(ws_id, &path_clone, app);
    Ok(ws)
}

#[tauri::command]
async fn close_workspace(workspace_id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.watcher_manager.unwatch_workspace(workspace_id);
    Ok(())
}

#[tauri::command]
async fn get_recent_workspaces() -> Result<Vec<Workspace>, String> {
    photos::get_recent_workspaces()
}

// ─── Photo Commands ───────────────────────────────────────────────────────────

#[tauri::command]
async fn get_photos(workspace_id: i64, filter: PhotoFilter) -> Result<Vec<Photo>, String> {
    photos::get_photos(workspace_id, &filter)
}

#[tauri::command]
async fn get_photos_basic(workspace_id: i64, filter: PhotoFilter) -> Result<Vec<Photo>, String> {
    photos::get_photos_basic(workspace_id, &filter)
}

#[tauri::command]
async fn get_workspace_photo_meta(workspace_id: i64) -> Result<Vec<PhotoMeta>, String> {
    photos::get_workspace_photo_meta(workspace_id)
}

#[tauri::command]
async fn get_workspace_present_photo_ids(workspace_id: i64) -> Result<Vec<i64>, String> {
    photos::get_workspace_present_photo_ids(workspace_id)
}

#[tauri::command]
async fn sync_created_files(
    workspace_id: i64,
    workspace_path: String,
    paths: Vec<String>,
) -> Result<usize, String> {
    photos::sync_created_files(workspace_id, &workspace_path, &paths)
}

#[tauri::command]
async fn sync_removed_files(
    workspace_id: i64,
    workspace_path: String,
    paths: Vec<String>,
) -> Result<usize, String> {
    photos::sync_removed_files(workspace_id, &workspace_path, &paths)
}

#[tauri::command]
async fn get_subfolders(workspace_id: i64, root_path: String) -> Result<Vec<String>, String> {
    photos::get_subfolders(workspace_id, &root_path)
}

#[tauri::command]
async fn get_workspace_files(root_path: String) -> Result<Vec<WorkspaceFile>, String> {
    photos::get_workspace_files(&root_path)
}

#[tauri::command]
async fn get_thumbnail(photo_path: String, size: u32) -> Result<String, String> {
    let bytes = imaging::generate_thumbnail(&photo_path, size)?;
    Ok(STANDARD.encode(&bytes))
}

#[tauri::command]
async fn ensure_preview_cache(
    photo_path: String,
    size: u32,
    profile: String,
    quality: u8,
) -> Result<String, String> {
    imaging::ensure_preview_cache_path(&photo_path, size, &profile, quality)
}

#[tauri::command]
async fn warmup_previews(
    workspace_id: i64,
    workspace_path: String,
    size: u32,
    profile: String,
    quality: u8,
    offset: usize,
    limit: usize,
    app: AppHandle,
) -> Result<usize, String> {
    imaging::warmup_preview_cache_with_progress(
        &workspace_path,
        size,
        &profile,
        quality,
        offset,
        limit,
        |progress| {
            let _ = app.emit(
                "warmup-progress",
                serde_json::json!({
                    "workspace_id": workspace_id,
                    "done": progress.done,
                    "total": progress.total,
                    "succeeded": progress.succeeded,
                    "current_file": progress.current_file,
                    "finished": progress.finished,
                }),
            );
        },
    )
}

#[tauri::command]
async fn rebuild_preview_cache() -> Result<usize, String> {
    imaging::rebuild_preview_cache()
}

#[tauri::command]
async fn update_photo_meta(
    photo_id: i64,
    star_rating: i64,
    color_label: String,
    notes: String,
) -> Result<(), String> {
    photos::update_photo_meta(photo_id, star_rating, &color_label, &notes)
}

#[tauri::command]
async fn batch_update_meta(updates: Vec<serde_json::Value>) -> Result<(), String> {
    for u in updates {
        let photo_id = u["photo_id"].as_i64().unwrap_or(0);
        let star_rating = u["star_rating"].as_i64().unwrap_or(0);
        let color_label = u["color_label"].as_str().unwrap_or("").to_string();
        let notes = u["notes"].as_str().unwrap_or("").to_string();
        photos::update_photo_meta(photo_id, star_rating, &color_label, &notes)?;
    }
    Ok(())
}

// ─── Export Command ───────────────────────────────────────────────────────────

#[tauri::command]
async fn export_photos(
    workspace_path: String,
    options: ExportOptions,
    app: AppHandle,
) -> Result<usize, String> {
    let (tx, rx) = std::sync::mpsc::channel::<(usize, String)>();
    let total = options.photo_ids.len();
    let app_clone = app.clone();

    tokio::spawn(async move {
        while let Ok((done, current)) = rx.recv() {
            let _ = app_clone.emit(
                "export-progress",
                serde_json::json!({
                    "total": total,
                    "done": done,
                    "current_file": current,
                    "finished": done >= total
                }),
            );
            if done >= total {
                break;
            }
        }
    });

    let results = imaging::export_photos(&workspace_path, &options, tx)?;
    Ok(results.len())
}

// ─── File System Commands ─────────────────────────────────────────────────────

#[tauri::command]
async fn open_in_explorer(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg("/select,")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(
                std::path::Path::new(&path)
                    .parent()
                    .unwrap_or(std::path::Path::new("/")),
            )
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn open_with_default_app(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn delete_photos(photo_ids: Vec<i64>, workspace_path: String) -> Result<Vec<i64>, String> {
    let mut deleted = vec![];
    for id in photo_ids {
        let rel_path: Result<String, rusqlite::Error> = with_db(|conn| {
            conn.query_row(
                "SELECT relative_path FROM photos WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
        });
        if let Ok(rel) = rel_path {
            let full = std::path::PathBuf::from(&workspace_path).join(&rel);
            if full.exists() {
                std::fs::remove_file(&full).map_err(|e| e.to_string())?;
            }
            with_db(|conn| {
                conn.execute("DELETE FROM photos WHERE id=?1", params![id])?;
                Ok(())
            })
            .map_err(|e: rusqlite::Error| e.to_string())?;
            deleted.push(id);
        }
    }
    Ok(deleted)
}

#[tauri::command]
async fn copy_photos(
    photo_ids: Vec<i64>,
    workspace_path: String,
    dest_folder: String,
) -> Result<usize, String> {
    let dest = std::path::PathBuf::from(&dest_folder);
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let mut count = 0;
    for id in photo_ids {
        let rel: Result<String, rusqlite::Error> = with_db(|conn| {
            conn.query_row(
                "SELECT relative_path FROM photos WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
        });
        if let Ok(rel) = rel {
            let src = std::path::PathBuf::from(&workspace_path).join(&rel);
            if src.exists() {
                let fname = src.file_name().unwrap_or_default();
                std::fs::copy(&src, dest.join(fname)).map_err(|e| e.to_string())?;
                count += 1;
            }
        }
    }
    Ok(count)
}

#[tauri::command]
async fn move_photos(
    photo_ids: Vec<i64>,
    workspace_path: String,
    dest_folder: String,
) -> Result<usize, String> {
    let dest = std::path::PathBuf::from(&dest_folder);
    std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    let mut count = 0;
    for id in photo_ids {
        let rel: Result<String, rusqlite::Error> = with_db(|conn| {
            conn.query_row(
                "SELECT relative_path FROM photos WHERE id=?1",
                params![id],
                |r| r.get(0),
            )
        });
        if let Ok(rel) = rel {
            let src = std::path::PathBuf::from(&workspace_path).join(&rel);
            if src.exists() {
                let fname = src.file_name().unwrap_or_default();
                std::fs::rename(&src, dest.join(fname)).map_err(|e| e.to_string())?;
                with_db(|conn| {
                    conn.execute("UPDATE photos SET is_missing=1 WHERE id=?1", params![id])?;
                    Ok(())
                })
                .ok();
                count += 1;
            }
        }
    }
    Ok(count)
}

#[tauri::command]
async fn rename_folder(path: String, new_name: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    let parent = p.parent().ok_or("No parent directory")?;
    let new_path = parent.join(&new_name);
    std::fs::rename(&p, &new_path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn rename_entry(path: String, new_name: String) -> Result<String, String> {
    let p = std::path::PathBuf::from(&path);
    let parent = p.parent().ok_or("No parent directory")?;
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    let new_path = parent.join(trimmed);
    std::fs::rename(&p, &new_path).map_err(|e| e.to_string())?;
    Ok(new_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn create_folder(parent_path: String, name: String) -> Result<String, String> {
    let new_path = std::path::PathBuf::from(&parent_path).join(&name);
    std::fs::create_dir_all(&new_path).map_err(|e| e.to_string())?;
    Ok(new_path.to_string_lossy().to_string())
}

#[tauri::command]
async fn delete_entry(path: String, is_dir: bool) -> Result<(), String> {
    let p = std::path::PathBuf::from(&path);
    if !p.exists() {
        return Ok(());
    }
    if is_dir {
        std::fs::remove_dir_all(&p).map_err(|e| e.to_string())?;
    } else {
        std::fs::remove_file(&p).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ─── Keybinding Commands ──────────────────────────────────────────────────────

#[tauri::command]
async fn get_keybindings() -> Result<Vec<Keybinding>, String> {
    with_db(|conn| {
        let mut stmt =
            conn.prepare("SELECT id, action_id, key_combo, enabled FROM keybindings ORDER BY id")?;
        let list = stmt
            .query_map([], |r| {
                Ok(Keybinding {
                    id: r.get(0)?,
                    action_id: r.get(1)?,
                    key_combo: r.get(2)?,
                    enabled: r.get::<_, i64>(3)? != 0,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(list)
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_keybinding(
    action_id: String,
    key_combo: String,
    enabled: bool,
) -> Result<(), String> {
    with_db(|conn| {
        conn.execute(
            "UPDATE keybindings SET key_combo=?1, enabled=?2 WHERE action_id=?3",
            params![key_combo, enabled as i64, action_id],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_workspace_settings(workspace_id: i64, settings_json: String) -> Result<(), String> {
    with_db(|conn| {
        conn.execute(
            "UPDATE workspaces SET settings_json=?1 WHERE id=?2",
            params![settings_json, workspace_id],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn rescan_workspace(
    workspace_id: i64,
    workspace_path: String,
    app: AppHandle,
) -> Result<(), String> {
    let app_clone = app.clone();
    tokio::spawn(async move {
        let app_for_progress = app_clone.clone();
        match photos::scan_workspace_with_progress(workspace_id, &workspace_path, |progress| {
            let _ = app_for_progress.emit(
                "scan-progress",
                serde_json::json!({
                    "workspace_id": workspace_id,
                    "phase": progress.phase,
                    "done": progress.done,
                    "total": progress.total,
                    "current_path": progress.current_path,
                }),
            );
        }) {
            Ok(count) => {
                let _ = app_clone.emit(
                    "scan-complete",
                    serde_json::json!({
                        "workspace_id": workspace_id,
                        "count": count
                    }),
                );
            }
            Err(e) => {
                let _ = app_clone.emit(
                    "scan-error",
                    serde_json::json!({
                        "workspace_id": workspace_id,
                        "error": e
                    }),
                );
            }
        }
    });
    Ok(())
}

// ─── App Entry ────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    db::init_db().expect("Failed to initialize database");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(AppState {
            watcher_manager: Arc::new(watcher::WatcherManager::new()),
        })
        .invoke_handler(tauri::generate_handler![
            open_workspace,
            close_workspace,
            get_recent_workspaces,
            get_photos,
            get_photos_basic,
            get_workspace_photo_meta,
            get_workspace_present_photo_ids,
            sync_created_files,
            sync_removed_files,
            get_subfolders,
            get_workspace_files,
            get_thumbnail,
            ensure_preview_cache,
            warmup_previews,
            rebuild_preview_cache,
            update_photo_meta,
            batch_update_meta,
            export_photos,
            open_in_explorer,
            open_with_default_app,
            delete_photos,
            copy_photos,
            move_photos,
            rename_folder,
            rename_entry,
            create_folder,
            delete_entry,
            get_keybindings,
            update_keybinding,
            save_workspace_settings,
            rescan_workspace,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
