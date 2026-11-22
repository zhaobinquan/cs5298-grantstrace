#!/usr/bin/env bash
set -euo pipefail

: "${LYQUID_ADDRESS:?set LYQUID_ADDRESS from shaker deploy output}"
: "${LYQUID_ID:?set LYQUID_ID from shaker deploy output}"
: "${ADMIN_KEY:?set ADMIN_KEY for the local devnet admin account}"
: "${NODE_IDS_JSON:?set NODE_IDS_JSON to a JSON array of four Node ID strings}"
RPC_URL="${RPC_URL:-http://127.0.0.1:10087/api}"
RPC_URLS="${RPC_URLS:-http://127.0.0.1:10087/api http://127.0.0.1:11087/api http://127.0.0.1:12087/api http://127.0.0.1:13087/api}"

committee_json="$(python3 -c 'import json,os; ids=json.loads(os.environ["NODE_IDS_JSON"]); assert len(ids)==4 and len(set(ids))==4; print(json.dumps({"node_ids":ids},separators=(",",":")))')"
cast send --rpc-url "$RPC_URL" --private-key "$ADMIN_KEY" "$LYQUID_ADDRESS" 'configure_committee(string)' "$committee_json"
target="$(shaker to-hex "$LYQUID_ID")"
cast call --rpc-url "$RPC_URL" "$LYQUID_ADDRESS" '__lyquor_oracle_advance_epoch(string,address,bool)(bool)' releases "$target" false
cast call --rpc-url "$RPC_URL" "$LYQUID_ADDRESS" '__lyquor_oracle_finalize_epoch(string,address,bool)(bool)' releases "$target" false
for attempt in $(seq 1 60); do
    all_ready=true
    for node_rpc in $RPC_URLS; do
        if ! status="$(cast call --rpc-url "$node_rpc" "$LYQUID_ADDRESS" 'get_committee_status()(string)' 2>/dev/null)"; then
            all_ready=false
            break
        fi
        if ! NODE_IDS_JSON="$NODE_IDS_JSON" python3 -c 'import json,os,sys; response=json.loads(json.loads(sys.argv[1])); x=response["data"]; expected=set(json.loads(os.environ["NODE_IDS_JSON"])); sys.exit(not (response["ok"] and x["configured"] and x["active"] and x["threshold"] == 3 and set(x["node_ids"]) == expected))' "$status"; then
            all_ready=false
            break
        fi
    done
    if "$all_ready"; then
        printf 'committee active on all nodes: %s\n' "$RPC_URLS"
        exit 0
    fi
    sleep 1
done
printf 'committee activation unconfirmed after 60 seconds\n' >&2
exit 1
