# Kit issues

Defects of the Amalfi kit (silica-design) found by the parity tests. The design docs
(`docs/design/amalfi/` in silica-desktop) win over the kit; each issue is a deviation in the
test that names it, until the kit is fixed.

- **Dark/Glass/Regular blur.** The shared style blurs 10 (Thick's value); `materials.md` and
  `tokens.json` give Glass/Regular σ 6. Seen in `Buttons/Push/Glass/*/{Rest,Focused,Disabled}/Dark`.
