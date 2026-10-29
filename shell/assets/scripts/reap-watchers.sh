#!/usr/bin/env bash
# Fail closed on missing/reused process identities. No heuristic orphan kills.
exec python3 -I "$(dirname -- "$0")/process-identity.py" reap "$@"
