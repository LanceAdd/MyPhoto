use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub last_opened_at: Option<String>,
    pub settings_json: String,
    pub photo_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photo {
    pub id: i64,
    pub workspace_id: i64,
    pub relative_path: String,
    pub filename: String,
    pub file_size: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub taken_at: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub shutter_speed: Option<String>,
    pub aperture: Option<f64>,
    pub iso: Option<i64>,
    pub focal_length: Option<f64>,
    pub file_modified_at: Option<String>,
    pub is_missing: bool,
    // from photo_meta join
    pub star_rating: i64,
    pub color_label: String,
    pub notes: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoMeta {
    pub photo_id: i64,
    pub star_rating: i64,
    pub color_label: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybinding {
    pub id: i64,
    pub action_id: String,
    pub key_combo: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoFilter {
    pub subfolder: Option<String>,
    pub star_min: Option<i64>,
    pub star_none: Option<bool>,
    pub color_labels: Option<Vec<String>>,
    pub color_none: Option<bool>,
    pub sort_by: Option<String>,   // "taken_at" | "filename" | "file_size" | "star_rating"
    pub sort_desc: Option<bool>,
    pub include_missing: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub photo_ids: Vec<i64>,
    pub dest_folder: String,
    pub format: String,        // "original" | "jpeg" | "png" | "webp"
    pub quality: u8,           // 0-100
    pub max_dimension: Option<u32>, // long edge limit, None = original
    pub naming_rule: String,   // "original" | "date_seq"
    pub conflict: String,      // "skip" | "overwrite" | "rename"
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportProgress {
    pub total: usize,
    pub done: usize,
    pub current_file: String,
    pub finished: bool,
    pub error: Option<String>,
}
