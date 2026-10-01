use crate::config::Config;
use anyhow::{Result, ensure};
use std::{fs, path::Path};
pub fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
pub fn snapshot(source: &Path, target: &Path) -> Result<()> {
    ensure!(!target.exists(), "backup destination already exists");
    let db =
        rusqlite::Connection::open_with_flags(source, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.backup("main", target, None)?;
    Ok(())
}
pub fn scrub_auth(path: &Path) -> Result<()> {
    let db = rusqlite::Connection::open(path)?;
    db.execute_batch("DELETE FROM sessions; DELETE FROM grants; PRAGMA wal_checkpoint(TRUNCATE);")?;
    Ok(())
}
/// Writes consistent SQLite snapshots; safe while the server is running.
pub fn create(c: &Config, output: &Path, include_auth: bool) -> Result<()> {
    ensure!(!output.exists(), "backup destination must not exist");
    private_dir(output)?;
    snapshot(&c.storage.business_db, &output.join("bookmarks.db"))?;
    if include_auth {
        snapshot(&c.auth.store, &output.join("auth.db"))?;
        scrub_auth(&output.join("auth.db"))?;
        fs::copy(&c.auth.user_id_file, output.join("user-id.txt"))?;
    }
    fs::write(
        output.join("manifest.json"),
        serde_json::to_string_pretty(
            &serde_json::json!({"format":"bookmarkd-backup","schema":1,"auth":include_auth,"rp_id":c.auth.rp_id,"created_at":bookmarkd_auth::now()}),
        )?,
    )?;
    Ok(())
}
