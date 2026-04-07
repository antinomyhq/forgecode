use std::sync::Arc;

use dashmap::DashMap;
use forge_app::domain::{AgentId, Error, ModelId, ProviderId};
use forge_app::{AgentRepository, EnvironmentInfra};
use forge_config::ForgeConfig;
use forge_domain::Agent;
use tokio::sync::RwLock;

/// AgentRegistryService manages the active-agent ID and a registry of runtime
/// Agents in-memory. It lazily loads agents from AgentRepository on first
/// access.
pub struct ForgeAgentRegistryService<R> {
    // Infrastructure dependency for loading agents
    repository: Arc<R>,

    // Runtime configuration used to resolve default provider/model.
    // Refreshed from disk on reload so model/provider switches apply immediately.
    config: RwLock<ForgeConfig>,

    // In-memory storage for agents keyed by AgentId string
    // Lazily initialized on first access
    // Wrapped in RwLock to allow invalidation
    agents: RwLock<Option<DashMap<String, Agent>>>,

    // In-memory storage for the active agent ID
    active_agent_id: RwLock<Option<AgentId>>,
}

impl<R> ForgeAgentRegistryService<R> {
    /// Creates a new AgentRegistryService with the given repository
    pub fn new(repository: Arc<R>, config: ForgeConfig) -> Self {
        Self {
            repository,
            config: RwLock::new(config),
            agents: RwLock::new(None),
            active_agent_id: RwLock::new(None),
        }
    }
}

impl<R: AgentRepository + EnvironmentInfra> ForgeAgentRegistryService<R> {
    /// Reloads Forge config using the same precedence order as startup.
    fn load_current_config(&self) -> anyhow::Result<ForgeConfig> {
        ForgeConfig::read().map_err(Into::into)
    }

    /// Refreshes the in-memory config snapshot from disk/env sources.
    async fn refresh_config(&self) -> anyhow::Result<()> {
        let config = self.load_current_config()?;
        *self.config.write().await = config;
        Ok(())
    }

    /// Lazily initializes and returns the agents map.
    /// Loads agents from repository on first call, subsequent calls return
    /// cached value.
    async fn ensure_agents_loaded(&self) -> anyhow::Result<DashMap<String, Agent>> {
        // Check if already loaded
        {
            let agents_read = self.agents.read().await;
            if let Some(agents) = agents_read.as_ref() {
                return Ok(agents.clone());
            }
        }

        // Not loaded yet, acquire write lock and load
        let mut agents_write = self.agents.write().await;

        // Double-check in case another task loaded while we were waiting for write
        // lock
        if let Some(agents) = agents_write.as_ref() {
            return Ok(agents.clone());
        }

        // Load agents
        let agents_map = self.load_agents().await?;

        // Store and return
        *agents_write = Some(agents_map.clone());
        Ok(agents_map)
    }

    /// Load agents from repository and populate the in-memory map.
    ///
    /// Reads the default provider and model from [`ForgeConfig`] and passes
    /// them to the repository so agents that do not specify their own
    /// provider/model receive the session-level defaults.
    async fn load_agents(&self) -> anyhow::Result<DashMap<String, Agent>> {
        let config = self.config.read().await;
        let session = config.session.as_ref().ok_or(Error::NoDefaultProvider)?;
        let provider_id = session
            .provider_id
            .as_ref()
            .map(|id| ProviderId::from(id.clone()))
            .ok_or(Error::NoDefaultProvider)?;
        let model_id = session
            .model_id
            .as_ref()
            .map(|id| ModelId::new(id.clone()))
            .ok_or_else(|| {
                anyhow::anyhow!("No default model configured for provider {}", provider_id)
            })?;

        let agents = self.repository.get_agents(provider_id, model_id).await?;

        let agents_map = DashMap::new();
        for agent in agents {
            agents_map.insert(agent.id.as_str().to_string(), agent);
        }

        Ok(agents_map)
    }
}

#[async_trait::async_trait]
impl<R: AgentRepository + EnvironmentInfra + Send + Sync> forge_app::AgentRegistry
    for ForgeAgentRegistryService<R>
{
    async fn get_active_agent_id(&self) -> anyhow::Result<Option<AgentId>> {
        let agent_id = self.active_agent_id.read().await;
        Ok(agent_id.clone())
    }

    async fn set_active_agent_id(&self, agent_id: AgentId) -> anyhow::Result<()> {
        let mut active_agent = self.active_agent_id.write().await;
        *active_agent = Some(agent_id);
        Ok(())
    }

    async fn get_agents(&self) -> anyhow::Result<Vec<Agent>> {
        let agents = self.ensure_agents_loaded().await?;
        Ok(agents.iter().map(|entry| entry.value().clone()).collect())
    }

    async fn get_agent(&self, agent_id: &AgentId) -> anyhow::Result<Option<Agent>> {
        let agents = self.ensure_agents_loaded().await?;
        Ok(agents.get(agent_id.as_str()).map(|v| v.value().clone()))
    }

    async fn reload_agents(&self) -> anyhow::Result<()> {
        self.refresh_config().await?;
        *self.agents.write().await = None;

        self.ensure_agents_loaded().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;
    use std::sync::{Mutex, MutexGuard};

    use forge_app::AgentRegistry;
    use forge_config::{ForgeConfig, ModelConfig};
    use forge_domain::{Agent, AgentId, ConfigOperation, Environment, ModelId, ProviderId};
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    use super::*;

    static HOME_ENV_MUTEX: Mutex<()> = Mutex::new(());

    struct HomeEnvGuard {
        original_home: Option<String>,
        _lock: MutexGuard<'static, ()>,
    }

    impl HomeEnvGuard {
        fn set(home: &Path) -> Self {
            let lock = HOME_ENV_MUTEX.lock().unwrap();
            let original_home = std::env::var("HOME").ok();
            unsafe {
                std::env::set_var("HOME", home);
            }
            Self {
                original_home,
                _lock: lock,
            }
        }
    }

    impl Drop for HomeEnvGuard {
        fn drop(&mut self) {
            if let Some(home) = &self.original_home {
                unsafe {
                    std::env::set_var("HOME", home);
                }
            } else {
                unsafe {
                    std::env::remove_var("HOME");
                }
            }
        }
    }

    #[derive(Default)]
    struct MockRepository;

    #[async_trait::async_trait]
    impl forge_app::AgentRepository for MockRepository {
        async fn get_agents(
            &self,
            provider_id: ProviderId,
            model_id: ModelId,
        ) -> anyhow::Result<Vec<Agent>> {
            Ok(vec![Agent::new(AgentId::new("forge"), provider_id, model_id)])
        }
    }

    impl forge_app::EnvironmentInfra for MockRepository {
        type Config = ForgeConfig;

        fn get_env_var(&self, _key: &str) -> Option<String> {
            None
        }

        fn get_env_vars(&self) -> BTreeMap<String, String> {
            BTreeMap::new()
        }

        fn get_environment(&self) -> Environment {
            Environment {
                os: "test".to_string(),
                pid: 0,
                cwd: std::path::PathBuf::from("."),
                home: None,
                shell: "zsh".to_string(),
                base_path: std::path::PathBuf::from("."),
            }
        }

        async fn update_environment(&self, _ops: Vec<ConfigOperation>) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_reload_agents_refreshes_provider_model_from_config_file() {
        let fixture_home = tempdir().unwrap();
        let _fixture_home_env_guard = HomeEnvGuard::set(fixture_home.path());
        let fixture_forge_dir = fixture_home.path().join("forge");
        fs::create_dir_all(&fixture_forge_dir).unwrap();

        let fixture_first_config = r#"
[session]
provider_id = "anthropic"
model_id = "claude-3-5-sonnet-20241022"
"#;
        fs::write(fixture_forge_dir.join(".forge.toml"), fixture_first_config).unwrap();

        let fixture_startup_config = ForgeConfig {
            session: Some(ModelConfig {
                provider_id: Some("openai".to_string()),
                model_id: Some("gpt-4".to_string()),
            }),
            ..Default::default()
        };

        let fixture_repository = Arc::new(MockRepository);
        let fixture_service = ForgeAgentRegistryService::new(fixture_repository, fixture_startup_config);

        let fixture_agent_id = AgentId::new("forge");
        let actual_before = fixture_service
            .get_agent(&fixture_agent_id)
            .await
            .unwrap()
            .unwrap();

        let expected_before_provider = ProviderId::OPENAI;
        let expected_before_model = ModelId::new("gpt-4");
        assert_eq!(actual_before.provider, expected_before_provider);
        assert_eq!(actual_before.model, expected_before_model);

        let fixture_second_config = r#"
[session]
provider_id = "modal"
model_id = "zai-org/GLM-5-FP8"
"#;
        fs::write(fixture_forge_dir.join(".forge.toml"), fixture_second_config).unwrap();

        fixture_service.reload_agents().await.unwrap();

        let actual_after = fixture_service
            .get_agent(&fixture_agent_id)
            .await
            .unwrap()
            .unwrap();

        let expected_after_provider = ProviderId::MODAL;
        let expected_after_model = ModelId::new("zai-org/GLM-5-FP8");
        assert_eq!(actual_after.provider, expected_after_provider);
        assert_eq!(actual_after.model, expected_after_model);
    }
}

