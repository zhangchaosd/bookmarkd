use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: Server,
    pub storage: Storage,
    pub auth: Auth,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Server {
    pub listen: String,
    pub public_url: String,
    #[serde(default)]
    pub allow_insecure_localhost: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Storage {
    pub business_db: PathBuf,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Auth {
    pub app_id: String,
    pub rp_id: String,
    pub rp_name: String,
    pub user_name: String,
    pub user_id_file: PathBuf,
    pub store: PathBuf,
    pub setup_mode: String,
}
impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let mut c: Self = toml::from_str(&std::fs::read_to_string(path)?)?;
        let base = path.parent().unwrap_or(Path::new("."));
        for p in [
            &mut c.storage.business_db,
            &mut c.auth.store,
            &mut c.auth.user_id_file,
        ] {
            if p.is_relative() {
                *p = base.join(&p);
            }
        }
        if let Ok(v) = std::env::var("BOOKMARKD_LISTEN") {
            c.server.listen = v;
        }
        if let Ok(v) = std::env::var("BOOKMARKD_PUBLIC_URL") {
            c.server.public_url = v;
        }
        if let Ok(v) = std::env::var("BOOKMARKD_SETUP_MODE") {
            c.auth.setup_mode = v;
        }
        c.validate()?;
        Ok(c)
    }
    pub fn validate(&self) -> Result<()> {
        let u = url::Url::parse(&self.server.public_url)?;
        ensure!(
            u.origin().ascii_serialization() == self.server.public_url,
            "public_url must be an exact origin without trailing slash"
        );
        ensure!(
            u.username().is_empty() && u.password().is_none(),
            "invalid origin"
        );
        let host = u
            .host_str()
            .ok_or_else(|| anyhow::anyhow!("missing host"))?;
        ensure!(
            host == self.auth.rp_id || host.ends_with(&format!(".{}", self.auth.rp_id)),
            "RP ID does not match origin"
        );
        ensure!(
            !self.auth.rp_id.contains(['/', ':']) && !self.auth.rp_id.is_empty(),
            "invalid RP ID"
        );
        ensure!(
            u.scheme() == "https"
                || (u.scheme() == "http"
                    && host == "localhost"
                    && self.server.allow_insecure_localhost),
            "HTTPS required except explicit localhost development"
        );
        ensure!(
            ["auto", "enabled", "disabled"].contains(&self.auth.setup_mode.as_str()),
            "invalid setup mode"
        );
        ensure!(
            !self.auth.app_id.is_empty()
                && self
                    .auth
                    .app_id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'),
            "invalid app_id"
        );
        Ok(())
    }
    pub fn identity(&self) -> Result<bookmarkd_auth::Identity> {
        Ok(bookmarkd_auth::Identity {
            rp_id: self.auth.rp_id.clone(),
            user_id: std::fs::read_to_string(&self.auth.user_id_file)?
                .trim()
                .into(),
        })
    }
    pub fn cookie_name(&self) -> String {
        format!(
            "{}{}_session",
            if self.server.public_url.starts_with("https:") {
                "__Host-"
            } else {
                ""
            },
            self.auth.app_id
        )
    }
}
