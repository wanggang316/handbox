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

#[command]
pub async fn clipboard_copy_image(path: String) -> Result<(), AppError> {
    let image_path = Path::new(&path);

    if !image_path.exists() {
        return Err(AppError {
            code: "FILE_NOT_FOUND".to_string(),
            message: format!("Image file not found: {}", path),
        });
    }

    let image_data = std::fs::read(image_path).map_err(|e| AppError {
        code: "READ_ERROR".to_string(),
        message: format!("Failed to read image file: {}", e),
    })?;

    let img = image::load_from_memory(&image_data).map_err(|e| AppError {
        code: "DECODE_ERROR".to_string(),
        message: format!("Failed to decode image: {}", e),
    })?;

    // arboard expects raw RGBA8.
    let rgba_img = img.to_rgba8();
    let (width, height) = rgba_img.dimensions();
    let rgba_data = rgba_img.into_raw();

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
