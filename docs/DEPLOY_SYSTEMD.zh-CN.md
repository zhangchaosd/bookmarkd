# 轻量部署：当前用户 + ~/bookmarkd + systemd

以 Ubuntu 24.04 为例，使用你当前的普通用户，不创建专用账户，不向 `/srv` 或 `/usr/local/bin` 安装程序。应用程序、配置、数据库、日志、备份及 service 文件都放在 `~/bookmarkd`。

以下命令在**部署用户自己的登录会话**中执行，`systemctl --user` 不加 sudo。网关上已有 Caddy，示例域名为 `bookmarks.example.com`，应用服务器 IP 为 `192.168.1.100`，请替换成实际值。

## 1. 准备程序

```bash
mkdir -p ~/bookmarkd
chmod 700 ~/bookmarkd
cd ~/bookmarkd
```

从 [v0.2.1 下载页](https://github.com/zhangchaosd/bookmarkd/releases/tag/v0.2.1) 下载对应的 Linux 文件和 `SHA256SUMS`，放到此目录。x86_64 选择 `bookmarkd-linux-x86_64`，ARM64 选择 `bookmarkd-linux-aarch64`。

以 x86_64 为例，校验成功后再执行安装命令：

```bash
sha256sum --check --ignore-missing SHA256SUMS
mv bookmarkd-linux-x86_64 bookmarkd
chmod +x bookmarkd
./bookmarkd version
```

ARM64 将上述文件名替换为 `bookmarkd-linux-aarch64`。运行不需要 Node.js、Rust 或外部数据库。Linux 制品使用 glibc，不能直接用于 Alpine；当前版本仍为预发布版。

## 2. 首次初始化

```bash
cd ~/bookmarkd
umask 077
./bookmarkd init \
  --data-dir ./data \
  --public-url https://bookmarks.example.com \
  --rp-id bookmarks.example.com \
  --setup-mode auto

./bookmarkd --config ./data/config.toml config validate
```

`public-url` 是浏览器实际访问的 HTTPS 地址，不带末尾 `/`；`rp-id` 只填写域名。注册 Passkey 后保持域名、RP ID 和 `data/user-id.txt` 不变。

**已有 `~/bookmarkd/data` 数据时跳过 init。** 从之前的 nohup 方式切换，只需先停止旧进程，保留原程序和数据，再执行下一步，不能同时运行两份实例。

## 3. 用用户级 systemd 启动

创建 `~/bookmarkd/bookmarkd.service`：

```bash
cat > ~/bookmarkd/bookmarkd.service <<'UNIT'
[Unit]
Description=bookmarkd

[Service]
Type=simple
WorkingDirectory=%h/bookmarkd
ExecStart=%h/bookmarkd/bookmarkd --config %h/bookmarkd/data/config.toml serve --listen 0.0.0.0:8765
Restart=on-failure
RestartSec=5
UMask=0077
StandardOutput=append:%h/bookmarkd/bookmarkd.log
StandardError=append:%h/bookmarkd/bookmarkd.log

[Install]
WantedBy=default.target
UNIT

systemctl --user enable --now "$HOME/bookmarkd/bookmarkd.service"
systemctl --user status bookmarkd --no-pager
```

`%h` 由 systemd 展开为当前用户的主目录。上面的配置也可从 [用户级服务模板](../examples/bookmarkd-user.service) 获取。

为了退出 SSH 后继续运行，并在服务器重启后自动启动，执行一次：

```bash
sudo loginctl enable-linger "$USER"
loginctl show-user "$USER" -p Linger
```

输出应为 `Linger=yes`。这是唯一需要管理员权限的应用服务设置；没有 sudo 权限时，请管理员为你的用户名开启 linger。主目录需在开机时可访问。

systemd 会在 `~/.config/systemd/user/` 下创建指向此 service 文件的链接；实际 service 文件和所有应用数据仍在 `~/bookmarkd`。这些注册链接是用户级 systemd 自动启动所必需的。

## 4. 网关 Caddy 反向代理

在**网关**现有 Caddyfile 中加入：

```caddyfile
bookmarks.example.com {
    reverse_proxy 192.168.1.100:8765
}
```

按网关原有方式检查并重载 Caddy。例如使用标准 systemd 服务时：

```bash
sudo caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
sudo systemctl reload caddy
```

域名解析到网关，使用浏览器信任的 HTTPS 证书；Caddy 到应用走局域网 HTTP，保留原始 Host。8765 仅需允许网关访问，不需要映射到公网。Caddy 的文件继续留在网关，不属于应用服务器的 `~/bookmarkd` 目录。

检查后端和代理入口：

```bash
curl -fsS http://127.0.0.1:8765/healthz
curl -fsS https://bookmarks.example.com/healthz
```

两者应返回包含 `"status":"ok"` 的结果。Caddy 返回 502 时，先从网关检查 `http://192.168.1.100:8765/healthz` 是否可达。参考 [Caddy 反向代理文档](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy)。

## 5. 注册 Passkey

在应用服务器执行：

```bash
cd ~/bookmarkd
./bookmarkd --config ./data/config.toml auth create-setup-token
```

浏览器打开 `https://bookmarks.example.com/setup`，输入令牌，注册 Passkey，再正常登录。

登录验证成功后，编辑 `~/bookmarkd/data/config.toml`，将已有 `[auth]` 下的 `setup_mode` 改为 `"disabled"`，然后：

```bash
systemctl --user restart bookmarkd
```

以后需要新增或恢复 Passkey，再临时改为 `enabled`、重启、生成令牌并注册，完成后改回 `disabled` 并重启。

## 6. 日常使用、备份和升级

日常管理（按需选择执行）：

```bash
systemctl --user status bookmarkd --no-pager
systemctl --user restart bookmarkd
systemctl --user stop bookmarkd
systemctl --user start bookmarkd
tail -f ~/bookmarkd/bookmarkd.log
```

应用输出追加到 `~/bookmarkd/bookmarkd.log`。如果启动失败而日志为空，查看 `journalctl --user -u bookmarkd -n 50 --no-pager`。日志文件不会自动轮转，定期检查大小；需要清空时可执行 `truncate -s 0 ~/bookmarkd/bookmarkd.log`。

修改 service 文件后执行 `systemctl --user daemon-reload`，再重启服务；只修改应用配置时直接重启即可。

完整备份（可在服务运行时执行）：

```bash
(
  set -eu
  cd ~/bookmarkd
  umask 077
  mkdir -p backups
  backup_path="./backups/full-$(date +%Y%m%d-%H%M%S)-$$"
  ./bookmarkd --config ./data/config.toml \
    backup --include-auth --output "$backup_path"
  cp ./data/config.toml "$backup_path/config.toml"
)
```

备份包含业务库、认证库、用户身份，并额外保存配置。定期执行并把已完成备份复制到另一台设备；不要直接复制运行中的数据库文件。恢复步骤见 [备份恢复说明](OPERATIONS.md#backup-and-restore--备份恢复)，将其中路径替换为本部署的路径，并使用 `systemctl --user stop/start bookmarkd` 停启。

升级时：先备份，下载并校验新程序，停止服务，保留旧程序后替换 `~/bookmarkd/bookmarkd`，执行 `chmod +x`、`config validate` 和 `doctor`，检查成功后启动服务。数据目录保持原样；不要重新 init。旧程序和升级前备份均可保存在 `~/bookmarkd` 中。

最终目录大致如下：

```text
~/bookmarkd/
├── bookmarkd
├── bookmarkd.service
├── bookmarkd.log
├── SHA256SUMS
├── data/
│   ├── config.toml
│   ├── bookmarks.db
│   ├── auth.db
│   └── user-id.txt
└── backups/
```
