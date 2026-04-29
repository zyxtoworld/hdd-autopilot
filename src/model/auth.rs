use serde::{Deserialize, Serialize};

use super::is_false;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionCookie {
    pub name: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub domain: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub expires_at: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub secure: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub http_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AuthSession {
    pub base_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cookies: Vec<SessionCookie>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AuthCache {
    pub email: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub password: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token_type: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cookies: Vec<SessionCookie>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sessions: Vec<AuthSession>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub base_url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accounts: Vec<AuthCache>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoginUser {
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoginResponseData {
    pub access_token: String,
    pub token_type: String,
    pub user: LoginUser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LoginResponse {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub reason: String,
    pub data: LoginResponseData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AuthMeData {
    pub email: String,
    #[serde(default)]
    pub balance: f64,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AuthMeResponse {
    #[serde(default)]
    pub code: i32,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub data: AuthMeData,
}
