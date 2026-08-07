# Zeabur Web 快速验证

这是一个最小单容器 Web 快速验证适配路径。它不是桌面版发布路径，也不引入新的 kernel 概念或包边界。请把任何公网 URL 都视为不可信环境：使用一次性 volume、设置访问 token，不要在该快速验证 host 中保存真实 secret。

## 运行内容

仓库根目录的 `Dockerfile` 会构建：

- 使用 `npm ci` 和 `npm run build` 构建 `clients/web`
- 构建 Rust release `plurora` 二进制
- 在 runtime 镜像中用同一个端口提供 Web 静态文件和 host API
- 打包 Web 安装与 secret-store 快速验证所需的最小官方 lab manifest（`git-tools-lab`、`integrity-lab`、`install-lab`、`secret-store-lab`）

容器启动时执行：

```sh
plurora host serve --http 0.0.0.0:$PORT --data-dir /data --profile /data/profiles/default.yaml --static-dir /app/public --access-token "$PLURORA_HTTP_ACCESS_TOKEN"
```

同一个 HTTP 服务暴露：

- `GET /` 以及 `/app/public` 下的 Web 资源
- `POST /rpc`（配置 token 时需要 token）
- `GET /journal/subscribe/:session_id`（配置 token 时需要 token）
- `GET /surface-bundles/...`（需 Host 身份）与授权解析后短期可读的 `/surface-assets/<lease>/...`
- `GET /healthz`

设置 `PLURORA_HTTP_ACCESS_TOKEN` 后，`/rpc` 与 `/kernel/...` 路由需要 `Authorization: Bearer <token>`。浏览器 SSE 使用 `?access_token=<token>`，因为 EventSource 不能发送自定义 header。正常使用时，直接打开 Web URL，在登录页粘贴 token；Web client 会验证 token、保存到 `localStorage`，之后自动用于 RPC/SSE。作为可选 bootstrap，也可以第一次访问时在 URL 加上 `?plurora_token=<token>` 或 `?access_token=<token>`；Web client 会读取一次并从地址栏移除该参数。

原始 `/surface-bundles/...` 走 Host 身份与项目授权。sandboxed iframe 不携带 Host credential；授权后的 resolver 会返回随机、五分钟、绑定 grant/bundle root 的 `/surface-assets/<lease>/...` 只读路径，让 dynamic import、stylesheet、font、image 稳定加载。asset lease 仍不是 secret 容器，不要把 secret 放进 bundle 或 asset；能力与数据访问继续由 Host token、项目 authority 和 SurfaceHost bridge policy 保护。

## Zeabur 配置

- 服务类型：从仓库根目录使用 Dockerfile
- 端口：Zeabur 未自动注入时可设置 `PORT`，默认 `8080`
- 健康检查：`GET /healthz`
- Volume：将持久化存储挂载到 `/data`

推荐环境变量：

| 变量 | 默认值 | 用途 |
| --- | --- | --- |
| `PORT` | `8080` | HTTP 监听端口；entrypoint 绑定 `0.0.0.0:$PORT`。 |
| `PLURORA_DATA_DIR` | `/data` | 持久化 Plurora 数据目录。 |
| `PLURORA_PROFILE` | `default` | 在 `/data/profiles` 下创建/使用的安全 profile id；entrypoint 会拒绝不安全值。 |
| `PLURORA_STATIC_DIR` | `/app/public` | 要服务的已构建 Web 静态目录。 |
| `PLURORA_HTTP_ACCESS_TOKEN` | 未设置 | 公网 URL 强烈建议设置；保护 RPC/SSE/service 路由。 |
| `PLURORA_REQUIRE_ACCESS_TOKEN` | `0` | 设为 `1` 时，如果缺少 `PLURORA_HTTP_ACCESS_TOKEN`，容器会启动失败。 |
| `PLURORA_APP_BASE_DOMAIN` | 未设置 | 可选 wildcard app 域名，例如 `apps.example.com`；启用后部署项目可通过 `<slug>.apps.example.com/` 访问。 |

如果 `/data/profiles/$PLURORA_PROFILE.yaml` 不存在，entrypoint 会创建一个轻量 SQLite profile。也可以通过挂载或写入同一路径来替换为自定义 profile。

在 Zeabur/公网验证时，请设置随机 token，打开应用 URL 后在登录页输入该 token。不要复用生产凭据，也不要在该验证实例中保存真实 provider secret。URL token（`?plurora_token=<token>` 或 `?access_token=<token>`）仍可作为可选的一次性 bootstrap 路径使用。

如果要验证虚拟主机部署入口，需要把 wildcard DNS（例如 `*.apps.example.com`）指向该 Zeabur 服务，并设置 `PLURORA_APP_BASE_DOMAIN=apps.example.com`。Plurora 管理 UI 仍在主域上使用 token；项目应用域不要求 Plurora token。

## 本地 smoke test

```sh
docker build -t plurora-zeabur-quick .
docker run --rm -p 8080:8080 -v plurora-data:/data \
  -e PLURORA_HTTP_ACCESS_TOKEN=dev-token \
  -e PLURORA_REQUIRE_ACCESS_TOKEN=1 \
  plurora-zeabur-quick
curl http://127.0.0.1:8080/healthz
```

## 限制

- 仅用于快速验证；不替代正式桌面分发。
- access token 只是 quick-validation auth，不是生产级 session auth。
- Surface bundle/asset 通过短期只读 lease 暴露给 sandbox，但仍绝不要嵌入 secret。
- 该适配器不引入新的官方包 namespace 或安装模型。
- 公网验证请使用一次性 `/data` volume，测试结束后删除。
- 不要在公网 quick-validation URL 中输入真实 API key 或敏感 secret。
- 内置 Web 应用是静态构建；本地热更新仍使用现有 Vite 工作流。
