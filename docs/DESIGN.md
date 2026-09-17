# Sentinel design system

Shared by the desktop app (`src/`) and the website (`website/`). Tokens live in
`src/styles/tokens.css`; the website imports the same file.

## Character

A night-watch instrument. The ground is a cool blue-graphite, like a monitoring room with the lights
down — not neutral black. One soft signal green marks what is live, healthy, or selected; amber and
coral appear only when something needs attention. Chrome stays quiet so the data carries the screen.

Spend boldness in one place per screen. In the app that place is the data itself: the world map of
live connections, the storage treemap, the per-core field. Tables, sidebars, and headers stay
disciplined around them.

Dials — app: variance 4, motion 6, density 7. Website: variance 8, motion 6, density 3.

## Color

| Token | Dark (default) | Light | Use |
|---|---|---|---|
| `--ground` | `#0E1217` | `#F3F5F7` | window background |
| `--panel` | `#131820` | `#FFFFFF` | panels, sidebars |
| `--raised` | `#1A2029` | `#EAEEF2` | hover rows, popovers |
| `--sunken` | `#0A0D11` | `#E3E8ED` | inputs, graph wells |
| `--line` | `rgb(214 226 240 / 0.07)` | `rgb(16 24 34 / 0.08)` | hairlines |
| `--line-strong` | `rgb(214 226 240 / 0.14)` | `rgb(16 24 34 / 0.16)` | focused borders |
| `--fg` | `#E3E8EE` | `#121820` | primary text |
| `--fg-muted` | `#97A1AE` | `#525D6A` | secondary text |
| `--fg-subtle` | `#5D6774` | `#87919D` | axes, tertiary text |
| `--signal` | `#74D3AE` | `#16895F` | live, healthy, selected, primary action |
| `--signal-ink` | `#062016` | `#FFFFFF` | text on signal |
| `--warn` | `#E3A857` | `#A86A10` | elevated usage, caution |
| `--danger` | `#E26458` | `#BF3A2E` | destructive actions, critical |

Rules: no pure black, no purple/blue glow gradients, no gradient text, no decorative gradient washes.
Shadows are tinted with `--shadow-tint`. Destructive confirm buttons use `--danger`; firewall confirms
additionally require typing the target to confirm.

Data-visualization palettes (treemap file kinds, per-core ramps, network in/out) live in
`src/styles/dataviz.ts`, built with the `dataviz` skill and validated for contrast on both grounds.
They are data colors, not accents — they never style chrome.

## Type

- UI: **Geist Variable** (`@fontsource-variable/geist`), sentence case everywhere.
- **Geist Mono Variable** with `font-variant-numeric: tabular-nums` only where alignment matters:
  numbers, PIDs, ports, addresses, paths, command lines. Labels and headings are never mono.
- Scale (px): 11 axis · 12 dense table · 13 body · 15 section title · 20 panel headline · 28 hero metric.
- Hierarchy through weight and color, not size jumps. Tracking -0.01em above 15px.

## Shape and space

- Radii scale with hierarchy: 5 controls · 8 panels · 12 dialogs and drawers.
- Structure is information: hairlines separate data; cards only when elevation carries meaning
  (dialogs, plan cards, detail drawer). No numbered markers unless the content is a real sequence.
- 4px base grid. Table rows 28px (dense) / 34px (comfortable).

## Motion

- Motion answers actions or shows change. Springs for interaction (`stiffness 380, damping 34`) and
  panels (`stiffness 260, damping 30`).
- Live numbers tween ~300ms ease-out; graphs scroll continuously rather than stepping.
- Ambient motion is reserved for genuinely live signals (map pulses, a live indicator), never decoration.
- Only `transform` and `opacity` animate. Honor `prefers-reduced-motion` (tweens become cuts, pulses stop).
- Loading is a skeleton of the real layout with a slow shimmer — never a bare spinner.

## Avoid (template tells)

All-caps eyebrow labels above headings · meta strings joined with middle dots · "→" appended to
button text · one accented word in a headline · identical rounded cards for every block · generic
"Submit"/"OK" buttons — actions keep one name through the flow ("Move to Trash" → "Moved to Trash").
Errors say what happened and how to fix it; empty states say how to get data.
