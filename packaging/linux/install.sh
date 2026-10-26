#!/usr/bin/env bash
# Optional per-user desktop integration for the extracted tar.gz bundle.
set -euo pipefail

bundle_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
export BANDIX_BUNDLE_DIR="$bundle_dir"
python3 - <<'PY'
import os
import shutil
from pathlib import Path

source = Path(os.environ['BANDIX_BUNDLE_DIR'])
data_home = Path(os.environ.get('XDG_DATA_HOME', str(Path.home() / '.local/share')))
destination = data_home / 'bandix' / source.name
if source.resolve() != destination.resolve():
    shutil.copytree(source, destination, dirs_exist_ok=True)

def exec_quote(value):
    for char in ('\\', '"', '`', '$'):
        value = value.replace(char, '\\' + char)
    return '"' + value.replace('%', '%%') + '"'

desktop = (destination / 'usr/share/applications/bandix-gui.desktop').read_text()
desktop = desktop.replace('Exec=bandix-gui', 'Exec=' + exec_quote(str(destination / 'AppRun')))
applications = data_home / 'applications'
applications.mkdir(parents=True, exist_ok=True)
(applications / 'bandix-gui.desktop').write_text(desktop)
shutil.copytree(destination / 'usr/share/icons', data_home / 'icons', dirs_exist_ok=True)
print('Bandix installed for the current user:', destination)
PY

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "${XDG_DATA_HOME:-$HOME/.local/share}/applications"
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"
fi
