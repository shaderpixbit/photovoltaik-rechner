#!/usr/bin/env bash
# Baut das SolarEdge-Sidecar als single-file Binary via PyInstaller.
# Nutzt stdlib only — kein venv noetig, aber PyInstaller selbst will eins.
#
# Nutzung (im Repo-Root):  ./vendor-import-solaredge/build-sidecar.sh
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/.." && pwd)"
VENV="$HERE/.venv"
OUT_DIR="$REPO_ROOT/src-tauri/binaries"

TARGET_TRIPLE="$(rustc -vV | sed -n 's|host: ||p')"
if [ -z "${TARGET_TRIPLE:-}" ]; then
  echo "rustc nicht gefunden — Target-Triple kann nicht ermittelt werden." >&2
  exit 1
fi
EXT=""
if [[ "$TARGET_TRIPLE" == *windows* ]]; then EXT=".exe"; fi
BIN_NAME="solaredge-${TARGET_TRIPLE}${EXT}"

if [ ! -d "$VENV" ]; then
  python3 -m venv "$VENV"
fi
# shellcheck source=/dev/null
source "$VENV/bin/activate"

pip install --quiet --upgrade pip
pip install --quiet pyinstaller

mkdir -p "$OUT_DIR"
pyinstaller \
  --onefile \
  --name "solaredge-${TARGET_TRIPLE}" \
  --distpath "$OUT_DIR" \
  --workpath "$HERE/.pyinstaller-build" \
  --specpath "$HERE/.pyinstaller-build" \
  --clean \
  "$HERE/main.py"

deactivate
echo
echo "✓ SolarEdge-Sidecar fertig: $OUT_DIR/$BIN_NAME"
