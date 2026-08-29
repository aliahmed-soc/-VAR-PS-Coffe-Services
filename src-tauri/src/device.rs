use std::path::Path;

use uuid::Uuid;

use crate::error::AppResult;

pub fn load_or_create(app_data: &Path) -> AppResult<String> {
    std::fs::create_dir_all(app_data)?;
    let path = app_data.join("device_id");
    if path.exists() {
        return Ok(std::fs::read_to_string(path)?.trim().to_string());
    }
    let id = Uuid::new_v4().to_string();
    std::fs::write(&path, &id)?;
    Ok(id)
}
