# Linux packaging

Run `./scripts/package-linux.sh` from the repository. It builds the release binary
with `Cargo.lock`, embeds the selected charcoal icon, and produces packages in
`target/dist/`:

- `bandix-<version>-linux-<arch>.AppImage`: one executable file.
- `bandix-<version>-linux-<arch>.tar.gz`: the same application directory, with
  bundled libraries, desktop entry and icons from 32 to 512 pixels.
- `bandix-<version>-linux-<arch>.sha256`: checksums for both packages.

The build needs Rust, curl, Python 3.11+, `desktop-file-validate`, binutils and the
GPUI Linux development libraries. The script caches the official
[linuxdeploy](https://github.com/linuxdeploy/linuxdeploy) and
[appimagetool](https://github.com/AppImage/appimagetool) tools under
`target/packaging-tools/`. Their hashes are recorded in `BUILD-INFO.txt` inside
the package. To refresh cached tools, remove that cache before rebuilding.

## Running

Make the AppImage executable and launch it. If FUSE is unavailable, use
`./bandix-<version>-linux-<arch>.AppImage --appimage-extract-and-run`.

Alternatively, extract the tar.gz and run `./AppRun` inside its directory. Run
`./install.sh` there to copy the bundle into the current user's data directory
and add the Bandix desktop launcher and icon. Installation requires Python 3
and does not need root. Keep the complete bundle together when running without
installation.

The build records its minimum glibc requirement in the packaged `README.txt`.
The current x86_64 build requires glibc 2.39 or later. X11/Wayland, fonts, keyboard
data and GPU drivers remain host dependencies. An AppImage built on a recent
distribution does not automatically support older glibc versions; build on an
older compatible Linux system to lower that requirement.

The selected artwork is `assets/bandix.png`; sized desktop icons are in
`assets/icons/`. The same icon is embedded in the binary for the sidebar,
client title bar and X11 window icon. The Wayland app ID, desktop filename and
icon name are all `bandix-gui` so desktop environments can associate them.
