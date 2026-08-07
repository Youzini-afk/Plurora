# 文档体例与红线

> [English](./STYLE.en.md) · [中文](./STYLE.md)

这份文档是 Plurora 仓库写文档的最低规则。目标是让读者清楚地区分平台身份、长期架构、官方产品选择、当前实现和建设方向，避免开发记录或某个产品 Profile 反向覆盖整个平台。

## 文档事实层级

发生冲突时，按以下层级理解：

1. [`CHARTER.md`](CHARTER.md) 定义平台身份、长期目标和不可妥协原则；
2. [`architecture/VISION.md`](architecture/VISION.md) 与长期架构文档定义层次、所有权和演化方向；
3. [`product/PLATFORM_PRODUCT_MODEL.md`](product/PLATFORM_PRODUCT_MODEL.md) 定义官方发行版的产品责任；具体 Product Profile 只约束选择它的产品；
4. Contract、spec 和 guide 定义当前公开合同与用法；
5. [`ALPHA_STATUS.md`](ALPHA_STATUS.md) 陈述当前实现事实；代码、生成 schema 和 CI 是具体数字与行为的最终证据；
6. [`roadmap/NEXT_STEPS.md`](roadmap/NEXT_STEPS.md) 描述建设方向和取舍，不自动代表已经实现或永久承诺；
7. 历史实施计划和 Git 记录解释“如何走到这里”，不能覆盖较新的长期文档和状态事实。

改变平台身份需要显式修订章程。改变官方产品观点不能静默改写基底。改变当前实现时，应在同一提交更新相关状态、合同或指南。

## 身份与命名

仓库身份属于公开契约，不是装饰性措辞。

- 产品与项目名称：叙事使用 `Plurora`，机器身份使用 `plurora`。
- 公开 method ID 使用 owner-based dot namespace：`context.*`、`journal.*`、`capability.*`、`authority.*`、`object.*`、`identity.*`、`host.*`、`protocol.*`、`change.*`、`projection.*` 与 `shell.*`。
- 平台拥有的 event kind 来自显式 registry，使用语义 slash namespace，并由 `plurora/runtime` 写入。
- Package capability 与 event ID 必须以精确 Package ID 加 `/` 开头。
- 第一方 Package 使用 publisher namespace `plurora/*`。该身份不授予 authority、routing priority、UI privilege 或 substrate ownership。
- Schema、OpenAPI 与 SDK 身份由可执行 registry 和 generator 生成；不得手工维护平行名称。
- 预发布工作树只保留被选定的一套身份。不要为了让改名看似安全而加入旧名称 alias、fallback 环境变量、重复 CLI 入口或 compatibility route。
- 稳定后的 breaking change 使用显式新 contract/profile/version 边界与 migration plan，不能隐藏在 alias 中。

长期文档直接使用当前身份。历史改名计划在结论进入 architecture、spec、status 与检查后删除。

## 写给读者，不写开发日志

读者关心：它是什么、为什么属于这一层、怎样使用、边界是什么、失败后怎样处理。

应该写：

- 平台、协议、组件、Host、发行版和产品分别拥有什么；
- 怎样运行、安装、调用、调试、迁移和恢复；
- 当前 implemented / partial / deferred 的准确状态；
- 权限、数据、错误、取消、兼容和迁移边界。

不应该写：

- “我们最近完成了 X”“Round 10A.4 推进了 Y”之类提交日志；
- 为了展示工作量而堆叠阶段名称、测试数量或功能数量；
- 把候选方向写成既成事实；
- 把已经完成的临时计划长期留在最短阅读路径。

## 不用阶段编号代替含义

长期文档不使用 `Round X`、`Phase Y`、`T-track`、`U-track` 等临时分组。标题直接写稳定语义，例如“设备授权”“Artifact 生命周期”“远程 Component”。

状态表达使用：

- `implemented`：公开路径已可用；
- `partial`：已有实质能力，但边界或生命周期仍缺失；
- `experimental` / `candidate`：成熟度尚未达到稳定；
- `deferred` / `planned`：尚未实现或主动延后。

如果确实需要一次性实施计划，可以放在 `docs/roadmap/`，完成后删除，把长期结论收敛进 architecture、spec、guide 或 status。

## 区分平台、发行版和 Product Profile

- 平台文档不得把 Project、Home、Play、Forge、Assist、Tavern、聊天、世界或部署写成 Plurora 唯一中心；
- 官方发行版可以有强观点，但必须注明它是可替换的产品选择；
- Product Profile 可以约束采用它的参与者，但不能声称是所有产品的强制本体；
- 某个产品需求若推动底层变化，文档必须说明它最终属于 Protocol、Host 还是确实无法上移的 substrate mechanism；
- “当前第一方实现方便”不是进入基底的理由。

## 测试与 conformance 的正确位置

测试、fixture、conformance、dogfood 和外部集成用于：

- 发现缺陷；
- 约束公开行为；
- 防止回归；
- 测量性能、兼容和可靠性；
- 支撑当前状态声明。

不要把它们写成项目存在的目的，或为了“证明某个抽象”而制造功能。先说明要为用户、创作者或生态建设什么，再说明质量系统如何保证它。

使用准确术语：

- 测试使用的具体 Package / repository → `fixture`、`integration fixture`、`compatibility case`；
- 稳定前必须满足的条件 → `adoption condition`、`compatibility condition`、`fitness condition`；
- 不要把一个产品称为“压力源”或“平台证明”。

## 概念文档与状态文档

### 长期 / 概念文档

`CHARTER`、`VISION`、`ARCHITECTURE`、`CONSTITUTIONAL_SUBSTRATE`、`CAPABILITY_PACKAGE`、`PLATFORM_PRODUCT_MODEL`、候选宪法和稳定协议文档，描述目标、所有权、机制和长期边界。

它们不应被具体提交或阶段污染，但平台目标、架构归属或合同本身改变时必须更新。

### 当前状态文档

`ALPHA_STATUS`、roadmap、compatibility / conformance matrix 等可以包含版本、数量、implemented / partial / deferred 和当前限制，但仍然不是开发日志。

### 指南

Guide 应描述读者完成一项任务的当前路径，并在开头说明它属于平台、Host、官方发行版还是某个 Profile。指南不能因为当前 UI 使用某个概念，就把它提升为平台宪法。

## 可验证实现事实

涉及以下内容时，先核对代码、生成物或 CI：

- Web / Desktop 使用的框架和生命周期；
- 方法、事件、schema、测试或 Package 数量；
- 当前支持的执行形态、数据库和 transport；
- 权限强制、secret、部署、恢复和迁移行为；
- 某个兼容声明或外部集成覆盖范围。

不要复制容易漂移的数字到过多文档。需要数字时，优先让状态文档引用单一来源。

## 机械检查

提交文档改动前运行：

```bash
python3 scripts/check-docs.py
python3 scripts/check-identity.py
```

该脚本只检查仓库内相对链接、英文文档的中文配对，以及已经采用标准语言切换的文档是否完整链接两种语言。它不判断平台方向、架构观点或措辞是否正确，不能替代人工审阅。

## ZH / EN 同步

主要叙事、导航和指南同时维护中英文：

- 中文默认文件名为 `xxx.md`，英文为 `xxx.en.md`；
- 顶部使用 `> [English](./xxx.en.md) · [中文](./xxx.md)`；
- 同一提交更新两种语言；措辞可以不同，事实、状态、边界和链接必须一致；
- 机器生成 inventory、生态约定的 npm/cargo README 等可以按其格式保留英文单份。

## 文档红线

- ❌ 把 raw stderr、API key、token、password 或 raw secret 放进文档；示例使用 `secret_ref`。
- ❌ 把具体用户的绝对路径写进面向读者的指南；使用 `~/.plurora/<area>/` 等约定路径。
- ❌ 宣称未经代码、fixture 或兼容检查支持的覆盖率和互操作结论。
- ❌ 把 YdlTavern 或其他产品的 chat、character、prompt 等语义写入平台基底。
- ❌ 把官方 Package ID、UI slot 或默认 provider 当作权限、路由或长期本体。
- ❌ 用“开放”掩盖难用、不完整和不可恢复；也不能用“好用”掩盖私有 API 和数据锁定。
- ❌ 把 temporary plan、已完成迁移清单或 commit message 永久留作规范。

## 新增或重大修改前

先回答：

1. 读者是谁，他需要完成或理解什么？
2. 这是平台原则、架构、协议、Host、发行版、Product Profile、指南还是状态？
3. 谁拥有这里的语义和生命周期？
4. 是否把当前官方选择写成了全平台要求？
5. 当前事实是否已由代码或生成物核对？
6. 错误、取消、恢复、迁移和删除是否说明清楚？
7. 中英文和相对链接是否同步？

## 一句话总结

**文档要让平台的目标、层级、产品观点与当前事实各归其位；它是稳定参考，不是证明书，也不是开发日志。**
