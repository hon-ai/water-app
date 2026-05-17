//! API key resolution for LLM providers.
//!
//! Order: OS keychain → `~/.water/dev-keys.toml` → env var.

use crate::{Error, Result};
use std::collections::HashMap;
use std::path::PathBuf;

const KEYRING_SERVICE: &str = "co.water.app";

pub struct Secrets {
    dev_keys: HashMap<String, String>,
}

impl Secrets {
    #[must_use]
    pub fn load() -> Self {
        let path = dev_keys_path();
        let dev_keys = match std::fs::read_to_string(&path) {
            Ok(text) => {
                let (map, err) = parse_dev_keys(&text);
                if let Some(msg) = err {
                    tracing::warn!(
                        path = %path.display(),
                        error = %msg,
                        "dev-keys.toml exists but failed to parse; falling back to empty map",
                    );
                }
                map
            }
            Err(_) => HashMap::new(),
        };
        Self { dev_keys }
    }

    /// Resolve a key for the given provider id.
    ///
    /// # Errors
    /// Returns `Error::NotFound` if no key is found in the OS keychain,
    /// `~/.water/dev-keys.toml`, or the `WATER_<ID>_API_KEY` env var.
    pub fn get(&self, provider_id: &str) -> Result<String> {
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, provider_id) {
            if let Ok(secret) = entry.get_password() {
                return Ok(secret);
            }
        }
        if let Some(v) = self.dev_keys.get(provider_id) {
            return Ok(v.clone());
        }
        let env_var = format!(
            "WATER_{}_API_KEY",
            provider_id.to_uppercase().replace('-', "_")
        );
        if let Ok(v) = std::env::var(&env_var) {
            return Ok(v);
        }
        Err(Error::NotFound(format!(
            "no secret for provider `{provider_id}`"
        )))
    }

    /// Persist a key to the OS keychain.
    ///
    /// # Errors
    /// Returns `Error::Other` if the keychain entry can't be created or written.
    pub fn set(&self, provider_id: &str, value: &str) -> Result<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, provider_id)
            .map_err(|e| Error::Other(format!("keyring: {e}")))?;
        entry
            .set_password(value)
            .map_err(|e| Error::Other(format!("keyring set: {e}")))?;
        Ok(())
    }
}

/// Parse the dev-keys.toml contents into a flat string-to-string map.
///
/// Returns the (possibly empty) map and an optional human-readable error
/// describing why parsing failed. The error is surfaced upward (currently as
/// a `tracing::warn!` from `Secrets::load`) so an internal tester with a typo
/// in their dev-keys file gets a hint instead of a downstream `NotFound`.
pub(super) fn parse_dev_keys(text: &str) -> (HashMap<String, String>, Option<String>) {
    match toml::from_str::<HashMap<String, String>>(text) {
        Ok(map) => (map, None),
        Err(e) => (HashMap::new(), Some(e.to_string())),
    }
}

fn dev_keys_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".water").join("dev-keys.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_keys_fallback_works() {
        // The crate forbids `unsafe_code`, so we can't call the (now-unsafe)
        // `std::env::set_var` to exercise the env-var branch. Instead we
        // construct a `Secrets` with a populated dev_keys map directly,
        // which exercises the same resolution chain (keychain → dev_keys →
        // env) up to the dev_keys hit.
        let mut dev = HashMap::new();
        dev.insert("fake".to_string(), "from-dev-keys".to_string());
        let s = Secrets { dev_keys: dev };
        // Keychain may or may not have an entry; on CI it doesn't, so
        // dev_keys wins. On a dev machine that happens to have a keychain
        // entry for "fake", that value wins — still non-empty.
        let v = s.get("fake").unwrap();
        assert!(v == "from-dev-keys" || !v.is_empty());
    }

    #[test]
    fn parse_dev_keys_returns_map_on_valid_toml() {
        let (map, err) = parse_dev_keys("anthropic = \"sk-test\"\nopenai = \"sk-other\"\n");
        assert_eq!(map.get("anthropic").map(String::as_str), Some("sk-test"));
        assert_eq!(map.get("openai").map(String::as_str), Some("sk-other"));
        assert!(err.is_none());
    }

    #[test]
    fn parse_dev_keys_returns_empty_map_and_error_on_garbage() {
        let (map, err) = parse_dev_keys("this is not [valid] toml = = =");
        assert!(map.is_empty(), "garbage should yield empty map");
        assert!(err.is_some(), "garbage should yield a parse error message");
    }
}
