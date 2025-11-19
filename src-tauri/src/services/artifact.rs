// Artifact 业务逻辑层

use crate::models::AppError;
use crate::storage::types::{
    Artifact, ArtifactFilter, ArtifactType, CreateArtifactRequest, ExecuteArtifactRequest,
    ExecutionConfig, ExecutionResult, InstallArtifactRequest, UpdateArtifactRequest,
};
use crate::storage::ArtifactRepository;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::process::Command;

/// Artifact 服务
#[derive(Clone)]
pub struct ArtifactService {
    repo: Arc<ArtifactRepository>,
    app_handle: AppHandle,
}

impl ArtifactService {
    pub fn new(repo: Arc<ArtifactRepository>, app_handle: AppHandle) -> Self {
        Self { repo, app_handle }
    }

    /// 创建 Artifact
    pub async fn create_artifact(&self, request: CreateArtifactRequest) -> Result<Artifact, AppError> {
        let now = chrono::Utc::now().timestamp_millis();
        let artifact = Artifact {
            id: uuid::Uuid::new_v4().to_string(),
            name: request.name,
            description: request.description,
            artifact_type: request.artifact_type,
            entry_file: request.entry_file,
            source_path: request.source_path,
            model_id: request.model_id,
            provider_id: request.provider_id,
            system_prompt: request.system_prompt,
            model_parameters: request.model_parameters,
            tools: request.tools,
            execution_config: request.execution_config.unwrap_or_default(),
            is_builtin: false, // User-created artifacts are not builtin
            is_installed: false,
            installed_version: None,
            installed_at: None,
            last_run_at: None,
            run_count: 0,
            tags: request.tags.unwrap_or_default(),
            icon: request.icon,
            author: None,
            created_at: now,
            updated_at: now,
        };

        self.repo.create_artifact(&artifact).await?;
        Ok(artifact)
    }

    /// 更新 Artifact
    pub async fn update_artifact(&self, request: UpdateArtifactRequest) -> Result<Artifact, AppError> {
        let mut artifact = self
            .repo
            .get_artifact_by_id(&request.id)
            .await?
            .ok_or_else(|| AppError::validation_error("Artifact not found"))?;

        if let Some(name) = request.name {
            artifact.name = name;
        }
        if let Some(description) = request.description {
            artifact.description = Some(description);
        }
        if let Some(entry_file) = request.entry_file {
            artifact.entry_file = entry_file;
        }
        if let Some(source_path) = request.source_path {
            artifact.source_path = Some(source_path);
        }
        if let Some(model_id) = request.model_id {
            artifact.model_id = Some(model_id);
        }
        if let Some(provider_id) = request.provider_id {
            artifact.provider_id = Some(provider_id);
        }
        if let Some(system_prompt) = request.system_prompt {
            artifact.system_prompt = Some(system_prompt);
        }
        if let Some(model_parameters) = request.model_parameters {
            artifact.model_parameters = Some(model_parameters);
        }
        if let Some(tools) = request.tools {
            artifact.tools = Some(tools);
        }
        if let Some(execution_config) = request.execution_config {
            artifact.execution_config = execution_config;
        }
        if let Some(tags) = request.tags {
            artifact.tags = tags;
        }
        if let Some(icon) = request.icon {
            artifact.icon = Some(icon);
        }

        artifact.updated_at = chrono::Utc::now().timestamp_millis();
        self.repo.update_artifact(&artifact).await?;
        Ok(artifact)
    }

    /// 获取 Artifact
    pub async fn get_artifact(&self, id: &str) -> Result<Artifact, AppError> {
        self.repo
            .get_artifact_by_id(&id.to_string())
            .await?
            .ok_or_else(|| AppError::validation_error("Artifact not found"))
    }

    /// 列出 Artifacts（合并内置的和已安装的）
    pub async fn list_artifacts(&self, filter: ArtifactFilter) -> Result<Vec<Artifact>, AppError> {
        // 获取数据库中已安装的 artifacts
        let installed = self.repo.list_artifacts(&filter).await?;

        // 获取内置 artifacts 定义
        let builtins = self.get_builtin_definitions();

        // 合并：内置 artifacts 中，如果已在数据库中，使用数据库版本（包含安装状态）
        let mut result_map: std::collections::HashMap<String, Artifact> = std::collections::HashMap::new();

        // 先添加已安装的
        for artifact in installed {
            result_map.insert(artifact.name.clone(), artifact);
        }

        // 再添加内置的（如果不存在）
        for builtin in builtins {
            if !result_map.contains_key(&builtin.name) {
                result_map.insert(builtin.name.clone(), builtin);
            }
        }

        let mut results: Vec<Artifact> = result_map.into_values().collect();

        // 应用过滤和排序
        if let Some(ref search) = filter.search {
            let search_lower = search.to_lowercase();
            results.retain(|a| {
                a.name.to_lowercase().contains(&search_lower) ||
                a.description.as_ref().map_or(false, |d| d.to_lowercase().contains(&search_lower))
            });
        }

        if let Some(artifact_type) = filter.artifact_type {
            results.retain(|a| a.artifact_type == artifact_type);
        }

        if let Some(is_builtin) = filter.is_builtin {
            results.retain(|a| a.is_builtin == is_builtin);
        }

        if let Some(is_installed) = filter.is_installed {
            results.retain(|a| a.is_installed == is_installed);
        }

        // 排序
        if let Some(sort_by) = &filter.sort_by {
            match sort_by.as_str() {
                "updated_at" => results.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
                "created_at" => results.sort_by(|a, b| b.created_at.cmp(&a.created_at)),
                "name" => results.sort_by(|a, b| a.name.cmp(&b.name)),
                _ => {}
            }
        }

        Ok(results)
    }

    /// 获取内置 artifacts 定义（仅定义，不存数据库）
    fn get_builtin_definitions(&self) -> Vec<Artifact> {
        let now = chrono::Utc::now().timestamp_millis();
        vec![
            Artifact {
                id: "builtin-shell-hello".to_string(),
                name: "Shell Hello World".to_string(),
                description: Some("A simple shell script that demonstrates artifact execution".to_string()),
                artifact_type: ArtifactType::Shell,
                entry_file: "main.sh".to_string(),
                source_path: Some("shell-hello".to_string()),
                model_id: None,
                provider_id: None,
                system_prompt: None,
                model_parameters: None,
                tools: None,
                execution_config: ExecutionConfig {
                    args: vec![],
                    env: std::collections::HashMap::new(),
                    permissions: vec![],
                    timeout: 5000,
                },
                is_builtin: true,
                is_installed: false,
                installed_version: None,
                installed_at: None,
                last_run_at: None,
                run_count: 0,
                tags: vec!["demo".to_string(), "shell".to_string(), "hello-world".to_string()],
                icon: Some("🐚".to_string()),
                author: Some("HandBox Team".to_string()),
                created_at: now,
                updated_at: now,
            },
            Artifact {
                id: "builtin-python-hello".to_string(),
                name: "Python Hello World".to_string(),
                description: Some("A simple Python script showcasing Python artifact execution".to_string()),
                artifact_type: ArtifactType::Python,
                entry_file: "main.py".to_string(),
                source_path: Some("python-hello".to_string()),
                model_id: None,
                provider_id: None,
                system_prompt: None,
                model_parameters: None,
                tools: None,
                execution_config: ExecutionConfig {
                    args: vec![],
                    env: std::collections::HashMap::new(),
                    permissions: vec![],
                    timeout: 10000,
                },
                is_builtin: true,
                is_installed: false,
                installed_version: None,
                installed_at: None,
                last_run_at: None,
                run_count: 0,
                tags: vec!["demo".to_string(), "python".to_string(), "hello-world".to_string()],
                icon: Some("🐍".to_string()),
                author: Some("HandBox Team".to_string()),
                created_at: now,
                updated_at: now,
            },
            Artifact {
                id: "builtin-web-chart".to_string(),
                name: "Interactive Chart Demo".to_string(),
                description: Some("An interactive chart web application with Chart.js".to_string()),
                artifact_type: ArtifactType::Web,
                entry_file: "index.html".to_string(),
                source_path: Some("web-chart".to_string()),
                model_id: None,
                provider_id: None,
                system_prompt: None,
                model_parameters: None,
                tools: None,
                execution_config: ExecutionConfig {
                    args: vec![],
                    env: std::collections::HashMap::new(),
                    permissions: vec![],
                    timeout: 0,
                },
                is_builtin: true,
                is_installed: false,
                installed_version: None,
                installed_at: None,
                last_run_at: None,
                run_count: 0,
                tags: vec!["demo".to_string(), "web".to_string(), "chart".to_string(), "visualization".to_string()],
                icon: Some("📊".to_string()),
                author: Some("HandBox Team".to_string()),
                created_at: now,
                updated_at: now,
            },
        ]
    }

    /// 删除 Artifact
    pub async fn delete_artifact(&self, id: &str) -> Result<(), AppError> {
        let artifact = self.get_artifact(id).await?;

        // 如果已安装，删除沙盒中的文件
        if artifact.is_installed {
            self.uninstall_artifact_files(&artifact).await?;
        }

        self.repo.delete_artifact(&id.to_string()).await
    }

    /// 安装 Artifact
    pub async fn install_artifact(&self, request: InstallArtifactRequest) -> Result<Artifact, AppError> {
        // 尝试从数据库获取，如果不存在，尝试从内置定义获取
        let mut artifact = match self.repo.get_artifact_by_id(&request.artifact_id).await? {
            Some(a) => a,
            None => {
                // 尝试从内置定义中查找
                let builtins = self.get_builtin_definitions();
                let builtin = builtins.into_iter()
                    .find(|a| a.id == request.artifact_id)
                    .ok_or_else(|| AppError::validation_error("Artifact not found"))?;

                // 创建到数据库
                self.repo.create_artifact(&builtin).await?;
                builtin
            }
        };

        // 如果 artifact 需要模型但用户没有选择，报错
        if artifact.model_id.is_some() && request.model_id.is_none() {
            return Err(AppError::validation_error(
                "This artifact requires a model. Please select a model.",
            ));
        }

        // 更新用户选择的模型
        if let Some(model_id) = request.model_id {
            artifact.model_id = Some(model_id);
        }
        if let Some(provider_id) = request.provider_id {
            artifact.provider_id = Some(provider_id);
        }

        // 复制文件到沙盒
        self.copy_to_sandbox(&artifact).await?;

        // 标记为已安装
        let now = chrono::Utc::now().timestamp_millis();
        let version = "1.0.0".to_string();
        self.repo.mark_installed(&artifact.id, &version, now).await?;

        artifact.is_installed = true;
        artifact.installed_at = Some(now);
        artifact.installed_version = Some(version);

        Ok(artifact)
    }

    /// 列出内置 artifacts（不再需要初始化到数据库）
    pub async fn init_builtin_artifacts(&self) -> Result<Vec<Artifact>, AppError> {
        // 返回内置应用定义（仅用于向后兼容）
        Ok(self.get_builtin_definitions())
    }

    /// 获取 artifact (优先数据库，再查内置定义)
    pub async fn get_artifact_with_fallback(&self, id: &str) -> Result<Artifact, AppError> {
        // 先从数据库查
        if let Some(artifact) = self.repo.get_artifact_by_id(&id.to_string()).await? {
            return Ok(artifact);
        }

        // 再从内置定义查
        let builtins = self.get_builtin_definitions();
        builtins.into_iter()
            .find(|a| a.id == id)
            .ok_or_else(|| AppError::validation_error("Artifact not found"))
    }

    /// 【已废弃】旧的初始化方法
    #[allow(dead_code)]
    async fn init_builtin_artifacts_old(&self) -> Result<Vec<Artifact>, AppError> {
        // 定义内置应用
        let builtins = vec![
            CreateArtifactRequest {
                name: "Shell Hello World".to_string(),
                description: Some("A simple shell script that demonstrates artifact execution".to_string()),
                artifact_type: ArtifactType::Shell,
                entry_file: "main.sh".to_string(),
                source_path: Some("shell-hello".to_string()),
                model_id: None,
                provider_id: None,
                system_prompt: None,
                model_parameters: None,
                tools: None,
                execution_config: Some(ExecutionConfig {
                    args: vec![],
                    env: std::collections::HashMap::new(),
                    permissions: vec![],
                    timeout: 5000,
                }),
                tags: Some(vec!["demo".to_string(), "shell".to_string(), "hello-world".to_string()]),
                icon: Some("🐚".to_string()),
            },
            CreateArtifactRequest {
                name: "Python Hello World".to_string(),
                description: Some("A simple Python script showcasing Python artifact execution".to_string()),
                artifact_type: ArtifactType::Python,
                entry_file: "main.py".to_string(),
                source_path: Some("python-hello".to_string()),
                model_id: None,
                provider_id: None,
                system_prompt: None,
                model_parameters: None,
                tools: None,
                execution_config: Some(ExecutionConfig {
                    args: vec![],
                    env: std::collections::HashMap::new(),
                    permissions: vec![],
                    timeout: 10000,
                }),
                tags: Some(vec!["demo".to_string(), "python".to_string(), "hello-world".to_string()]),
                icon: Some("🐍".to_string()),
            },
            CreateArtifactRequest {
                name: "Interactive Chart Demo".to_string(),
                description: Some("An interactive chart web application with Chart.js".to_string()),
                artifact_type: ArtifactType::Web,
                entry_file: "index.html".to_string(),
                source_path: Some("web-chart".to_string()),
                model_id: None,
                provider_id: None,
                system_prompt: None,
                model_parameters: None,
                tools: None,
                execution_config: Some(ExecutionConfig {
                    args: vec![],
                    env: std::collections::HashMap::new(),
                    permissions: vec![],
                    timeout: 0,
                }),
                tags: Some(vec!["demo".to_string(), "web".to_string(), "chart".to_string(), "visualization".to_string()]),
                icon: Some("📊".to_string()),
            },
        ];

        let mut created_artifacts = Vec::new();

        for request in builtins {
            // 检查是否已存在同名的内置应用
            let filter = ArtifactFilter {
                search: Some(request.name.clone()),
                artifact_type: None,
                is_builtin: Some(true),
                is_installed: None,
                tags: None,
                sort_by: None,
                sort_order: None,
                limit: Some(1),
                offset: Some(0),
            };

            let existing = self.repo.list_artifacts(&filter).await?;
            if existing.is_empty() {
                // 创建新的内置应用
                let now = chrono::Utc::now().timestamp_millis();
                let artifact = Artifact {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: request.name,
                    description: request.description,
                    artifact_type: request.artifact_type,
                    entry_file: request.entry_file,
                    source_path: request.source_path,
                    model_id: request.model_id,
                    provider_id: request.provider_id,
                    system_prompt: request.system_prompt,
                    model_parameters: request.model_parameters,
                    tools: request.tools,
                    execution_config: request.execution_config.unwrap_or_default(),
                    is_builtin: true,  // 标记为内置应用
                    is_installed: false,
                    installed_version: Some("1.0.0".to_string()),
                    installed_at: None,
                    last_run_at: None,
                    run_count: 0,
                    tags: request.tags.unwrap_or_default(),
                    icon: request.icon,
                    author: Some("HandBox Team".to_string()),
                    created_at: now,
                    updated_at: now,
                };

                self.repo.create_artifact(&artifact).await?;
                created_artifacts.push(artifact);
            }
        }

        tracing::info!("Initialized {} builtin artifacts", created_artifacts.len());
        Ok(created_artifacts)
    }

    /// 执行 Artifact
    pub async fn execute_artifact(&self, request: ExecuteArtifactRequest) -> Result<ExecutionResult, AppError> {
        let artifact = self.get_artifact(&request.artifact_id).await?;

        if !artifact.is_installed {
            return Err(AppError::validation_error(
                "Artifact is not installed. Please install it first.",
            ));
        }

        let start = std::time::Instant::now();

        // 获取沙盒路径
        let sandbox_path = self.get_sandbox_path(&artifact)?;
        let entry_path = sandbox_path.join(&artifact.entry_file);

        // 根据类型执行
        let result = match artifact.artifact_type {
            ArtifactType::Shell => self.execute_shell(&entry_path, &artifact, &request).await,
            ArtifactType::Python => self.execute_python(&entry_path, &artifact, &request).await,
            ArtifactType::Web => self.execute_web(&entry_path, &artifact).await,
        };

        let duration = start.elapsed().as_millis() as u64;

        // 更新运行统计
        let now = chrono::Utc::now().timestamp_millis();
        let _ = self.repo.update_run_stats(&artifact.id, now).await;

        match result {
            Ok((stdout, stderr, exit_code)) => Ok(ExecutionResult {
                success: exit_code == 0,
                stdout: Some(stdout),
                stderr: Some(stderr),
                exit_code: Some(exit_code),
                duration,
                error: None,
            }),
            Err(e) => Ok(ExecutionResult {
                success: false,
                stdout: None,
                stderr: None,
                exit_code: None,
                duration,
                error: Some(e.message),
            }),
        }
    }

    // === 私有辅助方法 ===

    /// 获取沙盒基础路径
    fn get_sandbox_base_path(&self) -> Result<PathBuf, AppError> {
        let app_data_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .map_err(|e| AppError::internal_error(&format!("Failed to get app data dir: {}", e)))?;

        Ok(app_data_dir.join("artifacts"))
    }

    /// 获取 artifact 沙盒路径
    fn get_sandbox_path(&self, artifact: &Artifact) -> Result<PathBuf, AppError> {
        let base = self.get_sandbox_base_path()?;
        Ok(base.join(&artifact.id))
    }

    /// 获取内置 artifact 源路径（从资源目录）
    fn get_builtin_source_path(&self, artifact: &Artifact) -> Result<PathBuf, AppError> {
        // 内置 artifacts 存储在 resources 目录
        let resource_dir = self
            .app_handle
            .path()
            .resource_dir()
            .map_err(|e| AppError::internal_error(&format!("Failed to get resource dir: {}", e)))?;

        let source = if let Some(source_path) = &artifact.source_path {
            resource_dir.join("internal_artifacts").join(source_path)
        } else {
            resource_dir
                .join("internal_artifacts")
                .join(&artifact.id)
        };

        Ok(source)
    }

    /// 复制文件到沙盒
    async fn copy_to_sandbox(&self, artifact: &Artifact) -> Result<(), AppError> {
        let dest = self.get_sandbox_path(artifact)?;

        // 创建目标目录
        tokio::fs::create_dir_all(&dest)
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create sandbox dir: {}", e)))?;

        if artifact.is_builtin {
            // 内置应用：从资源目录复制
            let source = self.get_builtin_source_path(artifact)?;
            self.copy_dir_recursive(&source, &dest).await?;
        } else {
            // 用户创建的应用：source_path 应该是用户提供的路径
            if let Some(source_path) = &artifact.source_path {
                let source = Path::new(source_path);
                self.copy_dir_recursive(source, &dest).await?;
            } else {
                // 单文件应用，创建空入口文件
                let entry_file = dest.join(&artifact.entry_file);
                tokio::fs::write(&entry_file, "")
                    .await
                    .map_err(|e| AppError::internal_error(&format!("Failed to create entry file: {}", e)))?;
            }
        }

        Ok(())
    }

    /// 递归复制目录
    fn copy_dir_recursive<'a>(&'a self, source: &'a Path, dest: &'a Path) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), AppError>> + Send + 'a>> {
        Box::pin(async move {
        tokio::fs::create_dir_all(dest)
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to create directory: {}", e)))?;

        let mut entries = tokio::fs::read_dir(source)
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to read source directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| AppError::internal_error(&format!("Failed to read entry: {}", e)))?
        {
            let file_type = entry
                .file_type()
                .await
                .map_err(|e| AppError::internal_error(&format!("Failed to get file type: {}", e)))?;

            let source_path = entry.path();
            let dest_path = dest.join(entry.file_name());

            if file_type.is_dir() {
                self.copy_dir_recursive(&source_path, &dest_path).await?;
            } else {
                tokio::fs::copy(&source_path, &dest_path)
                    .await
                    .map_err(|e| AppError::internal_error(&format!("Failed to copy file: {}", e)))?;
            }
        }

        Ok(())
        })
    }

    /// 卸载时删除沙盒文件
    async fn uninstall_artifact_files(&self, artifact: &Artifact) -> Result<(), AppError> {
        let sandbox_path = self.get_sandbox_path(artifact)?;

        if sandbox_path.exists() {
            tokio::fs::remove_dir_all(&sandbox_path)
                .await
                .map_err(|e| AppError::internal_error(&format!("Failed to remove sandbox files: {}", e)))?;
        }

        Ok(())
    }

    /// 执行 Shell 脚本
    async fn execute_shell(
        &self,
        entry_path: &Path,
        artifact: &Artifact,
        request: &ExecuteArtifactRequest,
    ) -> Result<(String, String, i32), AppError> {
        let mut cmd = Command::new("sh");
        cmd.arg(entry_path);

        // 添加参数
        if let Some(args) = &request.args {
            cmd.args(args);
        } else {
            cmd.args(&artifact.execution_config.args);
        }

        // 添加环境变量
        if let Some(env) = &request.env {
            cmd.envs(env);
        } else {
            cmd.envs(&artifact.execution_config.env);
        }

        // 设置工作目录为脚本所在目录
        if let Some(parent) = entry_path.parent() {
            cmd.current_dir(parent);
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_millis(artifact.execution_config.timeout),
            cmd.output(),
        )
        .await
        .map_err(|_| AppError::internal_error("Execution timeout"))?
        .map_err(|e| AppError::internal_error(&format!("Failed to execute shell script: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok((stdout, stderr, exit_code))
    }

    /// 执行 Python 脚本
    async fn execute_python(
        &self,
        entry_path: &Path,
        artifact: &Artifact,
        request: &ExecuteArtifactRequest,
    ) -> Result<(String, String, i32), AppError> {
        let mut cmd = Command::new("python3");
        cmd.arg(entry_path);

        if let Some(args) = &request.args {
            cmd.args(args);
        } else {
            cmd.args(&artifact.execution_config.args);
        }

        if let Some(env) = &request.env {
            cmd.envs(env);
        } else {
            cmd.envs(&artifact.execution_config.env);
        }

        if let Some(parent) = entry_path.parent() {
            cmd.current_dir(parent);
        }

        let output = tokio::time::timeout(
            std::time::Duration::from_millis(artifact.execution_config.timeout),
            cmd.output(),
        )
        .await
        .map_err(|_| AppError::internal_error("Execution timeout"))?
        .map_err(|e| AppError::internal_error(&format!("Failed to execute Python script: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok((stdout, stderr, exit_code))
    }

    /// 执行 Web 应用（使用系统浏览器打开）
    async fn execute_web(
        &self,
        entry_path: &Path,
        _artifact: &Artifact,
    ) -> Result<(String, String, i32), AppError> {
        use tauri_plugin_opener::OpenerExt;

        let absolute_path = entry_path
            .canonicalize()
            .map_err(|e| AppError::internal_error(&format!("Failed to resolve path: {}", e)))?;

        let path_str = absolute_path
            .to_str()
            .ok_or_else(|| AppError::internal_error("Invalid path encoding"))?;

        let url = format!("file://{}", path_str);

        // 使用 Tauri opener plugin 打开系统默认浏览器
        self.app_handle
            .opener()
            .open_url(&url, None::<&str>)
            .map_err(|e| AppError::internal_error(&format!("Failed to open browser: {}", e)))?;

        Ok((
            format!("Opened {} in system browser", path_str),
            String::new(),
            0,
        ))
    }
}
