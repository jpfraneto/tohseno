#!/bin/sh
set -eu

repository_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

if [ "$#" -ne 4 ]; then
  printf '%s\n' \
    "usage: lifecycle-mainnet.sh <app-name> <unix-deadline> foundry-account <name>" \
    "   or: lifecycle-mainnet.sh <app-name> <unix-deadline> hardware-wallet <ledger|trezor>" \
    "Set TOHSENO_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION only when authorizing a missing BuilderAccount deployment." >&2
  exit 2
fi

app_name="$1"
deadline="$2"
wallet_kind="$3"
wallet_value="$4"

case "$deadline" in
  ''|*[!0-9]*)
    printf '%s\n' "The public-action deadline must be a Unix integer." >&2
    exit 2
    ;;
esac
case "$wallet_kind:$wallet_value" in
  foundry-account:*)
    case "$wallet_value" in
      ''|*[!A-Za-z0-9._-]*)
        printf '%s\n' "The Foundry account must be a simple keystore filename." >&2
        exit 2
        ;;
    esac
    wallet_flag="--foundry-account"
    ;;
  hardware-wallet:ledger|hardware-wallet:trezor)
    wallet_flag="--hardware-wallet"
    ;;
  *)
    printf '%s\n' "Choose a named Foundry account or ledger/trezor hardware wallet." >&2
    exit 2
    ;;
esac

if [ "${TOHSENO_ALLOW_EXPERIMENTAL_MAINNET:-0}" != "1" ]; then
  printf '%s\n' "Set TOHSENO_ALLOW_EXPERIMENTAL_MAINNET=1 for the GENESIS candidate only." >&2
  exit 1
fi
if [ -z "${ROBINHOOD_RPC_URL:-}" ]; then
  printf '%s\n' "Set ROBINHOOD_RPC_URL to an explicit Robinhood Chain endpoint." >&2
  exit 1
fi

chain_id="$(cast chain-id --rpc-url "$ROBINHOOD_RPC_URL")"
if [ "$chain_id" != "4663" ]; then
  printf 'Refusing chain ID %s; GENESIS requires 4663.\n' "$chain_id" >&2
  exit 1
fi

"$repository_root/scripts/probe-p256.sh" --rpc-url "$ROBINHOOD_RPC_URL"
"$repository_root/scripts/deploy-candidate.sh"

set -- "$wallet_flag" "$wallet_value"
if [ -n "${TOHSENO_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION:-}" ]; then
  set -- \
    --confirm-builder-account-deployment \
    "$TOHSENO_BUILDER_ACCOUNT_DEPLOYMENT_CONFIRMATION" \
    "$@"
fi

cd "$repository_root"
cargo run --locked --quiet -p tohseno -- \
  publish "$app_name" \
  --rpc-url "$ROBINHOOD_RPC_URL" \
  --deadline "$deadline" \
  --submit \
  --confirm-experimental-mainnet \
  "I UNDERSTAND THIS WILL BROADCAST TO ROBINHOOD CHAIN MAINNET 4663" \
  "$@"

printf '%s\n' "Candidate contracts, BuilderAccount, and exact Shot state were receipt-verified by the guarded CLI."
