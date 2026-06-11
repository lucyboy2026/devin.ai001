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
