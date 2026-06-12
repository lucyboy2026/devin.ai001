//! 数据模型与查询辅助。

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: i64,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub status: String,
    pub max_devices: i64,
    pub expires_at: Option<String>,
    pub note: Option<String>,
    #[serde(skip_serializing)]
    pub subscription_key: Option<String>,
    pub created_at: String,
    pub authorized_at: Option<String>,
}

impl User {
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    pub fn is_expired(&self) -> bool {
        match self.expires_at.as_deref().and_then(parse_dt) {
            Some(exp) => Utc::now() >= exp,
            None => false,
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Device {
    pub id: i64,
    pub user_id: i64,
    pub device_fp: String,
    pub platform: Option<String>,
    #[serde(skip_serializing)]
    #[allow(dead_code)] // 由 SQL 按 token 查询使用，结构体字段本身不直接读取
    pub token: Option<String>,
    pub token_expires_at: Option<String>,
    pub revoked: i64,
    pub created_at: String,
    pub last_seen_at: Option<String>,
}

pub fn parse_dt(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}

pub async fn find_user_by_email(pool: &SqlitePool, email: &str) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
        .bind(email)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

pub async fn find_user_by_id(pool: &SqlitePool, id: i64) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

pub async fn find_user_by_subscription_key(pool: &SqlitePool, key: &str) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE subscription_key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(user)
}

/// 返回该用户已绑定的、最近活跃且未过期的设备 Token（用于固定订阅链接注入）。
pub async fn latest_active_token(pool: &SqlitePool, user_id: i64) -> Result<Option<String>> {
    let now = Utc::now().to_rfc3339();
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT token FROM devices
         WHERE user_id = ? AND revoked = 0 AND token IS NOT NULL
           AND (token_expires_at IS NULL OR token_expires_at > ?)
         ORDER BY last_seen_at DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(&now)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

/// 确保用户拥有固定订阅密钥；缺失时生成并持久化，返回该密钥。
pub async fn ensure_subscription_key(pool: &SqlitePool, user_id: i64) -> Result<String> {
    if let Some(user) = find_user_by_id(pool, user_id).await? {
        if let Some(key) = user.subscription_key.filter(|s| !s.is_empty()) {
            return Ok(key);
        }
    }
    let key = crate::auth::gen_token();
    sqlx::query("UPDATE users SET subscription_key = ? WHERE id = ?")
        .bind(&key)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(key)
}

pub async fn list_users(pool: &SqlitePool) -> Result<Vec<User>> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(users)
}

pub async fn count_user_devices(pool: &SqlitePool, user_id: i64) -> Result<i64> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM devices WHERE user_id = ? AND revoked = 0")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    Ok(row.0)
}

pub async fn list_user_devices(pool: &SqlitePool, user_id: i64) -> Result<Vec<Device>> {
    let devices = sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE user_id = ? ORDER BY created_at")
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    Ok(devices)
}

pub async fn find_device(pool: &SqlitePool, user_id: i64, device_fp: &str) -> Result<Option<Device>> {
    let device = sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE user_id = ? AND device_fp = ?")
        .bind(user_id)
        .bind(device_fp)
        .fetch_optional(pool)
        .await?;
    Ok(device)
}

pub async fn find_device_by_token(pool: &SqlitePool, token: &str) -> Result<Option<Device>> {
    let device = sqlx::query_as::<_, Device>("SELECT * FROM devices WHERE token = ? AND revoked = 0")
        .bind(token)
        .fetch_optional(pool)
        .await?;
    Ok(device)
}
