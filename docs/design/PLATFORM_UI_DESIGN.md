# 设计系统：Plurora 平台壳

> [English](./PLATFORM_UI_DESIGN.en.md) · [中文](./PLATFORM_UI_DESIGN.md)
>
> 作为 Stitch 屏生成与 React 实现的唯一来源，覆盖 `clients/web` 官方发行版 Shell。
> 本文描述的是当前官方产品的视觉与交互观点，不是 Plurora 的平台宪法。
> 第三方发行版、YdlTavern 与其他产品可以采用完全不同的导航和视觉语言。

---

## 风格刻度

| 维度 | 等级 | 说明 |
|------|------|------|
| **创意 Creativity** | `7` | 自信的"编辑工坊"调性。非对称布局、字重驱动层次、暖色克制。不是 gallery 安静 (4)，也不是艺术狂躁 (10)。 |
| **密度 Density** | `4` | 日常应用平衡。项目货架要呼吸，settings 表单要平静，不要 dashboard cockpit 感。 |
| **变异 Variance** | `8` | 非对称网格、分数列宽、慷慨留白。不要可预测的三等分卡片行。 |
| **动效 Motion** | `5` | 微妙永续 loop（状态脉冲、卡片悬停 lift、级联 mount）。不要电影化炫技。平台是工坊不是 demo reel。 |

> 当前官方发行版把开放平台能力组织成一张温暖、清楚、可逐步深入的工作台。
> 这是 `clients/web` 的产品观点：项目和内容在架上，控制与创作工具按需展开；
> 它不要求第三方 Shell 复制这种布局，也不把 Plurora 限定成 Steam、IDE 或 chatbot。

---

## 1. 视觉主题与气氛

一个温暖、克制的工坊。奶油纸背景，木炭墨色，单一的"做旧黄铜"强调色，
每一次出现都赚得到。布局非对称且自信 — 永不居中、永不对称。
气氛是打开笔记本准备开工的那一刻：安静、就绪、有意。
密度有呼吸，动效在感知边缘 loop。每个元素拥有自己清晰的空间区。

平台应该让人感觉像一家设计出版社的编辑工作台，
而不是科技公司的工具。
高级但不娇贵。现代但不机械。有设计感但不靠装饰。

---

## 2. 颜色系统

### Light mode（主模式）

- **Warm Bone** (#FAFAF7) — 主背景。带纸质暖意的略偏白，绝不临床蓝白，绝不纯 #FFFFFF 做背景。
- **Pure Surface** (#FFFFFF) — 卡片与抬升容器填充，配 whisper shadow。
- **Charcoal Ink** (#1B1A18) — 主文本。暖偏近黑，绝不纯 #000000。带轻微棕调暗示。
- **Steel Secondary** (#6B6862) — 正文文本、描述、元数据。暖灰。
- **Muted Tone** (#9C9890) — 三级文本、时间戳、disabled、placeholder。
- **Whisper Border** (rgba(40, 30, 20, 0.06)) — 卡片边框、1px 结构线。半透明，纸质深度感。
- **Diffused Shadow** (rgba(40, 30, 20, 0.05)) — 卡片抬升。40px blur，-15px y-offset。暖偏。

### Dark mode（并行核心）

- **Deep Bark** (#18171A) — 主背景。有机暖近黑，棕调暗示。绝不纯 #000。
- **Elevated Bark** (#22201E) — 卡片与抬升容器，比背景略暖。
- **Warm Ivory** (#F5F2EC) — 主文本。奶油偏 off-white。
- **Steel Secondary Light** (#9B968B) — 正文、描述、元数据。
- **Muted Tone Dark** (#6B6862) — 三级、disabled。
- **Whisper Border Dark** (rgba(245, 242, 236, 0.08)) — 卡片边框。
- **Diffused Shadow Dark** (rgba(0, 0, 0, 0.35)) — 卡片抬升，深一些以读出 bark 背景。

### 单一强调色 — Aged Brass 做旧黄铜

- **Aged Brass** (#B8956A) — 主 CTA、focus ring、active 高亮、Running 状态。Light 模式默认。
- **Aged Brass Deep** (#8A6F4F) — Light 模式 hover/active。
- **Aged Brass Glow** (#C9A87A) — Dark 模式强调色，略亮以读出 bark 背景。
- **Aged Brass Surface** (rgba(184, 149, 106, 0.10)) — 选中行、active tab、轻微强调区的染色面。

整个平台壳最多 **一种** 强调色。项目 iframe 内可以有自己的强调色，
平台从不试图与其托管的项目颜色协调。

### 状态语义（克制，绝不霓虹）

- **Running 运行中** — Aged Brass 配呼吸脉冲
- **Stopped/Installed 停止/已装** — Steel Secondary，无动画
- **Starting/Stopping 启动/停止中** — Muted Tone 配微妙 shimmer
- **Failed 失败** — Deep Rust (#9A4A33) — 去饱和的红棕，绝不亮红
- **Updating 更新中** — Muted Tone 配 shimmer

### 禁用颜色

- 纯黑 (#000000)
- 纯白 (#FFFFFF) 做 **背景**（卡片填充允许）
- 紫/紫罗兰/"AI 渐变"
- 任何霓虹 outer glow
- 强调色饱和度 > 70%
- 同一面里混合冷暖灰

---

## 3. 字体规则

### 字体栈

- **Display** — `Cabinet Grotesk`（700/800/900）。track-tight (`-0.025em`)，
  display 行高压缩到 `1.05`，标题到 `1.15`。用于：页标题、项目卡片标题、导航 eyebrow。
- **Body** — `Geist`（400/500/600）。relaxed leading (`1.55`)，长文 `65ch` 上限。
  用于：描述、settings 文字、对话框正文。
- **Mono** — `Geist Mono`（400/500）。用于：包 ID、版本号 (`v1.2.0`)、commit hash、
  密钥名 (`OPENAI_API_KEY`)、文件路径、密集场合的数字元数据。
- **CJK 备用** — `Noto Sans SC`（与 Geist 家族搭配）。
  中文标题（Cabinet Grotesk 没有中文）回退到 `Noto Sans SC` 700-900 weight。

### 字号阶梯

- Display 标题（Home eyebrow + section opener）：`clamp(2.5rem, 5vw, 4rem)`，
  weight 800，tracking `-0.03em`
- 页面标题（Settings sections）：`clamp(1.875rem, 3vw, 2.5rem)`，weight 700
- 卡片标题：`1.25rem`，weight 700，tracking `-0.015em`
- 正文：`1rem` / `1.0625rem`，weight 400
- 小正文（元数据、时间戳）：`0.8125rem`，weight 400，色 Steel Secondary
- Mono 元数据：`0.8125rem`，weight 400

### 字重驱动层次

层次来自 **字重对比 + 颜色对比**，不是单靠字号。
1.25rem 700-weight Charcoal Ink 标题坐在 1rem 400-weight Steel Secondary 描述上 —
这种对比就是关系。抗拒把每个标题都做大的冲动。

### 禁用字体

- `Inter` — 平台壳全场禁
- 系统通用 serif (`Times New Roman`、`Georgia`、`Garamond`、`Palatino`)
- 全大写炫技标题（`text-transform: uppercase` 仅允许在小 eyebrow 上，配 letter-spacing `0.14em`）
- Italic 正文做强调（用字重对比代替）
- 标题渐变填色
- 文字阴影

---

## 4. 组件样式

### 按钮

- **Primary** — Aged Brass 背景，Warm Ivory 文字（light 用白）。
  圆角 `0.625rem`（10px）。padding `0.625rem 1.125rem`。weight 500。
  无 outer glow。Active：`translateY(-1px)` 后弹回，模拟物理按压。
  Hover：light → Aged Brass Deep；dark → Aged Brass Glow。
- **Secondary** — Ghost / outline。1px Whisper Border、透明背景、Charcoal Ink 文字。
  Hover：Whisper Border 加深 + 背景 rgba(40, 30, 20, 0.03)。
- **Tertiary / 内联链接** — Charcoal Ink 下划线，`text-underline-offset: 4px`，
  `text-decoration-thickness: 1px`。不做图标塞满的内联链接。
- **Destructive** — Deep Rust 边框、Deep Rust 文字、透明填充。
  Hover 填 Deep Rust at 0.05 opacity。仅用于卸载、删数据。
- **Icon button** — 36px 方形，无边框，hover 背景 rgba(40, 30, 20, 0.04)。

### 卡片

- **项目卡片** — Pure Surface 填充、`1.5rem`（24px）圆角、1px Whisper Border、Diffused Shadow。
  内 padding `1.5rem`。每张目标宽度 `280-340px`。
  Hover：`translateY(-2px)` + 阴影从
  `0 20px 40px -15px rgba(40,30,20,0.05)` 加深到
  `0 24px 48px -15px rgba(40,30,20,0.08)`。Spring transition。
- **Settings 面板卡片** — 同形状但更大，用于聚组表单行。内 padding `2rem`。
  内部分段用 `1px Whisper Border` 横线分割，**不嵌套** 卡片。
- **空状态卡片** — 容器宽度满载，Whisper Border 双倍到 0.10 透明度的虚线，无填充，
  居中内容用组合 icon（不是 emoji），慷慨 padding `3rem`。
- **卡片仅在抬升传达层次时使用**。settings 行、元数据列表、密集数据
  应该用 `Whisper Border` 横线分隔代替嵌套卡片。

### 表单输入

- Label 在输入框 **上方**（label 与 input 间 `gap: 0.5rem`）
- Input：1px Whisper Border、透明背景、`0.625rem 0.875rem` padding、`0.5rem` 圆角。
  Focus：2px Aged Brass ring，`2px` offset，无阴影变化。
- Helper 在 input 下方，Steel Secondary `0.8125rem`
- Error 在 input 下方，Deep Rust `0.8125rem`，上 margin `0.5rem`
- 必填标记：label 后小 Aged Brass 圆点 (`•`)，绝不用 `*`
- Search input 用 16px Phosphor `MagnifyingGlass` 在 `0.875rem` Steel Secondary 内左嵌入，padding-left `2.5rem`
- Password 用 Phosphor `Eye` / `EyeSlash` 切换按钮在右

### 导航 / Topbar

- Sticky topbar 在 `top: 0`，高 `60px`，背景 Warm Bone 配 `0.85` 透明度 + `backdrop-filter: blur(20px)`，下方 1px Whisper Border
- 左：文字 logo `Plurora`，Cabinet Grotesk 700 weight、`1.125rem`，配当前 breadcrumb（Steel Secondary）
- 右：settings icon (Phosphor `GearSix`)、通知 bell（带状态点）、主题切换（sun/moon）
- 移动端不做汉堡 — 主导航（Home、Settings）始终通过 topbar 可达

### 项目框架 topbar（已挂载 iframe 的包装）

项目挂载时，平台在 iframe 上方显示 40px 细 topbar：
- 左：返回箭头（Phosphor `ArrowLeft`）→ 回 Home
- 中左：项目名 Cabinet Grotesk 700 + 状态 pill
- 右：Stop 按钮（仅 Running 时显示）、卸载菜单、项目设置 icon
- 背景：Elevated Bark（dark）或 Warm Bone（light），下方 1px Whisper Border
- iframe 内容占据视口余下，topbar 绝不渗入项目 UI

### Toast / 通知

- 从右下方 spring 滑入
- 宽 `360px`、padding `1rem`、圆角 `1rem`
- 1px Whisper Border、Pure Surface 填充、Diffused Shadow
- 可选左侧强调边（4px）用语义色（Aged Brass for info、Deep Rust for error）
- 4 秒自动消失，hover 暂停，点 X 立即关
- 间距 `0.5rem` 堆叠，最多 3 张可见，更老的自动消失

### 状态 pill

- 胶囊形状（圆角 999px），padding `0.25rem 0.625rem`
- 字体 Geist Mono 500 weight、`0.6875rem`、大写、letter-spacing `0.06em`
- 背景：中性状态用 Whisper Border 面，Running 用 Aged Brass Surface 配脉冲
- 文字前有 `8px` 彩色圆点，状态有动画时点也脉冲

### 模态框 / 对话框

- 居中遮罩，背景 light 用 `rgba(40, 30, 20, 0.5)` + backdrop-blur 8px；dark 用 `rgba(0, 0, 0, 0.6)` + blur 12px
- 模态容器：Pure Surface 填充、圆角 `1.5rem`、padding `2rem`、表单 max-width `560px`，wizard 用 `720px`
- 标题 Cabinet Grotesk 700 `1.5rem`
- 关闭按钮是右上 icon-only Phosphor `X`
- 操作按钮在右下：secondary 在前、primary 在后，gap `0.75rem`

### Loaders / Skeletons

- 骨架 shimmer 匹配确切布局尺寸
- 背景：linear-gradient 从 Whisper Border 0.04 到 0.08 opacity，1.6s 横向 loop
- 圆角匹配占位形状
- **绝不** 圆形 spinner，**绝不** 任何旋转圆圈

### 空状态

- 组合 icon（Phosphor outline weight 1.5）`48px`，Steel Secondary
- 标题 Cabinet Grotesk 700 `1.125rem`，Charcoal Ink
- 正文 Geist 400 `0.9375rem`，Steel Secondary，max-width `40ch`
- 可选 CTA 按钮（primary 或 secondary）
- 容器内居中，垂直 padding `4rem 2rem`

### 错误状态

- 内联上下文错误（表单字段）
- Banner 错误：1px Deep Rust 边框、Deep Rust at 0.04 背景、padding `0.875rem 1rem`、圆角 `0.75rem`
- 恢复 action 内联呈现为按钮或文字链接

---

## 5. Hero 区（Home 即 Hero）

Home 是平台第一印象 — 必须立即建立编辑工坊气氛，没有营销 chrome。

- **非对称布局** — 桌面 60/40 split。
  左区：eyebrow（"Plurora"小 caps）、Cabinet Grotesk 800 标题（如"你的工作台"）、一行正文。
  右区：环境细节 — 安静一句引文、mono 显示已装项目数量、轻盈装饰元素。
  右区主要是负空间。
- **无填充 chrome** — 禁用：「向下滚动探索」、「开始」、滚动箭头、动画 chevron、
  「欢迎来到你的平台」文案、跨行 hero 搜索栏、渐变背景。
- **唯一主操作** — 「+ 装入项目」入口作为项目卡片之一坐在 grid 中（永远在最后，虚线边框样式），
  不在 hero 里做单独大 CTA。Hero 没有 CTA 按钮。
- **项目货架是实质** — Hero 应占初始视口约 30vh，然后立刻让位给项目货架 grid。
  货架是真正的内容；Hero 只是承认用户已抵达工作台。
- **不做居中 hero** — 变异等级 8 禁止。强制非对称 60/40 或全左对齐。

---

## 6. 布局原则

- **CSS Grid** 处理所有结构布局。绝不 `calc(33% - 1rem)` flexbox math。
- **项目货架 grid** — `grid-template-columns: repeat(auto-fill, minmax(280px, 1fr))`，
  `gap: 1.25rem`。「+ Install」卡片是同一 grid 的一部分（永远最后）。
  4+ 张时自然形成非对称尾。
- **Bento 变体用于区位面板** — settings 页有 3+ 相关控件面板时，
  优先 2 列非对称 (`2fr 1fr`) 或一长面板跨两行的 3 列。
- **不做 3 等列卡片行**。「feature row」模式禁用。用非对称、zig-zag、或流式 grid。
- **不重叠**。文字绝不坐在图像上，没有 absolute 层叠堆内容。
  每个元素有自己清晰的空间区。
- **容器约束** — `max-width: 1400px` 居中，水平 padding `1rem`（移动）/ `2rem`（平板）/ `4rem`（桌面 ≥1024px）。
- **全高** — `min-height: 100dvh`，绝不 `height: 100vh`。
- **节段垂直节奏** — 主节段间 `clamp(3rem, 7vw, 5.5rem)`。

---

## 7. 响应式规则

每屏必须在 `375px`、`768px`、`1024px`、`1440px` 下测试。移动端 viewport 破坏算严重失败。

- **移动 (`< 768px`)** — 多列布局全部塌成单列。项目货架变每行一卡。
  Hero 塌：eyebrow + 标题 + 正文垂直叠，右环境区隐藏。
  Topbar 塌为 logo + icon row；settings 菜单通过单 icon 按钮可达。
- **平板 (`768-1023px`)** — 项目货架每行 2 卡。Settings 面板单列 max-width。
- **桌面 (`≥ 1024px`)** — 完整编辑布局。项目货架按视口 3-4 卡每行。
  Hero 取非对称形态。
- **触控目标** — 所有交互元素 ≥ 44px on mobile。按钮在移动端全宽。
- **字号** — 标题 `clamp()` 缩放。正文绝不低于 `15px` / `0.9375rem` on mobile。
  Mono 元数据保持 `0.8125rem`。
- **不做横向滚动** — 任何位置、任何 viewport、永远。

---

## 8. 动效与交互

> Stitch 生成静态屏。本节记录预期动画行为，让 React 实现知道该做什么。

- **Spring physics 唯一** — `stiffness: 100, damping: 20` baseline。无 linear easing。
  CSS transition 接受 `cubic-bezier(0.16, 1, 0.3, 1)`。
- **项目卡片 hover** — `translateY(-2px)` + 阴影加深。Spring transition。
- **项目卡片 mount cascade** — 列表项以 `calc(var(--index) * 60ms)` 错开延迟显现，
  fade-in + `translateY(8px)` 到 0。最多 12 项后无动画（性能盖）。
- **Running 状态脉冲** — 前置点在 2.4s ease-in-out loop，opacity 1 → 0.5 → 1。
  Pill 背景在 Aged Brass Surface 上微 glow 0.10 → 0.18 → 0.10。
- **Skeleton shimmer** — gradient sweep，1.6s 无限，仅 `transform: translateX`。
- **Toast spring 入场** — 从右下 `translateY(100%)` → 0，spring 100/15。
- **Modal 入场** — backdrop 200ms fade in，模态 `0.96` → `1` 缩放 + fade，spring 120/22。
- **Topbar sticky** — 立即 backdrop blur（无 transition）；滚过 hero 显示阴影 `0 1px 0 Whisper Border`。
- **路由切换** — fade + 4px translateY，240ms。
- **硬件规则** — 仅 `transform` 与 `opacity`。绝不 `top` / `left` / `width` / `height` / `margin`。
  grain / noise 滤镜仅在 fixed pointer-events-none 伪元素上。
- **性能** — 永续 loop 隔离在自己组件，绝不触发父级 re-render。
  60fps 最低目标。

---

## 9. 反模式（禁用）

### 视觉
- 不要 emoji — UI、code、alt、文案均禁
- 不要 `Inter`、不要系统 serif、不要 Times New Roman
- 不要纯黑 (#000000)、不要纯白做背景（卡片填充允许）
- 不要紫色 / 紫罗兰 / "AI 渐变"
- 不要霓虹 outer glow、不要默认 `box-shadow`
- 不要饱和度 > 70% 的强调色
- 不要标题渐变填色
- 不要文字阴影
- 不要自定鼠标 cursor
- 不要 hover 时移动内容（文字偏移、layout 因 hover 重排）

### 布局
- 不要居中 hero
- 不要 3 等列卡片行
- 不要 `h-screen`（永远 `min-h-[100dvh]`）
- 不要 flexbox 百分比 math (`calc(33% - 1rem)` 等)
- 不要重叠（无 z-index spam、无 absolute 层叠文字在图像上）
- 不要嵌套卡片（卡里卡里卡）
- 不要 `z-index` > 50，仅允许：navbar (10)、modal (40)、toast (50)

### 文案
- 不要 AI 文案套话："Elevate"、"Seamless"、"Unleash"、"Next-Gen"、"Revolutionize"、"Empower"、"赋能"、"无缝"、"释放"
- 不要填充 UI 文字："Scroll to explore"、"Swipe down"、"Discover more"、滚动箭头、跳跃 chevron、"欢迎登船"
- 不要通用占位名："John Doe"、"Sarah Chan"、"Acme"、"Nexus"、"SmartFlow"
- 不要假整数：`99.99%`、`50%`、`1234567`。用有机数据：`47.2%`、`+1 (312) 847-1928`
- 不要内部平台页用营销夸张

### 实现
- 不要破 Unsplash 链接 — 用 `picsum.photos/seed/{id}/800/600` 或本地 SVG 组合
- 不要通用 `shadcn/ui` 默认 — 每组件必须按本系统定制圆角、颜色、阴影
- 不要圆形 loading spinner — 仅 skeletal shimmer
- 不要 emoji 当 UI icon — 用 Phosphor outline weight 1.5 或 Radix
- 不要在同一屏混合 icon library

---

## 10. React 实现提示

- Tailwind v4；设计 token 通过 `src/styles/app.css` 的 `@theme` 和 CSS variables 定义，不使用 `tailwind.config.js`
- `@phosphor-icons/react` 通过 `src/components/icons.tsx` 做语义化 re-export，页面不直接混用 icon library
- Bricolage Grotesque、Geist 与 JetBrains Mono 通过 `@fontsource-variable/*` 随 bundle 分发，不依赖运行时 CDN
- Motion v12 负责进入/退出和 spring；永续 loop 隔离在 memoized leaf 组件，并遵守 `prefers-reduced-motion`
- `<html data-theme="light|dark">` 与 CSS variables 驱动主题，默认跟随系统并允许用户覆盖
- 复用 `client-core` 与公开 protocol/Host access client；不在页面组件里复制认证、RPC、SSE 或 pairing 逻辑

---

## 11. Stitch 生成指引

用本设计系统提示 Stitch 时：

- 始终声明屏是给「Plurora 平台壳」（不是项目、不是 YdlTavern、不是聊天产品），
  防止 Stitch 拉 SaaS / chatbot 参考
- 强化关键词：暖奶油 (#FAFAF7) 背景、单一 Aged Brass (#B8956A) 强调、
  非对称布局、不做居中 hero、不要 emoji、Cabinet Grotesk 做 display
- Home 屏：「非对称编辑工坊主页，60/40 split hero 配下方 Bento 风项目货架」
- Settings 屏：「平静表单配 section dividers，无嵌套卡片，字重驱动层次」
- Stitch 倾向加：渐变背景、居中营销 CTA、emoji 装饰、三等卡片行、AI 紫色 — 
  全部用本文档反模式列表压制
