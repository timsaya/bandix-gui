# Linux packaging

The Linux release baseline is **glibc 2.35 (Ubuntu 22.04)**.
Run `./scripts/package-linux.sh` from the repository. By default it builds and
packages inside an Ubuntu 22.04 container using Podman (preferred) or Docker.
It uses `Cargo.lock`, embeds the selected charcoal icon, and produces packages
in `target/dist/`:

- `bandix-gui-<version>-linux-<arch>.AppImage`: one executable file.
- `bandix-gui-<version>-linux-<arch>.tar.gz`: the same application directory, with
  bundled libraries, desktop entry and icons from 32 to 512 pixels.
- `bandix-gui-<version>-linux-<arch>.deb`: Debian/Ubuntu package (`amd64` or `arm64`).
- `bandix-gui-<version>-linux-<arch>.rpm`: RPM package (`x86_64` or `aarch64`).
- `bandix-gui-<version>-linux-<arch>.build-info.json`: architecture, build distribution,
  detected minimum glibc version, source commit/dirty state and tool hashes.
- `bandix-gui-<version>-linux-<arch>.sha256`: checksums for the selected packages and
  build information. Verify them with `sha256sum -c <filename>.sha256`.

Supported architectures are x86_64 and aarch64. Build natively on each
architecture; this script does not cross-package. Packages are named by Linux,
architecture and format, rather than by distribution. The recorded build
distribution is provenance, not a guarantee of compatibility with other systems.

```sh
./scripts/package-linux.sh                         # all four formats
./scripts/package-linux.sh --format appimage        # AppImage only
./scripts/package-linux.sh --format tar.gz         # extracted bundle only
./scripts/package-linux.sh --format deb            # Debian/Ubuntu package only
./scripts/package-linux.sh --format rpm            # RPM package only
./scripts/package-linux.sh --output-dir target/packages
./scripts/package-linux.sh --native                # use an existing compatible build environment
```

Relative output paths are resolved from the repository root. `CARGO_TARGET_DIR`
changes the default output and cache locations. The script obtains the
executable path from Cargo and checks its ELF architecture before packaging.
It builds in a temporary directory and publishes only after all selected formats
succeed, preserving existing packages on build failure. Unselected formats from
earlier builds are left untouched; the checksum file covers only this build.

The host needs Rust/rustup with the repository's toolchain installed, plus Podman
or Docker. Set `BANDIX_CONTAINER_ENGINE=docker` to choose Docker explicitly.
The source tree and selected Rust toolchain are mounted read-only; all system
headers and libraries come from the container. Building the container installs
the development and packaging dependencies listed in `Containerfile` without
modifying the host. Container builds default to four Cargo jobs; override with
`CARGO_BUILD_JOBS` if needed.

Container compilation, Cargo downloads and packaging tools use separate caches
under `target/linux-glibc-2.35/<arch>/`, avoiding reuse of host-linked binaries.
The optional `--native` mode needs the same dependencies on the current system,
including Python 3.10 with `tomli` (or Python 3.11+). It still enforces the glibc
baseline and will reject incompatible bundles, including native builds that
pull newer libraries from the host.
Native package builds also require `dpkg-dev` (DEB) or `rpm`/`rpmbuild` (RPM),
which are included in the container image.

The script caches the official
[linuxdeploy](https://github.com/linuxdeploy/linuxdeploy) and
[appimagetool](https://github.com/AppImage/appimagetool) tools under
the build directory's `packaging-tools/`. `appimagetool` is only needed when
selecting the AppImage format. Their hashes are recorded in `BUILD-INFO.json` inside
the package. To refresh cached tools, remove that cache before rebuilding.

Before publishing, `scripts/check-linux-abi.py` scans every ELF in the application
directory. It checks version requirements, ignores optional `WEAK` requirements,
and rejects any required glibc version above 2.35. The package records the release
baseline, detected minimum and per-file required/optional versions. This check
does not replace runtime testing of graphics drivers and desktop integrations.

```sh
python3 scripts/test-linux-abi.py
python3 scripts/check-linux-abi.py target/dist/bandix-gui-0.1.0-linux-x86_64/usr
```

## Running

### DEB and RPM

Install with a package manager so runtime dependencies are resolved:

```sh
sudo apt install ./bandix-gui-0.1.0-linux-x86_64.deb
sudo dnf install ./bandix-gui-0.1.0-linux-x86_64.rpm
```

Both packages install `/usr/bin/bandix-gui`, a system applications-menu entry,
and the Bandix icons. Launch from the menu or run `bandix-gui`. The native
packages use system libraries; DEB dependencies are generated with
`dpkg-shlibdeps`, and RPM uses automatic ELF dependency generation. Dynamically
loaded font, Wayland and graphics libraries are also declared explicitly.
They require glibc 2.35 or later and a working graphical environment.
Native packages do not need Python or FUSE to run, and do not use `--install`.

Installing a newer package upgrades the same `bandix-gui` package. Remove it
with `sudo apt remove bandix-gui` or `sudo dnf remove bandix-gui`. User preferences
are retained. The packages do not install the Bandix service itself.

Metadata is maintained in `packaging/linux/package.json`. The maintainer is the
GitHub profile URL, the homepage points to the project repository, and the
license is explicitly `Unspecified` (not declared). Packages are unsigned local
builds, not submissions to official distribution repositories.

### AppImage and tar.gz

Make the AppImage executable and launch it. If FUSE is unavailable, use
`./bandix-gui-<version>-linux-<arch>.AppImage --appimage-extract-and-run`.

To install the AppImage for the current user, including its application menu
entry and icons:

```sh
./bandix-gui-<version>-linux-<arch>.AppImage --install
# Without FUSE:
./bandix-gui-<version>-linux-<arch>.AppImage --appimage-extract-and-run --install
```

Installation requires Python 3 and no root access. It copies the AppImage to
`~/.local/share/bandix/Bandix.AppImage`, installs the `bandix-gui.desktop` entry
and icon sizes, and refreshes available desktop caches. `XDG_DATA_HOME` overrides
the data directory. The menu launcher works without FUSE. The original download
can be moved or deleted afterwards. Run the new version with `--install` to
replace the installed copy atomically; repeating installation is supported.
GNOME/GVfs file-icon metadata is also set on the installed copy when available;
the original download's file-manager thumbnail still depends on desktop support.

Alternatively, extract the tar.gz and run `./AppRun` inside its directory. Run
`./AppRun --install` (or `./install.sh`) there to copy the bundle into the current user's data directory
and add the Bandix desktop launcher and icon. Installation requires Python 3
and does not need root. Keep the complete bundle together when running without
installation.

Installer regression checks: `python3 scripts/test-linux-install.py` (requires
`gio` and `desktop-file-validate`; all installations use temporary directories).

The supported baseline is glibc 2.35 or later; the detected symbol requirement may
be lower. Both values are recorded in `README.txt` and the build information JSON.
X11/Wayland, fonts, keyboard data and GPU drivers remain host dependencies.
Wayland client/cursor and xkbcommon libraries are deliberately supplied by the
host: older copies can prevent newer Mesa/Vulkan drivers from loading or fail
to parse newer keyboard/Compose data. Ubuntu's packages are
`libwayland-client0`, `libwayland-cursor0`, `libxkbcommon0` and `libxkbcommon-x11-0`.
Packages still need runtime validation on the intended distributions; the glibc
baseline alone does not guarantee that every Linux installation can run them.

The selected artwork is `assets/bandix.png`; sized desktop icons are in
`assets/icons/`. The same icon is embedded in the binary for the sidebar,
client title bar and X11 window icon. The Wayland app ID, desktop filename and
icon name are all `bandix-gui` so desktop environments can associate them.
