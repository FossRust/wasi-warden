use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use serde::Deserialize;

use crate::cli::StepArgs;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct HostConfig {
    pub workspace_root: Utf8PathBuf,
    pub allowed_proc_commands: Vec<String>,
    pub llm: Option<LlmSettings>,
    pub browser: Option<BrowserSettings>,
}

#[derive(Debug, Clone)]
pub struct LlmSettings {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct BrowserSettings {
    pub webdriver_url: String,
    pub default_profile: Option<String>,
}

impl HostConfig {
    pub fn from_step_args(args: &StepArgs) -> Result<Self> {
        let file_cfg = FileConfig::load(&args.config)?;
        let workspace_path = args
            .workspace
            .clone()
            .or_else(|| file_cfg.workspace_root.clone().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("."));
        let workspace_root = normalize_path(&workspace_path).with_context(|| {
            format!(
                "invalid workspace path {}",
                workspace_path.to_string_lossy()
            )
        })?;
        let mut allowed_proc_commands = file_cfg.allow_proc.unwrap_or_default();
        allowed_proc_commands.extend(args.allow_proc.iter().cloned());
        allowed_proc_commands.sort();
        allowed_proc_commands.dedup();
        let llm = match file_cfg.llm {
            Some(cfg) => cfg.into_settings()?,
            None => None,
        };
        let browser = match file_cfg.browser {
            Some(cfg) => cfg.into_settings()?,
            None => None,
        };
        Ok(Self {
            workspace_root,
            allowed_proc_commands,
            llm,
            browser,
        })
    }

    pub fn is_proc_allowed(&self, program: &str) -> bool {
        if self.allowed_proc_commands.is_empty() {
            return false;
        }
        let base = Path::new(program)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(program);
        self.allowed_proc_commands
            .iter()
            .any(|entry| entry == program || entry == base)
    }
}

#[derive(Default, Deserialize)]
struct FileConfig {
    workspace_root: Option<String>,
    allow_proc: Option<Vec<String>>,
    llm: Option<LlmFileSettings>,
    browser: Option<BrowserFileSettings>,
}

impl FileConfig {
    fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let raw = fs::read_to_string(path)
                .with_context(|| format!("failed to read config {}", path.display()))?;
            toml::from_str(&raw)
                .with_context(|| format!("failed to parse config {}", path.display()))
        } else {
            Ok(Self::default())
        }
    }
}

#[derive(Deserialize)]
struct LlmFileSettings {
    api_base: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
}

impl LlmFileSettings {
    fn into_settings(self) -> Result<Option<LlmSettings>> {
        let api_key = match self.api_key {
            Some(key) if !key.trim().is_empty() => key,
            _ => return Ok(None),
        };
        let model = match self.model {
            Some(model) if !model.trim().is_empty() => model,
            _ => return Ok(None),
        };
        let api_base = self
            .api_base
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
        Ok(Some(LlmSettings {
            api_base,
            api_key,
            model,
        }))
    }
}

fn normalize_path(path: &Path) -> Result<Utf8PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to read current directory")?
            .join(path)
    };
    let canonical = fs::canonicalize(&absolute).unwrap_or(absolute);
    Utf8PathBuf::from_path_buf(canonical).map_err(|_| anyhow::anyhow!("path is not valid UTF-8"))
}

#[derive(Deserialize)]
struct BrowserFileSettings {
    webdriver_url: Option<String>,
    default_profile: Option<String>,
}

impl BrowserFileSettings {
    fn into_settings(self) -> Result<Option<BrowserSettings>> {
        let url = match self.webdriver_url {
            Some(url) if !url.trim().is_empty() => url,
            _ => return Ok(None),
        };
        Ok(Some(BrowserSettings {
            webdriver_url: url,
            default_profile: self.default_profile.filter(|p| !p.trim().is_empty()),
        }))
    }
}
