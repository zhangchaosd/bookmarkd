# 使用 systemd 和网关 Caddy 部署 bookmarkd

本文针对 **Ubuntu 24.04、systemd、独立网关 Caddy**，部署版本为 **v0.2.1 预发布版**。其他发行版需调整包管理、用户创建及防火墙命令；Linux 制品使用 glibc，不适用于 Alpine/musl，也不保证兼容旧版 glibc。

以下是部署操作说明，不会自动修改你现有的服务器。除明确标注“网关”外，命令均在 bookmarkd 服务器上通过具有 sudo 权限的管理员账户执行。不要使用 root 运行应用进程。

## 1. 确认域名、网络和目录

| 项目 | 本文示例 |
|---|---|
| 浏览器访问地址 | `https://bookmarks.example.com` |
| bookmarkd 服务器 IP | `192.168.1.100` |
| Caddy 网关 IP | `192.168.1.1` |
| 后端监听 | `0.0.0.0:8765`，明文 HTTP |
| 程序 | `/usr/local/bin/bookmarkd` |
| 配置和数据库 | `/srv/bookmarkd` |
| 运行账户 | `bookmarkd` |
| 本机备份 | `/var/backups/bookmarkd` |

将所有示例域名和 IP 换成实际值。浏览器通过 HTTPS 域名访问网关，网关通过局域网 HTTP 连接后端；`0.0.0.0` 是监听地址，不是 Caddy 的代理目标。

域名必须解析到客户端可访问的网关地址，HTTPS 证书必须被客户端信任。Caddy 自动签发公网证书需要满足相应 ACME 验证条件；纯内网环境可沿用网关已有的 DNS 验证方案或受客户端信任的内部 CA。不要忽略证书错误后继续配置 Passkey。参见 [Caddy 自动 HTTPS 文档](https://caddyserver.com/docs/automatic-https)。

后端 TCP 8765 仅允许网关访问，不映射到公网。通过现有防火墙/ACL 设置“允许网关 IP → 8765、拒绝其他来源 → 8765”，保留管理员 SSH 规则；如果网关使用容器或 NAT，以服务器实际看到的来源 IP 为准。本文不直接启用或重置防火墙，避免影响已有网络规则。

本版本仍为预发布：实机 Passkey 兼容性矩阵和独立安全审查尚未完成，见 [验证报告](TESTING.md)。

## 2. 下载、校验并安装程序

先安装下载所需工具：

```bash
sudo apt-get update
sudo apt-get install -y ca-certificates curl
```

在同一个 Bash 会话中执行，自动选择 x86_64 或 ARM64 制品：

```bash
(
  set -eu
  release_version=v0.2.1
  case "$(uname -m)" in
    x86_64) release_asset=bookmarkd-linux-x86_64 ;;
    aarch64|arm64) release_asset=bookmarkd-linux-aarch64 ;;
    *) echo '不支持的 CPU 架构'; exit 1 ;;
  esac
  release_dir=$(mktemp -d)
  cd "$release_dir"
  release_base="https://github.com/zhangchaosd/bookmarkd/releases/download/$release_version"
  curl -fLO "$release_base/$release_asset"
  curl -fLO "$release_base/SHA256SUMS"
  sha256sum --check --ignore-missing SHA256SUMS
  sudo install -o root -g root -m 0755 "$release_asset" /usr/local/bin/bookmarkd
  /usr/local/bin/bookmarkd version
)
```

校验应显示所选制品 `OK`，程序应显示 `bookmarkd 0.2.1`。程序内嵌前端，无需安装 Node.js、Rust 或外部数据库。下载文件暂存在输出命令使用的临时目录，不包含业务数据。

## 3. 创建专用账户和初始化数据

```bash
id bookmarkd >/dev/null 2>&1 || sudo useradd \
  --system --user-group --home-dir /srv/bookmarkd \
  --no-create-home --shell /usr/sbin/nologin bookmarkd

sudo install -d -o bookmarkd -g bookmarkd -m 0700 /srv/bookmarkd
```

**下面的 init 仅用于首次部署。已有收藏或认证数据时不要执行，改看第 10 节。**

```bash
sudo -u bookmarkd /usr/local/bin/bookmarkd init \
  --data-dir /srv/bookmarkd \
  --public-url https://bookmarks.example.com \
  --rp-id bookmarks.example.com \
  --setup-mode auto

sudo -u bookmarkd /usr/local/bin/bookmarkd \
  --config /srv/bookmarkd/config.toml config validate
```

生成的 `config.toml`、`bookmarks.db`、`auth.db` 和 `user-id.txt` 均由服务账户管理。数据库使用本机磁盘；保持目录为私有目录。

`server.public_url` 必须是浏览器实际使用的完整 origin，不带路径和末尾 `/`；非默认 HTTPS 端口需要写入 URL。`auth.rp_id` 仅填写域名，不带协议或端口。注册后不要随意改变 RP ID 或 `user-id.txt`，否则原 Passkey 无法继续正常使用。

## 4. 创建 systemd 服务

创建服务文件：

```bash
sudo tee /etc/systemd/system/bookmarkd.service >/dev/null <<'UNIT'
[Unit]
Description=Private bookmark hub
After=network.target

[Service]
Type=simple
User=bookmarkd
Group=bookmarkd
WorkingDirectory=/srv/bookmarkd
ExecStart=/usr/local/bin/bookmarkd --config /srv/bookmarkd/config.toml serve --listen 0.0.0.0:8765
Restart=on-failure
RestartSec=5
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/srv/bookmarkd
StandardOutput=journal
StandardError=journal
SyslogIdentifier=bookmarkd

[Install]
WantedBy=multi-user.target
UNIT

sudo systemd-analyze verify /etc/systemd/system/bookmarkd.service
sudo systemctl daemon-reload
sudo systemctl enable --now bookmarkd.service
sudo systemctl status bookmarkd.service --no-pager
```

应显示 `active (running)`。`enable` 设置开机启动，`Restart=on-failure` 用于异常退出后重启；手动执行 `systemctl stop` 不会被自动拉起。已有 nohup 实例时，先停止旧实例再启动 systemd，避免占用端口和数据库锁。

本文服务显式使用 `--listen 0.0.0.0:8765`，其优先级高于配置文件中的监听地址。无需改动 init 默认生成的 `127.0.0.1:8765`。如果 Caddy 和应用在同一台主机，可将服务监听参数和 Caddy 上游都改为 `127.0.0.1:8765`。

本例使用与 [原有服务模板](../examples/bookmarkd.service) 相同的权限隔离，增加局域网监听和明确的 journal 输出。服务仅允许写入 `/srv/bookmarkd`；不要把数据库放在被 `ProtectHome=true` 隔离的 `/home` 中。

## 5. 配置网关 Caddy

**以下操作在 Caddy 网关上执行。** 将站点块加入现有 `/etc/caddy/Caddyfile`，保留其他站点配置：

```caddyfile
bookmarks.example.com {
    reverse_proxy 192.168.1.100:8765
}
```

该上游默认使用 HTTP 并保留浏览器的 Host，无需把 Host 改为局域网 IP，也不需要开启 CORS。不要给认证和私有数据接口添加代理缓存。参见 [Caddy reverse_proxy 文档](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy)。

```bash
sudo caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
sudo systemctl reload caddy
```

这假定网关的 Caddy 由标准 systemd 服务管理；Docker 或其他管理方式应使用对应的配置重载方式。`validate` 验证成功后再重载，见 [Caddy 命令文档](https://caddyserver.com/docs/command-line)。

## 6. 验证网络并注册 Passkey

在 bookmarkd 服务器检查后端：

```bash
curl -fsS http://127.0.0.1:8765/healthz
sudo journalctl -u bookmarkd.service -n 50 --no-pager
```

在网关检查后端连通性：

```bash
curl -fsS http://192.168.1.100:8765/healthz
```

在客户端检查 HTTPS：

```bash
curl -fsS https://bookmarks.example.com/healthz
```

均应返回包含 `"status":"ok"` 的 JSON。健康检查只证明服务可达，接着需要实际完成注册和登录。

在 bookmarkd 服务器生成一次性令牌：

```bash
sudo -u bookmarkd /usr/local/bin/bookmarkd \
  --config /srv/bookmarkd/config.toml auth create-setup-token
```

浏览器打开 `https://bookmarks.example.com/setup`，在表单中输入令牌并注册 Passkey，然后正常登录。不要把令牌放入 URL 或公开日志。确认可以添加收藏、刷新后仍存在，且退出后能重新登录。

注册成功后关闭注册入口：

```bash
sudoedit /srv/bookmarkd/config.toml
```

将已有 `[auth]` 中的 `setup_mode = "auto"` 改为 `setup_mode = "disabled"`，保留其他字段，然后：

```bash
sudo -u bookmarkd /usr/local/bin/bookmarkd \
  --config /srv/bookmarkd/config.toml config validate
sudo systemctl restart bookmarkd.service
sudo systemctl is-active bookmarkd.service
```

再次确认登录正常。以后添加或恢复 Passkey 时，管理员临时将该字段设为 `enabled`、重启、生成令牌并注册，完成后重新设为 `disabled` 并重启。单独生成令牌不会解除 disabled 状态。

## 7. 日常管理和日志

```bash
sudo systemctl status bookmarkd.service --no-pager
sudo systemctl restart bookmarkd.service
sudo systemctl stop bookmarkd.service
sudo systemctl start bookmarkd.service
sudo journalctl -u bookmarkd.service -f
sudo journalctl -u bookmarkd.service --since today --no-pager
```

以上命令分别用于查看、重启、停止、启动和查看日志，按需执行，不必整段运行。systemd 直接管理前台进程，不需要 nohup、`&` 或 PID 文件。

日志由 journald 管理，保留时长和磁盘上限使用服务器现有策略。如果需要重启后仍保留日志，检查 journald 是否启用了持久存储。临时导出日志到文件：

```bash
sudo journalctl -u bookmarkd.service --since today --no-pager > bookmarkd.log
```

## 8. 完整备份和定时执行

完整备份包含业务库、认证库及 `user-id.txt`，但**不包含配置文件**，因此下面脚本额外复制 `config.toml`。备份中的会话和注册授权会被清理。JSON/HTML 导出不能代替认证备份。

```bash
sudo install -d -o bookmarkd -g bookmarkd -m 0700 /var/backups/bookmarkd
sudo tee /usr/local/bin/bookmarkd-backup >/dev/null <<'SCRIPT'
#!/bin/sh
set -eu
umask 077
backup_path="/var/backups/bookmarkd/full-$(date -u +%Y%m%dT%H%M%SZ)-$$"
/usr/local/bin/bookmarkd --config /srv/bookmarkd/config.toml \
  backup --include-auth --output "$backup_path"
cp /srv/bookmarkd/config.toml "$backup_path/config.toml"
printf 'Backup complete: %s\n' "$backup_path"
SCRIPT
sudo chmod 0755 /usr/local/bin/bookmarkd-backup
sudo -u bookmarkd /usr/local/bin/bookmarkd-backup
```

备份可以在线运行，使用 SQLite Backup API；不要直接复制运行中数据库和 WAL 文件。业务库与认证库各自一致，但不是跨两个数据库的单一事务快照。

创建每天凌晨 03:00 执行的服务及定时器（使用服务器本地时区）：

```bash
sudo tee /etc/systemd/system/bookmarkd-backup.service >/dev/null <<'UNIT'
[Unit]
Description=Back up bookmarkd data and authentication

[Service]
Type=oneshot
User=bookmarkd
Group=bookmarkd
ExecStart=/usr/local/bin/bookmarkd-backup
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/srv/bookmarkd /var/backups/bookmarkd
UNIT

sudo tee /etc/systemd/system/bookmarkd-backup.timer >/dev/null <<'UNIT'
[Unit]
Description=Daily bookmarkd backup

[Timer]
OnCalendar=*-*-* 03:00:00
Persistent=true
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
UNIT

sudo systemd-analyze verify /etc/systemd/system/bookmarkd-backup.service /etc/systemd/system/bookmarkd-backup.timer
sudo systemctl daemon-reload
sudo systemctl enable --now bookmarkd-backup.timer
sudo systemctl start bookmarkd-backup.service
sudo systemctl list-timers bookmarkd-backup.timer --all
sudo journalctl -u bookmarkd-backup.service -n 30 --no-pager
```

备份服务的两个可写目录用于 SQLite 源库访问及备份输出；备份不依赖网页服务处于启动状态。`Persistent=true` 会补跑关机期间错过的一次计划任务，随机延迟最多 5 分钟。

定期把已完成的备份目录复制到另一台设备的受限目录；设置磁盘用量监控、备份保留周期及失败告警。本例不会自动删除旧备份。至少做一次恢复演练后再依赖定时备份。

## 9. 恢复、升级与回滚

### 同机恢复

先选择已验证的完整备份目录，下面路径需要替换：

```bash
sudo systemctl stop bookmarkd-backup.timer
sudo systemctl stop bookmarkd-backup.service
sudo systemctl stop bookmarkd.service
sudo -u bookmarkd /usr/local/bin/bookmarkd \
  --config /srv/bookmarkd/config.toml restore \
  --input /var/backups/bookmarkd/full-REPLACE-ME \
  --include-auth --yes
sudo -u bookmarkd /usr/local/bin/bookmarkd \
  --config /srv/bookmarkd/config.toml doctor
```

逐条执行并检查结果，恢复失败时不要继续启动。恢复要求相同 RP ID 和 User Handle；若多个应用共享认证库，认证恢复前必须停止所有共享应用。恢复会使旧会话失效，需要重新登录；较早备份可能包含后来已撤销的凭据，应在开放访问前检查并撤销。

检查通过后：

```bash
sudo systemctl start bookmarkd.service
sudo systemctl start bookmarkd-backup.timer
```

跨机器恢复还需保留原始 `user-id.txt`、认证身份和域名，不能用新生成的用户身份替代。具体要求见 [恢复说明](OPERATIONS.md#backup-and-restore--备份恢复)。

### 升级

1. 阅读目标版本说明，下载并校验制品；不要直接覆盖运行中的可执行文件。
2. 停止备份定时器，等待正在运行的备份结束；手动执行一次完整备份并确认成功。
3. 停止 `bookmarkd.service`，保留旧二进制、配置和备份。
4. 安装新二进制，保持 root 所有、0755 权限；以 bookmarkd 用户执行 `config validate` 和 `doctor`。
5. 启动服务，检查健康接口、登录和已有收藏，再恢复备份定时器。

本次 v0.2.1 数据结构仍为 schema 1，不需要迁移；未来版本以各自发布说明为准。回滚前停止服务，使用与数据 schema 兼容的旧二进制；如需还原数据，执行上述恢复流程，不要让旧版本直接打开未知 schema。

## 10. 从已有 nohup 部署迁移

不要初始化第二份身份，也不要在运行中直接复制数据库。推荐先用原实例的 CLI 做完整备份，再停止 nohup 进程并确认退出。在停机期间复制**整个原数据目录**到 `/srv/bookmarkd`（包含配置、身份、数据库和仍存在的 WAL/SHM 文件），保留原目录作为回退。

将目标目录及文件的所有者改为 `bookmarkd:bookmarkd`，目录限制为 0700，数据文件限制为 0600。核对 `config.toml` 中的路径：相对路径按配置文件目录解析，原有绝对路径必须调整。保持 `public_url`、RP ID 和原 `user-id.txt` 不变。此后从第 4 节开始安装 systemd 服务，不执行 init。

## 11. 常见故障

| 现象 | 检查位置 |
|---|---|
| Caddy 返回 502 | 服务是否 active、网关能否访问 `192.168.1.100:8765/healthz`、后端监听及防火墙 |
| 地址占用或数据库锁错误 | 是否还有旧 nohup 实例或另一实例使用同一业务库 |
| systemd 退出，提示 Permission denied | 数据目录所有者、程序可执行权限、是否使用了 `/home` 或未加入 ReadWritePaths 的自定义数据路径 |
| 页面可打开，但注册/登录失败 | HTTPS 证书信任、实际访问地址与 public_url、RP ID、Host 是否被重写 |
| Setup 页面不可注册 | setup_mode 是否 disabled；修改后是否重启；令牌是否过期或已使用 |
| 命令行能跑，服务不能跑 | `systemctl cat bookmarkd` 与 journal；服务运行账户和工作目录与 SSH 会话不同 |
| 修改配置后仍监听原地址 | 本文 ExecStart 显式传入了 --listen；修改服务后需 daemon-reload 并 restart |
| 备份失败 | 备份 service 的 journal、源库及目标目录权限、磁盘空间 |

systemd 配置含义参见 [systemd.exec](https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html) 和 [systemd.timer](https://www.freedesktop.org/software/systemd/man/latest/systemd.timer.html)；本机也可用 `man systemd.exec`、`man systemd.timer` 查看已安装版本。
