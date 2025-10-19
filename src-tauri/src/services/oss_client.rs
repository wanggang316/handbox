use crate::models::AppError;
use aliyun_oss_rs::{Error as AliyunOssError, OssClient as AliyunClient};
use std::env;

/// 高阶封装的 OSS 客户端，负责根据环境变量构建并执行基础文件拉取。
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
    ) -> Self {
        let access_key_id = access_key_id.into();
        let access_key_secret = access_key_secret.into();
        let bucket = bucket.into();
        let endpoint = endpoint.into();

        let client = AliyunClient::new(&access_key_id, &access_key_secret);
        Self {
            client,
            bucket,
            endpoint,
        }
    }

    pub fn from_env() -> Result<Self, AppError> {
        let access_key_id = env::var("OSSAccessKeyId")
            .map_err(|_| AppError::internal_error("缺少 OSSAccessKeyId 环境变量"))?;
        let access_key_secret = env::var("OSSAccessKeySecret")
            .map_err(|_| AppError::internal_error("缺少 OSSAccessKeySecret 环境变量"))?;
        let bucket = env::var("OSSBucket")
            .map_err(|_| AppError::internal_error("缺少 OSSBucket 环境变量"))?;
        let endpoint = env::var("OSSEndpoint")
            .map_err(|_| AppError::internal_error("缺少 OSSEndpoint 环境变量"))?;

        Ok(Self::new(
            access_key_id,
            access_key_secret,
            bucket,
            endpoint,
        ))
    }

    pub async fn get_object_bytes(&self, key: &str) -> Result<Vec<u8>, AppError> {
        let bucket = self.client.bucket(&self.bucket, &self.endpoint);
        let object = bucket.object(key);
        let bytes = object
            .get_object()
            .download()
            .await
            .map_err(map_oss_error)?;
        Ok(bytes.to_vec())
    }

    pub async fn get_object_text(&self, key: &str) -> Result<String, AppError> {
        let bytes = self.get_object_bytes(key).await?;
        String::from_utf8(bytes).map_err(|err| {
            AppError::internal_error(&format!("OSS 文件不是合法的 UTF-8 文本: {err}"))
        })
    }
}

fn map_oss_error(error: AliyunOssError) -> AppError {
    match error {
        AliyunOssError::HyperError(err) => AppError::network_error(&format!("OSS 请求失败: {err}")),
        AliyunOssError::HyperClientError(err) => {
            AppError::network_error(&format!("OSS 客户端请求失败: {err}"))
        }
        AliyunOssError::IoError(err) => AppError::internal_error(&format!("OSS IO 错误: {err}")),
        AliyunOssError::HttpError(err) => {
            AppError::internal_error(&format!("OSS HTTP 构建失败: {err}"))
        }
        AliyunOssError::OssError(status, oss_err) => AppError::internal_error(&format!(
            "OSS 返回错误 (status: {status}): {} - {}",
            oss_err.code, oss_err.message
        )),
        AliyunOssError::OssInvalidError(status, _) => AppError::internal_error(&format!(
            "OSS 返回无法解析的错误响应，HTTP 状态码: {status}"
        )),
        AliyunOssError::OssInvalidResponse(_) => {
            AppError::internal_error("OSS 返回的响应体无法解析")
        }
        AliyunOssError::InvalidCharacter => AppError::internal_error("OSS 请求包含非法字符"),
        AliyunOssError::PathNotSupported => AppError::internal_error("OSS 不支持的文件路径"),
        AliyunOssError::InvalidFileSize => AppError::internal_error("OSS 文件大小不符合要求"),
        AliyunOssError::MissingRequestBody => AppError::internal_error("OSS 请求缺少必要的消息体"),
    }
}
