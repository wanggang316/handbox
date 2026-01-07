use crate::models::error::AppError;

/// 调试命令：检查文件是否存在及其权限
#[tauri::command]
pub async fn debug_check_file(file_path: String) -> Result<String, AppError> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let path = std::path::Path::new(&file_path);

    let mut info = String::new();
    info.push_str(&format!("Path: {}\n", file_path));
    info.push_str(&format!("Exists: {}\n", path.exists()));

    if path.exists() {
        if let Ok(metadata) = fs::metadata(path) {
            info.push_str(&format!("Is file: {}\n", metadata.is_file()));
            info.push_str(&format!("Is dir: {}\n", metadata.is_dir()));
            info.push_str(&format!("Size: {} bytes\n", metadata.len()));
            info.push_str(&format!(
                "Permissions: {:o}\n",
                metadata.permissions().mode()
            ));
        }

        // Try to read the file
        match fs::read(path) {
            Ok(data) => {
                info.push_str(&format!("Readable: true (read {} bytes)\n", data.len()));
            }
            Err(e) => {
                info.push_str(&format!("Readable: false ({})\n", e));
            }
        }
    }

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_debug_check_file() {
        // Test with a known file
        let result = debug_check_file("/etc/hosts".to_string()).await;
        assert!(result.is_ok());
    }
}
