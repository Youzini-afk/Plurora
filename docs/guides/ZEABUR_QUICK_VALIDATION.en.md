# Zeabur Web Quick Validation

This deployment path is a minimal single-container adapter for quick web validation. It is not the desktop release path and is not intended to define new kernel concepts or package boundaries. Treat any public URL as hostile: use a throwaway volume, set an access token, and do not put real secrets into this quick-validation host.

## What it runs

The root `Dockerfile` builds:

- `clients/web` with `npm ci` and `npm run build`
- the Rust `plurora` release binary
- a runtime image that serves the built web app and host API on one port
- the minimal official lab manifests needed for web install and secret-store quick validation (`git-tools-lab`, `integrity-lab`, `install-lab`, `secret-store-lab`)

At runtime the container starts:

```sh
plurora host serve --http 0.0.0.0:$PORT --data-dir /data --profile /data/profiles/default.yaml --static-dir /app/public --access-token "$PLURORA_HTTP_ACCESS_TOKEN"
```

The same HTTP service exposes:

- `GET /` and web assets from `/app/public`
- `POST /rpc` (requires token when configured)
- `GET /journal/subscribe/:session_id` (requires token when configured)
- `GET /surface-bundles/...` (Host identity required) and short-lived `/surface-assets/<lease>/...` reads after authorized resolution
- `GET /healthz`

When `PLURORA_HTTP_ACCESS_TOKEN` is set, `/rpc` and `/kernel/...` routes require `Authorization: Bearer <token>`. Browser SSE uses `?access_token=<token>` because EventSource cannot send custom headers. Normally, open the web URL directly and paste the token into the login screen; the web client validates it, stores it in `localStorage`, and sends it on future RPC/SSE calls. As an optional bootstrap path, you can also visit once with `?plurora_token=<token>` or `?access_token=<token>`; the web client reads it once and scrubs it from the address bar.

Raw `/surface-bundles/...` paths cross Host identity and project authorization. A sandboxed iframe carries no Host credential; after authorization, the resolver returns a random, five-minute, read-only `/surface-assets/<lease>/...` path bound to the grant and bundle root so dynamic imports, stylesheets, fonts, and images load reliably. An asset lease is still not a secret container, so do not put secrets in bundles or assets. Host credentials, project authority, and SurfaceHost bridge policy continue to protect capabilities and data.

## Zeabur settings

- Service type: Dockerfile from the repository root
- Port: set `PORT` if Zeabur does not inject it automatically; default is `8080`
- Health check: `GET /healthz`
- Volume: mount persistent storage at `/data`

Recommended environment variables:

| Variable | Default | Purpose |
| --- | --- | --- |
| `PORT` | `8080` | Public HTTP listen port; entrypoint binds `0.0.0.0:$PORT`. |
| `PLURORA_DATA_DIR` | `/data` | Persistent Plurora data directory. |
| `PLURORA_PROFILE` | `default` | Safe profile id created under `/data/profiles`; entrypoint rejects unsafe values. |
| `PLURORA_STATIC_DIR` | `/app/public` | Built web static directory to serve. |
| `PLURORA_HTTP_ACCESS_TOKEN` | unset | Strongly recommended for any public URL; protects RPC/SSE/service routes. |
| `PLURORA_REQUIRE_ACCESS_TOKEN` | `0` | Set to `1` to make the container fail fast if `PLURORA_HTTP_ACCESS_TOKEN` is absent. |
| `PLURORA_APP_BASE_DOMAIN` | unset | Optional wildcard app domain such as `apps.example.com`; deployed apps can then open at `<slug>.apps.example.com/`. |

If `/data/profiles/$PLURORA_PROFILE.yaml` does not exist, the entrypoint creates a small SQLite-backed profile. You can replace it by mounting or writing your own profile into the same path.

For Zeabur/public validation, set a random token, open the app URL, and enter that token on the login screen. Do not reuse production credentials and do not store real provider secrets in this validation instance. URL tokens (`?plurora_token=<token>` or `?access_token=<token>`) remain available as an optional one-time bootstrap path.

To validate virtual-host app entry, point wildcard DNS such as `*.apps.example.com` at this Zeabur service and set `PLURORA_APP_BASE_DOMAIN=apps.example.com`. The Ygg management UI still uses the main domain and token; app vhost domains do not require the Ygg token.

## Local smoke test

```sh
docker build -t plurora-zeabur-quick .
docker run --rm -p 8080:8080 -v plurora-data:/data \
  -e PLURORA_HTTP_ACCESS_TOKEN=dev-token \
  -e PLURORA_REQUIRE_ACCESS_TOKEN=1 \
  plurora-zeabur-quick
curl http://127.0.0.1:8080/healthz
```

## Limitations

- Quick validation only; this does not replace packaged desktop distribution.
- The access token is quick-validation auth only, not production-grade session auth.
- Surface bundles/assets reach the sandbox through short-lived read-only leases; never embed secrets in them.
- No new official package namespace or install model is introduced by this adapter.
- Use a throwaway `/data` volume for public validation and delete it after testing.
- Do not enter real API keys or sensitive secrets on a public quick-validation URL.
- The bundled web app is static; local development hot reload still uses the existing Vite workflow.
