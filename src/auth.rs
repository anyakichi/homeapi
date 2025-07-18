use std::collections::HashMap;

use anyhow::Result;
use axum::{
    extract::{FromRequestParts, State},
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::dynamodb::Client;
use crate::models::User;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    #[serde(default)]
    pub name: String,
    pub exp: usize,
    pub aud: Option<String>,
    pub iss: Option<String>,
}

#[derive(Clone)]
pub struct AuthUser {
    pub claims: Claims,
}

// Cache for Google's public keys
static GOOGLE_KEYS_CACHE: tokio::sync::OnceCell<RwLock<HashMap<String, jwk::Jwk>>> =
    tokio::sync::OnceCell::const_new();

async fn fetch_google_keys() -> Result<HashMap<String, jwk::Jwk>> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://www.googleapis.com/oauth2/v3/certs")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch Google keys: {}", e))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!("Failed to fetch Google keys"));
    }

    let jwks: jwk::JwkSet = response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse JWK set: {}", e))?;

    let mut keys = HashMap::new();
    for jwk in jwks.keys {
        if let Some(kid) = &jwk.common.key_id {
            keys.insert(kid.clone(), jwk);
        }
    }

    Ok(keys)
}

async fn get_google_keys() -> &'static RwLock<HashMap<String, jwk::Jwk>> {
    GOOGLE_KEYS_CACHE
        .get_or_init(|| async {
            let keys = fetch_google_keys().await.unwrap_or_default();
            RwLock::new(keys)
        })
        .await
}

async fn verify_google_token(token: &str, expected_aud: &str) -> Result<Claims> {
    // Decode the header to get the key ID
    let header = decode_header(token)
        .map_err(|e| anyhow::anyhow!("Failed to decode token header: {}", e))?;

    let kid = header
        .kid
        .ok_or_else(|| anyhow::anyhow!("Missing key ID in token header"))?;

    // Set up validation
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[expected_aud]);
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);

    let keys_lock = get_google_keys().await;

    // Helper function to try verification with current keys
    let try_verify = || async {
        let keys = keys_lock.read().await;
        if let Some(jwk) = keys.get(&kid) {
            let decoding_key = DecodingKey::from_jwk(jwk)
                .map_err(|e| anyhow::anyhow!("Failed to create decoding key: {}", e))?;
            let token_data = decode::<Claims>(token, &decoding_key, &validation)
                .map_err(|e| anyhow::anyhow!("Failed to verify token: {}", e))?;
            Ok(token_data.claims)
        } else {
            Err(anyhow::anyhow!("Key not found: {}", kid))
        }
    };

    // Try with cached keys first
    if let Ok(claims) = try_verify().await {
        return Ok(claims);
    }

    // Refresh keys and try again
    {
        let mut keys = keys_lock.write().await;
        *keys = fetch_google_keys().await?;
    }

    try_verify().await
}

pub async fn auth_middleware(
    State(dynamodb): State<Client>,
    mut req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    let expected_aud = match std::env::var("GOOGLE_CLIENT_ID") {
        Ok(aud) => aud,
        Err(_) => {
            eprintln!("Error: GOOGLE_CLIENT_ID environment variable not set");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Try to get token from Authorization header
    if let Some(auth_header) = req.headers().get(AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                // Verify Google ID token
                match verify_google_token(token, &expected_aud).await {
                    Ok(claims) => {
                        // Check if user exists in database
                        match dynamodb
                            .get_item::<User>(claims.email.clone(), "USER".to_string())
                            .await
                        {
                            Ok(_user) => {
                                req.extensions_mut().insert(AuthUser { claims });
                                return Ok(next.run(req).await);
                            }
                            Err(e) => {
                                eprintln!("Error checking user in database: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Token verification failed: {e}");
                    }
                }
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}
