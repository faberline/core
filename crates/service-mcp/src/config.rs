use anyhow::{bail, Result};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpTransportConfig {
    allowed_hosts: Vec<String>,
    allowed_origins: Vec<String>,
}

impl HttpTransportConfig {
    pub fn new(allowed_hosts: Vec<String>, allowed_origins: Vec<String>) -> Result<Self> {
        let config = Self {
            allowed_hosts,
            allowed_origins,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn from_env(
        hosts_env: &str,
        origins_env: &str,
        default_hosts: Vec<String>,
        default_origins: Vec<String>,
    ) -> Result<Self> {
        Self::new(
            csv_env(hosts_env, default_hosts)?,
            csv_env(origins_env, default_origins)?,
        )
    }

    fn validate(&self) -> Result<()> {
        if self.allowed_hosts.is_empty()
            || self
                .allowed_hosts
                .iter()
                .any(|value| value.trim().is_empty())
        {
            bail!("MCP allowed hosts must contain at least one nonempty value");
        }
        if self.allowed_origins.is_empty()
            || self
                .allowed_origins
                .iter()
                .any(|value| value.trim().is_empty())
        {
            bail!("MCP allowed origins must contain at least one nonempty value");
        }
        Ok(())
    }

    pub(crate) fn into_rmcp(
        self,
    ) -> rmcp::transport::streamable_http_server::StreamableHttpServerConfig {
        rmcp::transport::streamable_http_server::StreamableHttpServerConfig::default()
            .with_legacy_session_mode(true)
            .with_json_response(true)
            .with_allowed_hosts(self.allowed_hosts)
            .with_allowed_origins(self.allowed_origins)
    }
}

fn csv_env(name: &str, default: Vec<String>) -> Result<Vec<String>> {
    let Ok(value) = std::env::var(name) else {
        return Ok(default);
    };
    let values = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if values.is_empty() {
        bail!("{name} must contain at least one comma-separated value");
    }
    Ok(values)
}
