use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

pub struct WatcherManager {
    watchers: Mutex<HashMap<i64, RecommendedWatcher>>,
}

impl WatcherManager {
    pub fn new() -> Self {
        Self {
            watchers: Mutex::new(HashMap::new()),
        }
    }

    pub fn watch_workspace(&self, workspace_id: i64, path: &str, app: AppHandle) {
        let mut map: std::sync::MutexGuard<'_, HashMap<i64, notify::ReadDirectoryChangesWatcher>> =
            self.watchers.lock().unwrap();

        let app_clone = app.clone();
        let ws_id = workspace_id;

        let watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Create(_) => {
                        let paths: Vec<String> = event
                            .paths
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect();
                        let _ = app_clone.emit(
                            "file-created",
                            serde_json::json!({
                                "workspace_id": ws_id,
                                "paths": paths
                            }),
                        );
                    }
                    EventKind::Remove(_) => {
                        let paths: Vec<String> = event
                            .paths
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect();
                        let _ = app_clone.emit(
                            "file-removed",
                            serde_json::json!({
                                "workspace_id": ws_id,
                                "paths": paths
                            }),
                        );
                    }
                    EventKind::Modify(_) => {
                        let paths: Vec<String> = event
                            .paths
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect();
                        let _ = app_clone.emit(
                            "file-modified",
                            serde_json::json!({
                                "workspace_id": ws_id,
                                "paths": paths
                            }),
                        );
                    }
                    _ => {}
                }
            }
        });

        if let Ok(mut w) = watcher {
            let _ = w.watch(std::path::Path::new(path), RecursiveMode::Recursive);
            map.insert(workspace_id, w);
        }
    }

    pub fn unwatch_workspace(&self, workspace_id: i64) {
        let mut map = self.watchers.lock().unwrap();
        map.remove(&workspace_id);
    }
}
