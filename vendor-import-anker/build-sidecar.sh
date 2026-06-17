#!/usr/bin/env bash
# Baut das Anker-Cloud-Sidecar als single-file Binary via PyInstaller
# und legt es unter src-tauri/binaries/anker-solix-<target-triple>(.exe) ab,
# wie es Tauri 2 fuer `bundle.externalBin` erwartet.
#
# Nutzung (im Repo-Root):
#   ./vendor-import-anker/build-sidecar.sh
#
# Voraussetzungen: python3, pip, virtualenv. Wird einmalig vor `bun run tauri build`
# (und ggf. vor `bun run tauri dev`, wenn der Import-Button getestet werden soll)
# ausgefuehrt.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/.." && pwd)"
VENV="$HERE/.venv"
OUT_DIR="$REPO_ROOT/src-tauri/binaries"

# Tauri erwartet Binaries mit Rust-Target-Triple als Suffix.
TARGET_TRIPLE="$(rustc -vV | sed -n 's|host: ||p')"
if [ -z "${TARGET_TRIPLE:-}" ]; then
  echo "rustc nicht gefunden — Target-Triple kann nicht ermittelt werden." >&2
  exit 1
fi

EXT=""
if [[ "$TARGET_TRIPLE" == *windows* ]]; then
  EXT=".exe"
fi
BIN_NAME="anker-solix-${TARGET_TRIPLE}${EXT}"

echo "→ venv unter $VENV anlegen / aktivieren"
if [ ! -d "$VENV" ]; then
  python3 -m venv "$VENV"
fi
# shellcheck source=/dev/null
source "$VENV/bin/activate"

# anker-solix-api benoetigt Python >=3.12 — beim ersten Lauf hart pruefen.
PY_VERSION="$(python -c 'import sys; print(f"{sys.version_info.major}.{sys.version_info.minor}")')"
if [ "$(printf '%s\n3.12\n' "$PY_VERSION" | sort -V | head -1)" != "3.12" ]; then
  echo "Python >=3.12 erforderlich (gefunden: $PY_VERSION)." >&2
  echo "Im Nix-Dev-Shell ist python312 gesetzt — sonst venv loeschen ($VENV) und neu bauen." >&2
  deactivate
  exit 1
fi

echo "→ Abhaengigkeiten installieren"
pip install --quiet --upgrade pip
pip install --quiet -r "$HERE/requirements.txt"
pip install --quiet pyinstaller

echo "→ PyInstaller baut $BIN_NAME"
mkdir -p "$OUT_DIR"
pyinstaller \
  --onefile \
  --name "anker-solix-${TARGET_TRIPLE}" \
  --distpath "$OUT_DIR" \
  --workpath "$HERE/.pyinstaller-build" \
  --specpath "$HERE/.pyinstaller-build" \
  --clean \
  "$HERE/main.py"

deactivate

echo
echo "✓ Sidecar fertig: $OUT_DIR/$BIN_NAME"
echo
echo "Naechste Schritte:"
echo "  - Dev-Test:       bun run tauri dev   (Sidecar wird automatisch erkannt)"
echo "  - Release-Build:  bun run tauri:release"
echo "                    (entspricht: tauri build --config src-tauri/tauri.bundle.conf.json)"
