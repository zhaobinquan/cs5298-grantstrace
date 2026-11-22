#!/usr/bin/env bash
set -euo pipefail

: "${LYQUID_ADDRESS:?set LYQUID_ADDRESS from shaker deploy output}"
: "${ADMIN_KEY:?set ADMIN_KEY for the local devnet admin account}"
: "${NODE_IDS_JSON:?set NODE_IDS_JSON to a JSON array of four Node ID strings}"
RPC_URL="${RPC_URL:-http://127.0.0.1:10087/api}"

committee_json="$(python3 -c 'import json,os; ids=json.loads(os.environ["NODE_IDS_JSON"]); assert len(ids)==4 and len(set(ids))==4; print(json.dumps({"node_ids":ids},separators=(",",":")))')"
cast send --rpc-url "$RPC_URL" --private-key "$ADMIN_KEY" "$LYQUID_ADDRESS" 'configure_committee(string)' "$committee_json"
echo 'Committee staged. Activate and synchronize its epoch using the LDK oracle advance/finalize workflow, then query get_committee_status().'
cast call --rpc-url "$RPC_URL" "$LYQUID_ADDRESS" 'get_committee_status()(string)'
