#!/usr/bin/env bash
# Per-user desktop integration for either an AppImage or extracted bundle.
set -euo pipefail

command -v python3 >/dev/null || { echo 'Python 3 is required for desktop installation.' >&2; exit 1; }
bundle_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
export BANDIX_BUNDLE_DIR="$bundle_dir"
python3 - <<'PY'
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

source = Path(os.environ['BANDIX_BUNDLE_DIR'])
data_home = Path(os.environ.get('XDG_DATA_HOME') or Path.home() / '.local/share')
if not data_home.is_absolute():
    data_home = Path.home() / '.local/share'
appimage = os.environ.get('APPIMAGE')
if appimage:
    original = Path(appimage).resolve(strict=True)
    destination = data_home / 'bandix' / 'Bandix.AppImage'
    destination.parent.mkdir(parents=True, exist_ok=True)
    if original != destination.resolve() or destination.is_symlink():
        # Replacing the file atomically also permits upgrades while an older
        # installed AppImage is mounted/running.
        with tempfile.NamedTemporaryFile(dir=destination.parent, prefix='.Bandix-', delete=False) as temporary:
            temporary_path = Path(temporary.name)
        try:
            shutil.copyfile(original, temporary_path)
            temporary_path.chmod(0o755)
            temporary_path.replace(destination)
        finally:
            temporary_path.unlink(missing_ok=True)
    launcher = destination
    # A menu launch must work even on systems without FUSE.
    arguments = ' --appimage-extract-and-run'
else:
    destination = data_home / 'bandix' / source.name
    if source.resolve() != destination.resolve():
        shutil.copytree(source, destination, dirs_exist_ok=True)
    launcher = destination / 'AppRun'
    arguments = ''

def exec_quote(value):
    # Exec argument quoting is decoded after the desktop string escape layer.
    for char in ('\\', '"', '`', '$'):
        value = value.replace(char, '\\' + char)
    value = value.replace('\\', '\\\\').replace('\n', '\\n').replace('\r', '\\r').replace('\t', '\\t')
    return '"' + value.replace('%', '%%') + '"'

desktop = (source / 'usr/share/applications/bandix-gui.desktop').read_text()
# Keep the executable token free of field escapes: GLib checks it before
# expanding %% in paths. env receives the decoded path as a regular argument.
desktop = desktop.replace('Exec=bandix-gui', 'Exec=/usr/bin/env -- ' + exec_quote(str(launcher)) + arguments)
applications = data_home / 'applications'
applications.mkdir(parents=True, exist_ok=True)
(applications / 'bandix-gui.desktop').write_text(desktop)
shutil.copytree(source / 'usr/share/icons', data_home / 'icons', dirs_exist_ok=True)

for command in [
    ['update-desktop-database', str(applications)],
    ['gtk-update-icon-cache', '-f', '-t', str(data_home / 'icons/hicolor')],
]:
    if shutil.which(command[0]):
        result = subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
        if result.returncode:
            print(f'Icon/menu cache refresh skipped: {result.stderr.strip()}')

# GNOME/GVfs may also show a custom file icon for the installed AppImage.
# This is optional and machine-local; other desktops still use the menu icon.
if appimage and shutil.which('gio'):
    icon = data_home / 'icons/hicolor/512x512/apps/bandix-gui.png'
    subprocess.run(['gio', 'set', str(destination), 'metadata::custom-icon', icon.as_uri()],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
print('Bandix installed for the current user:', destination)
print('Launch Bandix from the applications menu. No root access is required.')
PY
