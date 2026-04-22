use std::fmt;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const VAULT_DIR: &str = ".openidiom";
const CONFIG_FILE: &str = "config.toml";
const DB_FILE: &str = "index.db";

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("No vault found. Run `oi init` first.")]
    NotFound,
    #[error("Vault already exists at {0}")]
    AlreadyExists(PathBuf),
    #[error("Invalid config: {field} — {reason}")]
    InvalidConfig { field: String, reason: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VaultConfig {
    pub vault: VaultSection,
    pub ai: AiSection,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VaultSection {
    pub name: String,
    pub daily_folder: String,
    pub daily_format: String,
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AiSection {
    pub provider: String,
    #[serde(default)]
    pub model: Option<String>,
    pub embedding_provider: String,
    pub embedding_model: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub ollama_url: Option<String>,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    #[serde(default = "default_search_top_k")]
    pub search_top_k: usize,
    #[serde(default = "default_context_top_k")]
    pub context_top_k: usize,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

fn default_chunk_size() -> usize { 500 }
fn default_search_top_k() -> usize { 10 }
fn default_context_top_k() -> usize { 5 }
fn default_batch_size() -> usize { 50 }

const VALID_PROVIDERS: &[&str] = &["claude", "openai", "ollama"];
const VALID_EMBEDDING_PROVIDERS: &[&str] = &["openai", "ollama"];

impl VaultConfig {
    pub fn validate(&self) -> Result<(), VaultError> {
        if self.vault.name.trim().is_empty() {
            return Err(VaultError::InvalidConfig {
                field: "vault.name".into(),
                reason: "must not be empty".into(),
            });
        }
        if !VALID_PROVIDERS.contains(&self.ai.provider.as_str()) {
            return Err(VaultError::InvalidConfig {
                field: "ai.provider".into(),
                reason: format!(
                    "invalid value '{}'. Valid options: {}",
                    self.ai.provider,
                    VALID_PROVIDERS.join(", ")
                ),
            });
        }
        if !VALID_EMBEDDING_PROVIDERS.contains(&self.ai.embedding_provider.as_str()) {
            return Err(VaultError::InvalidConfig {
                field: "ai.embedding_provider".into(),
                reason: format!(
                    "invalid value '{}'. Valid options: {}",
                    self.ai.embedding_provider,
                    VALID_EMBEDDING_PROVIDERS.join(", ")
                ),
            });
        }
        // Validate daily_format by attempting a format
        let now = chrono::Utc::now();
        let formatted = now.format(&self.vault.daily_format).to_string();
        if formatted.is_empty() {
            return Err(VaultError::InvalidConfig {
                field: "vault.daily_format".into(),
                reason: "invalid chrono format string".into(),
            });
        }
        Ok(())
    }

    pub fn default_config() -> Self {
        Self {
            vault: VaultSection {
                name: "my-vault".into(),
                daily_folder: "daily".into(),
                daily_format: "%Y-%m-%d".into(),
                ignore: vec![
                    ".openidiom".into(),
                    ".git".into(),
                    "node_modules".into(),
                    ".obsidian".into(),
                ],
            },
            ai: AiSection {
                provider: "ollama".into(),
                model: None,
                embedding_provider: "ollama".into(),
                embedding_model: "nomic-embed-text".into(),
                base_url: None,
                ollama_url: None,
                chunk_size: default_chunk_size(),
                search_top_k: default_search_top_k(),
                context_top_k: default_context_top_k(),
                batch_size: default_batch_size(),
            },
        }
    }
}

pub struct Vault {
    pub root: PathBuf,
    pub config: VaultConfig,
    pub db_path: PathBuf,
}

impl Vault {
    /// Discover vault by walking up from `from` looking for .openidiom/
    pub fn discover(from: &Path) -> Result<Vault, VaultError> {
        let mut current = from.to_path_buf();
        loop {
            let candidate = current.join(VAULT_DIR);
            if candidate.is_dir() {
                let config_path = candidate.join(CONFIG_FILE);
                let config_str = std::fs::read_to_string(&config_path)?;
                let config: VaultConfig = toml::from_str(&config_str)?;
                config.validate()?;
                let db_path = candidate.join(DB_FILE);
                return Ok(Vault {
                    root: current,
                    config,
                    db_path,
                });
            }
            if !current.pop() {
                return Err(VaultError::NotFound);
            }
        }
    }

    pub fn open_db(&self) -> Result<Connection, VaultError> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(conn)
    }
}

/// Initialize a new vault at the given path
pub fn init_vault(path: &Path) -> Result<(), VaultError> {
    let vault_dir = path.join(VAULT_DIR);
    if vault_dir.exists() {
        return Err(VaultError::AlreadyExists(vault_dir));
    }

    std::fs::create_dir_all(&vault_dir)?;

    let config = VaultConfig::default_config();
    config.validate()?;
    let config_str = toml::to_string_pretty(&config)
        .expect("default config should serialize");
    std::fs::write(vault_dir.join(CONFIG_FILE), config_str)?;

    let conn = Connection::open(vault_dir.join(DB_FILE))?;
    crate::db::schema::create_tables(&conn)?;

    Ok(())
}

/// Vault status information
#[derive(Debug, Serialize)]
pub struct VaultStatus {
    pub name: String,
    pub root: String,
    pub total_notes: usize,
    pub total_links: usize,
    pub total_tags: usize,
    pub last_indexed: Option<String>,
    pub stale_notes: usize,
}

impl fmt::Display for VaultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Vault: {} ({})", self.name, self.root)?;
        writeln!(f, "Notes: {} | Links: {} | Tags: {} unique",
            self.total_notes, self.total_links, self.total_tags)?;
        if let Some(ref ts) = self.last_indexed {
            writeln!(f, "Last indexed: {ts}")?;
        } else {
            writeln!(f, "Last indexed: never")?;
        }
        if self.stale_notes > 0 {
            writeln!(f, "Stale: {} notes modified since last index", self.stale_notes)?;
        }
        Ok(())
    }
}

pub fn vault_status(conn: &Connection, vault: &Vault) -> Result<VaultStatus, VaultError> {
    let total_notes: usize = conn
        .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))?;
    let total_links: usize = conn
        .query_row("SELECT COUNT(*) FROM links", [], |row| row.get(0))?;
    let total_tags: usize = conn
        .query_row("SELECT COUNT(DISTINCT tag) FROM tags", [], |row| row.get(0))?;
    let last_indexed: Option<String> = conn
        .query_row(
            "SELECT MAX(indexed_at) FROM notes",
            [],
            |row| row.get(0),
        )?;

    // Count stale notes: files modified after their indexed_at timestamp
    // For now, we report 0 — full implementation compares file mtimes
    let stale_notes = 0;

    Ok(VaultStatus {
        name: vault.config.vault.name.clone(),
        root: vault.root.display().to_string(),
        total_notes,
        total_links,
        total_tags,
        last_indexed,
        stale_notes,
    })
}
