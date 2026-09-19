use crate::model::Library;
use anyhow::{Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;
use std::path::{Path, PathBuf};
#[derive(Clone)]
pub struct Store {
    pub path: PathBuf,
}
impl Store {
    pub fn init(path: &Path) -> Result<Self> {
        ensure!(!path.exists(), "business database already exists");
        let db = Connection::open(path)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA user_version=1; CREATE TABLE library(id INTEGER PRIMARY KEY CHECK(id=1),data TEXT NOT NULL); CREATE TABLE idempotency(key TEXT PRIMARY KEY,hash TEXT NOT NULL,result TEXT NOT NULL);")?;
        db.execute(
            "INSERT INTO library VALUES(1,?1)",
            [serde_json::to_string(&Library::default())?],
        )?;
        Ok(Self { path: path.into() })
    }
    pub fn connection(&self) -> Result<Connection> {
        ensure!(self.path.is_file(), "business database absent");
        let db =
            Connection::open_with_flags(&self.path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        ensure!(
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? == 1,
            "unsupported business schema"
        );
        Ok(db)
    }
    pub fn read(&self) -> Result<Library> {
        Ok(serde_json::from_str(&self.connection()?.query_row(
            "SELECT data FROM library WHERE id=1",
            [],
            |r| r.get::<_, String>(0),
        )?)?)
    }
    pub fn mutate(
        &self,
        key: Option<&str>,
        hash: &str,
        f: impl FnOnce(&mut Library) -> Result<Value>,
    ) -> Result<Value> {
        let mut db = self.connection()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(key) = key {
            ensure!(
                key.len() <= 128 && !key.is_empty(),
                "invalid Idempotency-Key"
            );
            if let Some((h, v)) = tx
                .query_row(
                    "SELECT hash,result FROM idempotency WHERE key=?1",
                    [key],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                )
                .optional()?
            {
                ensure!(h == hash, "IDEMPOTENCY_CONFLICT");
                return Ok(serde_json::from_str(&v)?);
            }
        }
        let mut lib: Library = serde_json::from_str(&tx.query_row(
            "SELECT data FROM library WHERE id=1",
            [],
            |r| r.get::<_, String>(0),
        )?)?;
        let result = f(&mut lib)?;
        lib.validate()?;
        lib.revision += 1;
        tx.execute(
            "UPDATE library SET data=?1 WHERE id=1",
            [serde_json::to_string(&lib)?],
        )?;
        if let Some(key) = key {
            tx.execute(
                "INSERT INTO idempotency VALUES(?1,?2,?3)",
                params![key, hash, result.to_string()],
            )?;
        }
        tx.commit()?;
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn rollback_and_idempotency() {
        let d = tempfile::tempdir().unwrap();
        let s = Store::init(&d.path().join("bookmarks.db")).unwrap();
        assert!(
            s.mutate(None, "", |l| {
                l.revision = 99;
                anyhow::bail!("fail")
            })
            .is_err()
        );
        assert_eq!(s.read().unwrap().revision, 0);
        s.mutate(Some("k"), "h", |_| Ok(json!({"ok":true})))
            .unwrap();
        s.mutate(Some("k"), "h", |_| panic!("must not repeat"))
            .unwrap();
        assert_eq!(s.read().unwrap().revision, 1);
        assert!(s.mutate(Some("k"), "other", |_| Ok(Value::Null)).is_err());
    }
}
