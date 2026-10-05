---
name: enterprise-design-system
description: Visual language and UI patterns for the Telemetry ecosystem (telemetry-web portal and the telemetry-apps admin console) — sober, enterprise, trustworthy; explicitly not the generic "AI-generated" SaaS look. Use whenever building or restyling any page, layout or component under app/ or components/ (dashboard, onboarding, auth, marketing), choosing colors, typography, spacing, icons, or writing UI copy.
---

> **telemetry-apps note (admin-client).** This is the same design system as `telemetry-web` (and the rest of the owner's ecosystem: shared structure, one accent per product — Telemetry is red). In the admin app (Tauri + React 18 + Tailwind 3): tokens live in `admin-client/src/index.css` (light + `prefers-color-scheme: dark`) and are mapped in `admin-client/tailwind.config.js` (`bg-surface`, `text-muted`, `border-default`, `bg-accent`, `bg-ink`…). IBM Plex is bundled with `@fontsource` (no network). Canvas/SVG renderers (ECharts, Leaflet) take colors from `src/hooks/useThemeColors.ts`; map tiles and popups use the fixed light palette. The "dashboard shell" below maps to the admin sidebar (ink, active item = raised ink + 2px red bar + red icon) and the auth screen (ink top bar + centered card). Never use raw palette classes (`bg-indigo-600`, `text-slate-400`) or emojis in UI.


# Enterprise design system — telemetry-web

The portal handles billing, devices and the admin's encryption key. It must feel like a **bank or an infrastructure console**, not a startup landing page: calm, precise, dense enough to be useful, and honest about state.

**Brand direction (set by the product owner, 2026-09-30):** black / white / red corporate look. Always-dark "ink" surfaces (marketing hero and footer, page title bands, top bars, dashboard sidebar) alternate with white content sections; red is the single brand accent. Reference: a mockup plus an agency site with a black hero crossed by a red light streak, uppercase tracked CTAs and boxy corners. Follow the spirit, not the pixels.

Stack: Next.js App Router + React + Tailwind (see `.ai-context/architecture.md`). Copy in es-CL, addressing the admin as **tú**.

After building UI, review it with the `web-design-guidelines` skill.

## 1. Principles

1. **Calm over impressive.** Hierarchy comes from type weight, spacing and alignment — not color, gradients or motion.
2. **State is always explicit.** Every screen shows loading, empty, error and "no disponible" states. Never fake success or placeholder data (AGENTS.md: no fake data).
3. **Security is visible but not theatrical.** Explain consequences in plain words (losing the key = losing the data). No padlock illustrations, shields or "military-grade" copy.
4. **Consistency beats novelty.** Reuse the patterns in §5 before inventing a new one.

## 2. Avoid — the generic AI/SaaS look

Flag and don't produce:

- Gradients on buttons, text or cards, especially purple→pink/blue. (The only gradient allowed is the decorative hero backdrop: dark fade + red streak.)
- Glassmorphism, `backdrop-blur` cards, glows, colored or large shadows.
- Emojis or ✨ sparkle icons in UI; decorative blobs, grids or noise backgrounds.
- `rounded-2xl`/`rounded-3xl` everywhere, pill-shaped primary buttons, oversized cards with a giant centered icon.
- Hero sections, big illustrations or marketing headlines inside the dashboard.
- Gratuitous motion: entrance animations, bouncing, parallax, hover scale on cards.
- Enthusiastic copy: "¡Genial!", "Supercharge", exclamation marks, filler like "Bienvenido a tu nuevo hub".
- Invented social proof on marketing pages (logos, testimonials, "10.000+ empresas") — no fake data.

## 3. Tokens

Define tokens as CSS variables in `app/globals.css` (light + `.dark`/`prefers-color-scheme`) and map them in the Tailwind config. Components use **semantic** names (`bg-surface`, `text-muted`, `border-default`), never raw palette classes like `bg-indigo-600`.

### Color

| Token | Light | Dark | Use |
|:--|:--|:--|:--|
| `--bg` | `#fafafa` | `#0a0a0a` | Page background |
| `--surface` | `#ffffff` | `#141414` | Cards, tables, white sections |
| `--surface-subtle` | `#f5f5f5` | `#1f1f1f` | Table header, hovered row, code/fingerprint |
| `--border` / `--border-strong` | `#e5e5e5` / `#d4d4d4` | `#262626` / `#404040` | Dividers / inputs |
| `--text` / `--text-muted` | `#0a0a0a` / `#525252` | `#fafafa` / `#a3a3a3` | Text |
| `--accent` | `#dc2626` | `#ef4444` | Primary buttons, links, focus, kickers |
| `--ink`, `--ink-raised`, `--ink-line`, `--ink-fg`, `--ink-muted` | `#0a0a0a`, `#141414`, `#262626`, `#fafafa`, `#a3a3a3` | same | Always-dark brand surfaces (`bg-ink`, `text-ink-muted`…) |
| `brand` | `#dc2626` | same | Red that doesn't change with the theme (`bg-brand`, `text-brand`) on ink surfaces |
| `--success` / `--warning` / `--danger` / `--info` | green / amber / `#b91c1c` / blue | lighter | Status only |

- Neutral greys (no blue tint). **One accent: red**, used for primary actions, kickers, active nav and small marks — most of the UI stays black, white and grey.
- Danger (`#b91c1c`) is a darker red than the accent; destructive actions still live only inside confirmations or danger zones.
- Status colors only for status, always with text. Contrast: WCAG AA minimum.

### Typography

- Self-hosted via `next/font` (strict CSP, WEB-SEC-09): **IBM Plex Sans** for UI, **IBM Plex Mono** for fingerprints, IDs, hashes. One family each; no display fonts.
- Scale (px): 12 · 13 · 14 · 16 · 20 · 24 · 30. Dashboard body **14**, marketing body **16**.
- Weights: 400 body, 500 labels/nav/table headers, 600 dashboard titles; **700 only for marketing headlines**.
- **Kicker**: 11px, 600, `tracking-[0.18em]`, uppercase, red, with a short red rule before it — above marketing section titles.
- **CTA buttons** (marketing): size `cta` — uppercase, 12px, `tracking-[0.12em]`, optional chevron.
- Page title 24/600; section title 16/600; table header 12–13/500 muted.
- `tabular-nums` for money, dates, counts. Sentence case everywhere.

### Space, shape, elevation

- 4px grid; prefer 4 · 8 · 12 · 16 · 24 · 32 · 48.
- Radius: 4px controls, 6px cards/dialogs, 3px badges. Boxy, never pills.
- Borders do the separating; shadows only for overlays (menus, dialogs): `shadow-sm`/`shadow-md`, neutral.
- Icons: one outline set (e.g. `lucide-react`), 16px inline / 20px nav, `stroke-width` 1.5–2, `aria-hidden` when decorative.
- Motion: 120–200ms, `ease-out`, opacity/transform only, only for state changes (menus, dialogs, toasts). Respect `prefers-reduced-motion`.

## 4. Layout

**Dashboard shell**
- Left sidebar on **ink** (240px; stacks on mobile): logo, "Organización" label + name, nav (Suscripción, Facturación, Cuenta, Dispositivos); active item = slightly lighter ink + 2px red bar on the left + red icon; user and "Cerrar sesión" at the bottom.
- Global subscription banner above the content; content area light, `max-w-5xl`, 32px padding desktop / 16px mobile.
- **Page header**: title + one-line description on the left; at most one primary action on the right.

**Auth & onboarding**
- Ink top bar with the logo; white page with a single centered column (`max-w-md` auth, `max-w-xl` onboarding). No illustrations.
- Onboarding uses a numbered **stepper** with the real steps.

**Marketing** (`app/(marketing)`)
- Ink header (logo, uppercase tracked links, "Iniciar sesión" ghost, red "Crear cuenta") and ink footer.
- Landing rhythm: **ink hero** (kicker, bold headline with one phrase in red, short paragraph, red CTA + outline CTA, row of real technical facts) → **white** "Cómo funciona" (01/02/03 columns under a heavy rule) → **ink** differentiators (2×2 grid of raised cards, red icons) → white detail section → **red** closing band (white outline CTA) → ink footer.
- **The hero is not full height**: on a 1280×800 laptop the first screen must show the black hero *and* the start of the white section; on mobile, hide secondary hero content (fact row) to keep that true.
- Inner pages (pricing, downloads, legal) start with an ink title band (`PageHero`) followed by white content; cards get a 2px red top border.
- Only real facts: no invented stats, client logos or testimonials, even if a reference design has them.

## 5. Patterns

**Subscription status badge** — text + color, from `effective_status`:

| Status | Label | Tone |
|:--|:--|:--|
| `TRIAL` | Prueba · N días | warning if ≤ 7 days, else neutral |
| `ACTIVE` | Activa | success |
| `SUSPENDED` | Suspendida | danger |
| `CANCELLED` | Cancelada | danger |

**Banners** (dashboard layout, from `.ai-context/domain.md` §4): full-width, left icon, one sentence, one action. `TRIAL ≤ 7`: warning, "Te quedan N días de prueba." + "Elegir plan". `SUSPENDED`/`CANCELLED`: danger, copy per `reason` from `lib/errors.ts` + "Pagar ahora". Not dismissible while the state holds.

**Tables** (devices, payments): header `--surface-subtle`, rows 44–48px, 1px row dividers, no zebra striping. Right-align money; dates in es-CL. Row actions in a trailing "⋯" menu; destructive items last, in danger color. Empty state inside the table frame.

**Card** — `--surface`, 1px `--border`, 8px radius, 24px padding, title row + content. Use for grouping, not decoration; don't nest cards.

**Buttons** — Primary (accent, one per view), Secondary (surface + border), Ghost (text only), Danger (only inside confirmation dialogs or danger zones). Height 36px dashboard / 40px auth. Spinner + "…" label while pending.

**Forms** — label above the field, help text below in muted, inline error in danger with icon. Field width matches expected content (RUT, email, plan) — not always full width.

**Confirmation dialog** (revoke device, delete vault): title states the action ("¿Revocar “Laptop de Ana”?"), body states the consequence in one or two sentences, buttons "Cancelar" (secondary) + the specific verb (danger). For irreversible key/data consequences, require an explicit checkbox ("Entiendo que …").

**Danger zone** — at the bottom of Cuenta, bordered in `--danger` at low emphasis, each action with its consequence.

**Fingerprint / IDs** — Plex Mono, grouped in blocks of 4, `--surface-subtle` background, copy button with "Copiado" feedback (aria-live).

**States**
- Loading: skeletons matching the final layout for lists/cards; spinner only inside buttons. Long crypto work (Argon2) shows a determinate step message ("Derivando clave… esto puede tardar unos segundos").
- Empty: one sentence explaining why it's empty + the next action. No illustrations.
- Error: what failed + what to do, copy from `lib/errors.ts`.
- Not available: "No disponible por ahora." for unimplemented features — never a fake success.

**Toasts** — only for confirming completed actions ("Dispositivo revocado"). Errors that need action go inline, not in a toast.

## 6. Copy voice (es-CL)

- Tú, direct, calm, specific. Short sentences.
- Verbs in labels: "Guardar cambios", "Descargar respaldo", "Revocar dispositivo".
- Name things consistently: "contraseña maestra" (never just "contraseña" for it), "respaldo (.tkb)", "bóveda", "dispositivo", "suscripción".
- Say consequences plainly: "Si pierdes tu respaldo y tu contraseña maestra, nadie — ni nosotros — podrá recuperar tus datos."
