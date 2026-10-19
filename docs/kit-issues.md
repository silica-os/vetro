# Kit issues

Defects of the Amalfi kit (silica-design) found by the parity tests. The design docs
(`docs/design/amalfi/` in silica-desktop) win over the kit; each issue is a deviation in the
test that names it, until the kit is fixed.

- **Dark/Glass/Regular blur.** The shared style blurs 10 (Thick's value); `materials.md` and
  `tokens.json` give Glass/Regular σ 6. Seen in `Buttons/Push/Glass/*/{Rest,Focused,Disabled}/Dark`.
- **Small radio overlays.** `Toggles/Radio/*/Small/*`: the box is a circle (radius 8) but the
  hover wash and the focus halo keep radius 7.
- **Group rows.** `Toggles/*/Group/List`: the Radio rows keep a stale width (140, the widest
  row is 124), the heading frame of "Order" is a fixed 40 (36 on the 4 pt grid), and the
  Checkbox group PNG has a 6 pt empty margin below. (Text overrides of instances are in the
  export by object id only; the harness applies the i-th override to the i-th text layer.)
- **Segments, Icon + Label, Dark.** The Dark masters have the tint twice (`Track` carries it
  and so does `Track tint`): the body is darker than `materials.md`. Pixels are not compared.
- **Segments, One 6 Text, Pressed.** Track and Lens are scaled by 437/440 (Track 9.5 + 437
  wide in a 456 frame); their geometry is not compared, the PNG is 1 pt wider.
- **Segments, Mini items.** Items are 44 wide whatever the label; "Three" at 11 pt needs 48,
  so the test cuts the label to "Thr" (named deviation).
- **Segment lens radius.** The kit's segment lens radius is half its height; the lens rule
  (user decision 2026-10-16, as the switch lens) takes half minus 1 (`Lens#radii`).
- **Lens refraction.** The kit's `Glass/Clear` lens of a dragged switch refracts the track
  beneath it; the lens region is masked in the pixel comparison, its geometry is compared.
- **Resized nested instances.** A symbol instance resized in the kit keeps its nested layers'
  places (`Group Boxes/Box/Nested`: the inner box 256 wide over a 224 master, the switch row;
  `Fields/Token Field` and `Fields/Search Field/*/Token*`: tokens as wide as their label over
  a 64 wide master). The harness expands nested masters unscaled, so those layers are named
  in the tests and not compared.
- **Text measure.** Sketch measures a few strings a little wider than Skia: "Notifications"
  (Headline, 88 against 84 on the 4 pt grid), the hint "Drop image" (61 against 59.96), the
  label boxes of tokens (36 against a 32 frame, which is also 28 tall).
- **Swatch names.** `Wells/Swatch Row/Row 4` has a "Shadow/Warm" swatch: no such token (silica_ui
  shows Shadow/Soft); a swatch's colour is an override of the master's fill the export does not
  carry, so every chip's fill but the accent one is named as a deviation.
- **Icon + Label buttons, pop-up and pull-down `Open`, `Pop-up/App`**: not covered yet (menus,
  app icons).

## Menus and open pop-up buttons

- `Buttons/Pop-up/Open` and `Buttons/Pull-down/Open`: the Menu layer is above the Button and the
  kit's glass hides the button completely; with a real glass blur the button shows through as a
  ghost. Geometry matches; pixels are not compared.
- `Menus/Menu`: two `Separator/Rest/Line` layers share one name, so their layer path is
  ambiguous; geometry skips them (the `Item/Separator` masters cover them).
- `Menus/Item/App` and `Buttons/Pop-up/App` need app icons: not implemented.

## Sidebars

- `Sidebars/Footer`: the Settings button is a second plus button (the nested instance has no
  glyph override); ours shows a gear and the test sets the plus.
- `Row/*/Selected Inactive`: the light selection uses `Surface/Control Pressed` and the dark one
  `Surface/Control`; ours follows each.
- `Row/Volume`: the kit names the glyph `externaldrive`; ours is the `drive` symbol.
- `Row/Volume/Hover`: the eject button is hovered together with the row.

## Titlebars

- Several instances of one master share a name in the compact bar (`Toolbar button` ×4): their
  layer path is ambiguous, so geometry skips them (the pixels cover them).
- `Title Bar/Subtitle`: Sketch measures the subtitle 2 pt wider than we do; its frame is not
  compared.
- `Piece/Search Token`: the nested token master keeps its own label box (28 pt high).
- `Segmented Text/Inactive`: the labels stay primary in the kit while the icon segments dim.
- The window buttons' pitch in the kit is 24 pt (8 pt gap) and the bars place them 16 or 20 pt in;
  `windowControls` in the tokens (centre 26, pitch 23) is the measured placement of the shell.
- The 16 pt dot masters are compared with a loose mean (edge pixels dominate); their inside
  matches exactly.
