use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use anyhow::{Context, Result};
use async_trait::async_trait;
use forge_app::{
    CommandInfra, EnvironmentInfra, FileReaderInfra, SyncProgressCounter, WalkedFile, Walker,
    WalkerInfra, WorkspaceService, WorkspaceStatus, compute_hash,
};
use forge_domain::{
    AuthCredential, AuthDetails, FileHash, FileNode, ProviderId, ProviderRepository, SyncProgress,
    UserId, WorkspaceId, WorkspaceIndexRepository,
};
use forge_stream::MpscStream;
use futures::future::join_all;
use futures::stream::{Stream, StreamExt};
use tracing::{info, warn};

use crate::error::Error as ServiceError;

/// Error type for a single file that could not be read during workspace
/// operations, carrying the file path for downstream reporting.
#[derive(Debug, thiserror::Error)]
#[error("Failed to read file '{path}': {source}")]
struct FileReadError {
    path: PathBuf,
    #[source]
    source: anyhow::Error,
}

static ALLOWED_EXTENSIONS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let extensions_str = include_str!("allowed_extensions.txt");
    extensions_str
        .lines()
        .map(|line| line.trim().to_lowercase())
        .filter(|line| !line.is_empty())
        .collect()
});

/// Loads allowed file extensions from allowed_extensions.txt into a HashSet
fn allowed_extensions() -> &'static HashSet<String> {
    &ALLOWED_EXTENSIONS
}

/// Checks if a file has an allowed extension for workspace syncing (O(1)
/// lookup)
fn has_allowed_extension(path: &Path) -> bool {
    if let Some(extension) = path.extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        allowed_extensions().contains(&ext)
    } else {
        false
    }
}

/// Service for indexing workspaces and performing semantic search
pub struct ForgeWorkspaceService<F> {
    infra: Arc<F>,
}

impl<F> Clone for ForgeWorkspaceService<F> {
    fn clone(&self) -> Self {
        Self { infra: Arc::clone(&self.infra) }
    }
}

impl<F: 'static + ProviderRepository + WorkspaceIndexRepository> ForgeWorkspaceService<F> {
    /// Creates a new indexing service with the provided infrastructure.
    pub fn new(infra: Arc<F>) -> Self {
        Self { infra }
    }

    /// Fetches remote file hashes from the server.
    async fn fetch_remote_hashes(
        &self,
        user_id: &UserId,
        workspace_id: &WorkspaceId,
        auth_token: &forge_domain::ApiKey,
    ) -> anyhow::Result<Vec<FileHash>>
    where
        F: WorkspaceIndexRepository,
    {
        info!(workspace_id = %workspace_id, "Fetching existing file hashes from server to detect changes...");
        let workspace_files =
            forge_domain::CodeBase::new(user_id.clone(), workspace_id.clone(), ());

        self.infra
            .list_workspace_files(&workspace_files, auth_token)
            .await
    }

    /// Deletes a batch of files from the server.
    async fn delete(
        &self,
        user_id: &UserId,
        workspace_id: &WorkspaceId,
        token: &forge_domain::ApiKey,
        paths: Vec<String>,
    ) -> Result<()>
    where
        F: WorkspaceIndexRepository,
    {
        let deletion = forge_domain::CodeBase::new(user_id.clone(), workspace_id.clone(), paths);

        self.infra
            .delete_files(&deletion, token)
            .await
            .context("Failed to delete files")
    }

    /// Deletes files from the workspace and updates the progress counter.
    ///
    /// Returns the number of files that were successfully deleted.
    async fn delete_files(
        &self,
        user_id: &UserId,
        workspace_id: &WorkspaceId,
        token: &forge_domain::ApiKey,
        files_to_delete: Vec<String>,
    ) -> Result<usize>
    where
        F: WorkspaceIndexRepository,
    {
        if files_to_delete.is_empty() {
            return Ok(0);
        }

        self.delete(user_id, workspace_id, token, files_to_delete.clone())
            .await?;

        for path in &files_to_delete {
            info!(workspace_id = %workspace_id, path = %path, "File deleted successfully");
        }

        Ok(files_to_delete.len())
    }

    /// Uploads a batch of files to the server.
    async fn upload(
        &self,
        user_id: &UserId,
        workspace_id: &WorkspaceId,
        token: &forge_domain::ApiKey,
        files: Vec<forge_domain::FileRead>,
    ) -> Result<()>
    where
        F: WorkspaceIndexRepository,
    {
        let upload = forge_domain::CodeBase::new(user_id.clone(), workspace_id.clone(), files);

        self.infra
            .upload_files(&upload, token)
            .await
            .context("Failed to upload files")?;
        Ok(())
    }

    /// Uploads files in parallel, returning a stream of results.
    ///
    /// The caller is responsible for processing the stream and tracking
    /// progress.
    fn upload_files(
        &self,
        user_id: &UserId,
        workspace_id: &WorkspaceId,
        token: &forge_domain::ApiKey,
        files: Vec<forge_domain::FileNode>,
        batch_size: usize,
    ) -> impl Stream<Item = Result<usize, anyhow::Error>> + Send
    where
        F: WorkspaceIndexRepository,
    {
        let user_id = user_id.clone();
        let workspace_id = workspace_id.clone();
        let token = token.clone();

        let file_reads = files
            .into_iter()
            .map(|f| forge_domain::FileRead::new(f.file_path, f.content))
            .collect::<Vec<_>>();

        futures::stream::iter(file_reads)
            .map(move |file| {
                let user_id = user_id.clone();
                let workspace_id = workspace_id.clone();
                let token = token.clone();
                let file_path = file.path.clone();
                async move {
                    info!(workspace_id = %workspace_id, path = %file_path, "File sync started");
                    self.upload(&user_id, &workspace_id, &token, vec![file])
                        .await?;
                    info!(workspace_id = %workspace_id, path = %file_path, "File sync completed");
                    Ok::<_, anyhow::Error>(1)
                }
            })
            .buffer_unordered(batch_size)
    }

    /// Internal sync implementation that emits progress events.
    async fn sync_codebase_internal<E, Fut>(
        &self,
        path: PathBuf,
        batch_size: usize,
        emit: E,
    ) -> Result<()>
    where
        F: ProviderRepository
            + WorkspaceIndexRepository
            + FileReaderInfra
            + EnvironmentInfra
            + CommandInfra
            + WalkerInfra,
        E: Fn(SyncProgress) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = ()> + Send,
    {
        info!(path = %path.display(), "Starting workspace sync");

        emit(SyncProgress::Starting).await;

        let (token, user_id) = self.get_workspace_credentials().await?;
        let path = path
            .canonicalize()
            .with_context(|| format!("Failed to resolve path: {}", path.display()))?;

        // Find existing workspace - do NOT auto-create
        let workspace = self
            .find_workspace_by_path(path, &token)
            .await?
            .context("Workspace not indexed. Please run `forge workspace init` first.")?;

        let workspace_id = workspace.workspace_id.clone();

        // Use the canonical root stored in the workspace record so that file
        // discovery and remote-hash comparison are always relative to the same
        // base, even when `path` is a subdirectory of an ancestor workspace.
        let workspace_root = PathBuf::from(&workspace.working_dir);

        // Read all files and compute hashes from the workspace root path
        emit(SyncProgress::DiscoveringFiles {
            path: workspace_root.clone(),
            workspace_id: workspace_id.clone(),
        })
        .await;

        let results: Vec<Result<FileNode>> = self
            .read_files(batch_size, &workspace_root, &workspace_id)
            .collect()
            .await;
        let failed_statuses: Vec<forge_domain::FileStatus> = results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .filter_map(|e| e.downcast_ref::<FileReadError>())
            .map(|e| {
                forge_domain::FileStatus::new(
                    e.path.to_string_lossy().into_owned(),
                    forge_domain::SyncStatus::Failed,
                )
            })
            .collect();
        let local_files: Vec<FileNode> = results.into_iter().flatten().collect();

        let total_file_count = local_files.len() + failed_statuses.len();
        emit(SyncProgress::FilesDiscovered { count: total_file_count }).await;

        let remote_files = self
            .fetch_remote_hashes(&user_id, &workspace_id, &token)
            .await?;

        emit(SyncProgress::ComparingFiles {
            remote_files: remote_files.len(),
            local_files: total_file_count,
        })
        .await;

        let plan = WorkspaceStatus::new(workspace_root.clone(), remote_files);
        let local_file_hashes: Vec<forge_domain::FileHash> =
            local_files.iter().cloned().map(Into::into).collect();
        let mut statuses = plan.file_statuses(local_file_hashes);
        statuses.extend(failed_statuses);

        // Compute counts from statuses
        let added = statuses
            .iter()
            .filter(|s| s.status == forge_domain::SyncStatus::New)
            .count();
        let deleted = statuses
            .iter()
            .filter(|s| s.status == forge_domain::SyncStatus::Deleted)
            .count();
        let modified = statuses
            .iter()
            .filter(|s| s.status == forge_domain::SyncStatus::Modified)
            .count();
        let mut failed_files = statuses
            .iter()
            .filter(|s| s.status == forge_domain::SyncStatus::Failed)
            .count();

        // Compute total number of affected files
        let total_file_changes = added + deleted + modified;

        // Only emit diff computed event if there are actual changes
        if total_file_changes > 0 {
            emit(SyncProgress::DiffComputed { added, deleted, modified }).await;
        }

        let (files_to_delete, nodes_to_upload) = plan.get_operations(local_files);

        let total_operations = files_to_delete.len() + nodes_to_upload.len();
        let mut counter = SyncProgressCounter::new(total_file_changes, total_operations);

        emit(counter.sync_progress()).await;

        // Delete all files in a single batched call
        match self
            .delete_files(&user_id, &workspace_id, &token, files_to_delete.clone())
            .await
        {
            Ok(deleted_count) => {
                counter.complete(deleted_count);
                emit(counter.sync_progress()).await;
            }
            Err(e) => {
                warn!(workspace_id = %workspace_id, error = ?e,"Failed to delete files during sync");
                failed_files += files_to_delete.len();
            }
        }

        // Upload files in parallel
        let mut upload_stream =
            self.upload_files(&user_id, &workspace_id, &token, nodes_to_upload, batch_size);

        // Process uploads as they complete, updating progress incrementally
        while let Some(result) = upload_stream.next().await {
            match result {
                Ok(count) => {
                    counter.complete(count);
                    emit(counter.sync_progress()).await;
                }
                Err(e) => {
                    warn!(workspace_id = %workspace_id, error = ?e, "Failed to upload file during sync");
                    failed_files += 1;
                    // Continue processing remaining uploads
                }
            }
        }

        info!(
            workspace_id = %workspace_id,
            total_files = total_file_count,
            "Sync completed successfully"
        );

        emit(SyncProgress::Completed {
            total_files: total_file_count,
            uploaded_files: total_file_changes,
            failed_files,
        })
        .await;

        // Fail if there were any failed files
        if failed_files > 0 {
            Err(forge_domain::Error::sync_failed(failed_files).into())
        } else {
            Ok(())
        }
    }

    /// Gets the ForgeCode services credential and extracts workspace auth
    /// components
    ///
    /// # Errors
    /// Returns an error if the credential is not found, if there's a database
    /// error, or if the credential format is invalid
    async fn get_workspace_credentials(&self) -> Result<(forge_domain::ApiKey, UserId)>
    where
        F: ProviderRepository,
    {
        let credential = self
            .infra
            .get_credential(&ProviderId::FORGE_SERVICES)
            .await?
            .context("No authentication credentials found. Please authenticate first.")?;

        match &credential.auth_details {
            AuthDetails::ApiKey(token) => {
                // Extract user_id from URL params
                let user_id_str = credential
                    .url_params
                    .get(&"user_id".to_string().into())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Missing user_id in ForgeServices credential")
                    })?;
                let user_id = UserId::from_string(user_id_str.as_str())?;

                Ok((token.clone(), user_id))
            }
            _ => anyhow::bail!("ForgeServices credential must be an API key"),
        }
    }

    /// Finds a workspace by path from remote server, checking for exact match
    /// first, then ancestor workspaces.
    ///
    /// Business logic:
    /// 1. First tries to find an exact match for the given path
    /// 2. If not found, searches for ancestor workspaces
    /// 3. Returns the closest ancestor (longest matching path prefix)
    ///
    /// # Errors
    /// Returns an error if the path cannot be canonicalized or if there's a
    /// server error. Returns Ok(None) if no workspace is found.
    async fn find_workspace_by_path(
        &self,
        path: PathBuf,
        token: &forge_domain::ApiKey,
    ) -> Result<Option<forge_domain::WorkspaceInfo>>
    where
        F: WorkspaceIndexRepository,
    {
        let canonical_path = path
            .canonicalize()
            .with_context(|| format!("Failed to resolve path: {}", path.display()))?;

        // Get all workspaces from remote server
        let workspaces = self.infra.list_workspaces(token).await?;

        let canonical_str = canonical_path.to_string_lossy();

        // Business logic: choose which workspace to use
        // 1. First check for exact match
        if let Some(exact_match) = workspaces.iter().find(|w| w.working_dir == canonical_str) {
            return Ok(Some(exact_match.clone()));
        }

        // 2. Find closest ancestor (longest matching path prefix)
        let mut best_match: Option<(&forge_domain::WorkspaceInfo, usize)> = None;

        for workspace in &workspaces {
            let workspace_path = PathBuf::from(&workspace.working_dir);
            if canonical_path.starts_with(&workspace_path) {
                let path_len = workspace.working_dir.len();
                if best_match.is_none_or(|(_, len)| path_len > len) {
                    best_match = Some((workspace, path_len));
                }
            }
        }

        Ok(best_match.map(|(w, _)| w.clone()))
    }
    /// Runs `git ls-files` in `dir_path` and returns the tracked files as
    /// `WalkedFile` entries.
    ///
    /// # Errors
    ///
    /// Returns an error when the command fails to execute or exits with a
    /// non-zero status code (e.g. when the directory is not a git repository).
    async fn git_ls_files(&self, dir_path: &Path) -> anyhow::Result<Vec<WalkedFile>>
    where
        F: CommandInfra,
    {
        let output = self
            .infra
            .execute_command(
                "git ls-files".to_string(),
                dir_path.to_path_buf(),
                true,
                None,
            )
            .await?;

        if output.exit_code != Some(0) {
            let err = anyhow::anyhow!(output.stderr);
            return Err(match output.exit_code {
                Some(code) => err.context(format!("'git ls-files' exited with code {}", code)),
                None => err,
            });
        }

        let files = output
            .stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let path = line.trim().to_string();
                let file_name = std::path::Path::new(&path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string());
                WalkedFile { path, file_name, size: 0 }
            })
            .collect();

        Ok(files)
    }

    /// Walks a directory using the file-system walker and returns all
    /// non-directory entries.
    async fn walk_directory(
        &self,
        dir_path: &Path,
        workspace_id: &WorkspaceId,
    ) -> anyhow::Result<Vec<WalkedFile>>
    where
        F: WalkerInfra,
    {
        let walker_config = Walker::unlimited()
            .cwd(dir_path.to_path_buf())
            .skip_binary(true);
        match self
            .infra
            .walk(walker_config)
            .await
            .context("Failed to walk directory")
        {
            Ok(files) => {
                let files: Vec<_> = files.into_iter().filter(|f| !f.is_dir()).collect();
                info!(workspace_id = %workspace_id, file_count = files.len(), "Discovered files via walker fallback");
                Ok(files)
            }
            Err(err) => {
                warn!(workspace_id = %workspace_id, error = ?err, "Failed to get files via walker fallback");
                Err(err)
            }
        }
    }

    /// Discovers workspace files and filters them by allowed source extensions.
    async fn discover_sync_file_paths(
        &self,
        dir_path: &Path,
        workspace_id: &WorkspaceId,
    ) -> anyhow::Result<Vec<PathBuf>>
    where
        F: CommandInfra + WalkerInfra,
    {
        info!(workspace_id = %workspace_id, "Discovering files for sync via git ls-files");
        // `git ls-files` can succeed yet return an empty list (e.g. a freshly
        // initialized repo with no commits, or a directory outside the working
        // tree). Treat that the same as a failure and fall back to the walker so
        // we still discover files on disk.
        let walked_files: Vec<WalkedFile> = match self.git_ls_files(dir_path).await {
            Ok(walked_files) if !walked_files.is_empty() => {
                info!(workspace_id = %workspace_id, file_count = walked_files.len(), "Discovered files via git ls-files");
                walked_files
            }
            Ok(_) => {
                warn!(workspace_id = %workspace_id, "git ls-files returned no files, falling back to walker");
                self.walk_directory(dir_path, workspace_id).await?
            }
            Err(err) => {
                warn!(workspace_id = %workspace_id, error = ?err, "Failed to get files via git ls-files, falling back to walker");
                self.walk_directory(dir_path, workspace_id).await?
            }
        };

        let filtered_files: Vec<_> = walked_files
            .into_iter()
            .filter(|walked| {
                let file_path = dir_path.join(&walked.path);
                has_allowed_extension(&file_path)
            })
            .collect();

        info!(
            workspace_id = %workspace_id,
            filtered_count = filtered_files.len(),
            "Files after extension filtering"
        );

        if filtered_files.is_empty() {
            return Err(ServiceError::NoSourceFilesFound.into());
        }

        Ok(filtered_files
            .iter()
            .map(|walked| dir_path.join(&walked.path))
            .collect())
    }

    /// Only includes files with allowed extensions.
    fn read_files(
        &self,
        batch_size: usize,
        dir_path: &Path,
        workspace_id: &WorkspaceId,
    ) -> impl Stream<Item = Result<FileNode>> + Send
    where
        F: FileReaderInfra + EnvironmentInfra + CommandInfra + WalkerInfra,
    {
        let dir_path = dir_path.to_path_buf();
        let infra = self.infra.clone();
        let service = self.clone();
        let workspace_id = workspace_id.clone();

        async_stream::stream! {
            let file_paths: Vec<PathBuf> = match service.discover_sync_file_paths(&dir_path, &workspace_id).await {
                Ok(file_paths) => file_paths,
                Err(err) => {
                    yield Err(err);
                    return;
                }
            };

            // Use read_batch_utf8 with streaming for better memory efficiency with large
            // file sets
            let stream = infra.read_batch_utf8(batch_size, file_paths);
            futures::pin_mut!(stream);

            while let Some((absolute_path, result)) = stream.next().await {
                match result {
                    Ok(content) => {
                        let hash = compute_hash(&content);
                        let absolute_path_str = absolute_path.to_string_lossy().to_string();
                        yield Ok(FileNode { file_path: absolute_path_str, content, hash });
                    }
                    Err(e) => {
                        warn!(path = %absolute_path.display(), error = ?e, "Skipping unreadable file during sync");
                        yield Err(FileReadError { path: absolute_path, source: e }.into());
                    }
                }
            }
        }
    }

    async fn _init_workspace(&self, path: PathBuf) -> Result<(bool, WorkspaceId)> {
        let (token, _user_id) = self.get_workspace_credentials().await?;
        let path = path
            .canonicalize()
            .with_context(|| format!("Failed to resolve path: {}", path.display()))?;

        // Find workspace by exact match or ancestor from remote server
        let workspace = self.find_workspace_by_path(path.clone(), &token).await?;

        let (workspace_id, workspace_path, is_new_workspace) = match workspace {
            Some(workspace_info) => {
                // Found existing workspace - reuse it
                (workspace_info.workspace_id, path.clone(), false)
            }
            None => {
                // No workspace found - create new
                (WorkspaceId::generate(), path.clone(), true)
            }
        };

        let workspace_id = if is_new_workspace {
            // Create workspace on server
            self.infra
                .create_workspace(&workspace_path, &token)
                .await
                .context("Failed to create workspace on server")?
        } else {
            workspace_id
        };

        Ok((is_new_workspace, workspace_id))
    }
}

#[async_trait]
impl<
    F: ProviderRepository
        + WorkspaceIndexRepository
        + FileReaderInfra
        + EnvironmentInfra
        + CommandInfra
        + WalkerInfra
        + 'static,
> WorkspaceService for ForgeWorkspaceService<F>
{
    async fn sync_workspace(
        &self,
        path: PathBuf,
        batch_size: usize,
    ) -> Result<MpscStream<Result<SyncProgress>>> {
        let service = Clone::clone(self);

        let stream = MpscStream::spawn(move |tx| async move {
            // Create emit closure that captures the sender
            let emit = |progress: SyncProgress| {
                let tx = tx.clone();
                async move {
                    let _ = tx.send(Ok(progress)).await;
                }
            };

            // Run the sync and emit progress events
            let result = service.sync_codebase_internal(path, batch_size, emit).await;

            // If there was an error, send it through the channel
            if let Err(e) = result {
                let _ = tx.send(Err(e)).await;
            }
        });

        Ok(stream)
    }

    /// Performs semantic code search on a workspace.
    async fn query_workspace(
        &self,
        path: PathBuf,
        params: forge_domain::SearchParams<'_>,
    ) -> Result<Vec<forge_domain::Node>> {
        let (token, user_id) = self.get_workspace_credentials().await?;

        let workspace = self
            .find_workspace_by_path(path, &token)
            .await?
            .ok_or(forge_domain::Error::WorkspaceNotFound)?;

        let search_query =
            forge_domain::CodeBase::new(user_id, workspace.workspace_id.clone(), params);

        let results = self
            .infra
            .search(&search_query, &token)
            .await
            .context("Failed to search")?;

        Ok(results)
    }

    /// Lists all workspaces.
    async fn list_workspaces(&self) -> Result<Vec<forge_domain::WorkspaceInfo>> {
        let (token, _) = self.get_workspace_credentials().await?;

        self.infra
            .as_ref()
            .list_workspaces(&token)
            .await
            .context("Failed to list workspaces")
    }

    /// Retrieves workspace information for a specific path.
    async fn get_workspace_info(&self, path: PathBuf) -> Result<Option<forge_domain::WorkspaceInfo>>
    where
        F: WorkspaceIndexRepository + ProviderRepository,
    {
        let (token, _user_id) = self.get_workspace_credentials().await?;
        let workspace = self.find_workspace_by_path(path, &token).await?;

        Ok(workspace)
    }

    /// Deletes a workspace from the server.
    async fn delete_workspace(&self, workspace_id: &forge_domain::WorkspaceId) -> Result<()> {
        let (token, _) = self.get_workspace_credentials().await?;

        self.infra
            .as_ref()
            .delete_workspace(workspace_id, &token)
            .await
            .context("Failed to delete workspace from server")?;

        Ok(())
    }

    /// Deletes multiple workspaces in parallel from both the server and local
    /// database.
    async fn delete_workspaces(&self, workspace_ids: &[forge_domain::WorkspaceId]) -> Result<()> {
        // Delete all workspaces in parallel by calling delete_workspace for each
        let delete_tasks: Vec<_> = workspace_ids
            .iter()
            .map(|workspace_id| self.delete_workspace(workspace_id))
            .collect();

        let results = join_all(delete_tasks).await;

        // Collect all errors
        let errors: Vec<_> = results.into_iter().filter_map(|r| r.err()).collect();

        if !errors.is_empty() {
            return Err(anyhow::anyhow!(
                "Failed to delete {} workspace(s): [{}]",
                errors.len(),
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        Ok(())
    }

    async fn is_indexed(&self, path: &std::path::Path) -> Result<bool> {
        let (token, _user_id) = self.get_workspace_credentials().await?;
        match self
            .find_workspace_by_path(path.to_path_buf(), &token)
            .await
        {
            Ok(workspace) => Ok(workspace.is_some()),
            Err(_) => Ok(false), // Path doesn't exist or other error, so it can't be indexed
        }
    }

    async fn get_workspace_status(&self, path: PathBuf) -> Result<Vec<forge_domain::FileStatus>> {
        let (token, user_id) = self.get_workspace_credentials().await?;

        let workspace = self
            .find_workspace_by_path(path, &token)
            .await?
            .context("Workspace not indexed. Please run `workspace sync` first.")?;

        // Reuse the canonical path already stored in the workspace (resolved during
        // sync), avoiding a redundant canonicalize() IO call.
        let canonical_path = PathBuf::from(&workspace.working_dir);

        let batch_size = self.infra.get_environment().max_file_read_batch_size;
        let results: Vec<Result<FileNode>> = self
            .read_files(batch_size, &canonical_path, &workspace.workspace_id)
            .collect()
            .await;

        let mut failed_statuses: Vec<forge_domain::FileStatus> = results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .filter_map(|e| e.downcast_ref::<FileReadError>())
            .map(|e| {
                forge_domain::FileStatus::new(
                    e.path.to_string_lossy().into_owned(),
                    forge_domain::SyncStatus::Failed,
                )
            })
            .collect();

        let local_files: Vec<FileNode> = results.into_iter().flatten().collect();

        let remote_files = self
            .fetch_remote_hashes(&user_id, &workspace.workspace_id, &token)
            .await?;

        let plan = WorkspaceStatus::new(canonical_path, remote_files);
        let local_file_hashes: Vec<forge_domain::FileHash> =
            local_files.into_iter().map(Into::into).collect();
        let mut statuses = plan.file_statuses(local_file_hashes);
        statuses.append(&mut failed_statuses);
        Ok(statuses)
    }

    async fn is_authenticated(&self) -> Result<bool> {
        Ok(self
            .infra
            .get_credential(&ProviderId::FORGE_SERVICES)
            .await?
            .is_some())
    }

    async fn init_auth_credentials(&self) -> Result<forge_domain::WorkspaceAuth> {
        // Authenticate with the indexing service
        let auth = self
            .infra
            .authenticate()
            .await
            .context("Failed to authenticate with indexing service")?;

        // Convert to AuthCredential and store
        let mut url_params = HashMap::new();
        url_params.insert(
            "user_id".to_string().into(),
            auth.user_id.to_string().into(),
        );

        let credential = AuthCredential {
            id: ProviderId::FORGE_SERVICES,
            auth_details: auth.clone().into(),
            url_params,
        };

        self.infra
            .upsert_credential(credential)
            .await
            .context("Failed to store authentication credentials")?;

        Ok(auth)
    }

    async fn init_workspace(&self, path: PathBuf) -> Result<WorkspaceId> {
        let (is_new, workspace_id) = self._init_workspace(path).await?;

        if is_new {
            Ok(workspace_id)
        } else {
            Err(forge_domain::Error::WorkspaceAlreadyInitialized(workspace_id).into())
        }
    }
}
