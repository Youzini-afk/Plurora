# Error Codes (v1)

Plurora v1 reserves JSON-RPC application error numbers `-32000..-32099`. The current runtime `ProtocolError.code` remains a string identifier; the table assigns stable numeric equivalents for cross-language implementations and future JSON-RPC transports.

| Number | String identifier | Name | When emitted | Recovery hint |
|---:|---|---|---|---|
| -32000 | `runtime/error/internal` | Internal | Unexpected runtime failure. | Retry if transient; otherwise inspect host logs. |
| -32001 | `runtime/error/invalid_request` | InvalidRequest | Malformed frame, unknown method, or missing required parameter. | Fix request shape against the method schema. |
| -32002 | `runtime/error/permission_denied` | PermissionDenied | Caller lacks manifest permission, grant, or host policy allowance. | Request/declare the minimum permission or choose an allowed resource. |
| -32003 | `runtime/error/not_found` | NotFound | Session, package, capability provider, asset, projection, proposal, grant, or connection was not found. | Refresh state and retry with an existing identifier. |
| -32004 | `runtime/error/ambiguous_route` | AmbiguousRoute | Capability resolution found multiple providers. | Specify provider_package_id or a tighter version constraint. |
| -32005 | `runtime/error/schema_invalid` | SchemaInvalid | Manifest/capability input/output/event schema validation failed. | Validate locally with the public schemas and resend. |
| -32006 | `runtime/error/package_state` | PackageState | Package/session/stream is closed, not loaded, degraded, or not ready. | Load/restart/open the resource before retrying. |
| -32007 | `protocol/error/unsupported_contract` | UnsupportedContract | An explicitly requested contract profile, layer, or version cannot be satisfied exactly. | Read `host.info` and select an advertised profile/version; do not assume automatic downgrade. |
| -32010 | `manifest/invalid_package_id` | InvalidPackageId | Manifest package id is not a namespaced id. | Use an id like `org/package`. |
| -32011 | `manifest/invalid_namespaced_id` | InvalidNamespacedId | Capability, schema, surface, extension point, or hook id lacks namespace. | Use slash-separated package-owned ids. |
| -32012 | `manifest/invalid_version` | InvalidVersion | Semver-like version validation failed. | Use `MAJOR.MINOR.PATCH`. |
| -32013 | `manifest/invalid_schema` | InvalidSchema | Manifest schema field is neither object nor null. | Provide a JSON Schema object or null. |
| -32014 | `manifest/invalid_surface` | InvalidSurface | Surface id/title/version/capability references are invalid. | Fix surface id/title/version and referenced capability ids. |
| -32015 | `manifest/invalid_secret_ref` | InvalidSecretRef | permissions.secret_refs contains malformed or unsupported secret ref. | Use env-backed refs such as `secret_ref:env:NAME`. |
| -32016 | `manifest/invalid_network_method` | InvalidNetworkMethod | Network declaration uses unsupported HTTP/WebSocket method. | Use GET/POST/PUT/DELETE/PATCH/HEAD/OPTIONS/WEBSOCKET. |
