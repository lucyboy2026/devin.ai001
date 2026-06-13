# 分步部署 + 端到端实测指南

本文是「照着做」的分步手册：在一台全新的 Linux VPS 上把 node-auth 服务端跑起来，并完整验证
`注册 → 后台授权 → 登录 → 订阅/配置注入 → hysteria2 鉴权回调` 全流程。

- 偏简洁的部署参考见 [`DEPLOY.md`](./DEPLOY.md)；本文更偏「带预期输出的实测演练」。
- 示例 VPS 为 `77.73.8.38`、端口 `8080`，请按你的实际情况替换。
- 每个阶段末尾标注了「预期输出」，对不上就先停下排查，不要继续下一步。

---

## 阶段 1 · 装依赖 + 拉代码 + 编译

```bash
# 登录 VPS（在你本机执行）
ssh root@77.73.8.38

# 以下在 VPS 上执行
apt-get update && apt-get install -y git curl build-essential pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
rustc --version && cargo --version

rm -rf /opt/nodeauth
git clone https://github.com/lucyboy2026/devin.ai001.git /opt/nodeauth
cd /opt/nodeauth/server

cargo build --release            # 首次约 3-8 分钟
ls -lh /opt/nodeauth/server/target/release/nodeauth-server
```

**预期输出**：`cargo build` 结尾出现 `Finished release [optimized] target(s) in ...`，且最后 `ls -lh` 能看到
`nodeauth-server` 可执行文件（约 100+ MB）。

---

## 阶段 2 · 配置 `.env` + systemd 守护 + 健康检查

先写一个用于实测的最小 `.env`（不启用邮件/Telegram，先把核心跑通）：

```bash
cd /opt/nodeauth/server
cp .env.example .env

# 用实测配置覆盖关键项（无域名场景，直连 IP:8080）
cat >.env <<'EOF'
BIND_ADDR=0.0.0.0:8080
DATABASE_URL=sqlite://data/nodeauth.db
PUBLIC_BASE_URL=http://77.73.8.38:8080

ADMIN_USERNAME=admin
ADMIN_PASSWORD=请改成强密码

DEFAULT_MAX_DEVICES=1
DEFAULT_VALID_DAYS=30
TOKEN_TTL_DAYS=7
EOF

mkdir -p /opt/nodeauth/server/data
```

> 注意：`ADMIN_PASSWORD` 务必改成强密码，这是后台 `/admin` 登录密码。
> 有域名时改用 `PUBLIC_BASE_URL=https://你的域名`、`BIND_ADDR=127.0.0.1:8080`（见阶段 5 的 Caddy）。

配置 systemd 守护并启动：

```bash
cat >/etc/systemd/system/nodeauth.service <<'EOF'
[Unit]
Description=Clash Verge node-auth server
After=network.target

[Service]
WorkingDirectory=/opt/nodeauth/server
EnvironmentFile=/opt/nodeauth/server/.env
ExecStart=/opt/nodeauth/server/target/release/nodeauth-server
Restart=always
RestartSec=3
User=root

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now nodeauth
systemctl status nodeauth --no-pager
curl -s http://127.0.0.1:8080/healthz && echo
```

**预期输出**：`systemctl status` 显示 `active (running)`；`curl .../healthz` 返回 `ok`。

排查：`journalctl -u nodeauth -n 50 --no-pager` 看启动日志。

---

## 阶段 3 · 放行端口 / 连通性

无域名直连场景需放行 `8080`（有域名走 Caddy 443 时则不用对外开 8080）：

```bash
# ufw（若启用了防火墙）
ufw allow 8080/tcp || true
# 云厂商（阿里云/腾讯云/AWS 等）还需在控制台「安全组」放行 8080
```

在**你本机**验证公网可达：

```bash
curl -s http://77.73.8.38:8080/healthz && echo
```

**预期输出**：本机 curl 返回 `ok`。返回不了多半是云安全组没放行 8080。

---

## 阶段 4 · 端到端实测

下面用 `alice@example.com` 走一遍完整流程。**阶段 4.1 / 4.3 / 4.4 在 VPS 上用 curl 跑，4.2 在浏览器里点。**

### 4.1 注册（应为 202 pending）

```bash
B=http://127.0.0.1:8080
FP=test-device-001
curl -s -o /tmp/reg.json -w "HTTP %{http_code}\n" -X POST $B/register \
  -H 'Content-Type: application/json' \
  -d "{\"email\":\"alice@example.com\",\"password\":\"hunter2\",\"device_fp\":\"$FP\",\"platform\":\"linux\"}"
cat /tmp/reg.json; echo
```

**预期**：`HTTP 202`，body 里 `"status":"pending"`。

### 4.2 后台授权（浏览器）

1. 打开 `http://77.73.8.38:8080/admin`，用 `.env` 里的 `ADMIN_USERNAME / ADMIN_PASSWORD` 登录。
2. 在用户列表找到 `alice@example.com`，在「设备/天数」处填好（例如设备 `2`、天数 `30`），点 **授权/更新**。
3. 该用户状态应变为 `active`。

> 也可以纯命令行授权（无需浏览器）：先登录拿 cookie 再授权 `id=1`：
> ```bash
> B=http://127.0.0.1:8080
> curl -s -i -X POST $B/admin/login \
>   -d 'username=admin&password=请改成强密码' \
>   | grep -i '^set-cookie' | head -1 | sed 's/set-cookie: //I' | cut -d';' -f1 > /tmp/cookie.txt
> curl -s -o /dev/null -w "HTTP %{http_code}\n" -X POST $B/admin/users/1/authorize \
>   -H "Cookie: $(cat /tmp/cookie.txt)" -d 'max_devices=2&valid_days=30'
> ```
> **预期**：授权返回 `HTTP 303`。

### 4.3 登录 → 拿 Token + 订阅链接

```bash
B=http://127.0.0.1:8080
FP=test-device-001
curl -s -X POST $B/login -H 'Content-Type: application/json' \
  -d "{\"email\":\"alice@example.com\",\"password\":\"hunter2\",\"device_fp\":\"$FP\",\"platform\":\"linux\"}" > /tmp/login.json
cat /tmp/login.json; echo
TOKEN=$(grep -o '"token":"[0-9a-f]*"' /tmp/login.json | cut -d'"' -f4)
echo "TOKEN=$TOKEN"
```

**预期**：返回 `token`（64 位 hex）、`expires_at`、`subscription_url` 等；`TOKEN=...` 非空。

### 4.4 验证 Token 已注入 hysteria2 配置

```bash
B=http://127.0.0.1:8080
# /config?token= —— 该设备实时配置
curl -s "$B/config?token=$TOKEN" | grep -E 'type: hysteria2|password'
echo "--- 断言 ---"
curl -s "$B/config?token=$TOKEN" | grep -q "password: $TOKEN" \
  && echo "PASS: /config 注入了设备 Token" || echo "FAIL"

# /sub/{key} —— 长期固定订阅链接
SUB=$(grep -o '"subscription_url":"[^"]*"' /tmp/login.json | cut -d'"' -f4)
curl -s "$SUB" | grep -q "password: $TOKEN" \
  && echo "PASS: /sub 注入了最近活跃 Token" || echo "FAIL"

# /auth —— 模拟 hysteria2 鉴权回调
curl -s -X POST $B/auth -H 'Content-Type: application/json' -d "{\"auth\":\"$TOKEN\"}"; echo
```

**预期**：两个 `PASS`；`/auth` 返回 `{"ok":true,"id":"alice@example.com#test-device-001","msg":""}`。

### 4.5 负向用例（可选但建议）

```bash
B=http://127.0.0.1:8080
# 错误密码 -> 401
curl -s -o /dev/null -w "wrong-pass: HTTP %{http_code}\n" -X POST $B/login \
  -H 'Content-Type: application/json' \
  -d '{"email":"alice@example.com","password":"WRONG","device_fp":"test-device-001"}'
# 无效 token 取配置 -> 401
curl -s -o /dev/null -w "bad-token /config: HTTP %{http_code}\n" "$B/config?token=deadbeef"
# /auth 坏 token -> ok:false
curl -s -X POST $B/auth -H 'Content-Type: application/json' -d '{"auth":"deadbeef"}'; echo
```

**预期**：`HTTP 401`、`HTTP 401`、`{"ok":false,...}`。

---

## 阶段 5 · 可选增强

### 5.1 接入 hysteria2 节点（让真实节点用本服务鉴权）

在 hysteria2 **服务端** `config.yaml`：

```yaml
auth:
  type: http
  http:
    url: http://127.0.0.1:8080/auth   # 同机；或 https://你的域名/auth
```

原理：客户端登录后把设备 Token 写进节点 `password`；hysteria2 收到连接回调 `/auth`，本服务校验
「Token→设备→账号」有效后返回 `{"ok":true,...}`，否则拒绝。

### 5.2 HTTPS（有域名，推荐 Caddy 自动证书）

```bash
apt-get install -y debian-keyring debian-archive-keyring apt-transport-https
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | tee /etc/apt/sources.list.d/caddy-stable.list
apt-get update && apt-get install -y caddy

cat >/etc/caddy/Caddyfile <<'EOF'
你的域名 {
    reverse_proxy 127.0.0.1:8080
}
EOF
systemctl restart caddy
```

随后把 `.env` 改为 `BIND_ADDR=127.0.0.1:8080`、`PUBLIC_BASE_URL=https://你的域名`，`systemctl restart nodeauth`。

### 5.3 邮件 / Telegram 通知（可选）

在 `.env` 填好 `SMTP_*` 与 `TELEGRAM_*`（获取方式见 [`DEPLOY.md`](./DEPLOY.md) 第 3 节），`systemctl restart nodeauth`。
有公网 HTTPS 后注册 Telegram webhook：

```bash
curl "https://api.telegram.org/bot<TOKEN>/setWebhook?url=https://你的域名/tg/webhook"
```

---

## 升级

```bash
cd /opt/nodeauth && git pull
cd server && cargo build --release
systemctl restart nodeauth
```

## 常用排查

```bash
systemctl status nodeauth --no-pager       # 运行状态
journalctl -u nodeauth -n 100 --no-pager   # 最近日志
curl -s http://127.0.0.1:8080/healthz      # 健康检查
```
