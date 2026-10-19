# Amalfi motion

**Rule (user decision 2026-10-16):** every animation uses the curve and timing **measured on the reference
desktop**, identical: beziers, springs, durations, delays, overshoots and frame-by-frame behaviour from
`docs/spec/*.md` and the reference notes are binding. Amalfi changes only the **geometry**: distances,
sizes and overshoot amplitudes in points scale to the Amalfi measures; curves and times do not change.

The tokens below apply **only** to animations that were never measured (list at the end); mark each use
in code as an assumption, and replace it as soon as a measurement exists.

Faro and the Launcher are **one surface**: opening the Launcher morphs the Faro bar fluidly into the
Launcher container (and back on Esc), and switching grid ↔ list is a morph of the same container, not a
snap. Use the measured Launcher open/Esc curves for the container geometry.

## Tokens

Springs are given as response (s) and damping ratio ζ, and as Flutter `SpringDescription`
(mass 1, stiffness k = (2π / response)², damping c = 2 ζ √k).

| Token | Response | ζ | k | c | Overshoot | Use |
|---|---|---|---|---|---|---|
| `spring.snappy` | 0.25 | 0.90 | 631.7 | 45.2 | ~0 % | hover/press, lens grow, switch thumb, checkbox |
| `spring.panel` | 0.35 | 0.82 | 322.3 | 29.4 | ~1–2 % (+2 pt on a 270 pt growth) | Faro results growth, menus and popovers height, sheets |
| `spring.window` | 0.40 | 0.88 | 246.7 | 27.6 | <1 % | window open/close scale, minimise, Mission Control layouts |
| `spring.morph` | 0.50 | 0.70 | 157.9 | 17.6 | ~4–5 % | shape morphs: Faro buttons out/in, Launcher ↔ Faro, Control Center detail expand |
| `spring.bouncy` | 0.60 | 0.50 | 109.7 | 10.5 | ~16 % | Dock launch bounce, attention bounce |

| Token | Duration | Curve | Use |
|---|---|---|---|
| `fade.instant` | 100 ms | ease-out (0.2, 0, 0, 1) | closing transient surfaces (Faro, menus, tooltips) |
| `fade.quick` | 150 ms | ease-out | content swaps inside a panel, hover overlays |
| `fade.standard` | 250 ms | ease-in-out (0.4, 0, 0.2, 1) | notifications in/out, appearance changes |
| `fade.slow` | 400 ms | ease-in-out | wallpaper, display changes |
| `delay.hoverIntent` | 60 ms | — | pointer-enter morphs (Faro buttons) |
| `delay.tooltip` | 700 ms first, 0 ms while moving between items | — | tooltips |

Reduced motion (accessibility): springs become `fade.quick` cross-fades, no overshoot, no scale; bounces off.

## Rules

1. Opacity reaches 1 fast (≤ 60–100 ms) while geometry settles with a spring: content is readable at once.
2. Geometry animates from the **anchor edge** the reference uses (Faro: top edge fixed; menus: the edge
   attached to the menu title; popovers: the arrow).
3. Overshoot amplitudes are given in points in the specs; scale them by Amalfi size / reference size.
4. Glass never animates its blur radius; it animates its shape, opacity and the content under it.
5. A lens (`Glass/Clear`) grows in with `spring.snappy`, tracks the pointer 1:1, fades with `fade.quick`.
6. No animation is longer than the reference one; when the reference is unmeasured, use the token in
   the table below and mark it as an assumption in the code comment.

## Measured animations (binding)

The source of every curve is the spec section named; distances scale to Amalfi.

| Surface | Measured events | Source |
|---|---|---|
| Faro | open (opacity 55–65 ms, height overshoot +2, ~190 ms; width spring ~390 ms); buttons out; results growth; scopes; Esc; close | `faro.md`, reference FARO-02 |
| Launcher | open (6–8 % larger → 1 in ~120 ms, ~3 pt bounce, ~500 ms); category change; results swap; Esc to Faro bar; launch; caret blink | `launcher.md` |
| Menus, menu bar | open (one frame), item chosen (blink + ~190 ms fade), Esc fade, title switch, submenus | `menubar.md` |
| Tooltips | appear after ~1.5 s (one frame), switch, leave (~450 ms fade) | reference appkit-controls |
| Window buttons | symbols in/out | `windows.md` |
| Dock | label in/out, magnification (timing only), auto-hide, launch bounce `cubic-bezier(0.38,0,0.64,1)`, cell widen/narrow, running dot, attention bounce, reorder, remove, context menu, minimise, genie (partial), stacks fan/grid | `dock.md`, `dock-stacks.md`, `misc.md` |
| Windows | open (0.98 → 1, ~100–130 ms), close (fade 100–120 ms), minimise/restore (partial), full screen enter/leave, sidebar hide/show | `windows.md` |
| Spaces, Panorama | space switch (critically damped ω≈13.2/s), open/close (ω≈22–24/s), spaces bar expand, menu bar swap, add/remove space, click window, ⌃←/→, hot corner, Reduce motion cross-fades | `spaces.md`, `panorama.md` |
| App switcher | appear, selection, release fade ~190 ms, Esc | `app-switcher.md` |
| Control Center | open/close (alpha table, scale), module → detail morph, detail back (partial), edit gallery | `controlcenter.md` |
| Notifications | banner enter/stay/leave/replace, hover, stack expand/clear, Notification Centre open/close | `notifications.md` |
| HUDs | show, stay, hide, bar fill | `hud.md` |
| Screenshot UI | thumbnail in/stay/out, drag, crosshair | `screenshot-ui.md` |
| Sheets, alerts, save panel | expand/collapse, sheet/alert open and close | `dialogs.md`, reference appkit-controls |
| Segmented lens | press, move, release | reference clock notes |
| Stage View | switch groups | `stage-view.md` |

## Not measured (tokens apply, mark as assumption)

- Faro: typing width change, selection, hover and pressed states.
- Launcher: scroll inertia, close by clicking outside, icon drag/reorder; the Faro ↔ Launcher morph
  between the two measured end states, and grid ↔ list morph (use `spring.morph`).
- Menus: title capsule appear/disappear.
- Popovers (open/close in general).
- Windows: zoom (⌥ green +); exact genie curve.
- Dock: magnification law (shape of the fall-off), Trash empty ↔ full change, drop on a non-accepting app.
- Control Center: detail close, edit-mode drag/remove/resize, pressed and focus.
- Notifications: alert style, inline reply, scroll, drag out, widgets.
- Spaces: window drop-in entry, return when dropped outside; Panorama App Windows with Reduce motion.
- System alerts; context menu opening; Dock stack drag out.
- Switch, checkbox, slider and scroller fade timing.
- Brightness HUD; displays (Move to, cross-display drag); Stage View activation; screen recording.

## Surfaces

| Surface | Event | Motion | Reference |
|---|---|---|---|
| Faro | open | opacity 0.22 → 1 in ~60 ms; height from 0.97 with `spring.panel` (+2 pt), ~190 ms | `docs/spec/faro.md` |
| Faro | pointer enters (buttons out) | after `delay.hoverIntent`: bar 640 → 384, buttons slide out from behind it, `spring.morph`; bar −20 pt undershoot then +2; buttons +12 pt overshoot; settled ~690 / ~800 ms | reference notes FARO-02 |
| Faro | typing (buttons in) | reverse of buttons out with `spring.morph` (unmeasured: assumption) | — |
| Faro | results appear | ~500 ms after the last key, bottom edge grows with `spring.panel`, ~400 ms; content swaps in one frame | FARO-02 |
| Faro | enter/leave scope | height with `spring.panel`, +4 / −4 pt, ~450 ms; width fixed 640 | FARO-02 |
| Faro | close | `fade.instant` with a slight brighten and scale to 0.97 toward the centre, gone ~100 ms | FARO-02 |
| Launcher | open / Esc to bar | open: from ~1.06 scale to 1 with a small bounce; Esc: content fades, then the 844 × 580 panel shrinks into Faro's 640 × 56 bar (keep the measured curve) | `docs/spec/launcher.md` |
| Menus | open / close | open in one frame, no animation; after a choice (blink, +114 ms) or Esc a linear ~190 ms fade out (measured; the window popover is an assumption) | `docs/spec/menubar.md` |
| Popovers | open/close | fade + scale 0.96 → 1 from the arrow, `spring.panel` | `docs/spec/misc.md` |
| Tooltips | show/hide | `fade.quick` after `delay.tooltip` | `docs/spec/tip.md` |
| Notifications | banner in/out, Centre | banner in: exponential slide, tau 71 ms, fade 210 ms, blur sharp in 215 ms; out: measured steps in ~190 ms with blur; Centre in: two exponentials (12 and 70 ms) over 235 ms, out: ~300 ms steps; distances scale to Amalfi | `docs/spec/notifications.md` |
| Control Center | open / close, detail, hover | measured opacity tables (open 310 ms, close 293 ms) and scale 1.0075 to 1; module to detail on the measured slow-fast-slow table (333 ms, shorter for nearer modules; back x1.35); hover 10 % at 50 ms, half at 150 ms, done at 300 ms | `docs/spec/controlcenter.md` |
| Dock | magnification, launch bounce, attention, minimise | magnification timing measured (the fall-off shape is an assumption); launch bounce is the measured parabolic curve (17 pt scaled to Amalfi, 693 ms); attention hops 56/23/8 pt scaled; minimise genie and scale effect measured | `docs/spec/dock.md`, `misc.md` |
| Windows | open / close / zoom | measured: open 0.98 to 1 in ~100 ms, close fade ~125 ms with scale 0.995; zoom is unmeasured (assumption) | `docs/spec/windows.md` |
| Mission Control / Panorama | enter / exit, space switch | measured curves (space switch critically damped omega 13.2/s, Panorama open/close tables, spaces bar springs) | `docs/spec/panorama.md`, `spaces.md` |
| App switcher | show / release / Esc | appears in one frame; release fades linearly in ~190 ms; Esc closes in the next frame | `docs/spec/app-switcher.md` |
| Screenshot bar | show/hide | at once (nothing measured, no motion invented); the action button width keeps its measured 60 ms + 450 ms; the thumbnail keeps its measured 380 / 5400 / 250 ms | `docs/spec/screenshot-ui.md` |
| Segmented control | select | lens grows (`spring.snappy`), slides to the new segment (`spring.morph`), fades | — |
| Switch | toggle | thumb `spring.snappy`; while dragged the thumb is a Clear lens | — |
| Push / icon buttons | hover / press | overlay `fade.quick` in, 60 ms out | — |

When a measured spec gives a curve that a token cannot match (for example the Launcher Esc shrink),
keep the measured curve for that one surface and say so in the code.
