use crate::error::{AppError, AppResult};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone)]
pub struct JwtKeys {
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
}

impl JwtKeys {
    pub fn new(secret: &str) -> Self {
        Self {
            encoding: EncodingKey::from_secret(secret.as_bytes()),
            decoding: DecodingKey::from_secret(secret.as_bytes()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub is_superadmin: bool,
}

pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AppError::Internal)?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, password_hash: &str) -> AppResult<bool> {
    let parsed = PasswordHash::new(password_hash).map_err(|_| AppError::Internal)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub fn issue_jwt(user_id: Uuid, is_superadmin: bool, keys: &JwtKeys) -> AppResult<String> {
    let exp = (Utc::now() + Duration::hours(24)).timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        exp,
        is_superadmin,
    };
    jsonwebtoken::encode(&Header::default(), &claims, &keys.encoding)
        .map_err(|_| AppError::Internal)
}

pub fn decode_jwt(token: &str, keys: &JwtKeys) -> AppResult<Claims> {
    let data = jsonwebtoken::decode::<Claims>(token, &keys.decoding, &Validation::default())
        .map_err(|_| AppError::Unauthorized)?;
    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_roundtrip() {
        let hash = hash_password("secret123").unwrap();
        assert!(verify_password("secret123", &hash).unwrap());
        assert!(!verify_password("wrong", &hash).unwrap());
    }

    #[test]
    fn jwt_roundtrip() {
        let keys = JwtKeys::new("dev-secret");
        let uid = Uuid::new_v4();
        let token = issue_jwt(uid, true, &keys).unwrap();
        let claims = decode_jwt(&token, &keys).unwrap();
        assert_eq!(claims.sub, uid.to_string());
        assert!(claims.is_superadmin);
    }
}
