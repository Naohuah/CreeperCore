//! Offline account support and future account-provider seam.
//!
//! The local profile UUID mirrors Java's `UUID.nameUUIDFromBytes` so offline
//! servers recognise the same player identity as the vanilla launcher.

use crate::model::AccountProfile;
use anyhow::{anyhow, Result};
use chrono::Utc;
use md5::{Digest, Md5};
use uuid::Uuid;

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn validate_username(username: &str) -> Result<()> {
    let len = username.chars().count();
    if !(3..=16).contains(&len) {
        return Err(anyhow!("玩家名需要 3–16 个字符"));
    }
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(anyhow!("玩家名仅支持英文字母、数字和下划线"));
    }
    Ok(())
}

/// Java's offline UUID:
/// `UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(UTF_8))`
pub fn offline_uuid(username: &str) -> String {
    let digest = Md5::digest(format!("OfflinePlayer:{username}").as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x30; // version 3
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // IETF variant
    Uuid::from_bytes(bytes).to_string()
}

pub fn make_offline_account(username: &str) -> Result<AccountProfile> {
    validate_username(username)?;
    let now = now_iso();
    Ok(AccountProfile {
        id: Uuid::new_v4().to_string(),
        username: username.to_string(),
        uuid: offline_uuid(username),
        kind: "offline".to_string(),
        created_at: now,
        last_used: None,
        access_token: None,
        refresh_token: None,
        expires_at: None,
    })
}

pub fn compact_uuid(uuid: &str) -> String {
    uuid.replace('-', "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_java_for_notch() {
        // Known value produced by Java's UUID.nameUUIDFromBytes for OfflinePlayer:Notch.
        assert_eq!(
            offline_uuid("Notch"),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
    }

    #[test]
    fn validates_names() {
        assert!(validate_username("Steve").is_ok());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("玩家一号").is_err());
        assert!(validate_username("bad name").is_err());
    }

    #[test]
    fn compact_uuid_has_no_dashes() {
        assert!(!compact_uuid(&offline_uuid("Alex")).contains('-'));
    }
}
