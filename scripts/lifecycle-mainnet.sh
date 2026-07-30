#!/bin/sh
set -eu

printf '%s\n' \
  "lifecycle-mainnet.sh: the undeployed v0.7 Robinhood mainnet lifecycle is retired." \
  "No successor generation has been finalized or authorized for deployment." >&2
exit 1
