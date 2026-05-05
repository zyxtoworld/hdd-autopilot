use std::collections::HashMap;

use crate::model::{AuthCache, AuthConfig, AuthSession, LoginResponse};
use url::Url;

pub fn normalize_token_type(token_type: &str) -> String {
    let token_type = token_type.trim();
    if token_type.is_empty() || token_type.eq_ignore_ascii_case("bearer") {
        return "Bearer".to_string();
    }
    token_type.to_string()
}

pub fn normalize_base_url(base_url: &str) -> String {
    let base_url = base_url.trim();
    if base_url.is_empty() {
        return String::new();
    }
    let Ok(mut parsed) = Url::parse(base_url) else {
        return base_url.trim_end_matches('/').to_string();
    };
    if parsed.scheme().is_empty() || parsed.host_str().is_none() {
        return base_url.trim_end_matches('/').to_string();
    }
    let scheme = parsed.scheme().to_ascii_lowercase();
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    let path = parsed.path().trim_end_matches('/').to_string();
    let _ = parsed.set_scheme(&scheme);
    let _ = parsed.set_host(Some(&host));
    parsed.set_query(None);
    parsed.set_fragment(None);
    parsed.set_path(&path);
    parsed.to_string().trim_end_matches('/').to_string()
}

pub fn build_authorization(token_type: &str, access_token: &str) -> String {
    let access_token = access_token.trim();
    if access_token.is_empty() {
        return String::new();
    }
    format!("{} {}", normalize_token_type(token_type), access_token)
}

pub fn cache_usable(cache: &AuthCache) -> bool {
    !build_authorization(&cache.token_type, &cache.access_token).is_empty()
}

pub fn password_usable(cache: &AuthCache) -> bool {
    !cache.email.trim().is_empty() && !cache.password.trim().is_empty()
}

pub fn cache_from_login(
    login_response: &LoginResponse,
    fallback_email: &str,
    password: &str,
    base_url: &str,
) -> AuthCache {
    let mut email = login_response.data.user.email.trim().to_string();
    if email.is_empty() {
        email = fallback_email.trim().to_string();
    }
    let session = AuthSession {
        base_url: normalize_base_url(base_url),
        token_type: normalize_token_type(&login_response.data.token_type),
        access_token: login_response.data.access_token.trim().to_string(),
    };
    let mut account = AuthCache {
        email,
        password: password.trim().to_string(),
        token_type: session.token_type.clone(),
        access_token: session.access_token.clone(),
        ..AuthCache::default()
    };
    if session_usable(&session) {
        account.sessions = vec![session];
    }
    normalize_account(account)
}

fn session_usable(session: &AuthSession) -> bool {
    !build_authorization(&session.token_type, &session.access_token).is_empty()
}

fn normalize_session(mut session: AuthSession) -> AuthSession {
    session.base_url = normalize_base_url(&session.base_url);
    session.token_type = normalize_token_type(&session.token_type);
    session.access_token = session.access_token.trim().to_string();
    session
}

fn normalize_sessions(sessions: Vec<AuthSession>) -> Vec<AuthSession> {
    let mut result = Vec::with_capacity(sessions.len());
    let mut index_by_base_url = HashMap::<String, usize>::with_capacity(sessions.len());
    for session in sessions.into_iter().map(normalize_session) {
        if session.base_url.is_empty() || !session_usable(&session) {
            continue;
        }
        if let Some(index) = index_by_base_url.get(&session.base_url).copied() {
            result[index] = session;
        } else {
            index_by_base_url.insert(session.base_url.clone(), result.len());
            result.push(session);
        }
    }
    result
}

pub(super) fn normalize_account(mut account: AuthCache) -> AuthCache {
    account.email = account.email.trim().to_string();
    account.password = account.password.trim().to_string();
    account.token_type = normalize_token_type(&account.token_type);
    account.access_token = account.access_token.trim().to_string();
    account.sessions = normalize_sessions(account.sessions);
    account
}

pub(super) fn find_legacy_session(account: &AuthCache, base_url: &str) -> Option<AuthSession> {
    let account = normalize_account(account.clone());
    if let Some(index) = find_session(&account.sessions, base_url) {
        return Some(account.sessions[index].clone());
    }
    account.sessions.into_iter().next()
}

pub(super) fn normalize_account_for_base_url(account: AuthCache, base_url: &str) -> AuthCache {
    let mut account = normalize_account(account);
    if !cache_usable(&account)
        && let Some(session) = find_legacy_session(&account, base_url)
    {
        if account.token_type.is_empty() {
            account.token_type = session.token_type.clone();
        }
        if account.access_token.is_empty() {
            account.access_token = session.access_token.clone();
        }
    }
    account.sessions.clear();
    normalize_account(account)
}

fn normalize_accounts_for_base_url(accounts: Vec<AuthCache>, base_url: &str) -> Vec<AuthCache> {
    accounts
        .into_iter()
        .map(|account| normalize_account_for_base_url(account, base_url))
        .filter(|account| !account.email.is_empty())
        .collect()
}

pub(super) fn normalize_config(mut config: AuthConfig) -> AuthConfig {
    config.base_url = normalize_base_url(&config.base_url);
    if config.base_url.is_empty() {
        for account in &config.accounts {
            if let Some(session) = find_legacy_session(account, "") {
                config.base_url = session.base_url;
                break;
            }
        }
    }
    config.accounts = normalize_accounts_for_base_url(config.accounts, &config.base_url);
    config
}

pub fn find_account(accounts: &[AuthCache], email: &str) -> Option<usize> {
    let email = email.trim();
    accounts
        .iter()
        .position(|account| account.email.trim().eq_ignore_ascii_case(email))
}

pub fn find_session(sessions: &[AuthSession], base_url: &str) -> Option<usize> {
    let base_url = normalize_base_url(base_url);
    sessions
        .iter()
        .position(|session| normalize_base_url(&session.base_url) == base_url)
}

pub fn get_session(account: &AuthCache, base_url: &str) -> Option<AuthSession> {
    let account = normalize_account_for_base_url(account.clone(), base_url);
    if !cache_usable(&account) {
        return None;
    }
    Some(AuthSession {
        base_url: normalize_base_url(base_url),
        token_type: account.token_type,
        access_token: account.access_token,
    })
}

pub fn upsert_session(account: AuthCache, session: AuthSession) -> AuthCache {
    let mut account = normalize_account(account);
    let session = normalize_session(session);
    if !session_usable(&session) {
        account.token_type.clear();
        account.access_token.clear();
        account.sessions.clear();
        return normalize_account(account);
    }
    account.token_type = session.token_type;
    account.access_token = session.access_token;
    account.sessions.clear();
    normalize_account(account)
}

pub fn upsert_account(mut config: AuthConfig, account: AuthCache) -> AuthConfig {
    if config.base_url.is_empty()
        && let Some(session) = find_legacy_session(&account, "")
    {
        config.base_url = session.base_url;
    }
    config = normalize_config(config);
    let account = normalize_account_for_base_url(account, &config.base_url);
    if account.email.is_empty() {
        return config;
    }
    if let Some(index) = find_account(&config.accounts, &account.email) {
        let mut merged = config.accounts[index].clone();
        let usable = cache_usable(&account);
        merged.email = account.email.clone();
        if !account.password.is_empty() {
            merged.password = account.password.clone();
        }
        if usable {
            merged.token_type = account.token_type.clone();
            merged.access_token = account.access_token.clone();
        }
        config.accounts[index] = normalize_account_for_base_url(merged, &config.base_url);
    } else {
        config.accounts.push(account);
    }
    normalize_config(config)
}

pub(super) fn compact_config_for_save(mut config: AuthConfig) -> AuthConfig {
    config = normalize_config(config);
    config.accounts = config
        .accounts
        .into_iter()
        .map(|account| compact_account_for_save(account, &config.base_url))
        .collect();
    config
}

fn compact_account_for_save(account: AuthCache, base_url: &str) -> AuthCache {
    let mut account = normalize_account_for_base_url(account, base_url);
    account.token_type = normalize_token_type(&account.token_type);
    account.sessions.clear();
    account
}
