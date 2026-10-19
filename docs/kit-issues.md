# Kit issues

Defects of the Amalfi kit (silica-design) found by the parity tests. The design docs
(`docs/design/amalfi/` in silica-desktop) win over the kit; each issue is a deviation in the
test that names it, until the kit is fixed.

- **Dark/Glass/Regular blur.** The shared style blurs 10 (Thick's value); `materials.md` and
  `tokens.json` give Glass/Regular σ 6. Seen in `Buttons/Push/Glass/*/{Rest,Focused,Disabled}/Dark`.
- **Small radio overlays.** `Toggles/Radio/*/Small/*`: the box is a circle (radius 8) but the
  hover wash and the focus halo keep radius 7.
- **Group rows.** `Toggles/*/Group/List`: the rows are symbol instances whose text overrides
  are not exported (every row shows its master's default label), the Radio rows keep a stale
  width (140, the widest row is 124), the heading frame of "Order" is a fixed 40 (36 on the
  4 pt grid), and the Checkbox group PNG has a 6 pt empty margin below.
- **Lens refraction.** The kit's `Glass/Clear` lens of a dragged switch refracts the track
  beneath it; the lens region is masked in the pixel comparison, its geometry is compared.
