use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::sync::Arc;

use bytes::Bytes;
use forge_app::{
    CommandInfra, DirectoryReaderInfra, EnvironmentInfra, FileDirectoryInfra, FileInfoInfra,
    FileReaderInfra, FileRemoverInfra, FileWriterInfra, GrpcInfra, HttpInfra, McpServerInfra,
    StrategyFactory, UserInfra, WalkerInfra,
};
use forge_domain::{
    AuthMethod, CommandOutput, Environment, FileInfo as FileInfoData, McpServerConfig, ProviderId,
    URLParam,
};
use reqwest::header::HeaderMap;
use reqwest::{Response, Url};
use reqwest_eventsource::EventSource;

use crate::auth::{AnyAuthStrategy, ForgeAuthStrategyFactory};
use crate::console::StdConsoleWriter;
use crate::env::ForgeEnvironmentInfra;
use crate::executor::ForgeCommandExecutorService;
use crate::fs_create_dirs::ForgeCreateDirsService;
use crate::fs_meta::ForgeFileMetaService;
use crate::fs_read::ForgeFileReadService;
use crate::fs_read_dir::ForgeDirectoryReaderService;
use crate::fs_remove::ForgeFileRemoveService;
use crate::fs_write::ForgeFileWriteService;
use crate::grpc::ForgeGrpcClient;
use crate::http::ForgeHttpInfra;
use crate::inquire::ForgeInquire;
use crate::mcp_client::ForgeMcpClient;
use crate::mcp_server::ForgeMcpServer;
use crate::walker::ForgeWalkerService;

#[derive(Clone)]
pub struct ForgeInfra {
    // TODO: Drop the "Service" suffix. Use names like ForgeFileReader, ForgeFileWriter,
    // ForgeHttpClient etc.
    file_read_service: Arc<ForgeFileReadService>,
    file_write_service: Arc<ForgeFileWriteService>,
    file_remove_service: Arc<ForgeFileRemoveService>,
    environment_service: Arc<ForgeEnvironmentInfra>,
    file_meta_service: Arc<ForgeFileMetaService>,
    create_dirs_service: Arc<ForgeCreateDirsService>,
    directory_reader_service: Arc<ForgeDirectoryReaderService>,
    command_executor_service: Arc<ForgeCommandExecutorService>,
    inquire_service: Arc<ForgeInquire>,
    mcp_server: ForgeMcpServer,
    walker_service: Arc<ForgeWalkerService>,
    http_service: Arc<ForgeHttpInfra<ForgeFileWriteService>>,
    strategy_factory: Arc<ForgeAuthStrategyFactory>,
    grpc_client: Arc<ForgeGrpcClient>,
    output_printer: Arc<StdConsoleWriter>,
}

impl ForgeInfra {
    pub fn new(restricted: bool, cwd: PathBuf) -> Self {
        let environment_service = Arc::new(ForgeEnvironmentInfra::new(restricted, cwd));
        let env = environment_service.get_environment();

        let file_write_service = Arc::new(ForgeFileWriteService::new());
        let http_service = Arc::new(ForgeHttpInfra::new(env.clone(), file_write_service.clone()));
        let file_read_service = Arc::new(ForgeFileReadService::new());
        let file_meta_service = Arc::new(ForgeFileMetaService);
        let directory_reader_service =
            Arc::new(ForgeDirectoryReaderService::new(env.parallel_file_reads));
        let grpc_client = Arc::new(ForgeGrpcClient::new(env.workspace_server_url.clone()));
        let output_printer = Arc::new(StdConsoleWriter::default());

        Self {
            file_read_service,
            file_write_service,
            file_remove_service: Arc::new(ForgeFileRemoveService::new()),
            environment_service,
            file_meta_service,
            create_dirs_service: Arc::new(ForgeCreateDirsService),
            directory_reader_service,
            command_executor_service: Arc::new(ForgeCommandExecutorService::new(
                restricted,
                env.clone(),
                output_printer.clone(),
            )),
            inquire_service: Arc::new(ForgeInquire::new()),
            mcp_server: ForgeMcpServer,
            walker_service: Arc::new(ForgeWalkerService::new()),
            strategy_factory: Arc::new(ForgeAuthStrategyFactory::new()),
            http_service,
            grpc_client,
            output_printer,
        }
    }
}

impl EnvironmentInfra for ForgeInfra {
    fn get_environment(&self) -> Environment {
        self.environment_service.get_environment()
    }

    fn get_env_var(&self, key: &str) -> Option<String> {
        self.environment_service.get_env_var(key)
    }

    fn get_env_vars(&self) -> BTreeMap<String, String> {
        self.environment_service.get_env_vars()
    }

    fn is_restricted(&self) -> bool {
        self.environment_service.is_restricted()
    }
}

#[async_trait::async_trait]
impl FileReaderInfra for ForgeInfra {
    async fn read_utf8(&self, path: &Path) -> anyhow::Result<String> {
        self.file_read_service.read_utf8(path).await
    }

    fn read_batch_utf8(
        &self,
        batch_size: usize,
        paths: Vec<PathBuf>,
    ) -> impl futures::Stream<Item = (PathBuf, anyhow::Result<String>)> + Send {
        self.file_read_service.read_batch_utf8(batch_size, paths)
    }

    async fn read(&self, path: &Path) -> anyhow::Result<Vec<u8>> {
        self.file_read_service.read(path).await
    }

    async fn range_read_utf8(
        &self,
        path: &Path,
        start_line: u64,
        end_line: u64,
    ) -> anyhow::Result<(String, FileInfoData)> {
        self.file_read_service
            .range_read_utf8(path, start_line, end_line)
            .await
    }
}

#[async_trait::async_trait]
impl FileWriterInfra for ForgeInfra {
    async fn write(&self, path: &Path, contents: Bytes) -> anyhow::Result<()> {
        self.file_write_service.write(path, contents).await
    }

    async fn write_temp(&self, prefix: &str, ext: &str, content: &str) -> anyhow::Result<PathBuf> {
        self.file_write_service
            .write_temp(prefix, ext, content)
            .await
    }
}

#[async_trait::async_trait]
impl FileInfoInfra for ForgeInfra {
    async fn is_binary(&self, path: &Path) -> anyhow::Result<bool> {
        self.file_meta_service.is_binary(path).await
    }

    async fn is_file(&self, path: &Path) -> anyhow::Result<bool> {
        self.file_meta_service.is_file(path).await
    }

    async fn exists(&self, path: &Path) -> anyhow::Result<bool> {
        self.file_meta_service.exists(path).await
    }

    async fn file_size(&self, path: &Path) -> anyhow::Result<u64> {
        self.file_meta_service.file_size(path).await
    }
}
#[async_trait::async_trait]
impl FileRemoverInfra for ForgeInfra {
    async fn remove(&self, path: &Path) -> anyhow::Result<()> {
        self.file_remove_service.remove(path).await
    }
}

#[async_trait::async_trait]
impl FileDirectoryInfra for ForgeInfra {
    async fn create_dirs(&self, path: &Path) -> anyhow::Result<()> {
        self.create_dirs_service.create_dirs(path).await
    }
}

#[async_trait::async_trait]
impl CommandInfra for ForgeInfra {
    async fn execute_command(
        &self,
        command: String,
        working_dir: PathBuf,
        silent: bool,
        env_vars: Option<Vec<String>>,
    ) -> anyhow::Result<CommandOutput> {
        self.command_executor_service
            .execute_command(command, working_dir, silent, env_vars)
            .await
    }

    async fn execute_command_raw(
        &self,
        command: &str,
        working_dir: PathBuf,
        env_vars: Option<Vec<String>>,
    ) -> anyhow::Result<ExitStatus> {
        self.command_executor_service
            .execute_command_raw(command, working_dir, env_vars)
            .await
    }
}

#[async_trait::async_trait]
impl UserInfra for ForgeInfra {
    async fn prompt_question(&self, question: &str) -> anyhow::Result<Option<String>> {
        self.inquire_service.prompt_question(question).await
    }

    async fn select_one<T: Clone + std::fmt::Display + Send + 'static>(
        &self,
        message: &str,
        options: Vec<T>,
    ) -> anyhow::Result<Option<T>> {
        self.inquire_service.select_one(message, options).await
    }

    async fn select_many<T: std::fmt::Display + Clone + Send + 'static>(
        &self,
        message: &str,
        options: Vec<T>,
    ) -> anyhow::Result<Option<Vec<T>>> {
        self.inquire_service.select_many(message, options).await
    }
}

#[async_trait::async_trait]
impl McpServerInfra for ForgeInfra {
    type Client = ForgeMcpClient;

    async fn connect(
        &self,
        config: McpServerConfig,
        env_vars: &BTreeMap<String, String>,
    ) -> anyhow::Result<Self::Client> {
        self.mcp_server.connect(config, env_vars).await
    }
}

#[async_trait::async_trait]
impl WalkerInfra for ForgeInfra {
    async fn walk(&self, config: forge_app::Walker) -> anyhow::Result<Vec<forge_app::WalkedFile>> {
        self.walker_service.walk(config).await
    }
}

#[async_trait::async_trait]
impl HttpInfra for ForgeInfra {
    async fn http_get(&self, url: &Url, headers: Option<HeaderMap>) -> anyhow::Result<Response> {
        self.http_service.http_get(url, headers).await
    }

    async fn http_post(
        &self,
        url: &Url,
        headers: Option<HeaderMap>,
        body: Bytes,
    ) -> anyhow::Result<Response> {
        self.http_service.http_post(url, headers, body).await
    }

    async fn http_delete(&self, url: &Url) -> anyhow::Result<Response> {
        self.http_service.http_delete(url).await
    }

    async fn http_eventsource(
        &self,
        url: &Url,
        headers: Option<HeaderMap>,
        body: Bytes,
    ) -> anyhow::Result<EventSource> {
        self.http_service.http_eventsource(url, headers, body).await
    }
}
#[async_trait::async_trait]
impl DirectoryReaderInfra for ForgeInfra {
    async fn list_directory_entries(
        &self,
        directory: &Path,
    ) -> anyhow::Result<Vec<(PathBuf, bool)>> {
        self.directory_reader_service
            .list_directory_entries(directory)
            .await
    }

    async fn read_directory_files(
        &self,
        directory: &Path,
        pattern: Option<&str>,
    ) -> anyhow::Result<Vec<(PathBuf, String)>> {
        self.directory_reader_service
            .read_directory_files(directory, pattern)
            .await
    }
}

impl StrategyFactory for ForgeInfra {
    type Strategy = AnyAuthStrategy;
    fn create_auth_strategy(
        &self,
        provider_id: ProviderId,
        method: AuthMethod,
        required_params: Vec<URLParam>,
    ) -> anyhow::Result<Self::Strategy> {
        self.strategy_factory
            .create_auth_strategy(provider_id, method, required_params)
    }
}

impl GrpcInfra for ForgeInfra {
    fn channel(&self) -> tonic::transport::Channel {
        self.grpc_client.channel()
    }

    fn hydrate(&self) {
        self.grpc_client.hydrate();
    }
}

#[async_trait::async_trait]
impl forge_domain::ConsoleWriter for ForgeInfra {
    type Guard = <crate::console::StdConsoleWriter as forge_domain::ConsoleWriter>::Guard;

    async fn acquire(&self) -> Self::Guard {
        self.output_printer.acquire().await
    }
}
