use rusqlite::{Connection, Result, params};
use std::path::PathBuf;
use once_cell::sync::Lazy;
use std::sync::Mutex;

static DB: Lazy<Mutex<Option<Connection>>> = Lazy::new(|| Mutex::new(None));

pub fn get_db_path() -> PathBuf {
    let data_dir = dirs_next::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("myphoto");
    std::fs::create_dir_all(&data_dir).ok();
    data_dir.join("myphoto.db")
}

pub fn init_db() -> Result<()> {
    let path = get_db_path();
    let conn = Connection::open(&path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    create_tables(&conn)?;
    *DB.lock().unwrap() = Some(conn);
    Ok(())
}

fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS workspaces (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            last_opened_at TEXT,
            settings_json TEXT DEFAULT '{}'
        );

        CREATE TABLE IF NOT EXISTS photos (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace_id INTEGER NOT NULL,
            relative_path TEXT NOT NULL,
            filename TEXT NOT NULL,
            file_size INTEGER,
            width INTEGER,
            height INTEGER,
            taken_at TEXT,
            camera_make TEXT,
            camera_model TEXT,
            lens_model TEXT,
            shutter_speed TEXT,
            aperture REAL,
            iso INTEGER,
            focal_length REAL,
            file_modified_at TEXT,
            is_missing INTEGER DEFAULT 0,
            UNIQUE(workspace_id, relative_path)
        );

        CREATE TABLE IF NOT EXISTS photo_meta (
            photo_id INTEGER PRIMARY KEY,
            star_rating INTEGER DEFAULT 0,
            color_label TEXT DEFAULT '',
            notes TEXT DEFAULT '',
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY(photo_id) REFERENCES photos(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS keybindings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            action_id TEXT NOT NULL UNIQUE,
            key_combo TEXT NOT NULL,
            enabled INTEGER DEFAULT 1
        );

        CREATE INDEX IF NOT EXISTS idx_photos_workspace ON photos(workspace_id);
        CREATE INDEX IF NOT EXISTS idx_photos_missing ON photos(is_missing);
    ")?;

    // Insert default keybindings if empty
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM keybindings",
        [],
        |r| r.get(0),
    )?;

    if count == 0 {
        let defaults = vec![
            ("open_workspace", "Ctrl+O"),
            ("close_workspace", "Ctrl+W"),
            ("open_settings", "Ctrl+,"),
            ("show_help", "?"),
            ("nav_left", "ArrowLeft"),
            ("nav_right", "ArrowRight"),
            ("nav_up", "ArrowUp"),
            ("nav_down", "ArrowDown"),
            ("enter_lightbox", "Enter"),
            ("add_to_selection", "Space"),
            ("toggle_cull_mode", "Tab"),
            ("star_1", "1"),
            ("star_2", "2"),
            ("star_3", "3"),
            ("star_4", "4"),
            ("star_5", "5"),
            ("color_red", "6"),
            ("color_orange", "7"),
            ("color_yellow", "8"),
            ("color_green", "9"),
            ("clear_meta", "0"),
            ("delete_photos", "Delete"),
            ("zoom_in", "+"),
            ("zoom_out", "-"),
            ("zoom_reset", "Ctrl+0"),
            ("exit_lightbox", "Escape"),
        ];
        for (action_id, key_combo) in defaults {
            conn.execute(
                "INSERT OR IGNORE INTO keybindings (action_id, key_combo, enabled) VALUES (?1, ?2, 1)",
                params![action_id, key_combo],
            )?;
        }
    }

    Ok(())
}

pub fn with_db<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&Connection) -> Result<T>,
{
    let guard = DB.lock().unwrap();
    let conn = guard.as_ref().expect("DB not initialized");
    f(conn)
}

pub fn with_db_mut<F, T>(f: F) -> Result<T>
where
    F: FnOnce(&mut Connection) -> Result<T>,
{
    let mut guard = DB.lock().unwrap();
    let conn = guard.as_mut().expect("DB not initialized");
    f(conn)
}
