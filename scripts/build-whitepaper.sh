#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repository_root="$(CDPATH= cd -- "$script_dir/.." && pwd)"
output_path="${1:-$repository_root/output/pdf/tohseno-whitepaper-2026-08-31.pdf}"

mkdir -p "$(dirname -- "$output_path")"

pandoc "$repository_root/WHITEPAPER.md" \
  --from=markdown+raw_tex+pipe_tables+fenced_code_blocks \
  --to=pdf \
  --pdf-engine=xelatex \
  --include-in-header="$repository_root/docs/whitepaper-preamble.tex" \
  --resource-path="$repository_root" \
  --variable=fontsize:10pt \
  --variable=papersize:letter \
  --variable=colorlinks:true \
  --highlight-style=tango \
  --output="$output_path"

printf '%s\n' "$output_path"
