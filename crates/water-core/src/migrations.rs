//! Migration list for the Water project database.
//!
//! Each migration is `(version, up_sql)`. Migrations run in order under a
//! single transaction. We never drop columns in the same migration that
//! adds them; we never break forward compatibility within a major.

use rusqlite_migration::{Migrations, M};

#[must_use]
pub fn all() -> Migrations<'static> {
    Migrations::new(vec![M::up(V1_INIT)])
}

const V1_INIT: &str = include_str!("../sql/v1_init.sql");
