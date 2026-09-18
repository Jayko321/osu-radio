#!/usr/bin/env bash
set -euo pipefail
project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)
cd "$project_root"
qml_lint=${QMLLINT:-/usr/lib/qt6/bin/qmllint}
# Cargo generates QObject type descriptions here. Use the newest build if several
# build profiles / feature combinations have left artifacts.
lint_root=$(mktemp -d)
trap 'rm -rf -- "$lint_root"' EXIT
qml_import=$(python3 - <<'PY'
from pathlib import Path
import os
roots = list(Path(os.environ.get('CARGO_TARGET_DIR', 'target')).glob('debug/build/osu-radio-qt-*/out/qt-build-utils/qml_modules/OsuRadio/qmldir'))
if not roots:
    raise SystemExit('Build osu-radio-qt before linting QML.')
print(max(roots, key=lambda p: p.stat().st_mtime).parent.parent)
PY
)
mapfile -t qml_files < <(rg --files apps/osu-radio-qt/qml apps/osu-radio-qt/tests -g '*.qml' | sort)
mkdir -p "$lint_root/OsuRadio"
cp "$qml_import/OsuRadio/qmldir" "$qml_import/OsuRadio/plugin.qmltypes" "$lint_root/OsuRadio/"
ln -s "$project_root/apps/osu-radio-qt/qml" "$lint_root/OsuRadio/qml"
"$qml_lint" -I "$lint_root" "${qml_files[@]}"
