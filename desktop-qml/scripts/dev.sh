#!/usr/bin/env bash
# Run the QML client from the source tree. No compile step; QML changes only need a restart.
cd "$(dirname "$0")/.." && exec python3 main.py "$@"
