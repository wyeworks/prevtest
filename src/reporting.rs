//! Nextest wrapper reporting.

use std::{fs, path::Path};

pub enum Outcome {
    Cached,
    CacheMiss,
    Forced,
    CacheUnavailable,
    CacheUpdateFailed,
}

pub fn write(path: Option<&Path>, outcome: Outcome) {
    let Some(path) = path else {
        return;
    };
    let json = match outcome {
        Outcome::Cached => r#"{"label":"cached","category":"cached"}"#,
        Outcome::CacheMiss => r#"{"label":"rerun: cache miss","category":"rerun"}"#,
        Outcome::Forced => r#"{"label":"rerun: forced","category":"rerun"}"#,
        Outcome::CacheUnavailable => {
            r#"{"label":"rerun: cache unavailable","category":"rerun with cache errors"}"#
        }
        Outcome::CacheUpdateFailed => {
            r#"{"label":"rerun: cache update failed","category":"rerun with cache errors"}"#
        }
    };
    if let Err(error) = fs::write(path, json) {
        eprintln!("prevtest: could not write wrapper report: {error}");
    }
}
