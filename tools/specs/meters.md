# NodaAudio port spec — the meters family

Source of truth: `audiocn` at `/Users/solution/Development/rust/NodaAudio/audiocn`
(`components/ui/level-meter.tsx`, `lcd-meter.tsx`, `db-scale.tsx`, `db-readout.tsx`,
`clip-indicator.tsx`, `lib/audio/{types,decibels,zones,ballistics,taper}.ts`,
`hooks/use-clip-hold.ts`, `app/globals.css`, `components/ui/lcd-display.tsx` for the
LCD tokens, the `components/examples/*-{level-meter,lcd-meter,db-scale,db-readout,clip}*`
demos, and the tests `meter-parts.test.tsx`, `lcd-meter.test.tsx`, `level-meter.test.tsx`).

Global conventions:

- Tailwind v4 default scale, rem base = **16px**. `0.25rem`=4px, `0.5rem`=8px,
  `0.75rem`=12px, `1rem`=16px, `1.75rem`=28px, `w-px`=1px, `w-0.5`/`h-0.5`/`gap-0.5`=2px,
  `h-1`/`w-1`=4px, `h-1.5`/`w-1.5`=6px, `h-4`=16px, `h-5`=20px, `size-2`=8px,
  `min-h-32`=128px, `gap-1`=4px, `gap-2`=8px, `px-1`=4px, `py-0.5`=2px, `pb-3`=12px.
- Radii come from `--radius: 0.625rem` (globals.css `@theme inline`):
  `--radius-sm = calc(var(--radius)*0.6)` = 6px, `--radius-md = calc(var(--radius)*0.8)` = 8px.
  `rounded-full` = pill (radius = half the smaller side).
- `text-xs` = 12px, `text-[0.625rem]` = 10px, `text-[0.75em]` = 75% of parent font.
- Sign glyphs: minus is **U+2212 "−"**, infinity is **U+221E "∞"** (`decibels.ts`).
  NaN is printed as two ASCII hyphens `"--"` (comment: "hosts such as Videorc ban the
  em dash"). The dB unit suffix is exactly `" dB"` (leading space).
- Every position in a meter is a 0..1 float written to a CSS custom property with
  `toFixed(4)`; gradient stop offsets use `toFixed(3)`; bracket start `toFixed(4)`.

---

## 1. LevelMeter (`components/ui/level-meter.tsx`)

### 1.1 Purpose

A horizontal or vertical level meter (VU/peak bars) that paints itself from a frame
source or declarative dB values without re-rendering React per frame. It owns the
zone-gradient fill, peak-hold marker, RMS layering, scale/readout/clip satellites and
the ARIA meter semantics; `LcdMeter` composes it.

### 1.2 Props — `LevelMeter`

Type aliases: `Orientation = "horizontal" | "vertical"`,
`LevelMeterVariant = "solid" | "segmented" | "gradient"`,
`AudioSize = "sm" | "default" | "lg"`, `TaperInput = "linear" | "audio" | "log" | Taper`,
`BallisticsInput = "instant" | "peak" | "vu" | Partial<BallisticsOptions>`,
`MeterZone = { fromDb: number; zone: "ok" | "warn" | "clip" }`,
`ChannelLevel = { peakDb: number; rmsDb?: number }`, `MeterFrame = { channels: ChannelLevel[] }`,
`FrameSource<T> = { subscribe(listener: (frame: T) => void): () => void }` (`lib/audio/types.ts`).

| Prop | Type | Default | What it does |
|---|---|---|---|
| `source` | `FrameSource<MeterFrame> \| null` | — | Live frames; meter subscribes and self-paints. Wins over `peakDb`/`rmsDb`/`channels` (declarative is ignored while a source is set). |
| `peakDb` | `number` | — | Mono peak in dBFS, declarative. `-Infinity` = silence. |
| `rmsDb` | `number` | — | Mono RMS, declarative. |
| `channels` | `ChannelLevel[]` | — | Multi-channel declarative levels. |
| `channelCount` | `number` | 1 | Tracks to render before data arrives. Actual count = `declared` (from `channels`/`peakDb`) ?? observed from frames ?? `channelCount` prop ?? 1. |
| `minDb` | `number` | `DEFAULT_MIN_DB = -60` | Bottom of range; also the silence floor for ARIA text and `LevelMeterValue`. |
| `maxDb` | `number` | `DEFAULT_MAX_DB = 0` | Top of range (can be >0 for headroom). |
| `zones` | `MeterZone[]` | `DEFAULT_ZONES` = `[{fromDb:-Inf,zone:"ok"},{fromDb:-20,zone:"warn"},{fromDb:-9,zone:"clip"}]` | Colour zones; boundaries positioned by the taper. |
| `ballistics` | `BallisticsInput` | `"peak"` | Motion law (see §1.7). Also inherited from surrounding `AudioConfigProvider` (mixer/channel-strip) if not passed explicitly. |
| `taper` | `TaperInput` | `"linear"` | dB→position law (see §1.7). |
| `orientation` | `Orientation` | `"horizontal"` (cva default) | Bar direction; frames grow left→right or bottom→top. |
| `variant` | `"solid" \| "segmented" \| "gradient"` | `"solid"` | Fill style (see §1.4, §1.7). |
| `segments` | `number` | `DEFAULT_SEGMENTS = 24` | Segment count for `segmented`. |
| `size` | `"sm" \| "default" \| "lg"` | `"default"` | Bar thickness ladder (see §1.3). |
| `actionsRef` | `Ref<LevelMeterActions>` | — | Imperative `{ paint(frame), reset() }`; `reset` emits `{channels: []}` (drops level + hold to silence). |
| `children` | ReactNode | built-in default | If omitted: `channelCount` × (Channel→Track→Bar(peak)+Hold). |
| `className`, `style`, `ref`, all div props | | | Root is a `div role="meter"`; `style` merges after `--meter-fill`/`--meter-mask`. |

Custom colours: override `--meter-ok`, `--meter-warn`, `--meter-clip` on the root
(`level-meter-custom-colors.tsx` sets e.g.
`[--meter-clip:var(--foreground)] [--meter-ok:var(--muted-foreground)] [--meter-warn:var(--foreground)]`).

Sub-components (composed by default children; exported for custom layouts):

| Part | Props | Notes |
|---|---|---|
| `LevelMeterChannels` | div props | Row/column wrapper of channels. |
| `LevelMeterChannel` | div props + `index?: number` (default 0) | One channel; carries the live `--meter-level/--meter-rms/--meter-hold` vars and `data-zone`, `data-active`. |
| `LevelMeterTrack` | div props | The `bg-muted` pill; also renders the segmented ghost layer. |
| `LevelMeterBar` | div props + `measure?: "peak" \| "rms"` (default `"peak"`) | The reveal-clipped fill. Layer an rms bar and a peak bar (`opacity-40` on the peak bar) for a dual meter (`level-meter-dual.tsx`). |
| `LevelMeterHold` | div props (`className` lands on the inner marker) | 2px peak-hold marker. |
| `LevelMeterScale` | `DbScaleProps` minus `minDb/maxDb/taper/orientation` (inherited from context) | Printed dB scale; see §3. |
| `LevelMeterValue` | `DbReadoutProps` minus `source/value` | dB readout bound to the meter; default classes `text-muted-foreground text-xs`, `floorDb={minDb}`. Declarative levels render as static text; a stream gets the live ticker that falls to −∞ when it stops. |
| `LevelMeterClip` | `ClipIndicatorProps` minus `source` | Clip button bound to the meter frames; see §5. |

### 1.3 Geometry

| Thing | Value | Source |
|---|---|---|
| Root gap between channel block and trailing value/clip parts | 8px | `gap-2` |
| `--meter-gap` (gap between channels; scale offset) | 4px | arbitrary `[--meter-gap:0.25rem]` |
| `--meter-thickness` (track cross size) | sm **4px**, default **8px**, lg **12px** | `[0.25rem]` / `[0.5rem]` / `[0.75rem]` per size variant |
| Track radius | pill = thickness/2 (2/4/6px) | `rounded-full` + `overflow-hidden` |
| Horizontal root | `w-full flex-row items-center`; stretched by container along X; natural height = thickness (+20px when a `db-scale` is present: `has-[[data-slot=db-scale]]:pb-[calc(var(--meter-scale-size)+var(--meter-gap))]` = 16px + 4px) | |
| `--meter-scale-size` (horizontal) | 16px | `[--meter-scale-size:1rem]` |
| Vertical root | `min-h-32 flex-col items-center` → minimum height **128px**, stretched by container above that | |
| Channels wrapper | `flex min-h-0 min-w-0 flex-1 gap-(--meter-gap)`; horizontal→`flex-col`, vertical→`h-full flex-row` | bars stack top-to-bottom (horizontal) or left-to-right (vertical), 4px apart |
| Channel | horizontal `w-full`, vertical `h-full`; `min-h-0 min-w-0` | |
| Track | horizontal `h-(--meter-thickness) w-full`; vertical `h-full w-(--meter-thickness)` | the widget's only fixed dimension is thickness; length is always stretched |
| Hold marker | 2px thick: horizontal `inset-y-0 right-0 w-0.5` inside a full-track wrapper; vertical `inset-x-0 top-0 h-0.5` | `w-0.5`/`h-0.5` |
| Segment gap (segmented/gradient mask) | **2px transparent gap** per segment; segment length = trackLength/segments − 2px | `buildSegmentMask`: `repeating-linear-gradient(dir, black 0 calc(100%/N - 2px), transparent calc(100%/N - 2px) calc(100%/N))` |
| Scale (horizontal) | absolutely positioned `top-[calc(100%+var(--meter-gap))] left-0 h-(--meter-scale-size)` (16px tall, 4px below the bars, overlays the reserved padding-bottom) | |
| Scale (vertical) | in-flow inside `LevelMeterChannels` as a flex row sibling, 4px right of the last track | |
| Root | `[overflow-anchor:none]` (bars must not become scroll anchor) | |

Which is natural vs stretched: **thickness is natural (fixed by size), length fills the
container** in both orientations (hence `w-full` horizontal / `min-h-32` vertical).

### 1.4 Paint layers, back → front

The whole root is dimmed to **opacity 0.5** when `data-dimmed` is set
(`data-dimmed:opacity-50`; set when the surrounding mixer marks the channel muted/solo-dimmed).

Per channel, inside `LevelMeterTrack` (`className="bg-muted relative overflow-hidden rounded-full …"`):

1. **Track background** — pill rect, radius = thickness/2, fill `bg-muted` → `var(--muted)`.
2. **Unlit segment ghost** (only `variant="segmented"`) — `className="absolute inset-0 bg-(image:--meter-fill) mask-(--meter-mask) opacity-20"`, `aria-hidden`, rendered **before children** so it sits under the bars. Paint: the full-length zone gradient (`--meter-fill`) clipped by the 2px-gap segment mask (`--meter-mask`), at **20% opacity**, spanning the entire track. In egui: paint every segment rect with the zone colour at α=0.2.
3. **Bar(s)** (`LevelMeterBar`) — visible fill = **the same `--meter-fill` linear gradient, masked by `--meter-mask`** (mask is `"none"` for solid/gradient variants — literal string `none`), revealed only over the first `level` fraction of the track. Outer div `absolute inset-0 overflow-hidden` translated `(level−1)×100%` (horizontal) / `(1−level)×100%` (vertical, `translate-y`); inner div counter-translated so the gradient itself does not move. Net effect in paint terms: **clip the track-length gradient rect to `level × length`, anchored at the low end (left/bottom)**. The gradient is built once across the full track; a `measure="rms"` bar reads `--meter-rms` instead of `--meter-level`. In dual meters the peak bar is layered with `className="opacity-40"` (demo) under an opaque rms bar.
4. **Hold marker** (`LevelMeterHold`) — wrapper `pointer-events-none absolute inset-0 opacity-[calc(var(--meter-hold)*_50)]` (Tailwind: raw `opacity: calc(hold × 50)`; CSS clamps at 1, so the marker is invisible at hold=0 and **fully opaque from hold ≥ 0.02 onward**), translated like the bar outer so its inner edge lands at `hold × length`; the marker itself is `bg-foreground/80` → **`var(--foreground)` at 80% alpha**, 2px thick, full track cross size, flush at the position. It sits **on top of the bars** (last child).
5. **Track pill `overflow-hidden`** clips bars and marker to the rounded shape — that is what makes the 2px hold line look inset at the pill ends when hold ≈ 1.

`--meter-fill` gradient construction (`buildZoneFill`):

- Direction: horizontal `to right`, vertical `to top` (silence end first).
- Zones sorted ascending by `fromDb` (insertion sort `byFromDb`); boundary position =
  `taper.toPosition(zone.fromDb) × 100`, rounded `toFixed(3)`.
- **`solid` and `segmented`**: hard-edged stops — `var(--meter-ZONE) start% end%` where
  `start` = this zone's boundary, `end` = next zone's boundary (last zone runs to 100%).
  So each colour band occupies exactly its dB range on the current taper.
- **`gradient`**: soft stops — `var(--meter-ZONE) start%` per zone plus a final
  `var(--meter-lastZone) 100%`; colours blend between boundaries.
- Example (defaults, linear taper, −60..0): solid →
  `linear-gradient(to right, var(--meter-ok) 0% 66.667%, var(--meter-warn) 66.667% 85%, var(--meter-clip) 85% 100%)`
  (−20dB→0.6667, −9dB→0.85).

### 1.5 Colors

CSS variable → OKLCH verbatim from `app/globals.css` (`:root` = light, `.dark` = dark):

| Role in the widget | Class / var | Light (`:root`) | Dark (`.dark`) |
|---|---|---|---|
| Zone "ok" (green) | `--meter-ok` (used as `var(--meter-ok)` in the gradient) | `oklch(0.696 0.17 162.48)` | `oklch(0.765 0.177 163.223)` |
| Zone "warn" (amber) | `--meter-warn` | `oklch(0.795 0.184 86.047)` | `oklch(0.852 0.199 91.936)` |
| Zone "clip" (red) | `--meter-clip` = `var(--destructive)` | `oklch(0.577 0.245 27.325)` | `oklch(0.704 0.191 22.216)` |
| Track background | `bg-muted` → `--muted` | `oklch(0.97 0.001 106.424)` | `oklch(0.268 0.007 34.298)` |
| Hold marker | `bg-foreground/80` → `--foreground` @ 80% | `oklch(0.147 0.004 49.25)` | `oklch(0.985 0.001 106.423)` |
| Value text | `text-muted-foreground` → `--muted-foreground` | `oklch(0.5 0.013 58.071)` | `oklch(0.709 0.01 56.259)` |
| Foreground-safety variants (used by satellites, e.g. clip text, db-readout-zones demo) | `--meter-ok-foreground` = `color-mix(in oklch, var(--meter-ok), black 40%)`, same for warn/clip | those mixes | plain `var(--meter-ok)` / `var(--meter-warn)` / `var(--meter-clip)` (no mix in dark) |

Theme overrides exist (`[data-theme=ocean|rose|forest|violet|mono]`) that re-point
`--meter-ok`/`--meter-warn`/`--meter-clip` (e.g. mono: light ok `oklch(0.556 0 0)`,
warn `oklch(0.371 0 0)`, clip `oklch(0.145 0 0)`; dark ok `oklch(0.708 0 0)`, warn
`oklch(0.87 0 0)`, clip `oklch(0.985 0 0)`). Port the default light/dark pairs;
treat custom zones colours as user-settable inputs.

Note the zone gradient uses the **raw** `--meter-*` vars; the `--meter-*-foreground`
mixes (40% black, light only) are for text/lamps on top of the theme background.

### 1.6 States

| State | Trigger | Effect |
|---|---|---|
| Default | level 0 | empty `bg-muted` pill; hold marker opacity 0. |
| Idle/quiet | `peakDb ≤ minDb` (or −∞) | taper gives position 0 → no fill; ARIA text "−∞ dB"; `data-silent` on value readout. |
| Active | smoothed `level > 0` → `data-active` attribute toggled on the channel element (used by CSS consumers; no visual rule in the meter itself) | — |
| Zone change | `zoneForDb(smoothedPeak)` differs → `data-zone` written on each channel element and on the root (root = loudest channel) | visual zone colour comes from the gradient geometry, not the attribute |
| Clipping | any **input** (unsmoothed) `peakDb ≥ CLIP_THRESHOLD_DB = -1` → root gets `data-clipping` while `now < clipUntil` | no direct visual on the bars; `LevelMeterClip` reads its own `data-clipping` group (`group/level-meter`… actually via `group/clip-indicator`); mixer/strip CSS may dim. The lamp (§5) turns red. |
| Dimmed | `data-dimmed` on root | whole meter at `opacity: 0.5` |
| Hover / focus / disabled | **not present in source** — `LevelMeter` root is a plain div; disabled state comes from `AudioConfigProvider.disabled` only via surrounding components (not applied here) | — |
| Reduced motion | `prefers-reduced-motion: reduce` | ballistics forced to `"instant"`; repaints throttled to one frame per **250ms** (`REDUCED_MOTION_INTERVAL_MS`) |
| Off-screen | IntersectionObserver hidden | painter sleeps (returns no frames); wakes on re-entry |

### 1.7 Behaviour & maths

Constants (quote these in Rust):
`DEFAULT_MIN_DB = -60`, `DEFAULT_MAX_DB = 0`, `SILENCE_DB = -Infinity`,
`DEFAULT_SEGMENTS = 24`, `POSITION_EPSILON = 0.0005`, `SETTLE_EPSILON = 0.001`,
`ARIA_INTERVAL_MS = 250`, `REDUCED_MOTION_INTERVAL_MS = 250`,
`CLIP_THRESHOLD_DB = -1`, `CLIP_HOLD_MS = 1500`, `DEFAULT_ZONES` ok/-∞, warn/-20, clip/-9,
`MAX_FRAME_GAP_MS = 100` (painter clock clamp: dt per frame = `min(100, frameΔ)` ms,
so a woken-after-sleep meter never jumps).

**dB↔position** (`taper.toPosition`, output 0..1, written to `--meter-level`):

- `linear` (`linearTaper`): `pos = clamp((db − minDb)/(maxDb − minDb), 0, 1)`; anything
  `≤ minDb` (incl. −∞, NaN-safe via `!(value > min)`) → 0.
- `audio` (`audioTaper`): console law. `anchor = min(0, maxDb)`,
  `unity = maxDb > 0 ? 0.75 : 1` (**with the default meter max of 0 dB, unity=1**);
  above anchor: `pos = unity + (db−anchor)/(maxDb−anchor)·(1−unity)`;
  below: `ratio = (db−anchor)/(minDb−anchor)`, `pos = (1 − ratio^(1/2)) · unity`
  (exponent constant `AUDIO_CURVE_EXPONENT = 2`). Test-pinned: −15dB on −60..0 → 0.5
  ("a quarter of the range in dB is half the track"); −6dB → 0.9… (that one is linear);
  inverse `toValue`: `anchor + (minDb−anchor)·(1 − pos/unity)²` below unity.
- `log`: `pos = log(db/min)/log(max/min)` (not meaningful for negative dB; supported by type).

**Ballistics** (`lib/audio/ballistics.ts`) — smoothing runs on **linear amplitude**,
so a fall looks linear in dB (hardware-like):

- Presets `BALLISTICS` (ms): `peak {attack:15, release:350, peakHold:1200, peakRelease:600}`,
  `vu {300, 300, 0, 300}`, `instant {0,0,0,0}`. Partial objects merge over `peak`
  (`resolveBallistics`).
- `dbToGain(db) = 10^(db/20)`, −∞→0; `gainToDb(g) = 20·log10(g)`, ≤0→−∞.
- Step per frame, elapsed = clockΔ (clamped ≤100ms):
  `factor = 1 − exp(−elapsed/τ)`, τ = attack if rising else release;
  `amp += (target − amp) · factor`. τ≤0 → factor 1 (jump).
  First step after (re)creation jumps straight to the target (`lastMs === null`).
- Peak-hold: `peakHoldMs ≤ 0` disables hold (marker = level). Else marker tracks max:
  while `amp ≥ holdAmp`, `holdAmp = amp` and `holdUntil = now + peakHoldMs`; after
  `holdUntil`, `holdAmp` falls toward `amp` with time constant `peakReleaseMs` using
  elapsed from `max(lastFrame, holdUntil)` (hold stays pinned for the full hold window,
  then decays exponentially in amplitude).
- **Per channel there are two integrators**: one for `peakDb` (full ballistics incl. hold)
  and one for RMS which is `resolveBallistics(opts)` with **`peakHoldMs: 0`** forced.
  `rmsDb ?? peakDb` is the RMS input.
- Three positions written each frame: `--meter-level` = taper(peak.db),
  `--meter-rms` = taper(rms.db), `--meter-hold` = taper(peak.holdDb) — only if changed
  by > `POSITION_EPSILON` (0.0005), always `toFixed(4)`.

**Settling / frame economy**: a channel is `settled` when displayed level, rms and hold
are each within `SETTLE_EPSILON = 0.001` of the taper of the raw inputs (≈0.06 dB on
−60..0). The frame task keeps requesting frames while `!settled` or while a clip light
is counting down after the input dropped (`releasingClip = now < clipUntil && !inputClipping`);
otherwise it sleeps until a new frame/register/wake.

**Clip root logic**: scan all channels' **raw** `input.peakDb ≥ -1` → `clipUntil = now + 1500ms`
(refreshed on every frame that still clips); `data-clipping` on root iff `now < clipUntil`.
The lamp's own latch (§5) is a separate mechanism; the root attribute is what
`LevelMeter`'s CSS and satellites can key off.

**Zone segmentation of the bar**: zone boundaries are placed at taper positions (not dB
fractions), so under the `audio` taper the warn band (−20..−9dB) occupies
`(1−√(20/60))=0.423` … `(1−√(9/60))=0.612` of the track. The *bar colour at a pixel* is
purely the gradient under the reveal — the current zone only drives `data-zone`/ARIA.
`zoneForDb(db, zones)` = last zone in array order with `db ≥ fromDb` (defaults are
already ascending; `byFromDb` re-sorts for gradient stops only).

**ARIA**: root `role="meter"`, `aria-valuemin/aria-valuemax` = numbers,
initial `aria-valuenow = minDb`; each painted frame the loudest smoothed peak sets
`aria-valuenow = clamp(loudest, minDb, maxDb).toFixed(1)` and
`aria-valuetext = formatDb(loudest, { floorDb: minDb })` → e.g. `"−12.3 dB"`, `"−∞ dB"`.
Updates throttled to 250ms **unless settled** (a settled value is written immediately so
screen readers never see stale text; identical text+zone never rewrites).

**Declarative keying**: `channels` serialize as `peak:rms|peak:rms…`; mono as
`peakDb: rmsDb`. Level changes run through the same ballistics (a `peakDb` change
animates, it does not jump).

### 1.8 Interaction & a11y

- No pointer/keyboard handling on the meter itself (`pointer-events-none` on the hold
  wrapper; bars and ghosts are `aria-hidden`).
- Root exposes `role="meter"` + value/text attributes; parts are hidden from AT.
- `[overflow-anchor:none]` prevents scroll anchoring to animated bars.
- Reduced motion: instant ballistics + 250ms paint throttle (see §1.6).
- `data-*` hooks for consumers: `data-slot` on every part, `data-orientation`,
  `data-variant`, `data-size`, `data-zone`, `data-active`, `data-clipping`, `data-dimmed`,
  `data-measure` on bars, `data-index` on channels.

### 1.9 Gotchas

- The hold marker is **not a fixed-opacity line**: `opacity: calc(--meter-hold * 50)`
  (clamped to 1) makes it pop in above ~2% and stay solid; the visible marker is only
  2px (`w-0.5`/`h-0.5`) and `bg-foreground/80`, *not* the zone colours.
- Segmented = ghost + masked bar: the **same** gradient is painted twice — once full
  length at 20% opacity, once revealed at ~100% — both punched with the identical
  repeating mask, so an "unlit LED" always lines up with the lit ones. Gap between
  segments is a hard **2px**, segment period is `length/segments` (so gap eats into the
  black part: `black 0 (size−2px)`, transparent `(size−2px) size`).
- Bar reveal is implemented as two compensating translate-percents; in egui just clip
  the gradient rect to `level·length` from the leading edge; the gradient must span the
  **full track**, never the bar, or zone boundaries will slide.
- `solid` vs `gradient` differ only in the stop syntax (double-position hard stops vs
  single-position soft stops) — one gradient object per variant, same zone positions.
- Gradient stop offsets are rounded to **3 decimals of a percent**; CSS vars to **4
  decimals**. Reproduce to keep SSR-paint parity intent.
- Track is `rounded-full` **and** `overflow-hidden`; the fill is clipped by the pill, so
  at level=1 the green/amber/red bands end in a round cap.
- RMS bars don't get their own hold marker (hold is driven by peak ballistics only).
- `channelCount` from a live source is *observed*: the first frame's `channels.length`
  replaces the prop and re-renders tracks.
- A meter that has nothing to animate consumes **zero frames** — port as an explicit
  dirty/sleep flag, not a permanent timer.
- `LevelMeterValue` is `text-muted-foreground text-xs` (12px) with the DbReadout's mono
  tabular spacing; `LevelMeterClip` is the full §5 button.

---

## 2. LcdMeter (`components/ui/lcd-meter.tsx`)

### 2.1 Purpose

A recorder-style block-segment ladder drawn on an `LcdDisplay`'s dot-grid screen:
lit/unlit segments, held-peak segment, per-channel dim labels + bright readouts, a tick
scale with a clip-zone bracket and an inverting CLIP flag. Metering is delegated to
`LevelMeter`; geometry and colour come from the display's CSS variables. **It must be
rendered inside an `LcdDisplay`** (throws `"LcdMeter must be used inside LcdDisplay."`).

### 2.2 Props

`LcdMeterProps = Omit<LevelMeterProps, "orientation" | "segments" | "size" | "variant">`
plus a forced `taper = "audio"` default. So it takes: `source`, `peakDb`, `rmsDb`,
`channels`, `channelCount`, `minDb`, `maxDb`, `zones`, `ballistics`,
`taper` (`TaperInput`, default **`"audio"`**), `actionsRef`, `className`, `children`,
`ref`, and all div props. `orientation` is always `"horizontal"`; variant is always the
segmented LCD (no `variant`/`segments`/`size` — size follows the `LcdDisplay`).

Parts (all `data-slot="lcd-meter-*"`):

| Part | Props | Notes |
|---|---|---|
| `LcdMeterChannel` | `LevelMeterChannelProps` (`index`, default 0) | one grid row: `col-span-full grid grid-cols-subgrid items-center` |
| `LcdMeterLabel` | span props | dim channel name in column 1 |
| `LcdMeterTrack` | div props | the unlit segment bed, column 2 |
| `LcdMeterBar` | div props + `measure?: "peak" \| "rms"` (default `peak`) | lit segments |
| `LcdMeterHold` | div props | one lit segment at the held peak |
| `LcdMeterValue` | `DbReadoutProps` minus `source/value` (i.e. `measure`, `channel`, `intervalMs`, `holdMs`, `decimals`, `unit`, `floorDb`, `zones`, `format`, className…) with `unit` default **`false`** | bright digits, column 3; inside a channel row shows that channel, else the loudest ("max") |
| `LcdMeterScale` | `ticks?: number[]`, `format?: (db) => string`, `bracket?: boolean` (default `true`), div props | dim numbers + clip bracket, column 2 |
| `LcdMeterClip` | `ClipIndicatorProps` minus `source`, children default `"Clip"` | column 3 bottom-right |

Default children = per channel (label, track[bar+hold], value) then scale then clip.
Channel labels (`channelLabel`): count 2 → `"L"`, `"R"`; count > 2 → `"1"…"n"`; count 1 →
no label at all (test-pinned).

### 2.3 Geometry

All derived from the display's dot grid: **`--lcd-display-pitch: 2px`** (fixed in
`lcdDisplayVariants` base, regardless of display size; display size only changes font:
sm `text-sm` 14px, default `text-lg` 18px, lg `text-2xl` 24px).

| Token | Value (pitch = 2px) |
|---|---|
| `--lcd-meter-gap` | `var(--lcd-display-pitch)` = **2px** (segment gap) |
| `--lcd-meter-segment` | `calc(pitch*3)` = **6px** (segment width) — demos override: `[--lcd-meter-segment:var(--lcd-display-pitch)]` (2px "fine ladder"), `[--lcd-meter-gap:0px]` → solid bar |
| `--lcd-meter-period` | `segment + gap` = **8px** |
| `--lcd-meter-thickness` | sm `pitch*5`=**10px**, default `pitch*7`=**14px**, lg `pitch*10`=**20px** |
| `--lcd-meter-unlit` | `color-mix(in oklab, var(--lcd-display-lit) 14%, transparent)` |
| Grid | root `grid grid-cols-[auto_minmax(0,1fr)_auto]`, `gap-x: pitch*4` = **8px**, `gap-y: pitch*2` = **4px**, `pt: calc(0.75rem + nudge-y)` = **12px+**nudge, `pb-3` = **12px** |
| Width | track column is `1fr`: the ladder cuts itself to **whole segments**: `width = round(down, 100% − nudge-x + gap, period) − gap` (`ON_GRID`), with `margin-left: nudge-x` |
| Label | `min-w: round(up, 1.5em, pitch)` — a whole number of dots wide |
| Value | `min-w: calc(var(--db-readout-width) * 1.2)` — the ×1.2 compensates the screen's 0.12em letter-spacing so the readout never resizes (see §2.9) |
| Scale | flex column, `gap: pitch` = 2px; label line `h: round(up, 1em, pitch)`, font `text-[0.75em]`, tick **marks hidden** (labels only) |
| Bracket | container `h: calc(pitch*2)` = **4px**; borders `border-x border-t` each **2px** (`border-*-length:--lcd-display-pitch`), no bottom border |
| Clip flag | `rounded-sm` (6px), `px-1` (4px) `py-0.5` (2px), `text-[0.75em]`, `self-end justify-self-end`, `h-auto` |

The nudge (`--lcd-meter-nudge-x/y`, base `0px`) is computed at runtime by `useDotGrid`:
measure the first track's rect against the `LcdDisplay` content box; if an edge is a
fraction of a dot past the grid, shift the whole track column so **every segment edge
falls between two dots** (`GRID_EPSILON = 0.01px`; `toNextDot(d,p) = 0` when within ε of a
dot else `p − d mod p`). Re-runs on ResizeObserver of the content and of every label.
In egui you'd replicate this as: round the ladder origin to the dot grid.

### 2.4 Paint layers, back → front (per channel row)

Context from the display (`lcd-display.tsx`): content sits in a wrapper with
`drop-shadow-[0_0_3px_var(--lcd-display-glow)]` and, when the screen's dot matrix is on
(default), a dot mask `mask-[radial-gradient(circle,#000_58%,transparent_64%)]` at
`mask-size: pitch pitch` — so **all ink below is broken into round dots on the 2px grid,
with a 3px soft glow of `--lcd-display-glow` behind it**.

1. **Unlit segment bed** (`LcdMeterTrack > [data-slot=lcd-meter-segments]`) —
   `className="absolute inset-0 bg-(--lcd-meter-unlit)"` +
   `SEGMENT_MASK = "mask-[repeating-linear-gradient(to_right,#000_0_var(--lcd-meter-segment),transparent_var(--lcd-meter-segment)_var(--lcd-meter-period))]"`.
   Paint: each 8px period, a 6px-wide rect (thickness 10/14/20px) in
   `lit @ 14% alpha` (oklab mix), repeating. This is the "ghost LED" row. When the
   display has `matrix={false}`, the track adds `DOT_MASK` itself so it still reads as
   dots (test-pinned).
2. **Lit bar** (`LcdMeterBar`) — `className="absolute inset-0 bg-(--lcd-display-lit)
   [clip-path:inset(0_calc(100%_-_round(up,_var(--lcd-meter-bar-level)_*_100%,_var(--lcd-meter-period)))_0_0)]"`,
   `--lcd-meter-bar-level` = `var(--meter-level)` (peak) or `var(--meter-rms)`.
   Paint: **quantise the level up to whole segments**: lit segment count =
   `ceil(levelLength / period)` — a segment lights **whole or not at all** (comment in
   source); remaining width = `100% − round(up, level×100%, period)` clipped away from
   the right. Left-anchored, always at full lit opacity (no 20% ghost — brightness
   contrast comes from the 14% bed instead).
3. **Hold segment** (`LcdMeterHold`) — `className="absolute inset-y-0
   left-[calc(round(up,_var(--meter-hold)_*_100%,_var(--lcd-meter-period))_-_var(--lcd-meter-period))]
   w-(--lcd-meter-segment) bg-(--lcd-display-lit) opacity-[calc(var(--meter-hold)_*_1000)]"`.
   Paint: exactly **one segment of full lit colour**, placed in the segment slot that
   contains the hold position (round the hold up to a period boundary, step back one
   period); `opacity = clamp(hold × 1000, 0, 1)` → hidden at silence, solid from hold
   ≥ 0.1%. It overdraws one bed segment.
4. **Label** — text in `var(--lcd-display-ink-dim)` (lit @ 40% alpha).
5. **Value readout** — DbReadout in `var(--lcd-display-lit)`, font
   `font-(family-name:--lcd-display-font)` (= `var(--font-mono)` default), `unit=false`.
6. **Scale row** — bracket then tick labels, both in `var(--lcd-display-ink-dim)`;
   bracket = a `⊓` shape (top + left + right 2px strokes, open bottom) starting at the
   first segment of the clip zone.
7. **CLIP flag** — text in `var(--lcd-meter-unlit)` (14% lit — a ghost label); while
   clipping: background `var(--lcd-display-lit)`, text `var(--lcd-display-glass)`
   (an LCD-menu **inversion**, not a colour change).

### 2.5 Colors

| Role | Var | Value |
|---|---|---|
| Ink source | `--lcd-display-ink` | variant: `primary` = `var(--primary)` (light `oklch(0.216 0.006 56.043)` / dark `oklch(0.923 0.003 48.717)`); `amber` `oklch(0.76 0.13 52)`; `cyan` `oklch(0.84 0.12 205)`; `green` `oklch(0.85 0.19 142)`; `red` `oklch(0.68 0.2 27)` |
| Lit segment | `--lcd-display-lit` | dark screen (`appearance=dark`, or auto-in-`.dark`): `var(--lcd-display-ink)`. light backlit screen: `color-mix(in oklab, var(--lcd-display-ink), black 62%)` (dark pixels on pale glass) |
| Unlit segment | `--lcd-meter-unlit` | `color-mix(in oklab, var(--lcd-display-lit) 14%, transparent)` |
| Dim ink (labels, bracket) | `--lcd-display-ink-dim` | `color-mix(in oklab, var(--lcd-display-lit) 40%, transparent)` |
| Clip-flag inverted text / glass | `--lcd-display-glass` | light: `oklch(from var(--lcd-display-ink) 0.93 calc(c*0.35) h)` (near-white tinted); dark: `oklch(0.15 0.004 200)` |
| Glow (from the display wrapper) | `--lcd-display-glow` | light `color-mix(in oklab, lit 22%, transparent)`; dark `color-mix(in oklab, lit 60%, transparent)`, blurred `drop-shadow 0 0 3px` |
| Unlit dots behind screen (display) | `--lcd-display-ink-unlit` | `lit @ 7%` |

Everything the meter paints is derived from **one** ink variable; a port needs only
`lit`, `unlit=14%`, `dim=40%`, `glass`, `glow=22%/60%` mixes.

### 2.6 States

| State | Effect |
|---|---|
| Silence (level −∞) | 0 lit segments (bed only), hold segment opacity 0, readout prints `−∞` (no unit). |
| Moving level | segments quantise whole; ballistics identical to §1.7 (inherited from `LevelMeter`). |
| Peak hold | one full segment at the held position until `peakHoldMs` (peak preset 1200ms) then decays back to the bar; never below the bar. |
| Clipping (root `data-clipping`, i.e. raw input ≥ −1dBFS held 1500ms) | CLIP flag inverts to lit-on-glass; flag's own `ClipIndicator` state (its `data-clipping`) drives the inversion — inside `LevelMeter`, `LevelMeterClip` subscribes to the same frame source. |
| `matrix=false` display | track masks its own dots (radial dot mask added) — appearance parity kept by design (test). |
| Focus-visible (clip flag) | `focus-visible:ring-1 ring-(--lcd-display-lit)` (1px, ink-coloured) — replaces the default 3px ring via `transition-none`, `hover:bg-transparent`. |
| Reduced motion | inherited `LevelMeter` instant+throttle; clip flag explicitly `transition-none` so it never fades. |
| Disabled / hover | hover on the flag = `hover:bg-transparent` (cancels the base button's `hover:bg-muted`); otherwise **not present in source**. |

### 2.7 Behaviour & maths

- All metering maths = §1.7 unchanged (it *is* a `LevelMeter`); the LCD differences are:
  - **Default taper `"audio"`** → with default −60..0 range, `unity = 1`,
    `pos = 1 − √(|db|/60)`; "the loud end of the range gets most of the track".
  - **Segment quantisation**: period 8px (segment 6 + gap 2). Lit count =
    `round(up, level × trackLen, period) / period` (CSS `round(up, …)` = ceiling to the
    grid). Hold slot = `round(up, hold × trackLen, period) − period`.
  - **Bracket start** = `taper.toPosition(min clip fromDb)` `toFixed(4)`, then snapped
    `left: round(down, start × 100%, period)` ("starts where the first segment of the
    zone does"). Hidden if no `clip` zone or `clipFromDb ≥ maxDb` (test: single ok-zone
    array → no bracket; linear −60..0 with default zones → `--lcd-meter-bracket-start = "0.8500"`).
  - **Scale ticks**: `DEFAULT_TICKS = [6, 3, 0, -3, -6, -12, -24, -48]` filtered to
    `[minDb, maxDb]` — note the *as-written* order matters for rank tie-breaking but the
    filter keeps positive ticks only if `maxDb > 0`. Printed via `printMagnitude(db) =
    formatDb(db, { decimals: 0, sign: db > 0 ? "auto" : "never", unit: false })` →
    `"0" "3" "6" "12" "24" "48"`, and `"+6" "+3"` above unity (test-pinned). Magnitudes
    only: the screen omits minus signs; positive headroom keeps a plus.
  - **Readout**: `unit=false`, `floorDb = minDb` (inherited), 250ms cadence; each row's
    readout defaults to that row's channel via `ChannelContext`.
- Root ARIA/clip/hold/settle logic inherited verbatim (§1.7).

### 2.8 Interaction & a11y

- Root is the `role="meter"` div from `LevelMeter` (`aria-label` supplied by caller, e.g.
  `"Input level"`; `data-slot="lcd-meter"`).
- Bars/hold/bracket are `aria-hidden`; labels and readouts are plain text.
- The only interactive element is the CLIP flag button (§5).
- Keyboard/pointer/scroll: not present in source beyond the flag button.
- Reduced motion: inherited instant ballistics; display-level flicker/scanline
  animations are `motion-reduce:*` guarded in `lcd-display.tsx`.

### 2.9 Gotchas

- **Segment edges must land between dots** — hence nudge + `round(down … − gap)` width
  + `round(up)` quantisation everywhere. Without this, half-lit dots appear and it stops
  reading as hardware.
- The readout width factor **×1.2**: display tracking is `0.12em` on a `0.6em`-wide mono
  digit → each char is 1.2ch wide; `min-w: calc(--db-readout-width * 1.2)` keeps the
  column dead-still while digits change.
- Lit segments have **no gradient and no per-zone colour** — the ladder is monochrome
  ink; the only "colour" decision is light- vs dark-screen ink.
- Hold is drawn as a **whole segment**, not a 2px line like `LevelMeter`; and it can sit
  **inside** the lit run (no special casing — it just overdots the bed).
- CLIP text is the 14% ghost of the ink when idle — easy to miss; on clip it *inverts*
  (lit background, glass text), it never turns red.
- The scale prints **no minus signs** ("0 3 6 12 24 48") — magnitudes; only above-unity
  ticks get a `+`. Tick marks (the little lines) are hidden:
  `[&_[data-slot=db-scale-mark]]:hidden`, labels only, at 0.75em, colour `ink-dim`.
- Bracket is open at the bottom (top+x borders only) and exactly 2 dots tall.
- Gap override `--lcd-meter-gap:0px` collapses segments into a solid bar — the "LED
  ladder vs solid" distinction is one variable.
- Whole thing inherits display `size` (thickness ladder) — you cannot set `size` on
  `LcdMeter` directly (omitted from props).

---

## 3. DbScale (`components/ui/db-scale.tsx`)

### 3.1 Purpose

A ruler of dB tick marks and labels mapping a dB range through a taper, horizontal under
a meter/fader or vertical beside one; automatically thins labels that would collide.

### 3.2 Props — `DbScale`

| Prop | Type | Default | What it does |
|---|---|---|---|
| `minDb` | `number` | −60 (or `AudioConfig` min) | Bottom of range |
| `maxDb` | `number` | 0 (or config) | Top |
| `ticks` | `number[]` | `DEFAULT_TICKS` filtered to range: `[12, 6, 0, -6, -12, -18, -24, -36, -48, -60, -72, -90]` keep `minDb ≤ t ≤ maxDb` | Which dB values get a tick |
| `taper` | `TaperInput` | `"linear"` | Position law (§1.7) so ticks line up with the meter/fader using the same range+taper |
| `orientation` | `Orientation` | `"horizontal"` (config fallback) | ruler direction |
| `side` | `"start" \| "end"` | `"end"` | Which side of the marks labels sit on; `"start"` reverses the tick flex order |
| `labels` | `boolean` | `true` | Show/hide all labels |
| `format` | `(db) => string` | `defaultFormat = formatDb(db, { decimals: 0, unit: false })` → `"0"`, `"−12"` (U+2212) | Label text |
| `children` | ReactNode | mapped `DbScaleTick`s | Custom ticks |
| className/style/ref + div props | | | |

`DbScaleTick` props: `value: number` (required), `major?: boolean` (**default true**,
major marks are longer), children replace `format(value)` (demos: `Unity`, `−∞`).

### 3.3 Geometry

| Thing | Value | Source |
|---|---|---|
| Horizontal root | `h-4 w-full` = **16px** tall, stretches to container width | |
| Vertical root | `h-full w-7` = **28px** wide, stretches to container height | |
| Font | `text-[0.625rem]` = **10px**, `leading-none`, `tabular-nums`, `relative shrink-0 select-none` | |
| Tick position | `--tick-position: position × 100 %`; horizontal `left`, vertical `bottom` (0 dB end is top; position 0 at the bottom because vertical grows up) | |
| Tick container | `absolute flex items-center gap-0.5` (**2px** gap mark↔label); horizontal `top-0 h-full flex-col` (mark on top, label below — reversed for `side="start"`); vertical `left-0 w-full flex-row` (mark left, label right, `flex-row-reverse` for start) | |
| Mark (horizontal) | major `h-1.5 w-px` = **6px×1px**; minor `h-1 w-px` = **4px×1px** | |
| Mark (vertical) | major `h-px w-1.5` = 1px×6px; minor `h-px w-1` = 1px×4px | |
| Label edge alignment | within `EDGE = 0.02` of the ends, labels clamp inside: horizontal start `translate-x-0`, end `-translate-x-full`; vertical start `translate-y-0`, end `translate-y-full`; otherwise centred (`-translate-x-1/2` / `translate-y-1/2`) | `alignClass` |
| Vertical label box | `flex-1 text-end` — right-aligned so trailing digits line up | |

### 3.4 Paint layers, back → front

No background layer of its own (transparent). Per tick:

1. **Mark** — 1px-wide rect (`w-px`/`h-px`), length 6px major / 4px minor,
   `bg-border` → `var(--border)`.
2. **Label** — 10px text, root colour `text-muted-foreground`; hidden labels use
   `data-hidden:invisible` (`visibility:hidden` — **keeps its box**, so thinning doesn't
   shift the layout). In the LCD variant both colours are overridden by the parent class.

### 3.5 Colors

| Role | Class → var | Light | Dark |
|---|---|---|---|
| Tick marks | `bg-border` → `--border` | `oklch(0.923 0.003 48.717)` | `oklch(1 0 0 / 10%)` (white @10% alpha) |
| Labels | `text-muted-foreground` → `--muted-foreground` | `oklch(0.5 0.013 58.071)` | `oklch(0.709 0.01 56.259)` |

### 3.6 States

- Default: ticks + labels as above. It is `aria-hidden`, non-interactive; no hover/focus/disabled (**not present in source**).
- Thinned state: label `data-hidden` → invisible but still occupies space.
- Reduced motion: not present in source (static element).

### 3.7 Behaviour & maths

- Tick position = `taper.toPosition(value)` (§1.7) — a `maxDb=6, minDb=-48, taper="audio"`
  demo shows the non-linearity (0dB at `1 − … unity=0.75` → exactly 0.75).
- **Label thinning** (`thinDbScaleLabels`, exported and test-pinned):
  1. Collect all `[data-slot=db-scale-label]`s, rank 3 each.
  2. Re-rank: `value === 0` → rank **0**; `value === max(ticks)` or `min(ticks)` (the
     end values, not scale ends) → rank **1**; `value % MAJOR_STEP === 0` (12) → rank **2**.
  3. Sort by rank asc, then value desc; greedily keep labels whose measured span
     (`left..right`, or `top..bottom` vertical) doesn't collide with any kept span within
     `LABEL_GAP = 3px` (`start < keptEnd + 3 && end > keptStart − 3` → hidden).
  4. Unlaid-out (0×0) labels are skipped.
- Re-run triggers: layout effect on mount + every render's effect deps `[labels]`,
  `ResizeObserver` on the scale, `MutationObserver` (style/childList/characterData,
  subtree) — note style-only filter avoids feedback from `data-hidden`, and an extra
  pass after `document.fonts.ready`.
- Test-pinned defaults: `maxDb=0/minDb=-24` → labels `["0","−6","−12","−18","−24"]`;
  ticks `[0,-6,-12,-54,-60]` with -6/-54 boxes colliding 0/-60 → hidden `["−6","−54"]`.

### 3.8 Interaction & a11y

- `aria-hidden` root, `select-none` text, no pointer handling, no keyboard handling —
  purely visual; the owning meter carries the semantics.

### 3.9 Gotchas

- The greedy thinning order (0 dB → ends → multiples of 12 → rest) is what keeps a
  crowded 60px scale legible in a mixer strip; port it exactly, including the
  same-rank sort by **descending value**.
- End labels are pulled inside via translate-x-full/0 rather than changing `left` —
  ticks stay on their true positions.
- Marks are always 1px (`w-px`) — crisp hairline; horizontal marks attach at the **top**
  edge of the 16px band and labels hang below (`flex-col`, mark first).
- `leading-none` + 10px + `tabular-nums`: the whole horizontal scale is exactly 16px,
  matching `--meter-scale-size: 1rem` reserved by the meter.
- Vertical scales place ticks with `bottom:` so position 0 (minDb) is at the bottom.
- `min-w` is not set; the scale never reserves space for labels — the parent reserves it
  (meter's pb, fader's gutter).

---

## 4. DbReadout (`components/ui/db-readout.tsx`)

### 4.1 Purpose

A monospace, right-aligned, width-stable dB number ("−12.3 dB", "−∞ dB") that ticks from
a frame source at a fixed cadence (default 4Hz) without React re-renders, optionally
holding the windowed peak, and stamps zone/silence data attributes.

### 4.2 Props

`DbReadoutProps extends Omit<ComponentProps<"span">, "children">`:

| Prop | Type | Default | What it does |
|---|---|---|---|
| `value` | `number` | — | Declarative level (dBFS). Mutually exclusive with `source`. |
| `source` | `FrameSource<MeterFrame> \| null` | — | Live source. |
| `measure` | `"peak" \| "rms"` | `"peak"` | Which level to show; `rmsDb ?? peakDb` fallback for rms. |
| `channel` | `number \| "max"` | `"max"` | Channel index, or loudest across all. |
| `intervalMs` | `number` | `DEFAULT_INTERVAL_MS = 250` | Text update cadence. |
| `holdMs` | `number` | `0` | Show highest value seen within the window; 0 = no hold. |
| `decimals` | `number` | `1` | Digits after decimal point. |
| `unit` | `boolean` | `true` | Append `" dB"`. |
| `floorDb` | `number` | `DEFAULT_FLOOR_DB = -60` | At/below this → print `−∞`. `LevelMeterValue` passes `minDb`. |
| `zones` | `MeterZone[]` | `DEFAULT_ZONES` | For `data-zone` only (no colouring by default). |
| `format` | `(db) => string` | — | Replaces all formatting. |
| className/style/ref + span props | | | demos add e.g. `data-[zone=clip]:text-meter-clip-foreground data-[zone=warn]:text-meter-warn-foreground data-silent:text-muted-foreground text-2xl font-semibold` (`db-readout-zones.tsx`). |

Helper: `readChannel(frame, measure, channel)` — missing channel = `SILENCE_DB`; "max"
loops all channels.

### 4.3 Geometry

| Thing | Value | Source |
|---|---|---|
| Element | `inline-block`, right-aligned (`text-end`), `font-mono`, `tabular-nums` | |
| Width floor | `min-w: var(--db-readout-width)` where the var = `widest × 1ch`; `widest = max length of render()` over four probe values: `−∞`, `floorDb + 10^−decimals` (just above floor, e.g. −59.9), `−88.8` and `+88.8` (`WIDEST_MAGNITUDE_DB = 88.8`, "two-digit levels on either side of 0") | so the span never changes width mid-show |
| Font size | inherited (demo/LCD wrap at their own sizes: `text-xs` in `LevelMeterValue`; `text-(--lcd-display-lit)`+LCD font in `LcdMeterValue`) | |
| Text | e.g. `"−12.3 dB"` = 8 chars; `−∞ dB` path | |

### 4.4 Paint layers

None painted directly — pure text: `text-muted-foreground`-ish colour from context
(`LevelMeterValue` adds `className="text-muted-foreground text-xs"`); the LCD variant
paints in `var(--lcd-display-lit)` and inherits the display's glow dot-mask. No borders,
no background ("not present in source").

### 4.5 Colors

Default: inherits parent text colour. Documented demo hooks:
`--meter-ok/warn/clip-foreground` (light = `color-mix(in oklch, <zone>, black 40%)`;
dark = the raw zone colours, §1.5) and `--muted-foreground` for silence.

### 4.6 States

| State | Trigger | Effect |
|---|---|---|
| Silent | `db ≤ floorDb` or NaN-initial silence | text `−∞ dB` (or `−∞` with `unit:false`); attribute `data-silent` present |
| Unknown/NaN | `Number.isNaN(db)` | text `"-- dB"` (`formatDb`) |
| Zone stamp | every text write | `data-zone = zoneForDb(db)` ∈ ok/warn/clip (consumer colouring) |
| Hold | `holdMs > 0` | shows window max until a new frame exceeds it or the window expires (`now − peakAt > holdMs` → rebase) |
| Quiet source | frames stop | with `holdMs === 0` peak resets to −∞ each tick → falls to `−∞ dB`; ticker stops when nothing changed and no fresh frame arrived (costs no timer) |
| value↔source switch | prop change | React `key={"source"|"value"}` swaps the span so stale ticker-written text never survives |
| Disabled / hover / focus | not present in source | |

### 4.7 Behaviour & maths

- Cadence: `createTicker` = `setInterval(intervalMs)`; `step()` returns whether there is
  still work (`changed || fresh`); a new frame calls `wake()`.
- `render(db) = format ? format(db) : formatDb(db, { decimals, floorDb, unit })`.
- `formatDb` contract (§1.7 constants; verbatim algorithm):
  1. NaN → `"--" + suffix`;
  2. `db === −∞ || db ≤ floorDb` → `"−∞"+suffix` (sign `never` drops the −);
  3. `rounded = Number(db.toFixed(decimals))`, `magnitude = abs.toFixed(decimals)`;
  4. `rounded === 0` or sign `never` → bare magnitude (so −0.04 @1dec prints `"0.0 dB"`,
     not `−0.0`); negative → `"−"+magnitude`; positive → `"+"+magnitude` if sign auto.
  Default options: `decimals 1, unit true, sign "auto", floor −∞`.
- Hold logic per frame: `if (db >= peak || now − peakAt > holdMs) { peak = db; peakAt = now }`.
- DOM writes: replaces the first text node only (no re-render); `data-zone`/`data-silent`
  only when the text changed.
- Initial render uses `value ?? SILENCE_DB` (a source starts at `−∞ dB`).

### 4.8 Interaction & a11y

- No pointer/keyboard handling; plain `<span>`; text changes are **not** announced
  (no `aria-live` — callers add sr-only live text if wanted; the *clip* component owns
  the live region). Test-pinned: `value={-6}` → `"−6.0 dB"`; after 260ms from emit −12.34
  → `"−12.3 dB"`, `data-zone="warn"`; switching to `value={-∞}` → `"−∞ dB"`,
  `data-zone="ok"`, `data-silent`.

### 4.9 Gotchas

- The `min-w` in `ch` + `tabular-nums` + right-align means digits march in place and the
  left edge grows/shrinks only with the sign — set both or the layout jitters.
- Probe-value width sizing must include **just above the floor** (`floorDb + 10^−decimals`)
  or the widest number isn't what you measure.
- Negative values that round to zero print unsigned `"0.0"` — intentional, avoids "−0.0 dB".
- `−∞` uses U+2212+U+221E, NaN uses ASCII `--`. Don't swap them.
- `LevelMeterValue` passes `floorDb = minDb`: below the meter's own floor the readout and
  the empty bar agree.
- The ticker self-parks: silence costs no interval; the next frame wakes it.

---

## 5. ClipIndicator (`components/ui/clip-indicator.tsx`)

### 5.1 Purpose

A small round clip lamp button: lights (and counts) whenever any fed level reaches the
threshold, holds for `holdMs` (latching with `Infinity`), and is clicked to reset.

### 5.2 Props

Base UI `useRender` component — renders a `<button type="button">` by default, or a
custom element via `render` prop (asChild-style). `ClipIndicatorProps extends
useRender.ComponentProps<"button">`:

| Prop | Type | Default | What it does |
|---|---|---|---|
| `clipping` | `boolean` | — | Controlled; when set, **no detection** happens internally |
| `source` | `FrameSource<MeterFrame> \| null` | — | Auto-detect from loudest peak across channels |
| `thresholdDb` | `number` | `CLIP_THRESHOLD_DB = -1` | `db ≥ threshold` = a clip |
| `holdMs` | `number` | `CLIP_HOLD_MS = 1500`; `Infinity` = latch until reset | How long the light stays on after the last clip |
| `onClippingChange` | `(clipping: boolean) => void` | — | Edge callback |
| `showCount` | `boolean` | `false` | Print the clip count next to the light |
| `actionsRef` | `Ref<ClipIndicatorActions>` | — | `{ report(db), reset() }` |
| `render` | Base UI render prop | — | Swap element (e.g. span in `LcdMeterClip`) |
| `children` | ReactNode | the dot | Replaces the default lamp (demo uses `"Clip"` text) |
| className + button props | | | merged last (className wins) |

### 5.3 Geometry

| Thing | Value | Source |
|---|---|---|
| Button | height **20px** (`h-5`), `inline-flex items-center justify-center`, `gap-1` = 4px (dot↔count/text), `px-1` = 4px, `rounded-full`, `text-xs` (12px), `font-medium` | |
| Lamp dot | **8px × 8px** circle (`size-2 rounded-full shrink-0`) | |
| Hit area | invisible `after:-inset-1` (+4px each side → ≥28px target); coarse pointers `pointer-coarse:after:-inset-2.5` (+10px → ≥40px) | |
| Count text | `tabular-nums` | |
| sr-only live text | clip-rect hidden, `aria-live="polite"`, content `"Clipping"` or `""` | |
| LCD variant (`LcdMeterClip`) overrides | `h-auto self-end justify-self-end rounded-sm px-1 py-0.5 text-[0.75em] font-normal` + uppercase child text "Clip" | |

### 5.4 Paint layers, back → front

1. **Button backdrop** — transparent at rest (`hover:bg-muted` only), pill radius; no
   border, no gradient.
2. **Lamp dot** — solid circle: off = `bg-muted-foreground/30` (**--muted-foreground at
   30% alpha**), on = `group-data-clipping/clip-indicator:bg-meter-clip`
   (`--meter-clip` = destructive), `transition-colors`.
3. **Count / children text** — inherits button text colour: off
   `text-muted-foreground`, on `data-clipping:text-meter-clip-foreground` (the black-40%
   mix in light theme).
4. Focus ring (see states).
The dot and its colour switch are the whole visual; there is no glow/blur
(**not present in source** — unlike the LCD flag, which glows via the display).

### 5.5 Colors

| Role | Class → value | Light | Dark |
|---|---|---|---|
| Lamp off | `bg-muted-foreground/30` | `oklch(0.5 0.013 58.071)` @30% | `oklch(0.709 0.01 56.259)` @30% |
| Lamp on | `bg-meter-clip` → `--meter-clip`=`--destructive` | `oklch(0.577 0.245 27.325)` | `oklch(0.704 0.191 22.216)` |
| Text off | `text-muted-foreground` | `oklch(0.5 0.013 58.071)` | `oklch(0.709 0.01 56.259)` |
| Text on | `text-meter-clip-foreground` → `--meter-clip-foreground` | `color-mix(in oklch, var(--meter-clip), black 40%)` | `var(--meter-clip)` (no mix) |
| Hover bg | `bg-muted` | `oklch(0.97 0.001 106.424)` | `oklch(0.268 0.007 34.298)` |
| Focus ring | `ring-ring/30` → `--ring` @30% | `oklch(0.709 0.01 56.259)` @30% | `oklch(0.553 0.013 58.071)` @30% |
| LCD flag | §2.4/§2.5 lit/glass/unlit/ink vars | | |

### 5.6 States

| State | Effect |
|---|---|
| Default (quiet) | dim 30% dot, muted text; `data-clipping` absent |
| Clipping | dot → destructive, text → clip-foreground, `data-clipping` attr set, `aria-label` → `"Clipping. Reset clip indicator"`, sr-only `"Clipping"` announced (polite) |
| Latched | `holdMs = Infinity`: stays on until `reset()` (click or `actionsRef.reset`) |
| Counting down | after the last clip: `setTimeout(holdMs)` → off; repeated reports re-arm it; a level that drops below threshold does *not* cut it early (the 1500ms always runs from the **last** above-threshold report) |
| Hover | `hover:bg-muted` (LCD variant suppresses: `hover:bg-transparent`) |
| Focus-visible | `focus-visible:ring-3` (3px) `ring-ring/30`, `outline-none`; **ring offset: not present in source** (Tailwind v4 default no offset; LCD: `ring-1` in `--lcd-display-lit`) |
| Pressed/active | not present in source |
| Disabled | button default (`disabled:` styles not present in source) |
| Reduced motion | dot has `transition-colors` (≈150ms default); no explicit `motion-reduce` rule here — the LCD flag uses `transition-none`. |

### 5.7 Behaviour & maths (`hooks/use-clip-hold.ts`)

- `report(db)`: `above = db >= thresholdDb`. Rising edge only (`!wasAbove`) increments
  `count`. While above, every report sets `clipping = true` and **reschedules** release.
  Below threshold: does nothing (hold timer governs turn-off).
- Release: `setTimeout(update(false), holdMs)` unless `holdMs` is infinite.
  `reset()`: clear timer, `above = false`, `count = 0`, `clipping = false`.
- With a `source`, each frame feeds `loudestPeak(frame) = max(peakDb)` across channels
  (raw, un-ballistics'd values — clipping reacts instantly, unlike the bars).
- `LevelMeter` root runs the same pair of constants independently
  (`CLIP_THRESHOLD_DB`/`CLIP_HOLD_MS`, §1.7 root logic).
- Edge-callback only fires on state flips (`clippingRef` guard).
- Hidden-tab/"Activity" re-show re-arms a pending release (effect comment-pinned detail;
  a Rust port with a monotonic clock just needs "hold expiry is time-based, not
  frame-based").

### 5.8 Interaction & a11y

- Real `<button type="button">`: click, Enter and Space all reset the lamp and count.
- `aria-label` doubles as description + action hint (switches on clip).
- `aria-live="polite"` sr-only span announces "Clipping" at the moment it latches.
- Pointer-target grown 4px (10px coarse) via `::after`.
- Base UI `useRender` merges caller props last → consumer overrides always win;
  `state = { clipping, slot }` handed to custom renders.
- Test-pinned: report(0) → `data-clipping` + count "1"; click → off + "0";
  `holdMs=500` report(-0.5) → on at 10ms, off by 610ms; controlled `clipping` follows prop.

### 5.9 Gotchas

- Counting is **edge-triggered** (per excursion above threshold), not per frame — a
  sustained 0dBFS buzz is one clip, refreshed light, count 1.
- The lamp colour and the text colour come from *different* tokens (`--meter-clip` vs
  `--meter-clip-foreground` = clip darkened 40% in light theme only) — one red for the
  dot, a darker red for text, both switching together via the group selector
  `group/clip-indicator` + `group-data-clipping/...`.
- Threshold is −1dBFS, not 0 — digital clips are flagged *before* full scale.
- Off-state dot is not grey at full strength: it's `muted-foreground` **at 30%**, so it
  nearly disappears on a muted track — intentionally "unlit".
- `showCount` prints a bare number in the same pill; keep `tabular-nums` or the button
  width ticks with the count.
- In `LcdMeter` the identical semantics get a completely different skin (ghost label
  that inverts); port the state machine once and re-skin it.
