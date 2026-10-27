#!/usr/bin/env python3
"""Exercise desktop installation in temporary directories, without changing the user's menu."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]
GIO = shutil.which('gio')
VALIDATE = shutil.which('desktop-file-validate')
FAKE_APP = '#!/bin/sh\nprintf "%s\\n" "$@" > "$BANDIX_INSTALL_TEST_LOG"\n'


@unittest.skipUnless(GIO and VALIDATE, 'requires gio and desktop-file-validate')
class DesktopInstallTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='bandix-install-test-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.bundle = self.root / 'extracted bundle'
        self.bundle.mkdir()
        for name in ['AppRun', 'install.sh', 'bandix-gui.desktop']:
            destination = self.bundle / name if name != 'bandix-gui.desktop' else self.bundle / 'usr/share/applications' / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / 'packaging/linux' / name, destination)
        for size in [32, 48, 64, 128, 256, 512]:
            destination = self.bundle / f'usr/share/icons/hicolor/{size}x{size}/apps/bandix-gui.png'
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / f'assets/icons/bandix-{size}.png', destination)
        binary = self.bundle / 'usr/bin/bandix-gui'
        binary.parent.mkdir(parents=True)
        binary.write_text(FAKE_APP)
        binary.chmod(0o755)
        self.download = self.root / 'download.AppImage'
        self.download.write_text(FAKE_APP)
        self.download.chmod(0o755)
        self.data = self.root / 'user data $cash `tick` "quote" \\path %value'
        self.marker = self.root / 'launched-arguments'
        # Do not write real GVfs metadata or refresh the user's desktop caches.
        helpers = self.root / 'helpers'
        helpers.mkdir()
        for name in ['gio', 'update-desktop-database', 'gtk-update-icon-cache']:
            helper = helpers / name
            helper.write_text('#!/bin/sh\nexit 0\n')
            helper.chmod(0o755)
        self.env = dict(os.environ, XDG_DATA_HOME=str(self.data),
                        BANDIX_INSTALL_TEST_LOG=str(self.marker),
                        PATH=str(helpers) + os.pathsep + os.environ['PATH'])
        self.env.pop('APPDIR', None)
        self.env.pop('APPIMAGE', None)

    def install(self, appimage=None):
        env = dict(self.env)
        if appimage:
            env['APPIMAGE'] = str(appimage)
        subprocess.run([str(self.bundle / 'AppRun'), '--install'], env=env, check=True, capture_output=True, text=True)
        self.desktop = self.data / 'applications/bandix-gui.desktop'
        subprocess.run([VALIDATE, str(self.desktop)], check=True, capture_output=True, text=True)

    def launch(self):
        self.marker.unlink(missing_ok=True)
        result = subprocess.run([GIO, 'launch', str(self.desktop)], env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr + '\n' + self.desktop.read_text())
        deadline = time.monotonic() + 5
        while not self.marker.exists() and time.monotonic() < deadline:
            time.sleep(0.05)
        self.assertTrue(self.marker.exists(), 'desktop launcher did not reach the installed executable')
        return self.marker.read_text()

    def test_appimage_installs_icons_and_launches_after_download_is_deleted(self):
        self.install(self.download)
        installed = self.data / 'bandix/Bandix.AppImage'
        self.assertEqual(installed.read_bytes(), self.download.read_bytes())
        self.assertTrue(os.access(installed, os.X_OK))
        for size in [32, 48, 64, 128, 256, 512]:
            icon = self.data / f'icons/hicolor/{size}x{size}/apps/bandix-gui.png'
            self.assertEqual(hashlib.sha256(icon.read_bytes()).digest(),
                             hashlib.sha256((ROOT / f'assets/icons/bandix-{size}.png').read_bytes()).digest())
        self.download.unlink()
        shutil.rmtree(self.bundle)
        self.assertEqual(self.launch(), '--appimage-extract-and-run\n')

    def test_appimage_can_be_upgraded_and_reinstalled_from_installed_path(self):
        self.install(self.download)
        self.download.write_text(FAKE_APP + '# new version\n')
        self.install(self.download)
        installed = self.data / 'bandix/Bandix.AppImage'
        self.assertEqual(installed.read_bytes(), self.download.read_bytes())
        self.install(installed)
        self.assertEqual(installed.read_bytes(), self.download.read_bytes())
        self.assertEqual(len(list((self.data / 'applications').glob('bandix*.desktop'))), 1)
        self.assertFalse(list(installed.parent.glob('.Bandix-*')))

    def test_extracted_bundle_remains_installable(self):
        self.install()
        shutil.rmtree(self.bundle)
        self.assertEqual(self.launch(), '\n')

    def test_install_rejects_unexpected_arguments(self):
        result = subprocess.run([str(self.bundle / 'AppRun'), '--install', 'unexpected'],
                                env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertFalse(self.data.exists())


if __name__ == '__main__':
    unittest.main()
