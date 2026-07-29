use crate::models::AppError;

pub fn validate_uuid(uuid: &str) -> Result<(), AppError> {
    uuid::Uuid::parse_str(uuid).map_err(|_| AppError::validation_error("Invalid UUID format"))?;
    Ok(())
}

pub fn validate_url(url: &str) -> Result<(), AppError> {
    if url.is_empty() {
        return Err(AppError::validation_error("URL cannot be empty"));
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(AppError::validation_error(
            "URL must start with http:// or https://",
        ));
    }

    Ok(())
}

pub fn validate_temperature(temperature: f32) -> Result<(), AppError> {
    if !(0.0..=2.0).contains(&temperature) {
        return Err(AppError::validation_error(
            "Temperature must be between 0.0 and 2.0",
        ));
    }
    Ok(())
}

pub fn validate_top_p(top_p: f32) -> Result<(), AppError> {
    if !(0.0..=1.0).contains(&top_p) {
        return Err(AppError::validation_error(
            "Top-P must be between 0.0 and 1.0",
        ));
    }
    Ok(())
}
