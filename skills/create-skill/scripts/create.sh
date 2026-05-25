#!/usr/bin/env bash
# Cross-platform entry for create.ps1 (requires PowerShell).
set -euo pipefail
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if command -v pwsh >/dev/null 2>&1; then
  exec pwsh -NoProfile -ExecutionPolicy Bypass -File "$DIR/create.ps1" "$@"
elif command -v powershell >/dev/null 2>&1; then
  exec powershell -NoProfile -ExecutionPolicy Bypass -File "$DIR/create.ps1" "$@"
fi
echo "create.ps1 requires PowerShell (install pwsh)." >&2
exit 1
