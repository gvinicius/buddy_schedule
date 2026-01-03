use buddy_schedule_api::{config::Config, repo::PgRepo, AppState};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), String> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = Config::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&cfg.database_url)
        .await
        .map_err(|e| format!("Failed to connect to Postgres: {e}"))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| format!("Failed to run migrations: {e}"))?;

    let state = AppState {
        repo: Arc::new(PgRepo::new(pool)),
        jwt: buddy_schedule_api::auth::JwtKeys::new(&cfg.jwt_secret),
        cors_origin: cfg.cors_origin.clone(),
    };

    let app = buddy_schedule_api::build_router(state);
    let listener = tokio::net::TcpListener::bind(cfg.bind_addr)
        .await
        .map_err(|e| format!("Failed to bind {}: {e}", cfg.bind_addr))?;

    tracing::info!("Listening on http://{}", cfg.bind_addr);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| format!("Server error: {e}"))?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("Shutdown signal received");
}
