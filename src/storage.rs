//! Persistence layer with connection and migration handling.

use crate::{execution::CapturedOutput, identity::CacheKey};
use anyhow::{Result, ensure};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{fs, path::Path, time::Duration};

const DATABASE_FILENAME: &str = "cache.sqlite3";
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MIGRATIONS: &[&str] = &[include_str!("../migrations/001.sql")];

pub trait Cache {
    fn get(&self, key: &CacheKey) -> Result<Option<CapturedOutput>>;
    fn put(
        &self,
        key: &CacheKey,
        binary_id: &str,
        name: &str,
        output: &CapturedOutput,
    ) -> Result<()>;
    fn remove(&self, key: &CacheKey) -> Result<()>;
}

pub struct SqliteCache(Connection);

impl SqliteCache {
    pub fn open(directory: &Path) -> Result<Self> {
        fs::create_dir_all(directory)?;
        let mut db = Connection::open(directory.join(DATABASE_FILENAME))?;
        db.busy_timeout(BUSY_TIMEOUT)?;
        migrate(&mut db, MIGRATIONS)?;
        Ok(Self(db))
    }
}

fn migrate(db: &mut Connection, migrations: &[&str]) -> Result<()> {
    let version: u32 = db.pragma_query_value(None, "user_version", |row| row.get(0))?;
    ensure!(
        version as usize <= migrations.len(),
        "unsupported cache schema version {version}"
    );
    if version as usize == migrations.len() {
        return Ok(());
    }

    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // Another wrapper may have migrated the database while we waited.
    let version: u32 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    ensure!(
        version as usize <= migrations.len(),
        "unsupported cache schema version {version}"
    );
    for (index, sql) in migrations.iter().enumerate().skip(version as usize) {
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i32::try_from(index + 1)?)?;
    }
    tx.commit()?;
    Ok(())
}

impl Cache for SqliteCache {
    fn get(&self, key: &CacheKey) -> Result<Option<CapturedOutput>> {
        Ok(self
            .0
            .query_row(
                "SELECT stdout, stderr FROM passed_test_results WHERE cache_key = ?1",
                [&key.0[..]],
                |row| {
                    Ok(CapturedOutput {
                        stdout: row.get(0)?,
                        stderr: row.get(1)?,
                    })
                },
            )
            .optional()?)
    }

    fn put(
        &self,
        key: &CacheKey,
        binary_id: &str,
        name: &str,
        output: &CapturedOutput,
    ) -> Result<()> {
        self.0.execute(
            "INSERT INTO passed_test_results (cache_key, binary_id, name, stdout, stderr)
             VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(cache_key) DO UPDATE SET
             stdout = excluded.stdout, stderr = excluded.stderr",
            params![&key.0[..], binary_id, name, output.stdout, output.stderr],
        )?;
        Ok(())
    }

    fn remove(&self, key: &CacheKey) -> Result<()> {
        self.0.execute(
            "DELETE FROM passed_test_results WHERE cache_key = ?1",
            [&key.0[..]],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_corrupt_and_newer_databases_without_modifying_them() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(DATABASE_FILENAME);
        fs::write(&path, "not sqlite").unwrap();
        assert!(SqliteCache::open(directory.path()).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"not sqlite");

        fs::remove_file(&path).unwrap();
        let db = Connection::open(&path).unwrap();
        let newer = MIGRATIONS.len() as u32 + 1;
        db.pragma_update(None, "user_version", newer).unwrap();
        assert!(SqliteCache::open(directory.path()).is_err());
        assert_eq!(
            db.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
                .unwrap(),
            newer
        );
    }

    #[test]
    fn migrations_apply_only_pending_steps_and_roll_back_on_failure() {
        let mut db = Connection::open_in_memory().unwrap();
        let migrations = [
            "CREATE TABLE probe (value INTEGER); INSERT INTO probe VALUES (7);",
            "ALTER TABLE probe ADD COLUMN extra TEXT;",
        ];
        migrate(&mut db, &migrations[..1]).unwrap();
        migrate(&mut db, &migrations).unwrap();
        migrate(&mut db, &migrations).unwrap();
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row("SELECT value FROM probe", [], |r| r.get::<_, i32>(0))
                .unwrap(),
            7
        );

        let failing = [
            migrations[0],
            migrations[1],
            "UPDATE probe SET value = 8;",
            "INVALID SQL",
        ];
        assert!(migrate(&mut db, &failing).is_err());
        assert_eq!(
            db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            2
        );
        assert_eq!(
            db.query_row("SELECT value FROM probe", [], |r| r.get::<_, i32>(0))
                .unwrap(),
            7
        );
    }
}
