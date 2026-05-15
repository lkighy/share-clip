use std::path::{Component, Path, PathBuf};

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use crate::entity::local_files;

pub const ROOT_RELATIVE_PATH: &str = ".";

pub async fn load_local_share(
    db: &DatabaseConnection,
    share_id: &str,
) -> Result<local_files::Model, String> {
    local_files::Entity::find_by_id(share_id.to_string())
        .filter(local_files::Column::IsValid.eq(1))
        .one(db)
        .await
        .map_err(|e| format!("database error: {e}"))?
        .ok_or_else(|| "share not found".to_string())
}

pub fn resolve_share_path(root: &Path, relative_path: Option<&str>) -> Result<PathBuf, String> {
    let root = canonicalize_existing(root).map_err(|_| "share root not found".to_string())?;
    let relative_path = relative_path.unwrap_or(ROOT_RELATIVE_PATH).trim();
    let relative_path = if relative_path.is_empty() {
        ROOT_RELATIVE_PATH
    } else {
        relative_path
    };

    let mut normalized = PathBuf::new();
    if relative_path != ROOT_RELATIVE_PATH {
        let candidate = Path::new(relative_path);
        if candidate.is_absolute() {
            return Err("absolute path is not allowed".to_string());
        }
        for component in candidate.components() {
            match component {
                Component::Normal(part) => normalized.push(part),
                Component::CurDir => {}
                _ => return Err("invalid relative path".to_string()),
            }
        }
    }

    let joined = if normalized.as_os_str().is_empty() {
        root.clone()
    } else {
        root.join(normalized)
    };
    let canonical = canonicalize_existing(&joined).map_err(|_| "path not found".to_string())?;
    if !canonical.starts_with(&root) {
        return Err("path is outside share root".to_string());
    }
    Ok(canonical)
}

pub fn relative_path_for(root: &Path, target: &Path) -> String {
    target
        .strip_prefix(root)
        .ok()
        .and_then(|p| {
            let text = p.to_string_lossy().replace('\\', "/");
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        })
        .unwrap_or_else(|| ROOT_RELATIVE_PATH.to_string())
}

pub fn canonicalize_existing(path: &Path) -> std::io::Result<PathBuf> {
    std::fs::canonicalize(path)
}
