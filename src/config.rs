use std::net::SocketAddr;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub jwt_secret: String,
    pub cors_origin: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let bind_addr: SocketAddr = bind_addr
            .parse()
            .map_err(|e| format!("Invalid BIND_ADDR: {e}"))?;

        let database_url =
            std::env::var("DATABASE_URL").map_err(|_| "Missing DATABASE_URL".to_string())?;
        let jwt_secret =
            std::env::var("JWT_SECRET").map_err(|_| "Missing JWT_SECRET".to_string())?;
        let cors_origin = std::env::var("CORS_ORIGIN").ok();

        Ok(Self {
            bind_addr,
            database_url,
            jwt_secret,
            cors_origin,
        })
    }
}
