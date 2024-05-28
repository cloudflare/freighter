use crate::base64_serde;
use crate::{AuthError, AuthProvider, AuthResult};
use anyhow::Context;
use async_trait::async_trait;
use freighter_api_types::ownership::response::ListedOwner;
use parking_lot::MappedRwLockWriteGuard;
use parking_lot::RwLockWriteGuard;
use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard};
use serde::{Deserialize, Serialize};
use sha2::Sha224; // FIPS 180-4
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::fmt;
use std::io;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use tempfile::NamedTempFile;

/// 28 base64 chars
pub type BareToken = [u8; 21];
const TOKEN_PREFIX: &str = "fr1_";

pub struct FsAuthProvider {
    owners_file_path: PathBuf,
    owners: RwLock<Option<Owners>>,
    /// 24 base64 chars in config
    pepper: [u8; 18],
}

impl FsAuthProvider {
    pub fn new(config: Config) -> AuthResult<Self> {
        std::fs::create_dir_all(&config.auth_path)
            .with_context(|| format!("Auth root at {}", config.auth_path.display()))
            .map_err(AuthError::ServiceError)?;
        let owners_file_path = config.auth_path.join("owners.json");
        Ok(Self {
            pepper: config.auth_tokens_pepper,
            owners_file_path,
            owners: RwLock::default(),
        })
    }

    #[allow(clippy::unused_self)]
    fn random_token(&self) -> AuthResult<BareToken> {
        use rand::Rng;
        let mut token = [0; 21];
        rand::thread_rng().try_fill(&mut token).map_err(|e| AuthError::ServiceError(e.into()))?;
        Ok(token)
    }

    fn token_to_str(&self, bare_token: &BareToken) -> String {
        let mut out = String::with_capacity(4 + bare_token.len() * 8 / 6);
        out.push_str(TOKEN_PREFIX);
        base64_serde::encode(bare_token, &mut out);
        debug_assert!(out.starts_with(TOKEN_PREFIX) && out.len() > bare_token.len());
        debug_assert_eq!(self.token_from_str(&out).expect(&out), self.hash_token(bare_token));
        out
    }

    fn token_from_str(&self, token_str: &str) -> AuthResult<HashedToken> {
        let rest = token_str.strip_prefix(TOKEN_PREFIX).ok_or(AuthError::InvalidCredentials)?;
        Ok(self.hash_token(&base64_serde::decode(rest).ok_or(AuthError::InvalidCredentials)?))
    }

    fn hash_token(&self, bare_token: &BareToken) -> HashedToken {
        use hmac::{Hmac, Mac};
        let mut mac = Hmac::<Sha224>::new_from_slice(&self.pepper).unwrap();
        mac.update(bare_token);
        let hashed = mac.finalize().into_bytes();
        HashedToken(hashed.into())
    }

    fn load_owners_file(&self) -> AuthResult<Owners> {
        if self.owners_file_path.try_exists().context("access to owners dir").map_err(AuthError::ServiceError)? {
            std::fs::File::open(&self.owners_file_path)
                .map(BufReader::new).context("read owners")
                .and_then(|r| serde_json::from_reader(r).context("parse owners"))
                .map_err(AuthError::ServiceError)
        } else {
            Ok(Owners {
                crate_owners: HashMap::default(),
                owner_tokens: HashMap::default(),
                token_owners: HashMap::default(),
            })
        }
    }

    fn owners(&self) -> AuthResult<MappedRwLockReadGuard<Owners>> {
        let mut read_lock = self.owners.read();
        loop {
            if let Ok(loaded) = RwLockReadGuard::try_map(read_lock, |x| x.as_ref()) {
                return Ok(loaded);
            }
            let mut locked = self.owners.write();
            if locked.is_none() {
                *locked = Some(self.load_owners_file()?);
            }
            read_lock = parking_lot::RwLockWriteGuard::downgrade(locked);
        }
    }

    fn owners_mut(&self) -> AuthResult<MappedRwLockWriteGuard<Owners>> {
        RwLockWriteGuard::try_map(self.owners.write(), |x| x.as_mut()).or_else(|mut locked| {
            *locked = Some(self.load_owners_file()?);
            Ok(RwLockWriteGuard::map(locked, |x| x.as_mut().unwrap()))
        })
    }

    #[allow(unknown_lints)]
    #[allow(clippy::needless_pass_by_ref_mut)]
    fn sync_owners(&self, owners: &mut Owners) -> AuthResult<()> {
        fn inner(path: &Path, owners: &Owners) -> io::Result<()> {
            let parent = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
            let mut tmp = NamedTempFile::new_in(parent)?;
            serde_json::to_writer(io::BufWriter::new(tmp.by_ref()), owners)?;
            tmp.persist(path)?;
            Ok(())
        }
        inner(&self.owners_file_path, owners)
            .context("saving owners").map_err(AuthError::ServiceError)
    }

    fn ensure_valid_token(&self, token_str: &str) -> AuthResult<()> {
        let hashed_token = self.token_from_str(token_str)?;
        self.owners()?.login_for_token(&hashed_token)?;
        Ok(())
    }
}

#[derive(Deserialize, Clone)]
pub struct Config {
    pub auth_path: PathBuf,
    #[serde(with = "base64_serde")]
    pub auth_tokens_pepper: [u8; 18],
}

#[async_trait]
impl AuthProvider for FsAuthProvider {
    type Config = Config;

    async fn healthcheck(&self) -> anyhow::Result<()> {
        let _ = self.owners()?;
        Ok(())
    }

    async fn register(&self, username: &str) -> AuthResult<String> {
        let owners = &mut *self.owners_mut()?;
        let bare_token = self.random_token()?;
        let hashed_token = self.hash_token(&bare_token);
        let token_str = self.token_to_str(&bare_token);
        owners.register(username, &hashed_token)?;
        self.sync_owners(owners)?;
        tracing::info!("Registered {username}");
        Ok(token_str)
    }

    async fn list_owners(&self, _owner_list_is_public: &str, crate_name: &str) -> AuthResult<Vec<ListedOwner>> {
        let all_owners = &*self.owners()?;
        let owners = all_owners.crate_owners.get(crate_name).ok_or(AuthError::CrateNotFound)?;
        Ok(owners.iter().map(|login| ListedOwner {
            id: 0,
            login: login.to_string(),
            name: None,
        }).collect())
    }

    async fn add_owners(&self, token_str: &str, users: &[&str], crate_name: &str) -> AuthResult<()> {
        let hashed_token = self.token_from_str(token_str)?;
        let owners = &mut *self.owners_mut()?;
        owners.ensure_authorized_for_crate(&hashed_token, crate_name)?;
        let crate_owners = owners.crate_owners.get_mut(crate_name).ok_or(AuthError::CrateNotFound)?;
        crate_owners.extend(users.iter().map(|&login| login.into()));
        self.sync_owners(owners)?;
        Ok(())
    }

    async fn remove_owners(&self, token_str: &str, users: &[&str], crate_name: &str, ) -> AuthResult<()> {
        let hashed_token = self.token_from_str(token_str)?;
        let owners = &mut *self.owners_mut()?;
        owners.ensure_authorized_for_crate(&hashed_token, crate_name)?;
        let crate_owners = owners.crate_owners.get_mut(crate_name).ok_or(AuthError::CrateNotFound)?;
        for &login in users {
            if crate_owners.len() > 1 {
                crate_owners.remove(login);
            } else {
                self.sync_owners(owners)?;
                return Err(AuthError::Forbidden); // Can't remove all owners
            }
        }
        self.sync_owners(owners)?;
        Ok(())
    }

    async fn publish(&self, token_str: &str, crate_name: &str) -> AuthResult<()> {
        let hashed_token = self.token_from_str(token_str)?;
        let owners = &mut *self.owners_mut()?;

        // If the crate doesn't exist yet, allow anybody to publish
        if owners.crate_owners.get_mut(crate_name).is_none() {
            let login = owners.login_for_token(&hashed_token)?.into();
            owners.crate_owners.insert(crate_name.into(), [login].into_iter().collect());
        }

        owners.ensure_authorized_for_crate(&hashed_token, crate_name)?;
        Ok(())
    }

    async fn auth_yank(&self, token_str: &str, crate_name: &str) -> AuthResult<()> {
        let hashed_token = self.token_from_str(token_str)?;
        self.owners()?.ensure_authorized_for_crate(&hashed_token, crate_name).map(drop)
    }

    /// Fetch of config.json.
    async fn auth_config(&self, token_str: &str) -> AuthResult<()> {
        self.ensure_valid_token(token_str)
    }

    async fn auth_index_fetch(&self, token_str: &str, _all_users_can_read_crates: &str) -> AuthResult<()> {
        self.ensure_valid_token(token_str)
    }

    async fn auth_crate_download(&self, token_str: &str, _all_users_can_read_crates: &str) -> AuthResult<()> {
        self.ensure_valid_token(token_str)
    }

    async fn auth_view_full_index(&self, token_str: &str) -> AuthResult<()> {
        self.ensure_valid_token(token_str)
    }
}

#[derive(Serialize, Deserialize)]
struct Owners {
    token_owners: HashMap<HashedToken, Box<str>>,
    crate_owners: HashMap<Box<str>, BTreeSet<Box<str>>>,

    /// Reverse lookup index
    #[serde(skip, default)]
    owner_tokens: HashMap<Box<str>, HashedToken>,
}

impl Owners {
    pub fn register(&mut self, login: &str, token: &HashedToken) -> AuthResult<()> {
        if self.owner_tokens.is_empty() {
            self.owner_tokens = self.token_owners.iter().map(|(k, v)| (v.clone(), k.clone())).collect();
        }

        if self.owner_tokens.contains_key(login) {
            return Err(AuthError::Forbidden)
        }
        self.owner_tokens.insert(login.into(), token.clone());
        self.token_owners.insert(token.clone(), login.into());
        Ok(())
    }

    pub fn login_for_token(&self, token: &HashedToken) -> AuthResult<&str> {
        self.token_owners.get(token).map(|x| &**x).ok_or(AuthError::InvalidCredentials)
    }

    pub fn ensure_authorized_for_crate(&self, hashed_token: &HashedToken, crate_name: &str) -> AuthResult<()> {
        let owners = self.crate_owners.get(crate_name).ok_or(AuthError::CrateNotFound)?;
        let login = self.login_for_token(hashed_token)?;
        if owners.contains(login) {
            Ok(())
        } else {
            Err(AuthError::Forbidden)
        }
    }
}

/// Because it's hashed, it can have Eq without constant-time comparisons,
/// because attackers control only unhashed token, and won't be able to reliably
/// choose more than a few bytes for an oracle.
/// Additionally we have pepper, and hashtables using randomized siphash.
#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
struct HashedToken(#[serde(with = "base64_serde")] [u8; 28]);

/// Needed for assert
#[cfg(any(test, debug_assertions))]
impl fmt::Debug for HashedToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("HashedToken")
    }
}

#[cfg(test)]
#[tokio::test]
async fn test_fs_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let auth = FsAuthProvider::new(Config { auth_path: dir.path().to_path_buf(), auth_tokens_pepper: [123; 18] }).unwrap();
    let user1 = auth.register("user1").await.unwrap();
    let user2 = auth.register("user2").await.unwrap();
    assert_ne!(user1, user2);
    assert!(matches!(auth.auth_yank(&user1, "crate1").await, Err(AuthError::CrateNotFound)));
    assert!(matches!(auth.auth_yank("badtoken", "crate1").await, Err(AuthError::InvalidCredentials)));
    assert!(matches!(auth.publish("badtoken", "crate1").await, Err(AuthError::InvalidCredentials)));
    auth.publish(&user1, "crate1").await.unwrap();
    assert!(matches!(auth.publish(&user2, "crate1").await, Err(AuthError::Forbidden)));
    auth.auth_yank(&user1, "crate1").await.unwrap();
    auth.add_owners(&user1, &["user2"], "crate1").await.unwrap();
    auth.auth_yank(&user2, "crate1").await.unwrap();
    auth.publish(&user2, "crate1").await.unwrap();

    // reload
    let auth = FsAuthProvider::new(Config { auth_path: dir.path().to_path_buf(), auth_tokens_pepper: [123; 18] }).unwrap();

    assert!(matches!(auth.remove_owners(&user1, &["user1"], "bad_crate").await, Err(AuthError::CrateNotFound)));
    assert!(matches!(auth.auth_yank(&user1, "bad_crate").await, Err(AuthError::CrateNotFound)));
    auth.remove_owners(&user2, &["user1"], "crate1").await.unwrap();
    assert!(matches!(auth.publish(&user1, "crate1").await, Err(AuthError::Forbidden)));
    auth.publish(&user2, "crate1").await.unwrap();
    assert!(matches!(auth.remove_owners(&user1, &["user2"], "crate1").await, Err(AuthError::Forbidden)));
    assert!(matches!(auth.remove_owners(&user1, &["user1"], "crate1").await, Err(AuthError::Forbidden)));

    // change pepper to invalidate all tokens
    let auth = FsAuthProvider::new(Config { auth_path: dir.path().to_path_buf(), auth_tokens_pepper: [99; 18] }).unwrap();
    assert!(matches!(auth.auth_yank(&user2, "crate1").await, Err(AuthError::InvalidCredentials)));
    assert!(matches!(auth.publish(&user2, "crate1").await, Err(AuthError::InvalidCredentials)));
    assert!(matches!(auth.publish(&user1, "crate1").await, Err(AuthError::InvalidCredentials)));
}
