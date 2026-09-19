//! Embeddable single-user authentication. SQLite transactions are the authority;
//! host-scoped sessions never imply SSO. Protocol verification lives in engine.
pub mod engine;
use anyhow::{Result, bail, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use rand::{RngCore, rngs::OsRng};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn random() -> String {
    let mut b = [0u8; 32];
    OsRng.fill_bytes(&mut b);
    B64.encode(b)
}
pub fn digest(purpose: &str, value: &str) -> String {
    B64.encode(Sha256::digest(format!("{purpose}\0{value}").as_bytes()))
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Identity {
    pub rp_id: String,
    pub user_id: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub credential_id: String,
    pub csrf: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub recent_at: i64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct StoredCredential {
    pub id: String,
    pub name: String,
    pub data: serde_json::Value,
    pub version: i64,
}
/// Implementors must keep final credential/token/session transitions atomic.
pub trait AuthStore: Send + Sync {
    fn identity(&self) -> Result<Identity>;
    fn credentials(&self) -> Result<Vec<StoredCredential>>;
    fn session(&self, token: &str, app: &str, origin: &str) -> Result<Option<Session>>;
    fn finish_registration(
        &self,
        grant: &str,
        binding: &str,
        app: &str,
        origin: &str,
        mode: &str,
        credential: &StoredCredential,
    ) -> Result<()>;
    fn finish_login(
        &self,
        credential: &StoredCredential,
        app: &str,
        origin: &str,
        remember: bool,
    ) -> Result<(String, Session)>;
}
#[derive(Clone)]
pub struct SqliteAuthStore {
    pub path: PathBuf,
}
impl SqliteAuthStore {
    pub fn initialize(path: &Path, identity: &Identity) -> Result<Self> {
        ensure!(!path.exists(), "auth database already exists");
        let db = Connection::open(path)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA user_version=1;
  CREATE TABLE identity(rp_id TEXT NOT NULL,user_id TEXT NOT NULL);
  CREATE TABLE credentials(id TEXT PRIMARY KEY,name TEXT NOT NULL,data TEXT NOT NULL,version INTEGER NOT NULL DEFAULT 1,revoked INTEGER NOT NULL DEFAULT 0);
  CREATE TABLE grants(hash TEXT PRIMARY KEY,app TEXT NOT NULL,origin TEXT NOT NULL,expires INTEGER NOT NULL,binding TEXT);
  CREATE TABLE sessions(hash TEXT PRIMARY KEY,id TEXT UNIQUE NOT NULL,credential TEXT NOT NULL,app TEXT NOT NULL,origin TEXT NOT NULL,csrf TEXT NOT NULL,created INTEGER NOT NULL,expires INTEGER NOT NULL,recent INTEGER NOT NULL);
  CREATE INDEX sessions_scope ON sessions(app,origin);")?;
        db.execute(
            "INSERT INTO identity VALUES(?1,?2)",
            params![identity.rp_id, identity.user_id],
        )?;
        Ok(Self { path: path.into() })
    }
    pub fn open(path: &Path, expected: &Identity) -> Result<Self> {
        ensure!(path.is_file(), "auth database absent; run init explicitly");
        let s = Self { path: path.into() };
        let i = s.identity()?;
        ensure!(
            i.rp_id == expected.rp_id && i.user_id == expected.user_id,
            "authentication identity mismatch"
        );
        Ok(s)
    }
    pub fn connection(&self) -> Result<Connection> {
        let db =
            Connection::open_with_flags(&self.path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        ensure!(
            db.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))? == 1,
            "unsupported auth schema"
        );
        Ok(db)
    }
    pub fn allowed(db: &Connection, mode: &str) -> Result<bool> {
        Ok(match mode {
            "enabled" => true,
            "auto" => {
                db.query_row(
                    "SELECT count(*) FROM credentials WHERE revoked=0",
                    [],
                    |r| r.get::<_, i64>(0),
                )? == 0
            }
            _ => false,
        })
    }
    pub fn create_token(&self, app: &str, origin: &str) -> Result<String> {
        let mut db = self.connection()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let token = random();
        tx.execute(
            "DELETE FROM grants WHERE app=?1 OR expires<=?2",
            params![app, now()],
        )?;
        tx.execute(
            "INSERT INTO grants VALUES(?1,?2,?3,?4,NULL)",
            params![digest("setup", &token), app, origin, now() + 600],
        )?;
        tx.commit()?;
        Ok(token)
    }
    pub fn setup_available(&self, app: &str, origin: &str, mode: &str) -> Result<bool> {
        let db = self.connection()?;
        Ok(Self::allowed(&db, mode)?
            && db.query_row(
                "SELECT count(*) FROM grants WHERE app=?1 AND origin=?2 AND expires>?3",
                params![app, origin, now()],
                |r| r.get::<_, i64>(0),
            )? > 0)
    }
    pub fn bind_token(
        &self,
        token: &str,
        binding: &str,
        app: &str,
        origin: &str,
        mode: &str,
    ) -> Result<String> {
        let mut db = self.connection()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        ensure!(Self::allowed(&tx, mode)?, "setup unavailable");
        let hash = digest("setup", token);
        ensure!(tx.execute("UPDATE grants SET binding=?1 WHERE hash=?2 AND app=?3 AND origin=?4 AND expires>?5 AND (binding IS NULL OR binding=?1)",params![digest("binding",binding),hash,app,origin,now()])?==1,"invalid authorization");
        tx.commit()?;
        Ok(hash)
    }
    pub fn grant(
        &self,
        grant: &str,
        binding: &str,
        app: &str,
        origin: &str,
        mode: &str,
    ) -> Result<()> {
        let db = self.connection()?;
        ensure!(Self::allowed(&db, mode)?, "setup unavailable");
        ensure!(db.query_row("SELECT count(*) FROM grants WHERE hash=?1 AND binding=?2 AND app=?3 AND origin=?4 AND expires>?5",params![grant,digest("binding",binding),app,origin,now()],|r|r.get::<_,i64>(0))?==1,"invalid authorization");
        Ok(())
    }
    pub fn revoke_credential(&self, id: &str) -> Result<()> {
        let mut db = self.connection()?;
        let tx = db.transaction()?;
        ensure!(
            tx.execute(
                "UPDATE credentials SET revoked=1,version=version+1 WHERE id=?1",
                [id]
            )? == 1,
            "credential not found"
        );
        tx.execute("DELETE FROM sessions WHERE credential=?1", [id])?;
        tx.commit()?;
        Ok(())
    }
    pub fn rename(&self, id: &str, name: &str) -> Result<()> {
        ensure!(
            !name.trim().is_empty() && name.chars().count() <= 128,
            "invalid name"
        );
        self.connection()?.execute(
            "UPDATE credentials SET name=?1 WHERE id=?2",
            params![name, id],
        )?;
        Ok(())
    }
    pub fn sessions(&self, app: &str, origin: &str) -> Result<Vec<Session>> {
        let db = self.connection()?;
        let mut q=db.prepare("SELECT id,credential,csrf,created,expires,recent FROM sessions WHERE app=?1 AND origin=?2 AND expires>?3")?;
        Ok(q.query_map(params![app, origin, now()], session_row)?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn revoke_session(&self, id: &str, app: &str, origin: &str) -> Result<()> {
        self.connection()?.execute(
            "DELETE FROM sessions WHERE id=?1 AND app=?2 AND origin=?3",
            params![id, app, origin],
        )?;
        Ok(())
    }
    pub fn reauthenticate(
        &self,
        token: &str,
        app: &str,
        origin: &str,
        credential: &StoredCredential,
    ) -> Result<()> {
        let mut db = self.connection()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        ensure!(tx.execute("UPDATE credentials SET data=?1,version=version+1 WHERE id=?2 AND version=?3 AND revoked=0",params![credential.data.to_string(),credential.id,credential.version])?==1,"credential changed");
        ensure!(tx.execute("UPDATE sessions SET recent=?1 WHERE hash=?2 AND app=?3 AND origin=?4 AND expires>?1",params![now(),digest("session",token),app,origin])?==1,"session expired");
        tx.commit()?;
        Ok(())
    }
}
fn session_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    Ok(Session {
        id: r.get(0)?,
        credential_id: r.get(1)?,
        csrf: r.get(2)?,
        created_at: r.get(3)?,
        expires_at: r.get(4)?,
        recent_at: r.get(5)?,
    })
}
impl AuthStore for SqliteAuthStore {
    fn identity(&self) -> Result<Identity> {
        Ok(self
            .connection()?
            .query_row("SELECT rp_id,user_id FROM identity", [], |r| {
                Ok(Identity {
                    rp_id: r.get(0)?,
                    user_id: r.get(1)?,
                })
            })?)
    }
    fn credentials(&self) -> Result<Vec<StoredCredential>> {
        let db = self.connection()?;
        let mut q = db.prepare("SELECT id,name,data,version FROM credentials WHERE revoked=0")?;
        let rows = q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        rows.map(|r| {
            let (id, name, data, version) = r?;
            Ok(StoredCredential {
                id,
                name,
                data: serde_json::from_str(&data)?,
                version,
            })
        })
        .collect()
    }
    fn session(&self, token: &str, app: &str, origin: &str) -> Result<Option<Session>> {
        Ok(self.connection()?.query_row("SELECT s.id,s.credential,s.csrf,s.created,s.expires,s.recent FROM sessions s JOIN credentials c ON c.id=s.credential WHERE s.hash=?1 AND s.app=?2 AND s.origin=?3 AND s.expires>?4 AND c.revoked=0",params![digest("session",token),app,origin,now()],session_row).optional()?)
    }
    fn finish_registration(
        &self,
        grant: &str,
        binding: &str,
        app: &str,
        origin: &str,
        mode: &str,
        c: &StoredCredential,
    ) -> Result<()> {
        let mut db = self.connection()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        ensure!(Self::allowed(&tx, mode)?, "setup unavailable");
        ensure!(tx.execute("DELETE FROM grants WHERE hash=?1 AND binding=?2 AND app=?3 AND origin=?4 AND expires>?5",params![grant,digest("binding",binding),app,origin,now()])?==1,"invalid authorization");
        tx.execute(
            "INSERT INTO credentials(id,name,data) VALUES(?1,?2,?3)",
            params![c.id, c.name, c.data.to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }
    fn finish_login(
        &self,
        c: &StoredCredential,
        app: &str,
        origin: &str,
        remember: bool,
    ) -> Result<(String, Session)> {
        let mut db = self.connection()?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if tx.execute("UPDATE credentials SET data=?1,version=version+1 WHERE id=?2 AND version=?3 AND revoked=0",params![c.data.to_string(),c.id,c.version])?!=1 {bail!("credential changed");}
        let token = random();
        let s = Session {
            id: random(),
            credential_id: c.id.clone(),
            csrf: random(),
            created_at: now(),
            expires_at: now() + if remember { 2592000 } else { 43200 },
            recent_at: now(),
        };
        tx.execute("DELETE FROM sessions WHERE expires<=?1", [now()])?;
        tx.execute(
            "INSERT INTO sessions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                digest("session", &token),
                s.id,
                s.credential_id,
                app,
                origin,
                s.csrf,
                s.created_at,
                s.expires_at,
                s.recent_at
            ],
        )?;
        tx.commit()?;
        Ok((token, s))
    }
}

/// Constant-time comparison for fixed-length browser secrets.
pub fn secure_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && openssl::memcmp::eq(a.as_bytes(), b.as_bytes())
}

#[cfg(test)]
mod store_tests {
    use super::*;
    fn fixture() -> (SqliteAuthStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("bookmarkd-auth-{}", random()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = SqliteAuthStore::initialize(
            &dir.join("auth.db"),
            &Identity {
                rp_id: "example.com".into(),
                user_id: random(),
            },
        )
        .unwrap();
        (store, dir)
    }
    fn credential() -> StoredCredential {
        StoredCredential {
            id: random(),
            name: "Test".into(),
            data: serde_json::json!({}),
            version: 1,
        }
    }
    #[test]
    fn setup_binding_modes_and_atomic_consumption() {
        let (s, dir) = fixture();
        let origin = "https://a.example.com";
        let token = s.create_token("a", origin).unwrap();
        assert!(
            s.bind_token(&token, "browser", "a", origin, "disabled")
                .is_err()
        );
        let grant = s
            .bind_token(&token, "browser", "a", origin, "auto")
            .unwrap();
        assert!(s.bind_token(&token, "other", "a", origin, "auto").is_err());
        assert!(
            s.bind_token(&token, "browser", "b", origin, "auto")
                .is_err()
        );
        s.finish_registration(&grant, "browser", "a", origin, "auto", &credential())
            .unwrap();
        assert!(
            s.finish_registration(&grant, "browser", "a", origin, "auto", &credential())
                .is_err()
        );
        assert!(!s.setup_available("a", origin, "auto").unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn session_scope_revocation_and_absolute_expiry() {
        let (s, dir) = fixture();
        let origin = "https://a.example.com";
        let c = credential();
        let token = s.create_token("a", origin).unwrap();
        let grant = s
            .bind_token(&token, "browser", "a", origin, "auto")
            .unwrap();
        s.finish_registration(&grant, "browser", "a", origin, "auto", &c)
            .unwrap();
        let (token, session) = s.finish_login(&c, "a", origin, true).unwrap();
        assert_eq!(session.expires_at - session.created_at, 2592000);
        assert!(s.session(&token, "b", origin).unwrap().is_none());
        assert!(
            s.session(&token, "a", "https://b.example.com")
                .unwrap()
                .is_none()
        );
        assert_eq!(
            s.session(&token, "a", origin).unwrap().unwrap().expires_at,
            session.expires_at
        );
        assert!(s.finish_login(&c, "a", origin, false).is_err()); // stale credential snapshot
        s.revoke_credential(&c.id).unwrap();
        assert!(s.session(&token, "a", origin).unwrap().is_none());
        assert!(
            s.finish_login(
                &s.credentials().unwrap().first().cloned().unwrap_or(c),
                "a",
                origin,
                false
            )
            .is_err()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn cross_process_auto_setup_serializes_first_credential() {
        let (s, dir) = fixture();
        let mut pending = Vec::new();
        for app in ["a", "b"] {
            let token = s.create_token(app, "https://example.com").unwrap();
            let grant = s
                .bind_token(&token, "browser", app, "https://example.com", "auto")
                .unwrap();
            pending.push((app, grant));
        }
        let results: Vec<_> = pending
            .into_iter()
            .map(|(app, grant)| {
                let s = s.clone();
                std::thread::spawn(move || {
                    s.finish_registration(
                        &grant,
                        "browser",
                        app,
                        "https://example.com",
                        "auto",
                        &credential(),
                    )
                    .is_ok()
                })
            })
            .collect();
        assert_eq!(
            results
                .into_iter()
                .filter(|t| t.thread().id() != std::thread::current().id())
                .map(|t| usize::from(t.join().unwrap()))
                .sum::<usize>(),
            1
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn reopen_preserves_identity_and_sessions() {
        let (s, dir) = fixture();
        let identity = s.identity().unwrap();
        let c = credential();
        let token = s.create_token("a", "https://example.com").unwrap();
        let grant = s
            .bind_token(&token, "b", "a", "https://example.com", "auto")
            .unwrap();
        s.finish_registration(&grant, "b", "a", "https://example.com", "auto", &c)
            .unwrap();
        let (token, _) = s
            .finish_login(&c, "a", "https://example.com", false)
            .unwrap();
        let reopened = SqliteAuthStore::open(&s.path, &identity).unwrap();
        assert!(
            reopened
                .session(&token, "a", "https://example.com")
                .unwrap()
                .is_some()
        );
        assert!(
            SqliteAuthStore::open(
                &s.path,
                &Identity {
                    user_id: random(),
                    ..identity
                }
            )
            .is_err()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
