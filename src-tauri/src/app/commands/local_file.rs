//! 我分享的文件管理窗口

use crate::models::files::DirectoryList;
use crate::utils::fs::read_directory;
use crate::utils::path::validate_relative_path;
use std::path::PathBuf;
use tauri::AppHandle;

// TODO: 获取分享文件列表，查询 local_files 的文件列表数据
// 接收参数：{ id: Option<String>, path: Option<String> }
// 传参示例： {id: None, path: None} 返回所有分享文件列表
// {id: "xxx", path: None} 如果该id的文件为文件夹，则返回该文件夹下的所有文件和文件夹，否则返回错误，无法访问
// {id: "xxx", path: "documents/work"} 返回 xxx id 文件夹下documents/work 文件夹目录
// 注意确保 path 中不包含 `..` `/` 开头、或绝对路径组件。
// 返回值：
// {type: "directory", path: "documents/work", "entries": [
// {"name": "file1.txt", path: "documents/work/files1.txt", "is_dir": false, "size": 1024},
// { "name": "subfolder", "path": "documents/work/subfolder", "is_dir": true, "size": null },
// ...
// ]}
#[tauri::command]
pub async fn list_shared_files(
    app: AppHandle,
    id: Option<String>,
    path: Option<String>,
) -> Result<DirectoryList, String> {
    // TODO: 从数据库查询 id 对应的基础目录
    // 这里只是示例
    let base_dir: PathBuf = if let Some(id) = id {
        // TODO: 修改为在数据库中查找

        // 示例：根据 id 找到实际路径
        // let record = db.find_local_file(id)?;
        // PathBuf::from(record.path)

        PathBuf::from(format!("./shared/{}", id))
    } else {
        // 返回所有分享文件
        PathBuf::from("./shared")
    };

    if !base_dir.exists() {
        return Err("share path not found".into());
    }

    let rel_path = path.unwrap_or_default();

    if !rel_path.is_empty() && !validate_relative_path(&rel_path) {
        return Err("invalid path".into());
    }

    let target = base_dir.join(&rel_path);

    if !target.exists() {
        return Err("path not exists".into());
    }

    if target.is_file() {
        return Err("cannot list a file".into());
    }

    let entries = read_directory(&base_dir, &rel_path).await?;

    Ok(DirectoryList {
        type_: "directory".into(),
        path: rel_path,
        entries,
    })
}

// TODO: 新增分享文件

// TODO: 取消分享文件

// TODO: 打开文件所在路径

// TODO: 打开文件

// TODO: 收藏

// 前端展示的内容应该有
// 1. 能够在网格和缩略图之间切换
// 2. 鼠标放在上面一段时间后显示一些详细信息
// 3. 鼠标经过时，将来自同一个剪切板的数据使用相同的颜色标注，多个数据使用不同颜色标注，可以多层叠加
// 4. 右键应该有：取消分享、打开文件所在路径、打开文件，收藏/取消收藏
