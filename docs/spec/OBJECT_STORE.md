# Object / Artifact 基础（Experimental）

> [English](./OBJECT_STORE.en.md) · [中文](./OBJECT_STORE.md)

本文定义当前已实现的内容寻址对象基础。它是 Constitutional Substrate 的 Experimental 合同，不改变 `platform.asset.*` 的方法 ID 或现有请求形状。

## 身份与描述符

对象身份只由 bytes 的摘要决定。当前必选算法是严格格式的 `sha256:<64 个小写十六进制字符>`。算法前缀属于持久身份的一部分；读取器必须保留它，未知算法必须明确拒绝，不能静默解释为 SHA-256。

可移植元数据使用：

```text
ArtifactDescriptor
├── artifact_type_uri
├── media_type
├── digest
├── size_bytes
├── references[]
└── annotations{}
```

`artifact_type_uri` 是开放 URI。宿主不认识该类型时仍须能复制、导出、校验 bytes 与保存描述符。描述符不得把宿主绝对路径、PID、临时 URL 或其他本机瞬态值纳入可移植身份。

## ObjectStore 合同

`ObjectStore` 提供五个异步操作：

- `put(bytes)`：计算 SHA-256、幂等写入并返回 digest/size；
- `get(digest)`：返回完整 bytes，并在返回前验证摘要；
- `has(digest)`：检查对象是否存在；
- `verify(digest)`：流式重算摘要并返回已验证的大小；
- `stream(digest)`：完整性预检通过后打开读取流，并在流到 EOF 时再次核对实际输出摘要；调用方必须在终端校验错误时丢弃已读结果。

当前实现包括进程内存储和文件系统存储。文件系统布局属于实现细节；调用者只依赖 digest。并发写入相同 bytes 必须收敛到同一对象，临时写入必须先完成和同步，再原子发布。

## 字节与日志分离

对象 bytes 只进入 ObjectStore。journal、event 与后续 receipt 只保存 descriptor 或 digest ref，不得复制大正文。`object/put` 的事件 payload 保存 additive `AssetRecord.descriptor`，event metadata 只保存 `artifact_digest`、`size_bytes` 和 `content_included: false`。

这条边界不改变 secret policy：asset 内容仍是任意用户数据，不做原始 secret 扫描；asset metadata 继续执行现有 raw-secret 拒绝规则。

## v1 Asset adapter

`object.put/get/list` 保持 wire 兼容：

- `put` 把 UTF-8 content 提交为通用 blob artifact；
- `AssetRecord.hash` 现在是 canonical SHA-256 digest；
- `AssetRecord.descriptor` 是 additive 可选字段，旧客户端可以忽略；
- `get` 通过 descriptor 从 ObjectStore 读取并验证，再适配回 v1 String content；
- `list` 只列出 records，不读取对象正文。

FNV-1a 仅由 `legacy_content_address()` 和显式 `scheme: "fnv1a64"` 兼容路径提供，不能作为新对象的 canonical identity。

## 旧事件迁移

rehydration 读取含 `metadata.content` 的旧 `object/put` 事件时：

1. 把旧 content 幂等提交到 ObjectStore；
2. 计算 SHA-256 descriptor，并校正 canonical hash/size；
3. 在 annotations 中保留旧 asset id、旧 FNV hash、原始 event id、sequence 和 session id；
4. 不修改旧事件，也不追加迁移事件。

因此迁移可中断、可重复执行，并由 CAS 自然去重。新事件没有 inline content 时，对象缺失、摘要不一致、size 不一致或 media type 不一致都必须明确失败，不能恢复为空字符串。

## 故障与部署边界

对象先提交到 CAS，再追加引用事件。事件追加失败时可能留下无引用对象，但不会产生指向缺失 bytes 的成功响应；后续以 journal 可达性为依据的 GC 负责回收孤儿，不能在失败路径直接删除共享 digest。文件系统实现使用临时文件、文件同步和原子 rename；Unix 上在发布后同步父目录。

默认 host 把对象放在 `<data-dir>/objects`。迁移 SQLite 日志时必须同时迁移该目录；多个 host 共享 PostgreSQL event store 时也必须配置/部署共享的对象后端。远程对象后端与可达性 GC 属于后续运行时工作，不改变当前 digest/descriptor 合同。

## 可执行验收

- `asset.put_get_list`：以 1 MiB+ content 验证 SHA-256 descriptor、v1 读取和事件无正文；
- `asset.legacy_fnv_migration`：验证旧 FNV 事件的幂等迁移与 provenance 保留；
- `object_store.portability_integrity`：验证跨宿主同摘要、未知类型复制、流读取和篡改拒绝；
- `substrate.sqlite_rehydrate`：验证 SQLite 日志与独立文件对象目录共同完成重启恢复。
