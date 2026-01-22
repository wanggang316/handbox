use crate::error::LlmClientError;
use aliyun_oss_rs::{Error as AliyunOssError, OssClient as AliyunClient};
use std::env;

const OSS_ACCESS_KEY_ID: &str = "OSSAccessKeyId";
const OSS_ACCESS_KEY_SECRET: &str = "OSSAccessKeySecret";
const OSS_BUCKET: &str = "OSSBucket";
const OSS_ENDPOINT: &str = "OSSEndpoint";
const OSS_REGION: &str = "OSSRegion";

/// Minimal wrapper around aliyun OSS client to fetch supplemental model data.
#[derive(Clone)]
pub struct OssClient {
    client: AliyunClient,
    bucket: String,
    endpoint: String,
}

impl OssClient {
    pub fn new(
        access_key_id: impl Into<String>,
        access_key_secret: impl Into<String>,
        bucket: impl Into<String>,
        endpoint: impl Into<String>,
        region: impl Into<String>,
    ) -> Self {
        let access_key_id = access_key_id.into();
        let access_key_secret = access_key_secret.into();
        let region = region.into();

        let client = AliyunClient::new(&access_key_id, &access_key_secret, &region);

        Self {
            client,
            bucket: bucket.into(),
            endpoint: endpoint.into(),
        }
    }

    pub fn from_env() -> Result<Self, LlmClientError> {
        let access_key_id = env::var(OSS_ACCESS_KEY_ID).map_err(|_| {
            LlmClientError::configuration(format!("Missing {OSS_ACCESS_KEY_ID} env"))
        })?;
        let access_key_secret = env::var(OSS_ACCESS_KEY_SECRET).map_err(|_| {
            LlmClientError::configuration(format!("Missing {OSS_ACCESS_KEY_SECRET} env"))
        })?;
        let bucket = env::var(OSS_BUCKET)
            .map_err(|_| LlmClientError::configuration(format!("Missing {OSS_BUCKET} env")))?;
        let endpoint = env::var(OSS_ENDPOINT)
            .map_err(|_| LlmClientError::configuration(format!("Missing {OSS_ENDPOINT} env")))?;
        let region = env::var(OSS_REGION)
            .map_err(|_| LlmClientError::configuration(format!("Missing {OSS_ENDPOINT} env")))?;

        Ok(Self::new(
            access_key_id,
            access_key_secret,
            bucket,
            endpoint,
            region,
        ))
    }

    pub async fn get_object_text(&self, key: &str) -> Result<String, LlmClientError> {
        let bytes = self.get_object_bytes(key).await?;
        String::from_utf8(bytes).map_err(|err| {
            LlmClientError::unexpected(format!("OSS object is not valid UTF-8: {err}"))
        })
    }

    pub async fn get_object_bytes(&self, key: &str) -> Result<Vec<u8>, LlmClientError> {
        let bucket = self.client.bucket(&self.bucket);
        let object = bucket.object(key);
        let bytes = object
            .get_object()
            .download()
            .await
            .map_err(map_oss_error)?;
        Ok(bytes.to_vec())
    }
}

fn map_oss_error(error: AliyunOssError) -> LlmClientError {
    match error {
        AliyunOssError::HyperError(err) => {
            LlmClientError::network(format!("OSS request failed: {err}"))
        }
        AliyunOssError::HyperClientError(err) => {
            LlmClientError::network(format!("OSS request failed: {err}"))
        }
        AliyunOssError::IoError(err) => LlmClientError::unexpected(format!("OSS IO error: {err}")),
        AliyunOssError::HttpError(err) => {
            LlmClientError::unexpected(format!("OSS HTTP error: {err}"))
        }
        AliyunOssError::OssError(status, oss_err) => LlmClientError::api(format!(
            "OSS responded with error (status {status}): {} - {}",
            oss_err.code, oss_err.message
        )),
        AliyunOssError::OssInvalidError(status, _) => LlmClientError::unexpected(format!(
            "OSS responded with unrecognized error body (status {status})"
        )),
        AliyunOssError::OssInvalidResponse(_) => {
            LlmClientError::unexpected("OSS returned an invalid response body")
        }
        AliyunOssError::InvalidCharacter => {
            LlmClientError::validation("OSS request contained invalid characters")
        }
        AliyunOssError::PathNotSupported => {
            LlmClientError::validation("OSS does not support the requested object path")
        }
        AliyunOssError::InvalidFileSize => {
            LlmClientError::validation("OSS reported invalid file size for the request")
        }
        AliyunOssError::MissingRequestBody => {
            LlmClientError::validation("OSS request missing required body")
        }
    }
}
