# Surface Hosting 指南

> [English](./SURFACE_HOSTING.en.md) · [中文](./SURFACE_HOSTING.md)

本指南说明平台 shell 如何处理两类 surface：由平台渲染的结构化 descriptor，以及在 sandboxed iframe 中运行的静态 Web bundle。当前 shell 使用 React 19，但 surface 边界不依赖 React；第三方代码只能通过公开合同和显式 Host bridge 参与。

## 两类 surface

能力包通过 manifest 的 `contributes.surfaces` 声明 surface。宿主根据 descriptor 决定如何呈现：

- `quick_action`、`workshop_card` 和带 `metadata.shell_schema_version: 1` 的 `home_card` 由平台直接渲染；
- 需要自有前端的 Package 通过 `entry.kind: surface_bundle` 提供静态 ESM bundle，并由 `SurfaceHost` 放入隔离 iframe。

结构化 descriptor 只允许受限文本、icon hint、排序和同包 target。平台不会为它加载包 JS、解析 HTML 或创建 iframe。它们当前是发现入口；未来接线执行时仍必须经过公开协议、权限、proposal 和审计。

## 静态 bundle 包

最小 manifest：

```yaml
schema_version: 1
id: example/surface
version: 0.1.0
license: AGPL-3.0-only
entry:
  kind: surface_bundle
  bundle: dist/bundle.mjs
contributes:
  surfaces:
    - id: example/entry
      version: 0.1.0
      slot: experience_entry
      title: Example surface
      allowed_capability_ids:
        - example/surface/inspect
      activation:
        input_schema: {}
      required_permissions: []
permissions: {}
```

`surface_bundle` 是静态、不可执行的 package entry。Host 不把它作为 Rust、subprocess、WASM 或 remote package 启动；bundle 与同目录静态资源保留在 Package source / artifact closure 中。

原始 `/surface-bundles/packages/<package-id>/...` 路径要求 Host 身份。`host.surface.bundle.resolve` 成功后，Host 为当前 grant 和 bundle root 签发随机、五分钟、只读的 `/surface-assets/<lease>/...` URL。相对 module、stylesheet、font 和 image 必须留在同一 lease root。Grant 撤销或过期会立即使 lease 失效。

不要把 secret、token、私有配置、主机路径或 source map 放进 `dist/`。私有数据必须通过 capability、`secret_ref`、出站审计和 bridge 权限取得。

## SurfaceHost API

当前 Web 实现位于 `clients/web/src/surfaces/surface-host.ts`：

```ts
export interface SurfaceHostOptions {
  containerId: string;
  surfaceId: string;
  bundleUrl: string;
  exportName: string;
  wrapperClass?: string;
  hostBridge?: SurfaceHostBridge;
  initialProps?: unknown;
  stylesheets?: string[];
}

export interface SurfaceHostBridge {
  currentSessionId?: string;
  allowedCapabilityIds?: Iterable<string>;
  callRpc?(method: string, params: unknown): Promise<unknown>;
  subscribeEvents?(callback: (event: unknown) => void): () => void;
}

export interface SurfaceHostHandle {
  surfaceId: string;
  iframe: HTMLIFrameElement;
  unmount(): Promise<void>;
}

export function mountSurface(
  options: SurfaceHostOptions,
): Promise<SurfaceHostHandle>;
```

`mountSurface` 会：

1. 查找目标容器；
2. 创建只有 `sandbox="allow-scripts"` 的 iframe；
3. 等待 frame 的 `ready` 消息；
4. 生成 mount-scoped `bridge_token`；
5. 发送 bundle URL、export、样式和清理后的 `initialProps`；
6. 注册 RPC 与 stream message handler；
7. 在 `unmount()` 时关闭订阅、通知 frame、移除 listener 和 iframe。

宿主会把 `currentSessionId` 注入为 `sessionId` 与 `session_id`，并覆盖调用方在 `initialProps` 中提供的同名字段。Surface 不能自行选择另一个 session。

## Bundle mount contract

Bundle 必须是同源 lease URL 可动态导入的 ESM module，并暴露一个受限 JavaScript identifier 形式的具名 export。当前 frame 调用 export 的形状是：

```ts
export function ExampleSurface(
  root: HTMLElement,
  props: Record<string, unknown>,
): void | (() => void) {
  // render into root
  return () => {
    // release listeners and UI state
  };
}
```

React surface 可以在函数中调用 `createRoot(root).render(...)`，并返回 `root.unmount()` 包装函数。Plain DOM surface 可以直接操作 `root`。`wrapperClass` 会设置到 frame 的 `#root`；样式应限制在该 class 下。

## Iframe 与 CSP

宿主创建：

```html
<iframe sandbox="allow-scripts" src="/surface-frame.html"></iframe>
```

没有 `allow-same-origin`、`allow-forms`、`allow-popups` 或顶层导航权限，因此 frame 是 opaque origin，不能继承 Host 的 cookie、localStorage 或 DOM 权限。

`surface-frame.html` 的 CSP 是：

```text
default-src 'self';
script-src 'self';
connect-src 'none';
style-src 'self' 'unsafe-inline';
img-src 'self' data: blob:;
font-src 'self' data:;
```

Frame bootstrap 只接受同源 `/surface-assets/`、公开 `/assets/` 和自身 bootstrap script。Bundle 不能直接加载原始 `/surface-bundles/` 路径，也不能从 frame 直接访问公网；网络能力必须经过 Host-controlled capability/outbound 边界。

## postMessage 协议

主要消息：

```text
frame -> host: ready
host  -> frame: mount | unmount | rpc.result | stream.frame | stream.ended | stream.error
frame -> host: rpc.call | stream.subscribe | stream.unsubscribe | mount.error
```

除初始 `ready` 外，Host 与 frame 的消息都绑定当前 `bridge_token`。宿主还验证 `event.source`、当前 session、subscription identity 和 stream ownership。Asset lease 只授权静态读取；`bridge_token` 只认证当前 mount 的消息。两者都不是 Host credential。

## RPC bridge

Surface 通过 `window.pluroraHost.callRpc(method, params)` 发起调用。未配置 `hostBridge.callRpc` 时，调用返回标准化的 `no_bridge` 错误。

当前 bridge 方法 allowlist：

- `host.info`
- `host.ping`
- `capability.invoke`
- `capability.stream`
- `capability.cancel`

Capability invoke/stream 必须满足：

- `capability_id` 在 surface descriptor 的 `allowed_capability_ids` 中；
- `session_id` 由宿主重写为 `currentSessionId`；
- 只保留允许的 input、provider、version 和 bounded metadata 字段；
- stream 返回的 `stream_id` / `invocation_id` 记录为该 surface 所有；
- cancel 只能作用于该 surface 创建的 stream 或 invocation。

Host 不把 raw runtime object、管理员方法、secret 或未过滤诊断传给 surface。Bridge error 会映射为有限的公开 code/message。

## Stream bridge

Surface 只能订阅自己通过 `capability.stream` 创建的 stream。宿主从当前经过验证的 session 事件订阅中筛选对应 `capability/stream.*` 事件，再转成：

- `stream.frame`：`started`、`chunk`、`progress`；
- `stream.ended`；
- `stream.error`：error、cancelled、timeout。

当前实现对每个 surface 的 owned streams 和并发 subscriptions 设置硬上限，并在 unmount 时关闭全部订阅。Surface 不能用 subscription API 枚举同 session 的其他 stream。

## Installation / Run 边界

`/installation/<installation-id>` 展示 Installation projection 与 Library affordance；只有显式 `host.run.start` 成功后才绑定该 Run 的 context，再由 Shell 决定是否挂载 surface。Run journal 独立持有 lifecycle；关闭 iframe、tab 或 PWA 连接不会获得停止 Run 的权威，停止仍由宿主页通过 exact Installation + Run selector 调用 `host.run.stop`。Run start 不隐式 build/deploy，缺少本地匹配实现时显示结构化 gap；Exposure 与跨 Installation Binding 仍属 Phase 5。

Iframe 内存不是持久状态。可恢复状态应由 Package capability、事件、asset 或 projection 持有，并通过公开协议重新获取。`initialProps` 只适合 session、descriptor 和只读启动信息。

## 当前边界

- Bundle 只允许 Host 同源的 leased asset URL；cross-origin bundle 仍需独立的 origin allowlist、integrity pin 和 CSP 设计。
- Frame 不能直接访问 Tauri API；桌面能力必须由宿主设计成受控公开边界。
- Surface lifecycle callback（如 `onClose`、`onProposalDraft`）尚未形成稳定合同。
- Structured quick action 仍是发现入口，不会绕过 proposal、permission 或 audit 直接执行。

## 相关文档

- [`../architecture/ARCHITECTURE.md`](../architecture/ARCHITECTURE.md) — shell、Work 与能力包的架构位置。
- [`INSTALLATION_MODEL.md`](INSTALLATION_MODEL.md) — Work、Installation 与后续 Run 的边界。
- [`CAPABILITY_HANDLES.md`](CAPABILITY_HANDLES.md) — capability 权威与衰减。
- [`SECRET_MANAGEMENT.md`](SECRET_MANAGEMENT.md) — `secret_ref` 和 secret 边界。
- [`../ALPHA_STATUS.md`](../ALPHA_STATUS.md) — 当前实现状态。
