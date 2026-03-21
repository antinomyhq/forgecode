use std::sync::Arc;

use anyhow::Context;
use bytes::Bytes;
use forge_app::domain::{ProviderId, ProviderResponse};
use forge_app::{EnvironmentInfra, FileReaderInfra, FileWriterInfra, GrpcInfra, HttpInfra};
use forge_domain::{
    AnyProvider, ApiKey, AuthCredential, AuthDetails, Error, MigrationResult, Provider,
    ProviderRepository, ProviderType, URLParam, URLParamValue,
};
use merge::Merge;
use serde::Deserialize;

use crate::proto_generated::ListProvidersRequest;
use crate::proto_generated::forge_service_client::ForgeServiceClient;

/// Represents the source of models for a provider
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Models {
    /// Models are fetched from a URL
    Url(String),
    /// Models are hardcoded in the configuration
    Hardcoded(Vec<forge_app::domain::Model>),
}

#[derive(Debug, Clone, Deserialize, Merge)]
struct ProviderConfig {
    #[merge(strategy = overwrite)]
    id: ProviderId,
    #[serde(default)]
    #[merge(strategy = overwrite)]
    provider_type: ProviderType,
    #[serde(default)]
    #[merge(strategy = overwrite)]
    api_key_vars: Option<String>,
    #[serde(default)]
    #[merge(strategy = merge::vec::append)]
    url_param_vars: Vec<String>,
    #[serde(default)]
    #[merge(strategy = overwrite)]
    response_type: Option<ProviderResponse>,
    #[merge(strategy = overwrite)]
    url: String,
    #[serde(default)]
    #[merge(strategy = overwrite)]
    models: Option<Models>,
    #[merge(strategy = merge::vec::append)]
    auth_methods: Vec<forge_domain::AuthMethod>,
    #[serde(default)]
    #[merge(strategy = overwrite)]
    custom_headers: Option<std::collections::HashMap<String, String>>,
}

fn overwrite<T>(base: &mut T, other: T) {
    *base = other;
}

/// Transparent wrapper for Vec<ProviderConfig> that implements custom merge
/// logic
#[derive(Debug, Clone, Deserialize, Merge)]
#[serde(transparent)]
struct ProviderConfigs(#[merge(strategy = merge_configs)] Vec<ProviderConfig>);

fn merge_configs(base: &mut Vec<ProviderConfig>, other: Vec<ProviderConfig>) {
    let mut map: std::collections::HashMap<_, _> =
        base.drain(..).map(|c| (c.id.clone(), c)).collect();

    for other_config in other {
        let id = other_config.id.clone();
        map.entry(id)
            .and_modify(|base_config| base_config.merge(other_config.clone()))
            .or_insert(other_config);
    }

    base.extend(map.into_values());
}

impl From<&ProviderConfig> for forge_domain::ProviderTemplate {
    fn from(config: &ProviderConfig) -> Self {
        let models = config.models.as_ref().map(|m| match m {
            Models::Url(model_url_template) => forge_domain::ModelSource::Url(
                forge_domain::Template::<forge_domain::URLParameters>::new(model_url_template),
            ),
            Models::Hardcoded(model_list) => {
                forge_domain::ModelSource::Hardcoded(model_list.clone())
            }
        });

        Provider {
            id: config.id.clone(),
            provider_type: config.provider_type,
            response: config.response_type.clone(),
            url: forge_domain::Template::<forge_domain::URLParameters>::new(&config.url),
            auth_methods: config.auth_methods.clone(),
            url_params: config
                .url_param_vars
                .iter()
                .map(|v| URLParam::from(v.clone()))
                .collect(),
            credential: None,
            custom_headers: config.custom_headers.clone(),
            models,
        }
    }
}

impl TryFrom<crate::proto_generated::OAuthConfig> for forge_domain::OAuthConfig {
    type Error = anyhow::Error;

    fn try_from(proto: crate::proto_generated::OAuthConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            auth_url: proto.auth_url.parse().context("Invalid auth_url")?,
            token_url: proto.token_url.parse().context("Invalid token_url")?,
            client_id: forge_domain::ClientId::from(proto.client_id),
            scopes: proto.scopes,
            redirect_uri: proto.redirect_uri,
            use_pkce: proto.use_pkce,
            token_refresh_url: proto
                .token_refresh_url
                .map(|u| u.parse())
                .transpose()
                .context("Invalid token_refresh_url")?,
            custom_headers: if proto.custom_headers.is_empty() {
                None
            } else {
                Some(proto.custom_headers)
            },
            extra_auth_params: if proto.extra_auth_params.is_empty() {
                None
            } else {
                Some(proto.extra_auth_params)
            },
        })
    }
}

impl TryFrom<crate::proto_generated::AuthMethod> for forge_domain::AuthMethod {
    type Error = anyhow::Error;

    fn try_from(proto: crate::proto_generated::AuthMethod) -> Result<Self, Self::Error> {
        use crate::proto_generated::auth_method::Method;
        let method = proto.method.context("AuthMethod has no method set")?;
        match method {
            Method::ApiKey(_) => Ok(Self::ApiKey),
            Method::OauthDevice(cfg) => Ok(Self::OAuthDevice(cfg.try_into()?)),
            Method::OauthCode(cfg) => Ok(Self::OAuthCode(cfg.try_into()?)),
            Method::GoogleAdc(_) => Ok(Self::GoogleAdc),
            Method::CodexDevice(cfg) => Ok(Self::CodexDevice(cfg.try_into()?)),
        }
    }
}

impl TryFrom<crate::proto_generated::Provider> for ProviderConfig {
    type Error = anyhow::Error;

    fn try_from(proto: crate::proto_generated::Provider) -> Result<Self, Self::Error> {
        use crate::proto_generated::provider_models;

        let models = proto.models.and_then(|m| {
            m.kind.map(|kind| match kind {
                provider_models::Kind::Url(url) => Models::Url(url),
                provider_models::Kind::ModelList(list) => {
                    let domain_models = list
                        .models
                        .into_iter()
                        .map(|m| forge_app::domain::Model {
                            id: forge_app::domain::ModelId::new(m.id),
                            name: m.name,
                            description: m.description,
                            context_length: m.context_length,
                            tools_supported: m.tools_supported,
                            supports_parallel_tool_calls: m.supports_parallel_tool_calls,
                            supports_reasoning: m.supports_reasoning,
                            input_modalities: m
                                .input_modalities
                                .into_iter()
                                .filter_map(|s| s.parse::<forge_domain::InputModality>().ok())
                                .collect(),
                        })
                        .collect();
                    Models::Hardcoded(domain_models)
                }
            })
        });

        let response_type = proto
            .response_type
            .map(|s| serde_json::from_value(serde_json::Value::String(s)))
            .transpose()
            .context("Invalid response_type")?;

        let provider_type = proto
            .provider_type
            .map(|s| s.parse::<ProviderType>())
            .transpose()
            .map_err(|e| anyhow::anyhow!("Invalid provider_type: {e}"))?
            .unwrap_or_default();

        let auth_methods = proto
            .auth_methods
            .into_iter()
            .map(TryFrom::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ProviderConfig {
            id: ProviderId::from(proto.id),
            provider_type,
            api_key_vars: proto.api_key_vars,
            url_param_vars: proto.url_param_vars,
            response_type,
            url: proto.url,
            models,
            auth_methods,
            custom_headers: if proto.custom_headers.is_empty() {
                None
            } else {
                Some(proto.custom_headers)
            },
        })
    }
}

pub struct ForgeProviderRepository<F> {
    infra: Arc<F>,
}

impl<F: EnvironmentInfra + HttpInfra> ForgeProviderRepository<F> {
    pub fn new(infra: Arc<F>) -> Self {
        Self { infra }
    }
}

impl<F: EnvironmentInfra + FileReaderInfra + FileWriterInfra + HttpInfra + GrpcInfra>
    ForgeProviderRepository<F>
{
    /// Fetches provider configurations from the remote gRPC server.
    async fn get_provider_configs(&self) -> anyhow::Result<Vec<ProviderConfig>> {
        let channel = self.infra.channel();
        let mut client = ForgeServiceClient::new(channel);
        let response: crate::proto_generated::ListProvidersResponse = client
            .list_providers(tonic::Request::new(ListProvidersRequest {}))
            .await
            .context("Failed to call list_providers gRPC")?
            .into_inner();
        response
            .providers
            .into_iter()
            .map(TryFrom::try_from)
            .collect()
    }

    async fn get_custom_provider_configs(&self) -> anyhow::Result<Vec<ProviderConfig>> {
        let environment = self.infra.get_environment();
        let provider_json_path = environment.base_path.join("provider.json");

        let json_str = self.infra.read_utf8(&provider_json_path).await?;
        let configs = serde_json::from_str(&json_str)?;
        Ok(configs)
    }

    async fn get_providers(&self) -> anyhow::Result<Vec<AnyProvider>> {
        let configs = self.get_merged_configs().await?;

        let mut providers: Vec<AnyProvider> = Vec::new();
        for config in configs {
            // Skip Forge provider as it's handled specially
            if config.id == ProviderId::FORGE {
                continue;
            }

            // Try to create configured template provider, fallback to unconfigured
            let provider_entry = if let Ok(provider) = self.create_provider(&config).await {
                Some(provider.into())
            } else if let Ok(provider) = self.create_unconfigured_provider(&config) {
                Some(provider.into())
            } else {
                None
            };

            if let Some(entry) = provider_entry {
                providers.push(entry);
            }
        }

        // Sort by ProviderId enum order to ensure deterministic, priority-based
        // ordering
        providers.sort_by_key(|a| a.id());

        Ok(providers)
    }

    /// Migrates environment variable-based credentials to file-based
    /// credentials. This is a one-time migration that runs only if the
    /// credentials file doesn't exist.
    pub async fn migrate_env_to_file(&self) -> anyhow::Result<Option<MigrationResult>> {
        let path = self.infra.get_environment().credentials_path();

        // Check if credentials file already exists
        if self.infra.read_utf8(&path).await.is_ok() {
            return Ok(None);
        }

        let mut credentials = Vec::new();
        let mut migrated_providers = Vec::new();
        let configs = self.get_merged_configs().await?;

        let has_openai_url = self.infra.get_env_var("OPENAI_URL").is_some();
        let has_anthropic_url = self.infra.get_env_var("ANTHROPIC_URL").is_some();

        for config in configs {
            // Skip Forge provider and ContextEngine providers - they're not configurable
            // via env like other providers
            if config.id == ProviderId::FORGE || config.provider_type == ProviderType::ContextEngine
            {
                continue;
            }

            if config.id == ProviderId::OPENAI && has_openai_url {
                continue;
            }
            if (config.id == ProviderId::OPENAI_COMPATIBLE
                || config.id == ProviderId::OPENAI_RESPONSES_COMPATIBLE)
                && !has_openai_url
            {
                continue;
            }
            if config.id == ProviderId::ANTHROPIC && has_anthropic_url {
                continue;
            }
            if config.id == ProviderId::ANTHROPIC_COMPATIBLE && !has_anthropic_url {
                continue;
            }

            // Try to create credential from environment variables
            if let Ok(credential) = self.create_credential_from_env(&config) {
                migrated_providers.push(config.id);
                credentials.push(credential);
            }
        }

        // Only write if we have credentials to migrate
        if !credentials.is_empty() {
            self.write_credentials(&credentials).await?;
            Ok(Some(MigrationResult::new(path, migrated_providers)))
        } else {
            Ok(None)
        }
    }

    /// Creates a credential from environment variables for a given config
    fn create_credential_from_env(
        &self,
        config: &ProviderConfig,
    ) -> anyhow::Result<AuthCredential> {
        // Check API key environment variable (if specified)
        let api_key = if let Some(api_key_var) = &config.api_key_vars {
            self.infra
                .get_env_var(api_key_var)
                .ok_or_else(|| Error::env_var_not_found(config.id.clone(), api_key_var))?
        } else {
            // For context engine, we don't use env vars for API key
            String::new()
        };

        // Check URL parameter environment variables
        let mut url_params = std::collections::HashMap::new();

        for env_var in &config.url_param_vars {
            if let Some(value) = self.infra.get_env_var(env_var) {
                url_params.insert(URLParam::from(env_var.clone()), URLParamValue::from(value));
            } else {
                return Err(Error::env_var_not_found(config.id.clone(), env_var).into());
            }
        }

        // Create AuthCredential
        Ok(AuthCredential {
            id: config.id.clone(),
            auth_details: AuthDetails::ApiKey(ApiKey::from(api_key)),
            url_params,
        })
    }

    /// Creates a provider with template URLs (not rendered).
    /// The service layer is responsible for rendering templates.
    async fn create_provider(
        &self,
        config: &ProviderConfig,
    ) -> anyhow::Result<forge_domain::ProviderTemplate> {
        // Get credential from file
        let mut credential = self
            .get_credential(&config.id)
            .await?
            .ok_or_else(|| Error::provider_not_available(config.id.clone()))?;

        // Check if this is a Google ADC credential and refresh it
        // Google ADC tokens expire quickly, so we refresh them on every load
        if (credential.id == forge_domain::ProviderId::VERTEX_AI
            || credential.id == forge_domain::ProviderId::VERTEX_AI_ANTHROPIC)
            && let forge_domain::AuthDetails::ApiKey(ref api_key) = credential.auth_details
            && api_key.as_ref() == "google_adc_marker"
        {
            // Refresh the Google ADC credential, preserving url_params
            match self.refresh_google_adc_credential(&credential).await {
                Ok(refreshed) => {
                    credential = refreshed;
                    tracing::info!("Successfully refreshed Google ADC token");
                }
                Err(e) => {
                    tracing::error!("Failed to refresh Google ADC token: {e}");
                    return Err(e.context("Failed to refresh Google ADC token. Please run 'gcloud auth application-default login' to set up credentials."));
                }
            }
        }

        // Handle models - keep as templates
        let models = config.models.as_ref().map(|m| match m {
            Models::Url(model_url_template) => forge_domain::ModelSource::Url(
                forge_domain::Template::<forge_domain::URLParameters>::new(model_url_template),
            ),
            Models::Hardcoded(model_list) => {
                forge_domain::ModelSource::Hardcoded(model_list.clone())
            }
        });

        Ok(Provider {
            id: config.id.clone(),
            provider_type: config.provider_type,
            response: config.response_type.clone(),
            url: forge_domain::Template::<forge_domain::URLParameters>::new(&config.url),
            auth_methods: config.auth_methods.clone(),
            url_params: config
                .url_param_vars
                .iter()
                .map(|v| URLParam::from(v.clone()))
                .collect(),
            credential: Some(credential),
            custom_headers: config.custom_headers.clone(),
            models,
        })
    }

    /// Creates an unconfigured provider when environment variables are missing.
    fn create_unconfigured_provider(
        &self,
        config: &ProviderConfig,
    ) -> anyhow::Result<forge_domain::ProviderTemplate> {
        Ok(config.into())
    }

    /// Refreshes a Google ADC credential by fetching a fresh token
    async fn refresh_google_adc_credential(
        &self,
        original_credential: &forge_domain::AuthCredential,
    ) -> anyhow::Result<forge_domain::AuthCredential> {
        use google_cloud_auth::credentials::Builder;

        // Vertex AI requires cloud-platform scope
        const VERTEX_AI_SCOPES: &[&str] = &["https://www.googleapis.com/auth/cloud-platform"];

        // Create credentials with proper scopes using the Builder API
        let credentials = Builder::default()
            .with_scopes(VERTEX_AI_SCOPES.iter().map(|s| s.to_string()))
            .build_access_token_credentials()
            .map_err(|e| anyhow::anyhow!("Failed to create Google credentials builder: {e}. Please run 'gcloud auth application-default login' to set up credentials."))?;

        // Get fresh token
        let access_token = credentials.access_token().await.map_err(|e| {
            anyhow::anyhow!("Failed to fetch Google access token: {e}. Please run 'gcloud auth application-default login' to set up credentials.")
        })?;

        tracing::debug!(
            "Fetched Google ADC token (length: {})",
            access_token.token.len()
        );
        tracing::debug!(
            "Token starts with: {}",
            &access_token.token[..access_token.token.len().min(20)]
        );

        // Create new credential with fresh token, preserving url_params and provider ID
        Ok(forge_domain::AuthCredential::new_api_key(
            original_credential.id.clone(),
            forge_domain::ApiKey::from(access_token.token),
        )
        .url_params(original_credential.url_params.clone()))
    }

    async fn provider_from_id(
        &self,
        id: ProviderId,
    ) -> anyhow::Result<forge_domain::ProviderTemplate> {
        // Handle special cases first
        if id == ProviderId::FORGE {
            // Forge provider isn't typically configured via env vars in the registry
            return Err(Error::provider_not_available(ProviderId::FORGE).into());
        }

        // Look up provider from cached providers - return configured template providers
        self.get_providers()
            .await?
            .iter()
            .find_map(|p| match p {
                AnyProvider::Template(tp) if tp.id == id && tp.credential.is_some() => {
                    Some(tp.clone())
                }
                _ => None,
            })
            .ok_or_else(|| Error::provider_not_available(id).into())
    }

    /// Returns merged provider configs (gRPC + custom)
    async fn get_merged_configs(&self) -> anyhow::Result<Vec<ProviderConfig>> {
        let mut configs = ProviderConfigs(self.get_provider_configs().await?);
        // Merge custom configs into base configs
        configs.merge(ProviderConfigs(
            self.get_custom_provider_configs().await.unwrap_or_default(),
        ));

        Ok(configs.0)
    }

    async fn read_credentials(&self) -> Vec<AuthCredential> {
        let path = self.infra.get_environment().credentials_path();

        match self.infra.read_utf8(&path).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    /// Writes credentials to the JSON file
    async fn write_credentials(&self, credentials: &Vec<AuthCredential>) -> anyhow::Result<()> {
        let path = self.infra.get_environment().credentials_path();

        let content = serde_json::to_string_pretty(credentials)?;
        self.infra.write(&path, Bytes::from(content)).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl<F: EnvironmentInfra + FileReaderInfra + FileWriterInfra + HttpInfra + GrpcInfra + Sync>
    ProviderRepository for ForgeProviderRepository<F>
{
    async fn get_all_providers(&self) -> anyhow::Result<Vec<AnyProvider>> {
        self.get_providers().await
    }

    async fn get_provider(&self, id: ProviderId) -> anyhow::Result<forge_domain::ProviderTemplate> {
        self.provider_from_id(id).await
    }

    async fn upsert_credential(&self, credential: AuthCredential) -> anyhow::Result<()> {
        let mut credentials = self.read_credentials().await;
        let id = credential.id.clone();

        // Update existing credential or add new one
        if let Some(existing) = credentials.iter_mut().find(|c| c.id == id) {
            *existing = credential;
        } else {
            credentials.push(credential);
        }
        self.write_credentials(&credentials).await?;

        Ok(())
    }

    async fn get_credential(&self, id: &ProviderId) -> anyhow::Result<Option<AuthCredential>> {
        let credentials = self.read_credentials().await;
        Ok(credentials.into_iter().find(|c| &c.id == id))
    }

    async fn remove_credential(&self, id: &ProviderId) -> anyhow::Result<()> {
        let mut credentials = self.read_credentials().await;
        credentials.retain(|c| &c.id != id);
        self.write_credentials(&credentials).await?;

        Ok(())
    }

    async fn migrate_env_credentials(&self) -> anyhow::Result<Option<MigrationResult>> {
        self.migrate_env_to_file().await
    }
}

#[cfg(test)]
mod env_tests {
    use std::collections::{BTreeMap, HashMap};
    use std::path::PathBuf;
    use std::sync::Arc;

    use forge_app::domain::{
        ChatCompletionMessage, Context, Environment, Model, ModelId, ResultStream,
    };
    use forge_domain::{AnyProvider, ChatRepository, ProviderTemplate};
    use pretty_assertions::assert_eq;
    use url::Url;

    use super::*;

    // Mock infrastructure that provides environment variables
    struct MockInfra {
        env_vars: HashMap<String, String>,
        base_path: PathBuf,
        credentials: tokio::sync::Mutex<Option<Vec<AuthCredential>>>,
    }

    impl MockInfra {
        fn new(env_vars: HashMap<String, String>) -> Self {
            use fake::{Fake, Faker};
            Self {
                env_vars,
                base_path: Faker.fake(),
                credentials: tokio::sync::Mutex::new(None),
            }
        }
    }

    impl EnvironmentInfra for MockInfra {
        fn get_environment(&self) -> Environment {
            use fake::{Fake, Faker};
            let mut env: Environment = Faker.fake();
            env.base_path = self.base_path.clone();
            env
        }

        fn get_env_var(&self, key: &str) -> Option<String> {
            self.env_vars.get(key).cloned()
        }

        fn get_env_vars(&self) -> BTreeMap<String, String> {
            self.env_vars
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        }

        fn is_restricted(&self) -> bool {
            false
        }
    }

    #[async_trait::async_trait]
    impl FileReaderInfra for MockInfra {
        async fn read_utf8(&self, path: &std::path::Path) -> anyhow::Result<String> {
            // Check if it's the credentials file
            if path == self.get_environment().credentials_path() {
                let guard = self.credentials.lock().await;
                if let Some(ref creds) = *guard {
                    return Ok(serde_json::to_string(creds)?);
                }
            }
            Err(anyhow::anyhow!("File not found"))
        }

        fn read_batch_utf8(
            &self,
            _batch_size: usize,
            _paths: Vec<PathBuf>,
        ) -> impl futures::Stream<Item = (PathBuf, anyhow::Result<String>)> + Send {
            futures::stream::empty()
        }

        async fn read(&self, _path: &std::path::Path) -> anyhow::Result<Vec<u8>> {
            Err(anyhow::anyhow!("File not found"))
        }

        async fn range_read_utf8(
            &self,
            _path: &std::path::Path,
            _start_line: u64,
            _end_line: u64,
        ) -> anyhow::Result<(String, forge_domain::FileInfo)> {
            Err(anyhow::anyhow!("File not found"))
        }
    }

    #[async_trait::async_trait]
    impl FileWriterInfra for MockInfra {
        async fn write(&self, path: &std::path::Path, content: Bytes) -> anyhow::Result<()> {
            // Capture writes to credentials file
            if path == self.get_environment().credentials_path() {
                let content_str = String::from_utf8(content.to_vec())?;
                let creds: Vec<AuthCredential> = serde_json::from_str(&content_str)?;
                let mut guard = self.credentials.lock().await;
                *guard = Some(creds);
            }
            Ok(())
        }

        async fn write_temp(
            &self,
            _prefix: &str,
            _ext: &str,
            _content: &str,
        ) -> anyhow::Result<PathBuf> {
            Ok(PathBuf::from("/tmp/test"))
        }
    }

    #[async_trait::async_trait]
    impl HttpInfra for MockInfra {
        async fn http_get(
            &self,
            _url: &reqwest::Url,
            _headers: Option<reqwest::header::HeaderMap>,
        ) -> anyhow::Result<reqwest::Response> {
            Err(anyhow::anyhow!("HTTP not implemented in mock"))
        }

        async fn http_post(
            &self,
            _url: &reqwest::Url,
            _headers: Option<reqwest::header::HeaderMap>,
            _body: bytes::Bytes,
        ) -> anyhow::Result<reqwest::Response> {
            Err(anyhow::anyhow!("HTTP not implemented in mock"))
        }

        async fn http_delete(&self, _url: &reqwest::Url) -> anyhow::Result<reqwest::Response> {
            Err(anyhow::anyhow!("HTTP not implemented in mock"))
        }

        async fn http_eventsource(
            &self,
            _url: &reqwest::Url,
            _headers: Option<reqwest::header::HeaderMap>,
            _body: bytes::Bytes,
        ) -> anyhow::Result<reqwest_eventsource::EventSource> {
            Err(anyhow::anyhow!("HTTP not implemented in mock"))
        }
    }

    impl GrpcInfra for MockInfra {
        fn channel(&self) -> tonic::transport::Channel {
            tonic::transport::Channel::from_static("http://[::1]:50051").connect_lazy()
        }

        fn hydrate(&self) {}
    }

    #[async_trait::async_trait]
    impl ChatRepository for MockInfra {
        async fn chat(
            &self,
            _model_id: &ModelId,
            _context: Context,
            _provider: Provider<Url>,
        ) -> ResultStream<ChatCompletionMessage, anyhow::Error> {
            Ok(Box::pin(tokio_stream::iter(vec![])))
        }

        async fn models(&self, _provider: Provider<Url>) -> anyhow::Result<Vec<Model>> {
            Ok(vec![])
        }
    }

    #[async_trait::async_trait]
    impl ProviderRepository for MockInfra {
        async fn get_all_providers(&self) -> anyhow::Result<Vec<AnyProvider>> {
            Ok(vec![])
        }

        async fn get_provider(&self, _id: ProviderId) -> anyhow::Result<ProviderTemplate> {
            Err(anyhow::anyhow!("Provider not found"))
        }

        async fn upsert_credential(
            &self,
            _credential: forge_domain::AuthCredential,
        ) -> anyhow::Result<()> {
            Ok(())
        }

        async fn get_credential(
            &self,
            _id: &ProviderId,
        ) -> anyhow::Result<Option<forge_domain::AuthCredential>> {
            Ok(None)
        }

        async fn remove_credential(&self, _id: &ProviderId) -> anyhow::Result<()> {
            Ok(())
        }

        async fn migrate_env_credentials(
            &self,
        ) -> anyhow::Result<Option<forge_domain::MigrationResult>> {
            Ok(None)
        }
    }

    #[tokio::test]
    async fn test_migration_from_env_to_file() {
        let mut env_vars = HashMap::new();
        env_vars.insert("OPENAI_API_KEY".to_string(), "test-openai-key".to_string());
        env_vars.insert(
            "ANTHROPIC_API_KEY".to_string(),
            "test-anthropic-key".to_string(),
        );
        env_vars.insert(
            "OPENAI_URL".to_string(),
            "https://custom.openai.com/v1".to_string(),
        );

        let infra = Arc::new(MockInfra::new(env_vars));
        let registry = ForgeProviderRepository::new(infra.clone());

        // Trigger migration
        registry.migrate_env_to_file().await.unwrap();

        // Verify credentials were written
        let credentials_guard = infra.credentials.lock().await;
        let credentials = credentials_guard.as_ref().unwrap();

        // Should have migrated OpenAICompatible (not OpenAI) and Anthropic (not
        // AnthropicCompatible)
        assert!(
            !credentials.iter().any(|c| c.id == ProviderId::OPENAI),
            "Should NOT create OpenAI credential when OPENAI_URL is set"
        );
        assert!(
            credentials
                .iter()
                .any(|c| c.id == ProviderId::OPENAI_COMPATIBLE),
            "Should create OpenAICompatible credential when OPENAI_URL is set"
        );
        assert!(
            credentials.iter().any(|c| c.id == ProviderId::ANTHROPIC),
            "Should create Anthropic credential when ANTHROPIC_URL is NOT set"
        );
        assert!(
            !credentials
                .iter()
                .any(|c| c.id == ProviderId::ANTHROPIC_COMPATIBLE),
            "Should NOT create AnthropicCompatible credential when ANTHROPIC_URL is NOT set"
        );

        // Verify OpenAICompatible credential
        let openai_compat_cred = credentials
            .iter()
            .find(|c| c.id == ProviderId::OPENAI_COMPATIBLE)
            .unwrap();
        match &openai_compat_cred.auth_details {
            AuthDetails::ApiKey(key) => assert_eq!(key.as_str(), "test-openai-key"),
            _ => panic!("Expected API key"),
        }

        // Verify OpenAICompatible has URL param
        assert!(!openai_compat_cred.url_params.is_empty());
        let url_params = &openai_compat_cred.url_params;
        assert_eq!(
            url_params
                .get(&URLParam::from("OPENAI_URL".to_string()))
                .map(|v| v.as_str()),
            Some("https://custom.openai.com/v1")
        );

        // Verify Anthropic credential
        let anthropic_cred = credentials
            .iter()
            .find(|c| c.id == ProviderId::ANTHROPIC)
            .unwrap();
        match &anthropic_cred.auth_details {
            AuthDetails::ApiKey(key) => assert_eq!(key.as_str(), "test-anthropic-key"),
            _ => panic!("Expected API key"),
        }
    }

    #[tokio::test]
    async fn test_migration_should_not_create_forge_services_credential() {
        let mut env_vars = HashMap::new();
        env_vars.insert("OPENAI_API_KEY".to_string(), "test-key".to_string());

        let infra = Arc::new(MockInfra::new(env_vars));
        let registry = ForgeProviderRepository::new(infra.clone());

        // Trigger migration
        registry.migrate_env_to_file().await.unwrap();

        // Verify credentials were written
        let credentials_guard = infra.credentials.lock().await;
        let credentials = credentials_guard.as_ref().unwrap();

        // Verify forge_services was NOT created during migration
        assert!(
            !credentials
                .iter()
                .any(|c| c.id == ProviderId::FORGE_SERVICES),
            "Should NOT create forge_services credential during environment migration"
        );

        // Verify only OpenAI credential was created
        assert_eq!(
            credentials.len(),
            1,
            "Should only have one credential (OpenAI)"
        );
        assert!(
            credentials.iter().any(|c| c.id == ProviderId::OPENAI),
            "Should have OpenAI credential"
        );
    }

    #[tokio::test]
    async fn test_migration_both_compatible_urls() {
        let mut env_vars = HashMap::new();
        env_vars.insert("OPENAI_API_KEY".to_string(), "test-openai-key".to_string());
        env_vars.insert(
            "ANTHROPIC_API_KEY".to_string(),
            "test-anthropic-key".to_string(),
        );
        env_vars.insert(
            "OPENAI_URL".to_string(),
            "https://custom.openai.com/v1".to_string(),
        );
        env_vars.insert(
            "ANTHROPIC_URL".to_string(),
            "https://custom.anthropic.com/v1".to_string(),
        );

        let infra = Arc::new(MockInfra::new(env_vars));
        let registry = ForgeProviderRepository::new(infra.clone());

        // Trigger migration
        registry.migrate_env_to_file().await.unwrap();

        // Verify credentials were written
        let credentials_guard = infra.credentials.lock().await;
        let credentials = credentials_guard.as_ref().unwrap();

        // Should have migrated only compatible versions
        assert!(
            !credentials.iter().any(|c| c.id == ProviderId::OPENAI),
            "Should NOT create OpenAI credential when OPENAI_URL is set"
        );
        assert!(
            credentials
                .iter()
                .any(|c| c.id == ProviderId::OPENAI_COMPATIBLE),
            "Should create OpenAICompatible credential when OPENAI_URL is set"
        );
        assert!(
            !credentials.iter().any(|c| c.id == ProviderId::ANTHROPIC),
            "Should NOT create Anthropic credential when ANTHROPIC_URL is set"
        );
        assert!(
            credentials
                .iter()
                .any(|c| c.id == ProviderId::ANTHROPIC_COMPATIBLE),
            "Should create AnthropicCompatible credential when ANTHROPIC_URL is set"
        );

        // Verify AnthropicCompatible has URL param
        let anthropic_compat_cred = credentials
            .iter()
            .find(|c| c.id == ProviderId::ANTHROPIC_COMPATIBLE)
            .unwrap();
        assert!(!anthropic_compat_cred.url_params.is_empty());
        let url_params = &anthropic_compat_cred.url_params;
        assert_eq!(
            url_params
                .get(&URLParam::from("ANTHROPIC_URL".to_string()))
                .map(|v| v.as_str()),
            Some("https://custom.anthropic.com/v1")
        );
    }

    #[tokio::test]
    async fn test_create_azure_provider_with_handlebars_urls() {
        let mut env_vars = HashMap::new();
        env_vars.insert("AZURE_API_KEY".to_string(), "test-key-123".to_string());
        env_vars.insert(
            "AZURE_RESOURCE_NAME".to_string(),
            "my-test-resource".to_string(),
        );
        env_vars.insert(
            "AZURE_DEPLOYMENT_NAME".to_string(),
            "gpt-4-deployment".to_string(),
        );
        env_vars.insert(
            "AZURE_API_VERSION".to_string(),
            "2024-02-01-preview".to_string(),
        );

        let infra = Arc::new(MockInfra::new(env_vars));
        let registry = ForgeProviderRepository::new(infra);

        // Trigger migration to populate credentials file
        registry.migrate_env_to_file().await.unwrap();

        // Get Azure config from embedded configs
        let configs = registry.get_provider_configs().await.unwrap();
        let azure_config = configs
            .iter()
            .find(|c| c.id == ProviderId::AZURE)
            .expect("Azure config should exist");

        // Create provider using the registry's create_provider method
        let provider = registry
            .create_provider(azure_config)
            .await
            .expect("Should create Azure provider");

        // Verify all URLs are correctly rendered
        assert_eq!(provider.id, ProviderId::AZURE);
        assert_eq!(
            provider
                .credential
                .as_ref()
                .and_then(|c| match &c.auth_details {
                    forge_domain::AuthDetails::ApiKey(key) => Some(key.to_string()),
                    _ => None,
                }),
            Some("test-key-123".to_string())
        );

        // Check that URL template is returned (not rendered)
        let url_template = &provider.url;
        assert_eq!(
            url_template.template,
            "https://{{AZURE_RESOURCE_NAME}}.openai.azure.com/openai/deployments/{{AZURE_DEPLOYMENT_NAME}}/chat/completions?api-version={{AZURE_API_VERSION}}"
        );

        // Check that model URL template is returned (not rendered)
        match &provider.models.as_ref().unwrap() {
            forge_domain::ModelSource::Url(model_template) => {
                assert_eq!(
                    model_template.template,
                    "https://{{AZURE_RESOURCE_NAME}}.openai.azure.com/openai/models?api-version={{AZURE_API_VERSION}}"
                );
            }
            forge_domain::ModelSource::Hardcoded(_) => panic!("Expected ModelSource::Url variant"),
        }
    }

    #[tokio::test]
    async fn test_default_provider_urls() {
        let mut env_vars = HashMap::new();
        env_vars.insert("OPENAI_API_KEY".to_string(), "test-key".to_string());
        env_vars.insert("ANTHROPIC_API_KEY".to_string(), "test-key".to_string());

        let infra = Arc::new(MockInfra::new(env_vars));
        let registry = ForgeProviderRepository::new(infra);

        // Migrate environment variables to credentials file
        registry.migrate_env_to_file().await.unwrap();

        let providers = registry.get_all_providers().await.unwrap();

        let openai_provider = providers
            .iter()
            .find_map(|p| match p {
                AnyProvider::Template(cp) if cp.id == ProviderId::OPENAI => Some(cp),
                _ => None,
            })
            .unwrap();
        let anthropic_provider = providers
            .iter()
            .find_map(|p| match p {
                AnyProvider::Template(cp) if cp.id == ProviderId::ANTHROPIC => Some(cp),
                _ => None,
            })
            .unwrap();

        // Regular OpenAI and Anthropic providers return template URLs (not rendered)
        assert_eq!(
            openai_provider.url.template,
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            anthropic_provider.url.template,
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[tokio::test]
    async fn test_merge_base_provider_configs() {
        use std::io::Write;

        use tempfile::TempDir;

        // Create a temporary directory to act as base_path
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();

        // Create a custom provider.json in the base directory
        // Only override OpenAI, don't add custom providers
        let provider_json_path = base_path.join("provider.json");
        let mut file = std::fs::File::create(&provider_json_path).unwrap();
        let custom_config = r#"[
            {
                "id": "openai",
                "api_key_vars": "CUSTOM_OPENAI_KEY",
                "url_param_vars": [],
                "response_type": "OpenAI",
                "auth_methods": [],
                "url": "https://custom.openai.com/v1/chat/completions",
                "models": "https://custom.openai.com/v1/models"
            }
        ]"#;
        file.write_all(custom_config.as_bytes()).unwrap();
        drop(file);

        // Create mock infra with the custom base_path
        let mut env_vars = HashMap::new();
        env_vars.insert("CUSTOM_OPENAI_KEY".to_string(), "test-key".to_string());

        struct CustomMockInfra {
            env_vars: HashMap<String, String>,
            base_path: PathBuf,
        }

        impl EnvironmentInfra for CustomMockInfra {
            fn get_environment(&self) -> Environment {
                use fake::{Fake, Faker};
                let mut env: Environment = Faker.fake();
                env.base_path = self.base_path.clone();
                env
            }

            fn get_env_var(&self, key: &str) -> Option<String> {
                self.env_vars.get(key).cloned()
            }

            fn get_env_vars(&self) -> BTreeMap<String, String> {
                self.env_vars
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect()
            }

            fn is_restricted(&self) -> bool {
                false
            }
        }

        #[async_trait::async_trait]
        impl FileReaderInfra for CustomMockInfra {
            async fn read_utf8(&self, path: &std::path::Path) -> anyhow::Result<String> {
                tokio::fs::read_to_string(path).await.map_err(Into::into)
            }

            fn read_batch_utf8(
                &self,
                _batch_size: usize,
                _paths: Vec<PathBuf>,
            ) -> impl futures::Stream<Item = (PathBuf, anyhow::Result<String>)> + Send {
                futures::stream::empty()
            }

            async fn read(&self, path: &std::path::Path) -> anyhow::Result<Vec<u8>> {
                tokio::fs::read(path).await.map_err(Into::into)
            }

            async fn range_read_utf8(
                &self,
                _path: &std::path::Path,
                _start_line: u64,
                _end_line: u64,
            ) -> anyhow::Result<(String, forge_domain::FileInfo)> {
                Err(anyhow::anyhow!("Not implemented"))
            }
        }

        #[async_trait::async_trait]
        impl FileWriterInfra for CustomMockInfra {
            async fn write(&self, _path: &std::path::Path, _content: Bytes) -> anyhow::Result<()> {
                Ok(())
            }

            async fn write_temp(
                &self,
                _prefix: &str,
                _ext: &str,
                _content: &str,
            ) -> anyhow::Result<PathBuf> {
                Ok(PathBuf::from("/tmp/test"))
            }
        }

        #[async_trait::async_trait]
        impl HttpInfra for CustomMockInfra {
            async fn http_get(
                &self,
                _url: &reqwest::Url,
                _headers: Option<reqwest::header::HeaderMap>,
            ) -> anyhow::Result<reqwest::Response> {
                Err(anyhow::anyhow!("HTTP not implemented in mock"))
            }

            async fn http_post(
                &self,
                _url: &reqwest::Url,
                _headers: Option<reqwest::header::HeaderMap>,
                _body: bytes::Bytes,
            ) -> anyhow::Result<reqwest::Response> {
                Err(anyhow::anyhow!("HTTP not implemented in mock"))
            }

            async fn http_delete(&self, _url: &reqwest::Url) -> anyhow::Result<reqwest::Response> {
                Err(anyhow::anyhow!("HTTP not implemented in mock"))
            }

            async fn http_eventsource(
                &self,
                _url: &reqwest::Url,
                _headers: Option<reqwest::header::HeaderMap>,
                _body: bytes::Bytes,
            ) -> anyhow::Result<reqwest_eventsource::EventSource> {
                Err(anyhow::anyhow!("HTTP not implemented in mock"))
            }
        }

        impl GrpcInfra for CustomMockInfra {
            fn channel(&self) -> tonic::transport::Channel {
                tonic::transport::Channel::from_static("http://[::1]:50051").connect_lazy()
            }

            fn hydrate(&self) {}
        }

        #[async_trait::async_trait]
        impl ChatRepository for CustomMockInfra {
            async fn chat(
                &self,
                _model_id: &ModelId,
                _context: Context,
                _provider: Provider<Url>,
            ) -> ResultStream<ChatCompletionMessage, anyhow::Error> {
                Ok(Box::pin(tokio_stream::iter(vec![])))
            }

            async fn models(&self, _provider: Provider<Url>) -> anyhow::Result<Vec<Model>> {
                Ok(vec![])
            }
        }

        #[async_trait::async_trait]
        impl ProviderRepository for CustomMockInfra {
            async fn get_all_providers(&self) -> anyhow::Result<Vec<AnyProvider>> {
                Ok(vec![])
            }

            async fn get_provider(&self, _id: ProviderId) -> anyhow::Result<ProviderTemplate> {
                Err(anyhow::anyhow!("Provider not found"))
            }

            async fn upsert_credential(
                &self,
                _credential: forge_domain::AuthCredential,
            ) -> anyhow::Result<()> {
                Ok(())
            }

            async fn get_credential(
                &self,
                _id: &ProviderId,
            ) -> anyhow::Result<Option<forge_domain::AuthCredential>> {
                Ok(None)
            }

            async fn remove_credential(&self, _id: &ProviderId) -> anyhow::Result<()> {
                Ok(())
            }

            async fn migrate_env_credentials(
                &self,
            ) -> anyhow::Result<Option<forge_domain::MigrationResult>> {
                Ok(None)
            }
        }

        let infra = Arc::new(CustomMockInfra { env_vars, base_path });
        let registry = ForgeProviderRepository::new(infra);

        // Get merged configs
        let merged_configs = registry.get_merged_configs().await.unwrap();

        // Verify OpenAI config was overridden
        let openai_config = merged_configs
            .iter()
            .find(|c| c.id == ProviderId::OPENAI)
            .expect("OpenAI config should exist");
        assert_eq!(
            openai_config.api_key_vars,
            Some("CUSTOM_OPENAI_KEY".to_string())
        );
        assert_eq!(
            openai_config.url.as_str(),
            "https://custom.openai.com/v1/chat/completions"
        );

        // Verify other embedded configs still exist
        let openrouter_config = merged_configs
            .iter()
            .find(|c| c.id == ProviderId::OPEN_ROUTER);
        assert!(openrouter_config.is_some());
    }
}
