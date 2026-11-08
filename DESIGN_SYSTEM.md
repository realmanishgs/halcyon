# Design system

Principles: neutral foundation, **one** accent family, type and spacing carry hierarchy, no decorative
shadows/gradients/glass, motion only for state feedback, everything technical behind progressive disclosure.
Source of truth: CSS custom properties at the top of [`src/styles.css`](src/styles.css).

## Color

| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | `#f6f6f4` | `#0d0d0e` | App background |
| `--surface` | `#ffffff` | `#151517` | Sidebar, transport, panels |
| `--surface-2` | `#eeeeeb` | `#1d1d20` | Hover, selected, placeholders |
| `--border` | `#dcdcd8` | `#2a2a2e` | 1px dividers only where structure needs it |
| `--text` / `--text-2` / `--text-3` | `#111` / `#55554f` / `#8a8a84` | `#ededea` / `#a3a39e` / `#6d6d69` | Primary / secondary / tertiary-disabled |
| `--accent` | `#2f5bd3` | `#7d9bff` | Progress fill, active toggles, stream tags, primary action |
| `--danger` | `#b42318` | `#ff6b5e` | Errors only |
| `--video-bg` | `#000` | `#000` | Video letterbox |

Theme: follows the OS by default; `data-theme="light|dark"` on `<html>` forces either.
Accent is user-selectable in Phase 6 (single hue swap; contrast re-validated ≥ 4.5:1 for text use).

## Type

UI `Segoe UI Variable Text` → system-ui; display `Segoe UI Variable Display`; technical values `Cascadia Mono` → Consolas.
No web fonts (offline, privacy). Scale: 11 / 12 / 13 (base) / 16 / 22 / 32 px. Section labels: 11px, 600, uppercase, +0.08em.

## Space & size

Spacing 4 / 8 / 12 / 16 / 24 / 32 / 48. Radius 4 (controls), 6 (artwork) — nothing pill-shaped except the play button.
Control height 32, icon 18 (24-unit grid, 1.75 stroke), sidebar 216 / 56 collapsed, transport 88.

## Motion

`--dur-fast` 120ms, `--dur-base` 200ms, `cubic-bezier(.2,0,0,1)`. Both become 0 under `prefers-reduced-motion`.

## States

Hover → `--surface-2` background. Focus → 2px `--focus` outline, 2px offset (keyboard only, `:focus-visible`).
Disabled → `--text-3`, no hover. Pressed toggles → accent icon (`aria-pressed`). Playback: play button inverts (text-on-bg disc).

## Components (implemented)

Sidebar nav item (current / disabled-with-phase), icon button, play button, primary/secondary button,
range slider with accent fill (`--fill`), compact select, key-value list (`.kv`), stream block, banner, kbd hint.

## Layout rules

- Video box never has HTML overlays (native surface sits above the webview; see ARCHITECTURE).
- Technical detail lives in the Inspector (toggle `I`), not in the default view.
- ≤ 900px: transport drops the right cluster; inspector narrows. Touch targets ≥ 32px today, 44px planned for touch builds.
