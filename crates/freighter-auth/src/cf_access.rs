use crate::{AuthError, AuthResult};
use anyhow::bail;
use jsonwebtoken::jwk::{JwkSet, PublicKeyUse};
use jsonwebtoken::{DecodingKey, Validation};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Check for new keys (Cloudflare gives them 4h max-age)
const REFRESH_DURATION: Duration = Duration::from_secs(3600);

/// Cloudflare Access JWT verifier
pub struct CfAccess {
    jwks_url: String,
    validation: Validation,
    key_set: RwLock<KeySet>,
}

/// Allowed keys
struct KeySet {
    next_fetch: Instant,
    keys: HashMap<String, DecodingKey>,
}

/// Claims in the token
#[derive(serde::Deserialize)]
struct Claims {
    common_name: Option<String>,
    sub: Option<String>,
}

/// User from verified token
#[derive(Debug, Eq, PartialEq)]
pub struct UserId(pub String);

impl UserId {
    /// Used service auth token to authenticate
    pub fn is_service_token(&self) -> bool {
        // it's checked to match claims.sub when jwt is validated
        self.0.ends_with(".access")
    }
}

impl CfAccess {
    /// Team base URL must start with `https://`
    pub fn new(team_base_url: &str, audience: &str) -> Result<Self, anyhow::Error> {
        if team_base_url.len() < 13 || !team_base_url.starts_with("https://") || audience.is_empty()
        {
            bail!("invalid cf-access config")
        }
        let jwks_url = format!(
            "{}/cdn-cgi/access/certs",
            team_base_url.trim_end_matches('/')
        );

        let mut validation = Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_audience(&[audience]);

        Ok(Self {
            jwks_url,
            validation,
            key_set: RwLock::new(KeySet {
                next_fetch: Instant::now(),
                keys: HashMap::default(),
            }),
        })
    }

    /// Download new keys
    pub async fn refresh(&self) -> Result<(), anyhow::Error> {
        let mut locked_keys = self.key_set.write().await;
        let now = Instant::now();
        if locked_keys.next_fetch > now {
            if locked_keys.keys.is_empty() {
                anyhow::bail!("no usable keys");
            }
            return Ok(());
        }

        locked_keys.next_fetch = now + Duration::from_secs(1); // in case of failure, retry 1/s
        let set: JwkSet = async {
            reqwest::get(&self.jwks_url)
                .await?
                .error_for_status()?
                .json()
                .await
        }
        .await
        .inspect_err(|e| tracing::error!("{}: {e}", self.jwks_url))?;
        locked_keys.keys = set
            .keys
            .into_iter()
            .filter(|k| {
                k.common
                    .public_key_use
                    .as_ref()
                    .is_some_and(|s| *s == PublicKeyUse::Signature)
            })
            .filter_map(|k| {
                let key = DecodingKey::from_jwk(&k)
                    .inspect_err(|e| tracing::error!("{k:?}: {e}"))
                    .ok()?;
                let kid = k.common.key_id?;
                Some((kid, key))
            })
            .collect();
        if locked_keys.keys.is_empty() {
            tracing::error!("no usable keys");
            anyhow::bail!("no usable keys");
        }
        locked_keys.next_fetch = Instant::now() + REFRESH_DURATION;
        Ok(())
    }

    /// Returns a user ID
    pub async fn validated_user_id(&self, token: &str) -> AuthResult<UserId> {
        let key_id = jsonwebtoken::decode_header(token)
            .map_err(|e| {
                tracing::warn!("bad token: {token}: {e}");
                AuthError::InvalidCredentials
            })?
            .kid
            .ok_or(AuthError::InvalidCredentials)?;

        let locked_keys = loop {
            let tmp = self.key_set.read().await;
            if tmp.next_fetch < Instant::now() {
                drop(tmp);
                self.refresh().await?;
                continue;
            }
            break tmp;
        };

        let Some(key) = locked_keys.keys.get(key_id.as_str()) else {
            tracing::warn!("token for an unknown key: {token}: {key_id}");
            return Err(AuthError::InvalidCredentials);
        };

        let claims = jsonwebtoken::decode::<Claims>(token, key, &self.validation)
            .map_err(|e| {
                tracing::warn!("unauthorized: {token}: {e}");
                AuthError::Unauthorized
            })?
            .claims;

        let sub = claims.sub.filter(|s| !s.is_empty());
        let sub_was_empty = sub.is_none();

        let user_id = UserId(
            sub.or(claims.common_name)
                .ok_or_else(|| anyhow::anyhow!("empty claims.sub"))?,
        );

        // Service Token gets an empty string in `sub`!
        if user_id.is_service_token() != sub_was_empty {
            return Err(anyhow::anyhow!("claims.sub doesn't match claims.common_name service token pattern").into());
        }

        Ok(user_id)
    }
}

#[cfg(test)]
#[tokio::test]
#[ignore]
async fn cf_access_token_test() {
    // curl -sI -H "CF-Access-Client-Id: ….access" -H "CF-Access-Client-Secret: …" https://access.example.com | egrep -Eo 'CF_Authorization=[^;]+
    let token = "…"; // Needs non-expired token ;(
    let a = CfAccess::new(
        "https://cf-rust.cloudflareaccess.com",
        "1de8297ce3d45d1962a73a04fcef47b434d95f0ad2134d4d5bd9876086695262",
    )
    .unwrap();
    let user = a.validated_user_id(token).await.unwrap();
    assert!(user.is_service_token());
}
