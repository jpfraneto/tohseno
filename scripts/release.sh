#!/bin/sh
set -eu

printf '%s\n' \
  "release.sh: the v0.7.1 release is immutable and this branch is preparing a successor generation." \
  "Release assembly remains available from the v0.7.1 tag; a successor release path is not finalized." >&2
exit 1
