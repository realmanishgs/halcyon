# Design system

Direction from the UI UX Pro Max skill ("Dark Mode (OLED)", dark audio + play green), adapted to keep system fonts.
Principles: midnight-indigo foundation, **one** green accent family, type and spacing carry hierarchy, no decorative
shadows/gradients/glass, motion only for state feedback, everything technical behind progressive disclosure.
Source of truth: CSS custom properties at the top of [`src/styles.css`](src/styles.css).

## Color

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#f5f5fa` | `#0f0f23` | App background |
| `--surface` | `#ffffff` | `#16162e` | Sidebar, transport, panels |
| `--surface-2` | `#ebebf4` | `#22223d` | Hover, selected, placeholders |
| `--border` | `#d9d9e6` | `#2e2c5c` | 1px dividers only where structure needs it |
| `--text` / `--text-2` / `--text-3` | `#13122b` / `#4c4b66` / `#63627c` | `#f8fafc` / `#b4bccb` / `#94a3b8` | Primary / secondary / tertiary (≥ 4.5:1, WCAG AA) |
| `--accent` | `#15803d` | `#22c55e` | Play button, progress fill, active toggles, primary action |
| `--focus` | `#4338ca` | `#a5b4fc` | Keyboard focus ring |
| `--danger` | `#b42318` | `#ff6b5e` | Errors only |
| `--video-bg` | `#000` | `#000` | Video letterbox |

Theme: follows the OS by default; `data-theme="light|dark"` on `<html>` forces either.
Accent is user-selectable in Phase 6 (single hue swap; contrast re-validated ≥ 4.5:1 for text use).

## Type

UI `Segoe UI Variable Text` → system-ui; display `Segoe UI Variable Display`; technical values `Cascadia Mono` → Consolas.
No web fonts (offline, privacy). Scale: 11 / 12 / 13 (base) / 16 / 22 / 32 px. Section labels: 11px, 600, uppercase, +0.08em.

## Space & size

Spacing 4 / 8 / 12 / 16 / 24 / 32 / 48. Radius 6 (controls), 10 (artwork) — nothing pill-shaped except the play button.
Control height 32, icon 18 (24-unit grid, 1.75 stroke). Sidebar 216 default, drag-resizable 160–360, snaps to a 56 px icon rail below 120. Side panel 340 default, resizable 260–60 % of the window.

## Motion

`--dur-fast` 120ms, `--dur-base` 200ms, `cubic-bezier(.2,0,0,1)`. Both become 0 under `prefers-reduced-motion`.

## States

Hover → `--surface-2` background. Focus → 2px `--focus` outline, 2px offset (keyboard only, `:focus-visible`).
Disabled → `--text-3`, no hover. Pressed toggles → accent icon (`aria-pressed`). Playback: play button is an accent-filled disc.

## Components (implemented)

Sidebar nav item (current / disabled-with-phase), splitter, icon button (+ small), play button, primary /
secondary / small button, chip (toggle), seek bar (rail, fill, chapter ticks, hover time tip, thumb on
hover/drag), range slider with accent fill (`--fill`), stepper (delay), tabs, key-value list (`.kv`),
stream block with badges, banner (error / info), drop overlay, modal, queue row (hover actions), kbd hint.

## Layout rules

- Shell: sidebar (sections Play / Library / Tools, accent bar on the current page) | main column.
  Main column = top bar (page title, Stream + Open actions) → stage + side panel → floating transport dock.
- Transport dock: rounded card inset from the window edges; controls row (now-playing · transport · options)
  with the seek row below. In mini player it flattens back to an edge-to-edge bar.
- Every pane stretches: sidebar and panel are drag-resizable; the stage takes the rest.
- Breakpoints: ≤ 860 px wide → sidebar becomes an icon rail, speed chip and volume slider hide;
  ≤ 600 px → sidebar hides, secondary transport buttons hide, an open panel takes the full width;
  ≤ 420 px tall → artwork thumbnail and subtitle line hide. Minimum window 420 × 300.
- Fullscreen: sidebar and panel hide, controls auto-hide after 2.5 s idle (cursor hidden too) and return
  on any pointer movement over video or UI.
- Video box never has HTML overlays (native surface sits above the webview; see ARCHITECTURE). Things that
  must appear above video are native (context menu) or hide the surface while shown (URL dialog, drop
  overlay).
- Technical detail lives in the side panel (Info), not in the default view.
