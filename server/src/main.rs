//! Clash Verge 设备绑定两步鉴权 —— Auth Server + 后台管理平台（组件一）。

mod auth;
mod clash;
mod config;
mod db;
mod email;
mod error;
mod models;
mod notify;
mod routes;
mod state;
mod telegram;

use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

use crate::auth::SessionStore;
use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let cfg = Config::from_env();
    tracing::info!("启动配置: bind={} db={}", cfg.bind_addr, cfg.database_url);
    if cfg.smtp.is_none() {
        tracing::warn!("未配置 SMTP，邮件通知将被跳过（仅记录日志）");
    }
    if cfg.telegram.is_none() {
        tracing::warn!("未配置 Telegram，TG 通知/审批将被跳过");
    }
    if cfg.admin_password == "change-me" {
        tracing::warn!("管理员密码为默认值 change-me，请通过 ADMIN_PASSWORD 修改！");
    }

    let pool = db::init_pool(&cfg.database_url).await?;
    let state = AppState {
        pool,
        cfg: Arc::new(cfg.clone()),
        sessions: Arc::new(SessionStore::new()),
    };

    let app = Router::new()
        // 健康检查
        .route("/healthz", get(|| async { "ok" }))
        // 客户端 API
        .route("/register", post(routes::client::register))
        .route("/login", post(routes::client::login))
        .route("/config", get(routes::client::get_config))
        .route("/sub/:key", get(routes::client::get_subscription))
        .route("/auth", post(routes::client::hysteria_auth))
        // Telegram webhook
        .route("/tg/webhook", post(routes::tg::webhook))
        // 后台
        .route(
            "/admin/login",
            get(routes::admin::login_page).post(routes::admin::login_submit),
        )
        .route("/admin/logout", post(routes::admin::logout))
        .route("/admin", get(routes::admin::dashboard))
        .route(
            "/admin/template",
            get(routes::admin::template_page).post(routes::admin::template_submit),
        )
        .route("/admin/users/:id/authorize", post(routes::admin::authorize_user))
        .route("/admin/users/:id/extend", post(routes::admin::extend_user))
        .route("/admin/users/:id/suspend", post(routes::admin::suspend_user))
        .route("/admin/users/:id/activate", post(routes::admin::activate_user))
        .route("/admin/users/:id/reset-devices", post(routes::admin::reset_devices))
        .route("/admin/users/:id/delete", post(routes::admin::delete_user))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    tracing::info!("监听 http://{}", cfg.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
