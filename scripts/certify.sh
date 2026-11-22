#!/usr/bin/env bash
set -euo pipefail

: "${LYQUID_ADDRESS:?set LYQUID_ADDRESS from shaker deploy output}"
RPC_URL="${RPC_URL:-http://127.0.0.1:10087/api}"

echo 'Current node candidate:'
cast call --rpc-url "$RPC_URL" "$LYQUID_ADDRESS" 'get_candidate()(string)'
echo 'Submitting certification from this node:'
cast send --rpc-url "$RPC_URL" "$LYQUID_ADDRESS" 'request_certification()(string)'
echo 'Query get_grant_status() after the certified call is sequenced:'
cast call --rpc-url "$RPC_URL" "$LYQUID_ADDRESS" 'get_grant_status()(string)'
