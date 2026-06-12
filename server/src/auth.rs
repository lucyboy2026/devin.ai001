//! 密码哈希、设备 Token 生成、管理员会话。

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use chrono::{Duration, Utc};
use rand::RngCore;
use std::collections::HashMap;
use std::sync::Mutex;

/// 对密码做 Argon2 哈希。
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash 失败: {e}"))?
        .to_string();
    Ok(hash)
}

/// 校验密码。
pub fn verify_password(password: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    }
}

/// 生成 64 位（hex）设备 Token。
pub fn gen_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// 管理员会话存储（内存级，单进程足够）。
#[derive(Default)]
pub struct SessionStore {
    inner: Mutex<HashMap<String, chrono::DateTime<Utc>>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// 新建会话，返回 session id；有效期 7 天。
    pub fn create(&self) -> String {
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        let sid = hex::encode(bytes);
        let exp = Utc::now() + Duration::days(7);
        self.inner.lock().unwrap().insert(sid.clone(), exp);
        sid
    }

    /// 校验会话是否有效。
    pub fn validate(&self, sid: &str) -> bool {
        let mut guard = self.inner.lock().unwrap();
        match guard.get(sid) {
            Some(exp) if *exp > Utc::now() => true,
            Some(_) => {
                guard.remove(sid);
                false
            }
            None => false,
        }
    }

    pub fn destroy(&self, sid: &str) {
        self.inner.lock().unwrap().remove(sid);
    }
}
