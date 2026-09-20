# Deployment, recovery and upgrades / 部署与恢复

For a lightweight deployment under your current user's `~/bookmarkd`, with user
systemd and a separate Caddy gateway, see [轻量部署指南（中文）](DEPLOY_SYSTEMD.zh-CN.md).
It keeps the application files together and does not require a dedicated account.

The dedicated-account layout below is an alternative for centrally managed hosts.
Use a dedicated OS account and a private local directory. `init` creates mode 0700
on Unix. On Windows, place data under an account-private directory and configure
its NTFS ACL. The application serves plain HTTP behind an HTTPS proxy. Use loopback for a
local proxy, or `serve --listen 0.0.0.0:8765` with access restricted to the gateway
for a separate LAN proxy;
the configured public URL must remain the browser's exact origin, without a trailing
slash. Preserve the original Host header. No proxy-supplied identity is trusted.

Example Caddy configuration:

```caddyfile
bookmark.example.com {
    reverse_proxy 127.0.0.1:8765
}
```

Example systemd service: [bookmarkd.service](../examples/bookmarkd.service).
Set `WorkingDirectory` and all paths for your machine. Do not expose the loopback
backend directly. Business instances use an OS file lock; a second writer against
the same business database fails at startup.

## Initialization / 初始化

```sh
bookmarkd init --data-dir /srv/bookmarkd --public-url https://bookmark.example.com --rp-id example.com --setup-mode auto
bookmarkd --config /srv/bookmarkd/config.toml config validate
bookmarkd --config /srv/bookmarkd/config.toml serve
# In another SSH/admin terminal:
bookmarkd --config /srv/bookmarkd/config.toml auth create-setup-token
```

Enter the token at `/setup`, register a Passkey, then explicitly log in. Set
`setup_mode = "disabled"` in config and restart after initialization. Tokens are
never added to URLs. A token does not override disabled mode.

Configuration paths are relative to the config file. `BOOKMARKD_CONFIG`,
`BOOKMARKD_LISTEN`, `BOOKMARKD_PUBLIC_URL`, and `BOOKMARKD_SETUP_MODE` override the
corresponding file values. `serve --listen` / `--setup-mode` take precedence over
environment values. Identity/name/storage settings are explicit TOML fields;
`init --help` lists initialization overrides. Unknown schema versions fail closed.
Version 0.2.1 has schema 1; `auth migrate` validates that version and does not perform
speculative or destructive migration of unknown schemas.

## Lost Passkey / 丢失 Passkey

1. SSH into the server. Check `auth list` and revoke compromised credentials with
   `auth revoke <id>` when appropriate. This affects every sharing host.
2. Explicitly set `setup_mode = "enabled"`, restart, and run
   `auth create-setup-token`. Creating a token alone never changes the running mode.
3. Register at `/setup`, verify a normal login, then disable Setup and restart.

## Backup and restore / 备份恢复

```sh
bookmarkd --config /srv/bookmarkd/config.toml backup --output /secure/backups/business-20260919
bookmarkd --config /srv/bookmarkd/config.toml backup --include-auth --output /secure/backups/full-20260919
# Stop the business server, and ALL sharing hosts for an auth restore:
bookmarkd --config /srv/bookmarkd/config.toml restore --input /secure/backups/full-20260919 --include-auth --yes
bookmarkd --config /srv/bookmarkd/config.toml doctor
```

Backups use SQLite Online Backup, not file copying of a live WAL database. Each
snapshot is individually consistent; business/auth snapshots are not a single
cross-database transaction. Auth backup copies contain identity/credentials but
have all sessions and setup grants removed. Restores validate schema and business
references first, preserve a pre-restore business snapshot, and invalidate old
sessions and grants. Auth restore requires the same RP and User Handle and explicit
`--include-auth --yes`. All shared hosts must remain stopped until it completes,
as their in-memory challenges must not survive an auth rollback.

For a new machine, initialize the target explicitly, stop it, and provision the
**original** `user-id.txt` and auth identity from the protected full backup before
starting it. Never generate a different RP/handle and expect the same Passkey to
work. Restore to the original public hostname/RP. Restoring old authentication
state can revive credentials revoked after that backup; review/revoke them before
reopening network access.

Business JSON export includes no authentication state and is not an auth backup.
Full JSON restore in the UI requires an empty library; it preserves entity IDs,
versions, folder relationships, timestamps, tags, ordering, trash and preferences.
Merge import remaps IDs. HTML carries browser-compatible hierarchy/title/URL/time;
notes and other app-only fields are not preserved by HTML export.

## Upgrade / 升级

Back up, stop, replace the executable, run `config validate` and `doctor`, then
restart. Keep the previous executable and backup for rollback. Restore data only
with a matching schema. The release manifests identify the actual runner OS and
link dependencies. Linux binaries use glibc from Ubuntu 24.04; they are **not**
claimed to run on older glibc or musl/Alpine. macOS/Windows binaries are unsigned.
