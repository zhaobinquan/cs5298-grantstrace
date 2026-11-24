#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname -- "${BASH_SOURCE[0]}")/.."
export GRANTSTRACE_DEMO_KEYS=1
exec python3 frontend/server.py
