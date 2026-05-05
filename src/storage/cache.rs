use std::fs;
use std::io;
use std::path::Path;

use crate::model::{AuthCache, AuthConfig};

use super::normalize::{
    compact_config_for_save, find_legacy_session, normalize_account,
    normalize_account_for_base_url, normalize_config,
};

pub fn load_cache(path: impl AsRef<Path>) -> io::Result<AuthConfig> {
    let path = path.as_ref();
    let content = match fs::read(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(AuthConfig::default()),
        Err(error) => return Err(error),
    };
    if String::from_utf8_lossy(&content).trim().is_empty() {
        return Ok(AuthConfig::default());
    }

    if let Ok(config) = serde_json::from_slice::<AuthConfig>(&content) {
        let config = normalize_config(config);
        if !config.accounts.is_empty() {
            return Ok(config);
        }
    }

    let cache: AuthCache = serde_json::from_slice(&content)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let cache = normalize_account(cache);
    let base_url = find_legacy_session(&cache, "")
        .map(|session| session.base_url)
        .unwrap_or_default();
    let cache = normalize_account_for_base_url(cache, &base_url);
    if cache.email.is_empty() {
        return Ok(AuthConfig::default());
    }
    Ok(normalize_config(AuthConfig {
        base_url,
        accounts: vec![cache],
    }))
}

pub fn save_cache(path: impl AsRef<Path>, config: AuthConfig) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let config = compact_config_for_save(config);
    let mut content = serde_json::to_string_pretty(&config)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    content.push('\n');
    fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AuthSession, LoginResponse};
    use crate::storage::{cache_from_login, get_session, upsert_account};
    use tempfile::tempdir;

    #[test]
    fn supports_legacy_single_account_format() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("auth.json");
        fs::write(
            &path,
            r#"{
  "email": "demo@example.com",
  "password": "pw",
  "token_type": "Bearer",
  "access_token": "legacy-token"
}"#,
        )
        .unwrap();

        let config = load_cache(&path).unwrap();
        assert_eq!(config.accounts.len(), 1);
        assert_eq!(config.accounts[0].access_token, "legacy-token");
    }

    #[test]
    fn saves_and_loads_flattened_session_with_top_level_base_url() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("auth.json");
        let config = AuthConfig {
            base_url: "HTTPS://SUB.HDD.SB/".to_string(),
            accounts: vec![AuthCache {
                email: "demo@example.com".to_string(),
                password: "pw".to_string(),
                token_type: "bearer".to_string(),
                access_token: "token-a".to_string(),
                ..AuthCache::default()
            }],
        };

        save_cache(&path, config).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("\"sessions\""));
        assert!(raw.contains("\"token_type\": \"Bearer\""));

        let loaded = load_cache(&path).unwrap();
        assert_eq!(loaded.base_url, "https://sub.hdd.sb");
        assert_eq!(loaded.accounts.len(), 1);
        let account = &loaded.accounts[0];
        assert!(account.sessions.is_empty());
        assert_eq!(account.token_type, "Bearer");
        assert_eq!(account.access_token, "token-a");

        let session = get_session(account, &loaded.base_url).unwrap();
        assert_eq!(session.base_url, "https://sub.hdd.sb");
    }

    #[test]
    fn upsert_keeps_top_level_base_url_when_first_account_has_legacy_session() {
        let updated = AuthCache {
            email: "demo@example.com".to_string(),
            password: "pw".to_string(),
            sessions: vec![AuthSession {
                base_url: "https://staging.example.com/".to_string(),
                token_type: "bearer".to_string(),
                access_token: "staging-token-new".to_string(),
            }],
            ..AuthCache::default()
        };

        let config = upsert_account(AuthConfig::default(), updated);
        assert_eq!(config.base_url, "https://staging.example.com");
        assert_eq!(config.accounts.len(), 1);
        let session = get_session(&config.accounts[0], &config.base_url).unwrap();
        assert_eq!(session.access_token, "staging-token-new");
    }

    #[test]
    fn save_cache_creates_parent_directory() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("var").join("data").join("auth.json");
        let config = AuthConfig::default();

        save_cache(&path, config).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn flattens_legacy_session_format() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("auth.json");
        fs::write(
            &path,
            r#"{
  "selected_email": "demo@example.com",
  "accounts": [
    {
      "email": "demo@example.com",
      "password": "pw",
      "sessions": [
        {
          "base_url": "https://prod.example.com/",
          "token_type": "bearer",
          "access_token": "prod-token"
        }
      ]
    }
  ]
}"#,
        )
        .unwrap();

        let config = load_cache(&path).unwrap();
        assert_eq!(config.base_url, "https://prod.example.com");
        assert_eq!(config.accounts.len(), 1);
        let account = &config.accounts[0];
        assert!(account.sessions.is_empty());
        assert_eq!(account.access_token, "prod-token");
    }

    #[test]
    fn cache_from_login_normalizes_session_state() {
        let login: LoginResponse = serde_json::from_str(
            r#"{
  "code": 0,
  "message": "ok",
  "reason": "",
  "data": {
    "access_token": "token-a",
    "token_type": "bearer",
    "user": {"email": "demo@example.com"}
  }
}"#,
        )
        .unwrap();

        let account = cache_from_login(&login, "fallback@example.com", "pw", "HTTPS://SUB.HDD.SB/");

        assert_eq!(account.email, "demo@example.com");
        assert_eq!(account.token_type, "Bearer");
        assert_eq!(account.access_token, "token-a");
    }
}
