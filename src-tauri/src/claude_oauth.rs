//! Claude Code official OAuth account store.
//!
//! Claude Code keeps official claude.ai login material in
//! `~/.claude/.credentials.json` (or an override config directory). This module
//! lets cc-switch keep multiple local snapshots of that file and project one of
//! them back to the live credentials file when a Claude Official provider is
//! enabled.

use crate::config::{atomic_write, get_app_config_dir, get_claude_config_dir};
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const STORE_FILE: &str = "claude_oauth_auth.json";
const CREDENTIALS_FILE: &str = ".credentials.json";
const AUTH_PROVIDER: &str = "claude_oauth";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeOAuthAccount {
    pub id: String,
    pub login: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_tier: Option<String>,
    pub authenticated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClaudeOAuthAccountData {
    pub id: String,
    pub login: String,
    pub credential_json: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_tier: Option<String>,
    pub authenticated_at: i64,
}

impl From<&ClaudeOAuthAccountData> for ClaudeOAuthAccount {
    fn from(data: &ClaudeOAuthAccountData) -> Self {
        Self {
            id: data.id.clone(),
            login: data.login.clone(),
            subscription_type: data.subscription_type.clone(),
            rate_limit_tier: data.rate_limit_tier.clone(),
            authenticated_at: data.authenticated_at,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ClaudeOAuthStore {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    accounts: HashMap<String, ClaudeOAuthAccountData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default_account_id: Option<String>,
}

fn store_path() -> PathBuf {
    get_app_config_dir().join(STORE_FILE)
}

fn live_credentials_path() -> PathBuf {
    get_claude_config_dir().join(CREDENTIALS_FILE)
}

fn now_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

fn load_store() -> Result<ClaudeOAuthStore, AppError> {
    let path = store_path();
    if !path.exists() {
        return Ok(ClaudeOAuthStore::default());
    }

    let text = fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    serde_json::from_str(&text).map_err(|e| AppError::json(&path, e))
}

fn save_store(mut store: ClaudeOAuthStore) -> Result<(), AppError> {
    store.version = 1;
    let path = store_path();
    let text =
        serde_json::to_vec_pretty(&store).map_err(|e| AppError::JsonSerialize { source: e })?;
    atomic_write(&path, &text)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }

    Ok(())
}

fn oauth_entry(credentials: &Value) -> Result<&Value, AppError> {
    credentials
        .get("claudeAiOauth")
        .or_else(|| credentials.get("claude.ai_oauth"))
        .ok_or_else(|| AppError::Config("No Claude OAuth entry found in credentials JSON".into()))
}

fn string_field(entry: &Value, key: &str) -> Option<String> {
    entry
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_account_data(
    credentials: Value,
    label: Option<String>,
) -> Result<ClaudeOAuthAccountData, AppError> {
    let entry = oauth_entry(&credentials)?;
    let access_token = string_field(entry, "accessToken")
        .ok_or_else(|| AppError::Config("Claude credentials missing accessToken".into()))?;
    let refresh_token = string_field(entry, "refreshToken");
    let subscription_type = string_field(entry, "subscriptionType");
    let rate_limit_tier = string_field(entry, "rateLimitTier");

    let mut hasher = Sha256::new();
    hasher.update(refresh_token.as_deref().unwrap_or(&access_token).as_bytes());
    let digest = hasher.finalize();
    let id = format!("claude_{:x}", &digest)[..23].to_string();
    let short = id.trim_start_matches("claude_");
    let short = &short[..short.len().min(8)];

    let login = label
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let sub = subscription_type.as_deref().unwrap_or("official");
            match rate_limit_tier.as_deref() {
                Some(tier) if !tier.is_empty() => format!("Claude {sub} / {tier} ({short})"),
                _ => format!("Claude {sub} ({short})"),
            }
        });

    Ok(ClaudeOAuthAccountData {
        id,
        login,
        credential_json: credentials,
        subscription_type,
        rate_limit_tier,
        authenticated_at: now_secs(),
    })
}

fn sorted_accounts(store: &ClaudeOAuthStore) -> Vec<ClaudeOAuthAccount> {
    let mut accounts: Vec<_> = store
        .accounts
        .values()
        .map(ClaudeOAuthAccount::from)
        .collect();
    let default_id = store.default_account_id.as_deref();
    accounts.sort_by(|a, b| {
        let a_default = default_id == Some(a.id.as_str());
        let b_default = default_id == Some(b.id.as_str());
        b_default
            .cmp(&a_default)
            .then_with(|| a.login.to_lowercase().cmp(&b.login.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
    });
    accounts
}

fn fallback_default_account_id(
    accounts: &HashMap<String, ClaudeOAuthAccountData>,
) -> Option<String> {
    let mut ids: Vec<_> = accounts.keys().cloned().collect();
    ids.sort();
    ids.into_iter().next()
}

fn resolve_account_id(
    store: &ClaudeOAuthStore,
    account_id: Option<&str>,
) -> Result<String, AppError> {
    if let Some(id) = account_id.map(str::trim).filter(|id| !id.is_empty()) {
        if store.accounts.contains_key(id) {
            return Ok(id.to_string());
        }
        return Err(AppError::Config(format!(
            "Claude OAuth account not found: {id}"
        )));
    }

    if let Some(id) = store.default_account_id.as_deref() {
        if store.accounts.contains_key(id) {
            return Ok(id.to_string());
        }
    }

    fallback_default_account_id(&store.accounts)
        .ok_or_else(|| AppError::Config("No Claude OAuth account has been imported".into()))
}

pub fn list_accounts() -> Result<Vec<ClaudeOAuthAccount>, AppError> {
    let store = load_store()?;
    Ok(sorted_accounts(&store))
}

pub fn default_account_id() -> Result<Option<String>, AppError> {
    let store = load_store()?;
    Ok(store
        .default_account_id
        .filter(|id| store.accounts.contains_key(id)))
}

pub fn import_current_credentials(label: Option<String>) -> Result<ClaudeOAuthAccount, AppError> {
    let path = live_credentials_path();
    if !path.exists() {
        return Err(AppError::Config(format!(
            "Claude credentials file does not exist: {}",
            path.display()
        )));
    }

    let text = fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    let credentials: Value = serde_json::from_str(&text).map_err(|e| AppError::json(&path, e))?;
    let account = parse_account_data(credentials, label)?;

    let mut store = load_store()?;
    store.accounts.insert(account.id.clone(), account.clone());
    if store.default_account_id.is_none() {
        store.default_account_id = Some(account.id.clone());
    }
    save_store(store)?;

    Ok(ClaudeOAuthAccount::from(&account))
}

pub fn refresh_account_from_live(account_id: &str) -> Result<(), AppError> {
    let path = live_credentials_path();
    if !path.exists() {
        return Ok(());
    }

    let text = fs::read_to_string(&path).map_err(|e| AppError::io(&path, e))?;
    let credentials: Value = serde_json::from_str(&text).map_err(|e| AppError::json(&path, e))?;

    let mut store = load_store()?;
    let Some(existing) = store.accounts.get(account_id).cloned() else {
        return Ok(());
    };

    let mut updated = parse_account_data(credentials, Some(existing.login.clone()))?;
    updated.id = existing.id;
    updated.authenticated_at = existing.authenticated_at;
    store.accounts.insert(account_id.to_string(), updated);
    save_store(store)
}

pub fn remove_account(account_id: &str) -> Result<(), AppError> {
    let mut store = load_store()?;
    if store.accounts.remove(account_id).is_none() {
        return Err(AppError::Config(format!(
            "Claude OAuth account not found: {account_id}"
        )));
    }
    if store.default_account_id.as_deref() == Some(account_id) {
        store.default_account_id = fallback_default_account_id(&store.accounts);
    }
    save_store(store)
}

pub fn set_default_account(account_id: &str) -> Result<(), AppError> {
    let mut store = load_store()?;
    if !store.accounts.contains_key(account_id) {
        return Err(AppError::Config(format!(
            "Claude OAuth account not found: {account_id}"
        )));
    }
    store.default_account_id = Some(account_id.to_string());
    save_store(store)
}

pub fn clear_accounts() -> Result<(), AppError> {
    let path = store_path();
    if path.exists() {
        fs::remove_file(&path).map_err(|e| AppError::io(&path, e))?;
    }
    Ok(())
}

pub fn apply_account_to_live(account_id: Option<&str>) -> Result<(), AppError> {
    let store = load_store()?;
    let id = resolve_account_id(&store, account_id)?;
    let account = store
        .accounts
        .get(&id)
        .ok_or_else(|| AppError::Config(format!("Claude OAuth account not found: {id}")))?;

    let path = live_credentials_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| AppError::io(parent, e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
    }

    let text = serde_json::to_vec_pretty(&account.credential_json)
        .map_err(|e| AppError::JsonSerialize { source: e })?;
    atomic_write(&path, &text)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }

    log::info!(
        "Applied Claude OAuth account '{}' ({}) to {}",
        account.login,
        account.id,
        path.display()
    );

    Ok(())
}

pub fn auth_provider() -> &'static str {
    AUTH_PROVIDER
}
