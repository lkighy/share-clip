use crate::models::files::DirEntry;
use tokio::fs;

pub async fn read_directory(base: &std::path::Path, rel: &str) -> Result<Vec<DirEntry>, String> {
    let dir = base.join(rel);

    let mut entries = Vec::new();

    let mut rd = fs::read_dir(&dir).await.map_err(|e| e.to_string())?;

    while let Some(entry) = rd.next_entry().await.map_err(|e| e.to_string())? {
        let meta = entry.metadata().await.map_err(|e| e.to_string())?;

        let file_name = entry.file_name().to_string_lossy().to_string();

        let is_dir = meta.is_dir();

        let size = if is_dir { None } else { Some(meta.len()) };

        let child_path = if rel.is_empty() {
            file_name.clone()
        } else {
            format!("{}/{}", rel, file_name)
        };

        entries.push(DirEntry {
            name: file_name,
            path: child_path,
            is_dir,
            size,
        });
    }

    Ok(entries)
}
