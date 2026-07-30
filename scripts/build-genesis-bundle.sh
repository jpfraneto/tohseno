#!/bin/sh
set -eu

printf '%s\n' \
  "build-genesis-bundle.sh: the v0.7 Genesis bundle is immutable and may only be reproduced from the v0.7.1 tag." \
  "Use the signed v0.7.1 release artifact for legacy verification. Main must not mix successor sources with the frozen v0.7 deployment plan." >&2
exit 1
