//! Checks GitHub Releases for newer versions and installs them in place.
//!
//! Scheduled checks only record what is available; installing always needs an
//! explicit request. Downloads are verified against the release's SHA256SUMS,
//! the new binary must report the expected version, and the data is backed up
//! before the executable is swapped (the previous one is kept as `*.old`).
use crate::{backup, config::Config};
use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Datelike, Duration, Local, NaiveTime, TimeZone};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_API: &str = "https://api.github.com/repos/zhangchaosd/bookmarkd";
const NOTES_LIMIT: usize = 20_000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// `off`, `daily` or `weekly`.
    pub schedule: String,
    /// ISO weekday for weekly checks: 1 = Monday … 7 = Sunday.
    pub weekday: u32,
    /// Server-local time of day, `HH:MM`.
    pub time: String,
    /// `prerelease` includes prereleases; `stable` ignores them.
    pub channel: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schedule: "off".into(),
            weekday: 1,
            time: "04:00".into(),
            channel: "prerelease".into(),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            ["off", "daily", "weekly"].contains(&self.schedule.as_str()),
            "无效的检查频率"
        );
        ensure!((1..=7).contains(&self.weekday), "无效的星期");
        NaiveTime::parse_from_str(&self.time, "%H:%M").context("时间格式应为 HH:MM")?;
        ensure!(
            ["prerelease", "stable"].contains(&self.channel.as_str()),
            "无效的更新渠道"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Release {
    pub version: String,
    pub url: String,
    pub notes: String,
    pub published_at: String,
    pub prerelease: bool,
    pub asset_url: Option<String>,
    pub sums_url: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub settings: Settings,
    pub checked_at: Option<i64>,
    pub latest: Option<Release>,
    pub error: Option<String>,
}

pub struct Installed {
    pub version: String,
    pub backup: PathBuf,
    pub executable: PathBuf,
}

/// Release asset built by `.github/workflows/release.yml` for this platform.
pub fn asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("bookmarkd-linux-x86_64"),
        ("linux", "aarch64") => Some("bookmarkd-linux-aarch64"),
        ("macos", "aarch64") => Some("bookmarkd-macos-arm64"),
        ("macos", "x86_64") => Some("bookmarkd-macos-x86_64"),
        ("windows", "x86_64") => Some("bookmarkd-windows-x86_64.exe"),
        _ => None,
    }
}
/// A running Windows executable cannot be swapped and restarted in place.
pub fn installable() -> bool {
    cfg!(unix) && asset_name().is_some()
}

/// Picks the newest release above `current` from a GitHub releases listing.
pub fn select(releases: &Value, channel: &str, current: &str) -> Option<Release> {
    let current = Version::parse(current).ok()?;
    releases
        .as_array()?
        .iter()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false))
        .filter(|r| channel != "stable" || !r["prerelease"].as_bool().unwrap_or(false))
        .filter_map(|r| {
            let tag = r["tag_name"].as_str()?;
            let version = Version::parse(tag.trim_start_matches('v')).ok()?;
            (version > current).then_some((version, r))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(version, r)| {
            let asset = |name: &str| {
                r["assets"].as_array().and_then(|assets| {
                    assets
                        .iter()
                        .find(|a| a["name"] == name)
                        .and_then(|a| a["browser_download_url"].as_str())
                        .map(String::from)
                })
            };
            Release {
                version: version.to_string(),
                url: r["html_url"].as_str().unwrap_or_default().into(),
                notes: r["body"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(NOTES_LIMIT)
                    .collect(),
                published_at: r["published_at"].as_str().unwrap_or_default().into(),
                prerelease: r["prerelease"].as_bool().unwrap_or(false),
                asset_url: asset_name().and_then(asset),
                sums_url: asset("SHA256SUMS"),
            }
        })
}

/// Finds `name`'s digest in a `sha256sum` listing.
pub fn expected_digest(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| hash.to_ascii_lowercase())
    })
}

/// Whether a scheduled check is due: the latest scheduled slot at or before
/// `now` has not been covered by a check yet. A slot missed while the server
/// was down is caught up once.
pub fn due(settings: &Settings, last_check: Option<i64>, now: DateTime<Local>) -> bool {
    let Ok(time) = NaiveTime::parse_from_str(&settings.time, "%H:%M") else {
        return false;
    };
    let mut date = now.date_naive();
    let period = match settings.schedule.as_str() {
        "daily" => 1,
        "weekly" => {
            let today = now.weekday().number_from_monday();
            date -= Duration::days(((today + 7 - settings.weekday) % 7).into());
            7
        }
        _ => return false,
    };
    let slot_at = |d: chrono::NaiveDate| Local.from_local_datetime(&d.and_time(time)).earliest();
    let Some(mut slot) = slot_at(date) else {
        return false;
    };
    if slot > now {
        match slot_at(date - Duration::days(period)) {
            Some(s) => slot = s,
            None => return false,
        }
    }
    last_check.is_none_or(|t| t < slot.timestamp())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(120)))
        .user_agent(format!("bookmarkd/{CURRENT}"))
        .build()
        .into()
}
fn download(url: &str, limit: u64) -> Result<Vec<u8>> {
    let mut response = agent()
        .get(url)
        .call()
        .with_context(|| format!("下载失败：{url}"))?;
    Ok(response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()?)
}

pub struct Updater {
    path: PathBuf,
    api: String,
    busy: Mutex<()>,
}
impl Updater {
    pub fn new(config: &Config) -> Self {
        Self {
            path: config.storage.business_db.with_file_name("update.json"),
            // Overridable so tests and forks can point at another releases API.
            api: std::env::var("BOOKMARKD_UPDATE_API").unwrap_or_else(|_| DEFAULT_API.into()),
            busy: Mutex::new(()),
        }
    }
    pub fn state(&self) -> State {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    fn save(&self, state: &State) -> Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(state)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(tmp, &self.path)?;
        Ok(())
    }
    pub fn save_settings(&self, settings: Settings) -> Result<()> {
        settings.validate()?;
        let _guard = self.busy.lock().unwrap();
        let mut state = self.state();
        if state.settings.channel != settings.channel {
            // A result from the other channel would be misleading.
            state.latest = None;
        }
        state.settings = settings;
        self.save(&state)
    }
    pub fn status(&self) -> Value {
        let state = self.state();
        let current = Version::parse(CURRENT).ok();
        // A stored result becomes stale once this version has caught up with it.
        let latest = state.latest.filter(|r| {
            Version::parse(&r.version)
                .ok()
                .zip(current.clone())
                .is_some_and(|(v, c)| v > c)
        });
        json!({
            "current": CURRENT,
            "asset": asset_name(),
            "installable": installable(),
            "settings": state.settings,
            "checked_at": state.checked_at,
            "error": state.error,
            "available": latest.is_some(),
            "latest": latest,
        })
    }
    pub fn check(&self) -> Result<Option<Release>> {
        let _guard = self
            .busy
            .try_lock()
            .map_err(|_| anyhow::anyhow!("更新任务正在进行，请稍后再试"))?;
        let mut state = self.state();
        let result = (|| -> Result<Option<Release>> {
            let mut response = agent()
                .get(&format!("{}/releases?per_page=30", self.api))
                .header("Accept", "application/vnd.github+json")
                .call()
                .context("无法连接 GitHub")?;
            let releases: Value = serde_json::from_slice(
                &response
                    .body_mut()
                    .with_config()
                    .limit(8 << 20)
                    .read_to_vec()?,
            )?;
            Ok(select(&releases, &state.settings.channel, CURRENT))
        })();
        state.checked_at = Some(bookmarkd_auth::now());
        match &result {
            Ok(latest) => {
                state.latest = latest.clone();
                state.error = None;
            }
            Err(e) => state.error = Some(format!("{e:#}")),
        }
        self.save(&state)?;
        result
    }
    /// Runs a check when the schedule says one is due; errors are recorded in the state.
    pub fn tick(&self) {
        let state = self.state();
        if due(&state.settings, state.checked_at, Local::now()) {
            match self.check() {
                Ok(Some(r)) => eprintln!("bookmarkd update available: v{}", r.version),
                Ok(None) => {}
                Err(e) => eprintln!("bookmarkd update check failed: {e:#}"),
            }
        }
    }
    /// Downloads, verifies and swaps in `version`, which must be the last checked release.
    pub fn install(&self, config: &Config, version: &str) -> Result<Installed> {
        ensure!(
            installable(),
            "当前平台不支持自动安装，请从 Release 页面手动下载"
        );
        let _guard = self
            .busy
            .try_lock()
            .map_err(|_| anyhow::anyhow!("更新任务正在进行，请稍后再试"))?;
        let release = self
            .state()
            .latest
            .filter(|r| r.version == version)
            .ok_or_else(|| anyhow::anyhow!("版本信息已过期，请重新检查更新"))?;
        let name = asset_name().unwrap_or_default();
        let asset_url = release
            .asset_url
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("该版本没有 {name} 文件"))?;
        let sums_url = release
            .sums_url
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("该版本没有 SHA256SUMS，拒绝安装"))?;
        let sums = String::from_utf8(download(sums_url, 1 << 20)?)?;
        let expected = expected_digest(&sums, name)
            .ok_or_else(|| anyhow::anyhow!("SHA256SUMS 中没有 {name}"))?;
        let binary = download(asset_url, 256 << 20)?;
        let actual = format!("{:x}", Sha256::digest(&binary));
        ensure!(actual == expected, "校验失败：下载文件与 SHA256SUMS 不一致");

        let executable = std::env::current_exe()?.canonicalize()?;
        let dir = executable
            .parent()
            .ok_or_else(|| anyhow::anyhow!("无法确定程序目录"))?;
        let staged = dir.join(format!(".bookmarkd-{version}.download"));
        fs::write(&staged, &binary).context("无法写入程序目录，请检查权限")?;
        let verified = (|| -> Result<()> {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&staged, fs::Permissions::from_mode(0o755))?;
            }
            let output = std::process::Command::new(&staged)
                .arg("version")
                .output()?;
            let reported = String::from_utf8_lossy(&output.stdout);
            ensure!(
                output.status.success() && reported.starts_with(&format!("bookmarkd {version} ")),
                "新版本无法在本机运行：{}",
                reported.trim()
            );
            Ok(())
        })();
        if let Err(e) = verified {
            let _ = fs::remove_file(&staged);
            return Err(e);
        }

        let backup = config
            .storage
            .business_db
            .with_file_name("backups")
            .join(format!(
                "pre-update-{CURRENT}-to-{version}-{}",
                bookmarkd_auth::now()
            ));
        backup::private_dir(backup.parent().unwrap_or(Path::new(".")))?;
        if let Err(e) = backup::create(config, &backup, true) {
            let _ = fs::remove_file(&staged);
            return Err(e.context("更新前备份失败，已取消安装"));
        }
        swap(&executable, &staged)?;
        Ok(Installed {
            version: version.into(),
            backup: std::path::absolute(&backup).unwrap_or(backup),
            executable,
        })
    }
}

fn old_path(executable: &Path) -> PathBuf {
    let mut name = executable.file_name().unwrap_or_default().to_os_string();
    name.push(".old");
    executable.with_file_name(name)
}
/// Replaces `executable` with `staged`, keeping the previous binary as `*.old`.
fn swap(executable: &Path, staged: &Path) -> Result<()> {
    let old = old_path(executable);
    if old.exists() {
        fs::remove_file(&old)?;
    }
    fs::rename(executable, &old)?;
    if let Err(e) = fs::rename(staged, executable) {
        fs::rename(&old, executable)?;
        bail!("替换程序失败：{e}");
    }
    Ok(())
}
/// Swaps the current binary with the `*.old` one kept by the last install.
pub fn rollback() -> Result<PathBuf> {
    let executable = std::env::current_exe()?.canonicalize()?;
    let old = old_path(&executable);
    ensure!(old.exists(), "没有可回滚的旧版本：{}", old.display());
    let parked = executable.with_extension("rollback");
    fs::rename(&executable, &parked)?;
    fs::rename(&old, &executable)?;
    fs::rename(&parked, &old)?;
    Ok(executable)
}
/// Replaces this process with `executable`, keeping arguments, environment and PID
/// so service managers see the same process. Open descriptors are close-on-exec,
/// which releases the listener and the instance lock for the new binary.
#[cfg(unix)]
pub fn reexec(executable: &Path) -> ! {
    use std::os::unix::process::CommandExt;
    let error = std::process::Command::new(executable)
        .args(std::env::args_os().skip(1))
        .exec();
    eprintln!("bookmarkd restart after update failed: {error}");
    std::process::exit(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn listing() -> Value {
        json!([
            {"tag_name":"v0.5.0-beta.1","draft":false,"prerelease":true,"assets":[]},
            {"tag_name":"v0.4.1","draft":true,"prerelease":false,"assets":[]},
            {"tag_name":"v0.4.0","draft":false,"prerelease":false,"html_url":"https://example.com/r","body":"notes","assets":[
                {"name":"SHA256SUMS","browser_download_url":"https://example.com/sums"},
                {"name":asset_name().unwrap_or("x"),"browser_download_url":"https://example.com/bin"}
            ]},
            {"tag_name":"v0.3.0","draft":false,"prerelease":true,"assets":[]},
            {"tag_name":"nightly","draft":false,"prerelease":true,"assets":[]}
        ])
    }
    #[test]
    fn selects_newest_by_channel_ignoring_drafts() {
        let pre = select(&listing(), "prerelease", "0.3.0").unwrap();
        assert_eq!(pre.version, "0.5.0-beta.1");
        let stable = select(&listing(), "stable", "0.3.0").unwrap();
        assert_eq!(stable.version, "0.4.0");
        assert_eq!(stable.sums_url.as_deref(), Some("https://example.com/sums"));
        if asset_name().is_some() {
            assert_eq!(stable.asset_url.as_deref(), Some("https://example.com/bin"));
        }
        assert!(select(&listing(), "stable", "0.4.0").is_none());
        assert!(select(&json!({"message":"rate limited"}), "stable", "0.3.0").is_none());
    }
    #[test]
    fn reads_sha256sums() {
        let sums = "ABC123  bookmarkd-linux-x86_64\ndef456 *SHA256SUMS\n";
        assert_eq!(
            expected_digest(sums, "bookmarkd-linux-x86_64").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            expected_digest(sums, "SHA256SUMS").as_deref(),
            Some("def456")
        );
        assert!(expected_digest(sums, "bookmarkd").is_none());
    }
    #[test]
    fn schedules_daily_and_weekly_slots() {
        let at = |s: &str| {
            Local
                .from_local_datetime(
                    &chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap(),
                )
                .earliest()
                .unwrap()
        };
        let daily = Settings {
            schedule: "daily".into(),
            time: "04:00".into(),
            ..Settings::default()
        };
        // 2026-10-01 is a Thursday.
        let now = at("2026-10-01 10:00");
        assert!(due(&daily, None, now));
        assert!(due(&daily, Some(at("2026-10-01 03:59").timestamp()), now));
        assert!(!due(&daily, Some(at("2026-10-01 04:00").timestamp()), now));
        assert!(!due(
            &daily,
            Some(at("2026-09-30 05:00").timestamp()),
            at("2026-10-01 03:00")
        ));
        let weekly = Settings {
            schedule: "weekly".into(),
            weekday: 1,
            ..daily.clone()
        };
        // The last Monday 04:00 slot before Thursday is 2026-09-28.
        assert!(!due(&weekly, Some(at("2026-09-28 04:30").timestamp()), now));
        assert!(due(&weekly, Some(at("2026-09-28 03:00").timestamp()), now));
        assert!(!due(&Settings::default(), None, now));
    }
    #[test]
    fn validates_settings() {
        assert!(Settings::default().validate().is_ok());
        for bad in [
            Settings {
                schedule: "hourly".into(),
                ..Settings::default()
            },
            Settings {
                weekday: 0,
                ..Settings::default()
            },
            Settings {
                time: "25:00".into(),
                ..Settings::default()
            },
            Settings {
                channel: "nightly".into(),
                ..Settings::default()
            },
        ] {
            assert!(bad.validate().is_err());
        }
    }
    #[test]
    fn swap_and_rollback_keep_previous_binary() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("bookmarkd");
        let staged = dir.path().join(".new");
        fs::write(&exe, "old").unwrap();
        fs::write(&staged, "new").unwrap();
        swap(&exe, &staged).unwrap();
        assert_eq!(fs::read_to_string(&exe).unwrap(), "new");
        assert_eq!(fs::read_to_string(old_path(&exe)).unwrap(), "old");
    }
}
