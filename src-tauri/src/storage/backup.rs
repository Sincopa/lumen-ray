//! Versioned password-encrypted portable backup; DPAPI ciphertext is never exported as a portable key.
use crate::{
    domain::errors::{BackendError, Result},
    storage::repository::Repository,
};
use aes_gcm::{
    aead::{rand_core::RngCore, Aead, AeadCore, KeyInit, OsRng, Payload},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use zeroize::Zeroizing;
const MAGIC: &[u8] = b"LUMEN-BACKUP-1\0";
const MAX: usize = 16 * 1024 * 1024;
fn error() -> BackendError {
    BackendError::new("BACKUP_INVALID", "Invalid backup or incorrect password")
}
fn key(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    if password.chars().count() < 12 || password.len() > 1024 {
        return Err(BackendError::new(
            "BACKUP_PASSWORD",
            "Use a password of at least 12 characters",
        ));
    }
    let params = argon2::Params::new(65536, 3, 1, Some(32)).map_err(|_| error())?;
    let mut key = Zeroizing::new([0; 32]);
    argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, &mut *key)
        .map_err(|_| error())?;
    Ok(key)
}
pub fn encrypt(repo: &Repository, password: &str) -> Result<String> {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let key = key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&*key).map_err(|_| error())?;
    drop(key);
    let bytes = Zeroizing::new(serde_json::to_vec(repo).map_err(|_| error())?);
    if bytes.len() > MAX {
        return Err(error());
    }
    let result = cipher.encrypt(
        &nonce,
        Payload {
            msg: &bytes,
            aad: MAGIC,
        },
    );
    drop(bytes);
    let encrypted = result.map_err(|_| error())?;
    let mut envelope = MAGIC.to_vec();
    envelope.extend(salt);
    envelope.extend(nonce);
    envelope.extend(encrypted);
    Ok(STANDARD.encode(envelope))
}
pub fn decrypt(input: &str, password: &str) -> Result<Repository> {
    if input.len() > MAX * 2 {
        return Err(error());
    }
    let bytes = STANDARD.decode(input.trim()).map_err(|_| error())?;
    if !bytes.starts_with(MAGIC) || bytes.len() < MAGIC.len() + 44 || bytes.len() > MAX + 128 {
        return Err(error());
    }
    let start = MAGIC.len();
    let key = key(password, &bytes[start..start + 16])?;
    let cipher = Aes256Gcm::new_from_slice(&*key).map_err(|_| error())?;
    drop(key);
    let plain = Zeroizing::new(
        cipher
            .decrypt(
                Nonce::from_slice(&bytes[start + 16..start + 28]),
                Payload {
                    msg: &bytes[start + 28..],
                    aad: MAGIC,
                },
            )
            .map_err(|_| error())?,
    );
    let repo = serde_json::from_slice(&plain).map_err(|_| error());
    drop(plain);
    let repo: Repository = repo?;
    if repo.version != 1 || repo.nodes.len() > 10000 || repo.rules.len() > 10000 {
        return Err(error());
    }
    crate::commands::settings::validate_settings(&repo.settings)?;
    let ids: std::collections::HashSet<_> = repo.nodes.iter().map(|n| n.id.as_str()).collect();
    let subscriptions: std::collections::HashSet<_> =
        repo.subscriptions.iter().map(|s| s.id.as_str()).collect();
    let rule_ids: std::collections::HashSet<_> = repo.rules.iter().map(|r| r.id.as_str()).collect();
    if ids.len() != repo.nodes.len()
        || subscriptions.len() != repo.subscriptions.len()
        || rule_ids.len() != repo.rules.len()
        || repo
            .selected_server_id
            .as_ref()
            .is_some_and(|id| !ids.contains(id.as_str()))
        || repo
            .selected_subscription_id
            .as_ref()
            .is_some_and(|id| !subscriptions.contains(id.as_str()))
        || repo
            .nodes
            .iter()
            .any(|node| !subscriptions.contains(node.subscription_id.as_str()))
        || repo.rules.iter().any(|rule| {
            rule.name.is_empty()
                || rule.name.len() > 1024
                || (!matches!(rule.server_id.as_str(), "direct" | "block" | "subscription")
                    && !ids.contains(rule.server_id.as_str())
                    && !subscriptions.contains(rule.server_id.as_str()))
        })
    {
        return Err(error());
    }
    for node in &repo.nodes {
        node.validate()?;
    }
    if !["global", "rules"].contains(&repo.routing_mode.as_str())
        || !["tun", "proxy"].contains(&repo.settings.network_mode.as_str())
    {
        return Err(error());
    }
    Ok(repo)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authenticated_roundtrip_and_tampering() {
        let repo = Repository::default();
        let password = "correct horse battery";
        let encrypted = encrypt(&repo, password).unwrap();
        assert_eq!(decrypt(&encrypted, password).unwrap().version, 1);
        assert!(decrypt(&encrypted, "different password").is_err());
        let mut bytes = STANDARD.decode(encrypted).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert!(decrypt(&STANDARD.encode(bytes), password).is_err());
    }
}
