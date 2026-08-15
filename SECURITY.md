# 安全政策

> [English](./SECURITY.en.md) · [中文](./SECURITY.md)

我们重视 Plurora Host、公开合同和官方客户端中的安全问题。

## 请私下报告

不要为安全漏洞开公开 Issue、Discussion 或 Pull Request。

优先使用 GitHub 的 [Private vulnerability reporting](https://github.com/Youzini-afk/Plurora/security/advisories/new)。如果该入口不可用，请向仓库维护者发送私信，并在标题中标明这是安全报告。

## 报告里请包含

- 受影响的版本、提交或运行方式（Host、Web、Desktop、CLI）；
- 复现所需的最短步骤，不要附带真实 secret；
- 实际影响：谁能做什么、作用在哪些资源上；
- 若已有缓解办法，一并说明。

## 范围

欢迎报告：

- 未授权访问 Host、Installation、Run、secret 或设备授权；
- 路径逃逸、symlink 绕过、linked-local 源码被删除；
- 第一方旁路、隐藏权限或 raw secret 泄漏；
- 官方 Web / Desktop 中可导致跨 Installation 越权的问题。

通常不在本政策内：

- 尚未实现或文档已标明 deferred 的能力；
- 仅在用户显式批准 Realization / outbound 之后才会发生的预期效果；
- 第三方 Package 自己的漏洞（请向该 Package 的维护者报告）。

## 处理方式

维护者会确认收到报告，评估影响，并在修复或公开披露前与报告人协调。请给合理时间，不要在修复前公开利用细节。
