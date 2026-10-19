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
