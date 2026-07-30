#!/bin/sh
set -eu

printf '%s\n' \
  "deploy-candidate.sh: the undeployed v0.7 contract generation is retired and will never be deployed." \
  "The successor generation is not finalized. No deployment command is currently available." >&2
exit 1
