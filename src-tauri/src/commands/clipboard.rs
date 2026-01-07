use std::path::Path;
use tauri::command;

#[derive(Debug, serde::Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

/// 复制图片文件到剪贴板
#[command]
pub async fn clipboard_copy_image(path: String) -> Result<(), AppError> {
    let image_path = Path::new(&path);

    // 检查文件是否存在
    if !image_path.exists() {
        return Err(AppError {
            code: "FILE_NOT_FOUND".to_string(),
            message: format!("Image file not found: {}", path),
        });
    }

    // 读取图片文件
    let image_data = std::fs::read(image_path).map_err(|e| AppError {
        code: "READ_ERROR".to_string(),
        message: format!("Failed to read image file: {}", e),
    })?;

    // 使用 image crate 来解码图片
    let img = image::load_from_memory(&image_data).map_err(|e| AppError {
        code: "DECODE_ERROR".to_string(),
        message: format!("Failed to decode image: {}", e),
    })?;

    // 转换为 RGBA8 格式
    let rgba_img = img.to_rgba8();
    let (width, height) = rgba_img.dimensions();
    let rgba_data = rgba_img.into_raw();

    // 使用 arboard 复制到剪贴板
    let mut clipboard = arboard::Clipboard::new().map_err(|e| AppError {
        code: "CLIPBOARD_ERROR".to_string(),
        message: format!("Failed to access clipboard: {}", e),
    })?;

    let img_data = arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: rgba_data.into(),
    };

    clipboard.set_image(img_data).map_err(|e| AppError {
        code: "CLIPBOARD_ERROR".to_string(),
        message: format!("Failed to copy image to clipboard: {}", e),
    })?;

    Ok(())
}
