# Design System: Yggdrasil Platform Shell

> [English](./PLATFORM_UI_DESIGN.en.md) · [中文](./PLATFORM_UI_DESIGN.md)
>
> Single source of truth for Stitch screen generation and React implementation
> of the official `clients/web` distribution shell. This document expresses the
> current official product's visual and interaction opinions, not Yggdrasil's
> platform constitution. Third-party distributions and products may use a
> completely different navigation and visual language.

---

## Configuration — Set Your Style

| Dial | Level | Description |
|------|-------|-------------|
| **Creativity**     | `7` | Confident editorial-workshop aesthetic. Asymmetric layouts, weight-driven typography, warm restraint. Not gallery-quiet (4), not artsy-loud (10). |
| **Density**        | `4` | Daily app balanced. Project shelf breathes; settings forms are calm; nothing feels like a cockpit. |
| **Variance**       | `8` | Asymmetric grids, fractional column splits, generous empty zones. No predictable 3-up card rows. |
| **Motion Intent**  | `5` | Subtle perpetual loops (state pulse, card hover-lift, cascade mount). No cinematic theatrics. The platform is a calm workshop, not a demo reel. |

> The current official distribution organizes open platform capability into a
> warm, legible workbench that can disclose depth gradually. Projects and
> content sit on shelves while control and creation tools appear as needed.
> Third-party shells need not copy this layout, and Yggdrasil is not defined as
> Steam, an IDE, or a chatbot.

---

## 1. Visual Theme & Atmosphere

A warm, restrained workshop. Cream paper background, charcoal ink, a single aged
brass accent that earns every appearance. Layouts are asymmetric and confident
— never centered, never symmetrical. The atmosphere is the moment between
opening a notebook and starting to work: quiet, ready, intentional. Density
breathes. Motion loops at the edge of perception. Every element has its own
clear spatial zone.

The platform should feel like an editor's workbench from a design publisher,
not a tool from a tech company. Premium without being precious. Modern without
being mechanical. Designed without being decorative.

---

## 2. Color Palette & Roles

### Light mode (primary)

- **Warm Bone** (#FAFAF7) — Primary background. Slightly off-white with paper
  warmth. Never clinical blue-white, never pure #FFFFFF for backgrounds. The
  background is a solid flat color — NO gradient shapes, NO colored blobs,
  NO decorative blurs, NO ambient lighting effects, NO mesh gradients, NO
  noise/grain textures. Just one solid Warm Bone fill behind everything.
- **Pure Surface** (#FFFFFF) — Card and elevated container fill, used with
  whisper shadow.
- **Charcoal Ink** (#1B1A18) — Primary text. Warm-shifted near-black, never
  pure #000000. Carries a hint of brown undertone.
- **Steel Secondary** (#6B6862) — Body text, descriptions, metadata. Warm gray.
- **Muted Tone** (#9C9890) — Tertiary text, timestamps, disabled hints,
  placeholder text.
- **Whisper Border** (rgba(40, 30, 20, 0.06)) — Card borders, structural 1px
  lines. Semi-transparent for paper-like depth.
- **Diffused Shadow** (rgba(40, 30, 20, 0.05)) — Card elevation. Wide-spreading
  40px blur, -15px y-offset. Tinted warm to background.

### Dark mode (parallel core)

- **Deep Bark** (#18171A) — Primary background. Organic warm-near-black with
  brown undertone. Never pure #000.
- **Elevated Bark** (#22201E) — Card and elevated container fill, slightly
  warmer than background.
- **Warm Ivory** (#F5F2EC) — Primary text. Cream-shifted off-white.
- **Steel Secondary Light** (#9B968B) — Body text, descriptions, metadata.
- **Muted Tone Dark** (#6B6862) — Tertiary text, disabled.
- **Whisper Border Dark** (rgba(245, 242, 236, 0.08)) — Card borders.
- **Diffused Shadow Dark** (rgba(0, 0, 0, 0.35)) — Card elevation, deeper to
  read against bark backgrounds.

### Single accent — Aged Brass

- **Aged Brass** (#B8956A) — Single accent for primary CTAs, focus rings,
  active state highlights, status running pulse. Light mode default.
- **Aged Brass Deep** (#8A6F4F) — Hover and active state in light mode.
- **Aged Brass Glow** (#C9A87A) — Dark mode accent, slightly lighter to read
  against bark backgrounds.
- **Aged Brass Surface** (rgba(184, 149, 106, 0.10)) — Tinted surface for
  selected rows, active tab underlines, subtle accent zones.

Maximum **one** accent across the entire platform shell. Project surfaces in
their iframes may use their own accents; the platform never tries to color-coordinate
with the projects it hosts.

### Status semantics (subtle, never neon)

- **Running** — Aged Brass with breathing pulse animation
- **Stopped / Installed** — Steel Secondary, no animation
- **Starting / Stopping** — Muted Tone with subtle shimmer
- **Failed** — Deep Rust (#9A4A33) — desaturated red-brown, never bright red
- **Updating** — Muted Tone with shimmer

### Banned colors

- Pure black (#000000)
- Pure white (#FFFFFF) for **backgrounds** (it's allowed for card fills only)
- Purple / violet / "AI gradient"
- Neon outer glows of any kind
- Saturation above 70% on accents
- Mixing warm and cool grays in the same surface

---

## 3. Typography Rules

### Stack

- **Display** — `Cabinet Grotesk` (700, 800, 900). Track-tight (`-0.025em`),
  leading compressed (`1.05` for display, `1.15` for headings). Used for page
  titles, project card titles, navigational eyebrows.
- **Body** — `Geist` (400, 500, 600). Relaxed leading (`1.55`), max-width
  `65ch` for long-form. Used for descriptions, settings text, dialog body.
- **Mono** — `Geist Mono` (400, 500). Used for: package IDs, version strings
  (`v1.2.0`), commit hashes, secret-store names (`OPENAI_API_KEY`), file paths,
  numeric metadata in dense contexts.
- **CJK fallback** — `Noto Sans SC` (paired with Geist family). Display titles
  in Chinese fall back to `Noto Sans SC` weight 700-900 — there is no Cabinet
  Grotesk Chinese, so Chinese display headings inherit Noto Sans SC's character.

### Scale

- Display title (Home eyebrow + section openers): `clamp(2rem, 3vw, 2.5rem)`
  (≈32-40px), weight **700**, tracking `-0.02em`. Resist headline-poster
  scale — this is a workshop app, not a magazine cover.
- Page title (Settings sections): `clamp(1.5rem, 2.5vw, 2rem)`, weight 700
- Card title: `1.125rem` (18px), weight 700, tracking `-0.015em`
- Body: `1rem` / `1.0625rem`, weight 400
- Body small (metadata, timestamps): `0.8125rem`, weight 400, color Steel
  Secondary
- Mono metadata: `0.8125rem`, weight 400

### Weight-driven hierarchy

Hierarchy comes from **weight contrast and color contrast**, not just from
size. A 1.25rem 700-weight Charcoal Ink title sits above a 1rem 400-weight
Steel Secondary description — that contrast is the relationship. Resist the
urge to make every heading bigger.

### Banned typography

- `Inter` font — banned everywhere on platform shell
- Generic system serifs (`Times New Roman`, `Georgia`, `Garamond`, `Palatino`)
- All-caps stylized headings except small eyebrows (`text-transform: uppercase`
  is allowed only for eyebrow elements with letter-spacing `0.14em`)
- Italic body text for emphasis (use weight contrast instead)
- Gradient-fill on display text
- Drop shadows on text

---

## 4. Component Stylings

### Buttons

- **Primary** — Aged Brass background, Warm Ivory text (light mode: white text).
  Border-radius `0.625rem` (10px). Padding `0.625rem 1.125rem`. Font weight 500.
  No outer glow. Active state: `translateY(-1px)` then snap back, simulating
  tactile push. Hover: shift to Aged Brass Deep (light) or Aged Brass Glow
  (dark).
- **Secondary** — Ghost / outline. 1px Whisper Border, transparent background,
  Charcoal Ink text. Hover: subtle Whisper Border darken + background shift to
  rgba(40, 30, 20, 0.03).
- **Tertiary / inline link** — Underlined Charcoal Ink with `text-underline-offset:
  4px`, `text-decoration-thickness: 1px`. No icon-stuffed inline links.
- **Destructive** — Deep Rust border, Deep Rust text, transparent fill. Hover
  fills Deep Rust at 0.05 opacity. Used only for uninstall, delete data.
- **Icon button** — 36px square, no border, hover shifts background to
  rgba(40, 30, 20, 0.04).

### Cards

- **Project card** — Pure Surface fill, `1.25rem` (20px) border-radius, 1px
  Whisper Border, Diffused Shadow. Internal padding `1.25rem`. Width target
  `280-340px` per card. Hover: `translateY(-2px)` + shadow deepens slightly.
  Cards must feel **dense**, not airy: each card carries an icon, title,
  description, status pill, version metadata, last-active timestamp, and a
  bottom action bar with two buttons (primary + secondary like ⋯ menu). Avoid
  big empty zones inside the card — every region earns its space. The shelf
  reads like a craftsman's tool rack, not a gallery wall.
- **Settings panel card** — Same shape but larger, used for grouping form
  rows. Internal padding `2rem`. Internal sections divided by `1px Whisper
  Border` horizontal lines, not nested cards.
- **Empty state card** — Full-width within container, dashed Whisper Border
  doubled to 0.10 opacity, no fill, centered content with composed icon (not
  emoji), generous padding `3rem`.
- **Cards used only when elevation communicates hierarchy.** Settings rows,
  metadata lists, and dense data should use horizontal `Whisper Border` dividers
  instead of nested cards.

### Inputs / Forms

- Label sits **above** the input (`gap: 0.5rem` between label and input)
- Input: 1px Whisper Border, transparent background, `0.625rem 0.875rem` padding,
  `0.5rem` border-radius. Focus: 2px Aged Brass ring with `2px` offset, no
  shadow change.
- Helper text below input in Steel Secondary `0.8125rem`
- Error text below input in Deep Rust `0.8125rem` with `0.5rem` top margin
- Required indicator: small Aged Brass dot (`•`) after label, never `*`
- Search input has a 16px Phosphor icon (`MagnifyingGlass`) in `0.875rem`
  Steel Secondary inset on the left, padding-left `2.5rem`
- Password fields use a Phosphor `Eye` / `EyeSlash` toggle button on the right

### Navigation / Topbar

- Sticky topbar at `top: 0`, height `60px`, background Warm Bone with `0.85`
  opacity + `backdrop-filter: blur(20px)`, 1px Whisper Border bottom
- Left side: text logo `Yggdrasil` in Cabinet Grotesk 700 weight at `1.125rem`,
  paired with current breadcrumb in Steel Secondary
- Right side: settings icon (Phosphor `GearSix`), notification bell with status
  dot, theme toggle (sun/moon icons)
- No mobile hamburger — primary nav surfaces (Home, Settings) accessible via
  topbar always

### Project frame topbar (mounted iframe wrapper)

When a project's surface is mounted, the platform shows a thin (40px) topbar
above the iframe:
- Left: back arrow icon (Phosphor `ArrowLeft`) → returns to Home
- Center-left: project name in Cabinet Grotesk 700 + state pill
- Right: Stop button (only when state is Running), uninstall menu, project
  settings icon
- Background: Elevated Bark (dark mode) or Warm Bone (light mode), 1px Whisper
  Border bottom
- The iframe content occupies the rest of the viewport; the topbar never
  intrudes into the project's UI

### Toasts / Notifications

- Slide in from bottom-right with spring physics
- Width `360px`, padding `1rem`, border-radius `1rem`
- 1px Whisper Border, Pure Surface fill, Diffused Shadow
- Optional left border accent (4px) in semantic color (Aged Brass for info,
  Deep Rust for error)
- Auto-dismiss after 4s, hover pauses, click X to dismiss
- Stack with `0.5rem` gap, max 3 visible, older auto-dismiss

### Status pills

- Pill shape (border-radius 999px), `0.25rem 0.625rem` padding
- Font Geist Mono 500 weight, `0.6875rem`, uppercase, letter-spacing `0.06em`
- Backgrounds: Whisper Border surface for neutral states, Aged Brass Surface
  for Running with subtle pulse animation
- A small `8px` colored dot leads the text, dot pulses when state is animated

### Modals / Dialogs

- Centered overlay with `rgba(40, 30, 20, 0.5)` backdrop + backdrop-blur 8px
  in light mode, `rgba(0, 0, 0, 0.6)` + blur 12px in dark mode
- Modal container: Pure Surface fill, border-radius `1.5rem`, padding `2rem`,
  max-width `560px` for forms, `720px` for plans / wizards
- Title in Cabinet Grotesk 700 `1.5rem`
- Close button is icon-only Phosphor `X` in upper-right
- Action buttons in lower-right: secondary first, primary last, gap `0.75rem`

### Loaders / Skeletons

- Skeletal shimmer matching exact layout dimensions
- Background: linear-gradient from Whisper Border at 0.04 to 0.08 opacity,
  shifting horizontally on a 1.6s loop
- Border-radius matches the shape it's standing in for
- **Never** circular spinners, **never** spinning circles of any kind

### Empty states

- Composed icon (Phosphor outline weight 1.5) at `48px`, Steel Secondary color
- Heading in Cabinet Grotesk 700 `1.125rem`, Charcoal Ink
- Body in Geist 400 `0.9375rem`, Steel Secondary, max-width `40ch`
- Optional CTA button in primary or secondary style
- Centered within container, generous vertical padding `4rem 2rem`

### Error states

- Inline contextual error (form fields)
- Banner errors: 1px Deep Rust border, Deep Rust at 0.04 background, padding
  `0.875rem 1rem`, border-radius `0.75rem`
- Recovery action presented inline as a button or text link

---

## 5. Hero Section (Home as Hero)

The Home is the platform's first impression — it must establish the editorial
workshop atmosphere immediately, without marketing chrome. **The hero must NOT
read as a magazine cover or poster** — it is a workshop entrance, not an
editorial spread. Headline scale is restrained; surrounding UI density grounds
the type.

- **Restrained scale** — title at `clamp(2rem, 3vw, 2.5rem)` (≈32-40px), weight
  700, NOT 800. Tracking `-0.02em`. The title sits among utility chrome, not
  alone on a page.
- **Asymmetric layout** — split 55/45 desktop. Left zone holds the eyebrow
  ("WORKSHOP" small caps in mono), the title (e.g., "Welcome back, Hana"),
  and a single line of context. Right zone holds an **activity micro-card**
  with concrete utility: a recent activity log line ("Last active YdlTavern
  · 2h ago"), an inline action button ("Resume"), and metadata in mono.
  The right zone is NOT empty negative space — it is a small functional panel.
- **Hero chrome row** — directly below the hero, before the project shelf,
  insert a horizontal **utility strip** with: a search input ("Search projects,
  packages, settings…" placeholder, 320px wide), a row of filter chips ("All
  10 · Running 1 · Stopped 8 · Failed 1"), and a small "Sort by" dropdown.
  This strip transforms the screen from "magazine page" to "workshop control
  bench" — it is essential, not optional.
- **No filler chrome** — banned: "Scroll to explore", "Get started", scroll
  arrows, animated chevrons, "Welcome to your platform" marketing copy,
  hero search bars spanning full width, gradient backdrops.
- **One primary action** — the "+ Install project" entry sits inline as one of
  the project cards (always last, dashed border style), not as a separate
  giant CTA in the hero.
- **Hero is contextual, not decorative** — the right ambient zone always shows
  *something the user can act on or learn from*: recent activity, inline
  resume, package update count, system status. Never just a quote sitting in
  empty space.
- **Project shelf is the substance** — hero + utility strip should occupy
  roughly 35vh, then immediately give way to the project shelf grid.
- **No centered hero layout** — variance level 8 forbids it. Force the
  asymmetric 55/45 or fully-left-aligned arrangement.

---

## 6. Layout Principles

- **CSS Grid** for all structural layouts. Never `calc(33% - 1rem)` flexbox
  math.
- **Project shelf grid** — `grid-template-columns: repeat(auto-fill, minmax(280px,
  1fr))`, `gap: 1.25rem`. The "+ Install" card is part of the same grid (always
  last). At 4+ cards, this naturally creates an asymmetric tail.
- **Bento variation for landing zones** — when a settings page has 3+ panels
  of related controls, prefer 2-column asymmetric (`grid-template-columns:
  2fr 1fr`) or 3-column with one tall panel spanning two rows.
- **No 3-equal-column card rows.** The "feature row" pattern is banned. Use
  asymmetric splits, zig-zag, or fluid grids instead.
- **No element overlapping.** Text never sits on images, no absolute-positioned
  layers stacking content. Every element occupies its own clear spatial zone.
- **Containment** — `max-width: 1400px`, centered, with horizontal padding
  `1rem` (mobile), `2rem` (tablet), `4rem` (desktop ≥1024px).
- **Full-height** — `min-height: 100dvh`, never `height: 100vh`.
- **Section vertical rhythm** — `clamp(3rem, 7vw, 5.5rem)` between major
  sections.

---

## 7. Responsive Rules

Every screen must be tested at `375px`, `768px`, `1024px`, `1440px`. Mobile
viewport breaks are critical failures.

- **Mobile (`< 768px`)** — All multi-column layouts collapse to single column.
  Project shelf becomes one card per row. Hero collapses: eyebrow + title +
  body stack vertically; the right ambient zone is hidden. Topbar collapses
  to logo + icon row; settings menu accessible via single icon button.
- **Tablet (`768px - 1023px`)** — Project shelf shows 2 cards per row.
  Settings panels show single column with max-width.
- **Desktop (`≥ 1024px`)** — Full editorial layout. Project shelf shows 3-4
  cards per row depending on viewport. Hero takes its asymmetric shape.
- **Touch targets** — All interactive elements ≥ 44px on mobile. Buttons
  full-width on mobile.
- **Typography** — Headlines scale via `clamp()`. Body never below `15px` /
  `0.9375rem` on mobile. Mono metadata stays at `0.8125rem`.
- **No horizontal scroll** — anywhere, ever, at any viewport.

---

## 8. Motion & Interaction

> Stitch generates static screens. This section documents intended animation
> behavior so React implementation knows what to build.

- **Spring physics exclusively** — `stiffness: 100, damping: 20` baseline. No
  linear easing. Cubic-bezier acceptable for CSS transitions: `cubic-bezier(0.16,
  1, 0.3, 1)`.
- **Project card hover** — `translateY(-2px)` + shadow deepens from
  `0 20px 40px -15px rgba(40,30,20,0.05)` to `0 24px 48px -15px rgba(40,30,20,0.08)`.
  Spring transition.
- **Project card mount cascade** — list items reveal with staggered delay
  `calc(var(--index) * 60ms)`, fade-in + `translateY(8px)` to 0. Capped at 12
  items, after which they appear without animation (perf cap).
- **Status pill running pulse** — leading dot pulses on a 2.4s ease-in-out
  loop, opacity 1 → 0.5 → 1. Pill background subtle glow on Aged Brass Surface
  shifting opacity 0.10 → 0.18 → 0.10.
- **Skeleton shimmer** — gradient sweep, 1.6s infinite, `transform: translateX`
  only.
- **Toast spring entrance** — slide in from bottom-right with `translateY(100%)`
  → 0, spring 100/15.
- **Modal entrance** — backdrop fades in 200ms, modal scales `0.96` → `1` +
  fades, spring 120/22.
- **Topbar sticky** — appears with backdrop blur immediately (no transition);
  scroll past hero shows shadow `0 1px 0 Whisper Border`.
- **Page route transitions** — fade + 4px translateY, 240ms.
- **Hardware rules** — animate only `transform` and `opacity`. Never `top`,
  `left`, `width`, `height`, `margin`. Grain or noise filters on fixed
  pointer-events-none pseudo-elements only.
- **Performance** — perpetual loops isolated in their own components, never
  trigger parent re-renders. Target 60fps minimum.

---

## 9. Anti-Patterns (Banned)

### Visual

- No emojis anywhere — UI, code, alt text, copy
- No `Inter` font, no generic system serifs, no Times New Roman
- No pure black (#000000), no pure white backgrounds (only as card fill)
- No purple / violet / "AI gradient" anything
- No neon outer glows, no `box-shadow` defaults
- No oversaturated accents above 70%
- No gradient text on headings
- No drop shadows on text
- No custom mouse cursors
- No hover effects that move content (text shifts, layout reflows on hover)

### Layout

- No centered hero sections
- No 3-equal-column card rows
- No `h-screen` (always `min-h-[100dvh]`)
- No flexbox percentage math (`calc(33% - 1rem)` and similar)
- No overlapping elements (no z-index spam, no absolute layered text on images)
- No nested cards (cards inside cards inside cards)
- No `z-index` above 50 except for: navbar (10), modal (40), toast (50)

### Copy / content

- No AI copywriting clichés: "Elevate", "Seamless", "Unleash", "Next-Gen",
  "Revolutionize", "Empower"
- No filler UI text: "Scroll to explore", "Swipe down", "Discover more",
  scroll arrows, bouncing chevrons, "Welcome aboard"
- No generic placeholder names: "John Doe", "Sarah Chan", "Acme", "Nexus",
  "SmartFlow"
- No fake round numbers: `99.99%`, `50%`, `1234567`. Use organic data:
  `47.2%`, `+1 (312) 847-1928`
- No marketing hyperbole on internal platform pages

### Implementation

- No broken Unsplash links — use `picsum.photos/seed/{id}/800/600` or local
  SVG composed assets
- No generic `shadcn/ui` defaults — every component must have radii, colors,
  shadows customized to this system
- No circular loading spinners — skeletal shimmer only
- No emoji icons for UI — use Phosphor outline weight 1.5 or Radix icons
- No mixed icon libraries within a screen

---

## 10. React implementation hints

- Tailwind v4; define design tokens through `@theme` and CSS variables in `src/styles/app.css`, with no `tailwind.config.js`
- Re-export `@phosphor-icons/react` semantically from `src/components/icons.tsx`; page code does not mix icon libraries directly
- Bundle Bricolage Grotesque, Geist, and JetBrains Mono through `@fontsource-variable/*`; do not require a runtime CDN
- Use Motion v12 for entrance, exit, and spring behavior; isolate perpetual loops in memoized leaf components and honor `prefers-reduced-motion`
- Drive themes through CSS variables and `<html data-theme="light|dark">`, following the system by default with an explicit user override
- Reuse `client-core` and the public protocol/Host access clients; page components do not duplicate authentication, RPC, SSE, or pairing logic

---

## 11. Stitch generation guidance

When prompting Stitch with this design system:

- Always specify the screen is for "Yggdrasil platform shell" (not a project,
  not YdlTavern, not a chat product) so it doesn't pull SaaS / chatbot
  references
- Reinforce: warm cream (#FAFAF7) background, single Aged Brass (#B8956A)
  accent, asymmetric layouts, no centered heroes, no emojis, Cabinet Grotesk
  for display
- For the Home screen: "An asymmetric editorial workshop home with a 60/40
  split hero and a Bento-style project shelf below"
- For Settings screens: "Calm forms with section dividers, no nested cards,
  weight-driven hierarchy"
- Avoid Stitch's tendency to add: gradient backgrounds, centered marketing
  CTAs, emoji decorations, three-up feature card rows, AI purple. Bind it
  to this document's anti-pattern list.
