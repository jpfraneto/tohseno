#!/bin/sh
set -eu

printf '%s\n' \
  "build-genesis-archive.sh: v0.7 Genesis archives are immutable release artifacts." \
  "Reproduce one only from the v0.7.1 tag; current main is a successor generation." >&2
exit 1
