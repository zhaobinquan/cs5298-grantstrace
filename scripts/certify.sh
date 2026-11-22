#!/usr/bin/env bash
set -euo pipefail

: "${LYQUID_ADDRESS:?set LYQUID_ADDRESS from shaker deploy output}"
RPC_URL="${RPC_URL:-http://127.0.0.1:10087/api}"

echo 'Current node candidate:'
cast call --rpc-url "$RPC_URL" "$LYQUID_ADDRESS" 'get_candidate()(string)'
echo 'Submitting certification from this node:'
submission="$(cast call --rpc-url "$RPC_URL" "$LYQUID_ADDRESS" 'request_certification()(string)')"
printf '%s\n' "$submission"
python3 -c 'import json,sys; x=json.loads(json.loads(sys.argv[1])); sys.exit(not (x["ok"] and x["data"]["submitted"]))' "$submission"
for attempt in $(seq 1 60); do
    status="$(cast call --rpc-url "$RPC_URL" "$LYQUID_ADDRESS" 'get_grant_status()(string)')"
    if python3 -c 'import json,sys; x=json.loads(json.loads(sys.argv[1])); sys.exit(not (x["ok"] and x["data"]["state"]["snapshot"] is not None))' "$status"; then
        printf '%s\n' "$status"
        exit 0
    fi
    sleep 1
done
printf 'certified snapshot unconfirmed after 60 seconds; inspect state before retrying\n' >&2
exit 1
