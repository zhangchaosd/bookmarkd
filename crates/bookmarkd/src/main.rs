mod config;
mod http;
mod model;
mod ordering;
mod store;
mod transfer;
use anyhow::{Result, ensure};
use bookmarkd_auth::{AuthStore, Identity, SqliteAuthStore, random};
use clap::{Parser, Subcommand};
use config::{Auth, Config, Server, Storage};
use fs2::FileExt;
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Parser)]
#[command(version, about = "Private bookmark hub with Passkey authentication")]
struct Cli {
    #[arg(
        long,
        global = true,
        default_value = "data/config.toml",
        env = "BOOKMARKD_CONFIG"
    )]
    config: PathBuf,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Init {
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        #[arg(long, default_value = "https://bookmark.example.com")]
        public_url: String,
        #[arg(long, default_value = "example.com")]
        rp_id: String,
        #[arg(long, default_value = "Private Services")]
        rp_name: String,
        #[arg(long, default_value = "owner")]
        user_name: String,
        #[arg(long, default_value = "auto")]
        setup_mode: String,
        #[arg(long)]
        allow_insecure_localhost: bool,
    },
    Serve {
        #[arg(long)]
        listen: Option<String>,
        #[arg(long)]
        setup_mode: Option<String>,
    },
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    Doctor,
    Version,
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
    Backup {
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        include_auth: bool,
    },
    Restore {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        include_auth: bool,
        #[arg(long)]
        yes: bool,
    },
}
#[derive(Subcommand)]
enum ConfigCommand {
    Validate,
}
#[derive(Subcommand)]
enum AuthCommand {
    List,
    Rename {
        id: String,
        name: String,
    },
    Revoke {
        id: String,
    },
    RevokeAll {
        #[arg(long)]
        yes: bool,
    },
    CreateSetupToken,
    Sessions,
    RevokeSession {
        id: String,
    },
    RevokeAllSessions {
        #[arg(long)]
        all_apps: bool,
        #[arg(long)]
        yes: bool,
    },
    Migrate,
}
fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
fn snapshot(source: &Path, target: &Path) -> Result<()> {
    ensure!(!target.exists(), "backup destination already exists");
    let db =
        rusqlite::Connection::open_with_flags(source, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    db.backup("main", target, None)?;
    Ok(())
}
fn scrub_auth(path: &Path) -> Result<()> {
    let db = rusqlite::Connection::open(path)?;
    db.execute_batch("DELETE FROM sessions; DELETE FROM grants; PRAGMA wal_checkpoint(TRUNCATE);")?;
    Ok(())
}
fn lock(c: &Config) -> Result<fs::File> {
    let path = c.storage.business_db.with_extension("lock");
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    file.try_lock_exclusive().map_err(|_| {
        anyhow::anyhow!("business instance is running; stop it before this operation")
    })?;
    Ok(file)
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Command::Version = cli.command {
        println!(
            "bookmarkd {} · {} · schema 1",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::ARCH
        );
        return Ok(());
    }
    if let Command::Init {
        data_dir,
        public_url,
        rp_id,
        rp_name,
        user_name,
        setup_mode,
        allow_insecure_localhost,
    } = cli.command
    {
        ensure!(
            !data_dir.join("config.toml").exists()
                && !data_dir.join("auth.db").exists()
                && !data_dir.join("bookmarks.db").exists(),
            "refusing to overwrite initialized data"
        );
        let c = Config {
            server: Server {
                listen: "127.0.0.1:8765".into(),
                public_url,
                allow_insecure_localhost,
            },
            storage: Storage {
                business_db: "bookmarks.db".into(),
            },
            auth: Auth {
                app_id: "bookmarkd".into(),
                rp_id: rp_id.clone(),
                rp_name,
                user_name,
                user_id_file: "user-id.txt".into(),
                store: "auth.db".into(),
                setup_mode,
            },
        };
        c.validate()?;
        private_dir(&data_dir)?;
        let user_id = random();
        SqliteAuthStore::initialize(
            &data_dir.join("auth.db"),
            &Identity {
                rp_id,
                user_id: user_id.clone(),
            },
        )?;
        store::Store::init(&data_dir.join("bookmarks.db"))?;
        fs::write(data_dir.join("user-id.txt"), user_id)?;
        fs::write(data_dir.join("config.toml"), toml::to_string_pretty(&c)?)?;
        println!(
            "Initialized {}. Start serve, then auth create-setup-token.",
            data_dir.display()
        );
        return Ok(());
    }
    let mut c = Config::load(&cli.config)?;
    if let Command::Serve { listen, setup_mode } = &cli.command {
        if let Some(v) = listen {
            c.server.listen = v.clone();
        }
        if let Some(v) = setup_mode {
            c.auth.setup_mode = v.clone();
        }
    }
    c.validate()?;
    let auth = SqliteAuthStore::open(&c.auth.store, &c.identity()?)?;
    match cli.command {
        Command::Serve { .. } => {
            let _lock = lock(&c)?;
            let listener = tokio::net::TcpListener::bind(&c.server.listen).await?;
            eprintln!("bookmarkd listening on {}", c.server.listen);
            let app = http::App::new(c)?;
            axum::serve(
                listener,
                http::router(app).into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await?;
        }
        Command::Config { .. } | Command::Doctor => {
            http::App::new(c)?;
            println!("Configuration, identity and schema validated (schema 1).");
        }
        Command::Auth { command } => match command {
            AuthCommand::CreateSetupToken => {
                let token = auth.create_token(&c.auth.app_id, &c.server.public_url)?;
                println!(
                    "Setup URL: {}/setup\nMode: {} (token does not change mode)\nOne-time token (10 minutes):\n{}",
                    c.server.public_url, c.auth.setup_mode, token
                );
            }
            AuthCommand::List => {
                for x in auth.credentials()? {
                    println!("{}\t{}", x.id, x.name);
                }
            }
            AuthCommand::Rename { id, name } => auth.rename(&id, &name)?,
            AuthCommand::Revoke { id } => auth.revoke_credential(&id)?,
            AuthCommand::RevokeAll { yes } => {
                ensure!(
                    yes,
                    "--yes required: revokes credentials and sessions across ALL hosts"
                );
                for x in auth.credentials()? {
                    auth.revoke_credential(&x.id)?;
                }
            }
            AuthCommand::Sessions => {
                for x in auth.sessions(&c.auth.app_id, &c.server.public_url)? {
                    println!("{}\texpires={}", x.id, x.expires_at);
                }
            }
            AuthCommand::RevokeSession { id } => {
                auth.revoke_session(&id, &c.auth.app_id, &c.server.public_url)?
            }
            AuthCommand::RevokeAllSessions { all_apps, yes } => {
                ensure!(yes, "--yes required");
                if all_apps {
                    auth.connection()?.execute("DELETE FROM sessions", [])?;
                } else {
                    for x in auth.sessions(&c.auth.app_id, &c.server.public_url)? {
                        auth.revoke_session(&x.id, &c.auth.app_id, &c.server.public_url)?;
                    }
                }
            }
            AuthCommand::Migrate => {
                let _lock = lock(&c)?;
                auth.connection()?;
                println!("Auth schema 1 is current. Unknown schemas are refused.");
            }
        },
        Command::Backup {
            output,
            include_auth,
        } => {
            ensure!(!output.exists(), "backup destination must not exist");
            private_dir(&output)?;
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
            println!("Consistent SQLite snapshots saved to {}", output.display());
        }
        Command::Restore {
            input,
            include_auth,
            yes,
        } => {
            ensure!(
                yes,
                "--yes required; stop all shared-auth hosts before auth restore"
            );
            let _lock = lock(&c)?;
            let manifest: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(input.join("manifest.json"))?)?;
            ensure!(
                manifest["format"] == "bookmarkd-backup" && manifest["schema"] == 1,
                "unsupported backup"
            );
            let source = store::Store {
                path: input.join("bookmarks.db"),
            };
            source.read()?.validate()?;
            if include_auth {
                ensure!(manifest["auth"] == true, "backup has no auth");
                let identity = Identity {
                    rp_id: manifest["rp_id"].as_str().unwrap_or("").into(),
                    user_id: fs::read_to_string(input.join("user-id.txt"))?.trim().into(),
                };
                ensure!(
                    identity.rp_id == c.auth.rp_id && identity.user_id == c.identity()?.user_id,
                    "identity mismatch: restore only to the same RP and User Handle"
                );
                SqliteAuthStore::open(&input.join("auth.db"), &identity)?;
            }
            let rescue = c
                .storage
                .business_db
                .with_extension(format!("before-restore-{}.db", bookmarkd_auth::now()));
            snapshot(&c.storage.business_db, &rescue)?;
            {
                let src = rusqlite::Connection::open(input.join("bookmarks.db"))?;
                let mut dst = rusqlite::Connection::open(&c.storage.business_db)?;
                let backup = rusqlite::backup::Backup::new(&src, &mut dst)?;
                backup.run_to_completion(128, std::time::Duration::from_millis(5), None)?;
            }
            if include_auth {
                let src = rusqlite::Connection::open(input.join("auth.db"))?;
                let mut dst = auth.connection()?;
                let backup = rusqlite::backup::Backup::new(&src, &mut dst)?;
                backup.run_to_completion(128, std::time::Duration::from_millis(5), None)?;
                drop(backup);
                drop(dst);
                scrub_auth(&c.auth.store)?;
            } else {
                let db = auth.connection()?;
                db.execute("DELETE FROM sessions WHERE app=?1", [&c.auth.app_id])?;
                db.execute("DELETE FROM grants WHERE app=?1", [&c.auth.app_id])?;
            }
            println!(
                "Restored; old sessions and setup grants invalidated. Previous business snapshot: {}",
                rescue.display()
            );
        }
        Command::Version | Command::Init { .. } => unreachable!(),
    }
    Ok(())
}
