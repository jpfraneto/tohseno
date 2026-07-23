# Shot icon sources

These four 1254 × 1254 PNG contact sheets were supplied by the TOHSENO owner
for the public landing page. They remain outside `public/` so browsers never
download the full-resolution sheets.

`bun run site:icons` runs
[`apps/site/scripts/extract-shot-icons.ts`](../../scripts/extract-shot-icons.ts).
The script:

1. verifies the SHA-256 digest of every source sheet;
2. selects 25 documented grid cells from each sheet;
3. crops the icon face using fixed pixel coordinates;
4. resizes each crop to 192 × 192;
5. writes 100 quality-82 WebP files to `apps/site/public/shot-icons`;
6. derives `apps/site/public/favicon.png` from the amber eclipse crop; and
7. records the source sheet, row, and column of every output in
   `apps/site/assets/shot-icon-manifest.json`.

The selection intentionally mixes practical, playful, minimal, mystical,
psychedelic, and raw darkroom treatments while avoiding the most obvious
repeated motifs across sheets.
