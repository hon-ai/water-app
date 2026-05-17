//! Stable string IDs for Water entities. We use ULIDs everywhere.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(String);

impl Id {
    /// Mint a new ULID-backed Id.
    #[must_use]
    pub fn new() -> Self {
        Self(ulid::Ulid::new().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Id {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id({})", self.0)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Id {
    type Err = crate::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ulid::Ulid::from_string(s)
            .map_err(|e| crate::Error::Other(format!("invalid ulid: {e}")))?;
        Ok(Self(s.to_string()))
    }
}

impl From<Id> for String {
    fn from(id: Id) -> Self {
        id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_id_is_26_chars() {
        let id = Id::new();
        assert_eq!(id.as_str().len(), 26);
    }

    #[test]
    fn id_round_trips_through_from_str() {
        let id = Id::new();
        let parsed: Id = id.as_str().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn invalid_string_rejected() {
        let bad = Id::from_str("not-a-ulid");
        assert!(bad.is_err());
    }

    #[test]
    fn two_ids_are_unique() {
        let a = Id::new();
        let b = Id::new();
        assert_ne!(a, b);
    }
}
