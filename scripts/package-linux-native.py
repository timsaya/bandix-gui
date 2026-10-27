#!/usr/bin/env python3
"""Build DEB/RPM packages with system-managed runtime libraries and desktop integration."""
import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess


ROOT = Path(__file__).resolve().parents[1]
CACHE_HOOK = '''#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -f -t /usr/share/icons/hicolor || true
fi
exit 0
'''
# These libraries are opened dynamically and are not all visible in DT_NEEDED.
DEB_RUNTIME = ['libfontconfig1', 'libfreetype6', 'libwayland-client0',
               'libwayland-cursor0', 'libvulkan1', 'libegl1']
RPM_RUNTIME = ['libfontconfig.so.1', 'libfreetype.so.6', 'libwayland-client.so.0',
               'libwayland-cursor.so.0', 'libvulkan.so.1', 'libEGL.so.1']


def run(*args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def stage_files(stage, binary, info, metadata, abi_path):
    executable = stage / 'usr/bin/bandix-gui'
    executable.parent.mkdir(parents=True)
    shutil.copy2(binary, executable)
    executable.chmod(0o755)
    run('strip', '--strip-unneeded', str(executable))
    desktop = stage / 'usr/share/applications/bandix-gui.desktop'
    desktop.parent.mkdir(parents=True)
    shutil.copy2(ROOT / 'packaging/linux/bandix-gui.desktop', desktop)
    for size in [32, 48, 64, 128, 256, 512]:
        icon = stage / f'usr/share/icons/hicolor/{size}x{size}/apps/bandix-gui.png'
        icon.parent.mkdir(parents=True)
        shutil.copy2(ROOT / f'assets/icons/bandix-{size}.png', icon)
    with abi_path.open('w') as output:
        run('python3', str(ROOT / 'scripts/check-linux-abi.py'), str(stage / 'usr'),
            '--max-glibc', info['glibc_baseline'], stdout=output)
    abi = json.loads(abi_path.read_text())
    native_info = dict(info, minimum_glibc=abi['minimum_glibc'], glibc_requirements=abi['files'],
                       runtime_libraries='system', package_metadata=metadata)
    docs = stage / 'usr/share/doc/bandix-gui'
    docs.mkdir(parents=True)
    (docs / 'BUILD-INFO.json').write_text(json.dumps(native_info, indent=2) + '\n')
    (docs / 'README').write_text(
        f"Bandix {info['version']}\n\nLaunch Bandix from the applications menu or run bandix-gui.\n"
        f"Connect to your Bandix service in Settings.\n"
        f"Minimum supported glibc: {info['glibc_baseline']}.\n"
        f"Maintainer: {metadata['maintainer']}\nHomepage: {metadata['homepage']}\n"
        'License: not declared.\n\n'
        'Install, upgrade and remove this package using your system package manager.\n'
        'Graphics drivers and fonts must be available on the host system.\n'
    )


def build_deb(stage, work, output, info, metadata):
    arch = {'x86_64': 'amd64', 'aarch64': 'arm64'}[info['architecture']]
    debian = work / 'debian'
    debian.mkdir()
    # dpkg-shlibdeps derives direct dependencies from the actual ELF and the
    # Ubuntu baseline's symbols/shlibs database, including minimum versions.
    (debian / 'control').write_text(
        f"Source: bandix-gui\nSection: net\nPriority: optional\nMaintainer: {metadata['maintainer']}\n\n"
        'Package: bandix-gui\nArchitecture: any\nDescription: Bandix desktop client\n'
    )
    result = run('dpkg-shlibdeps', '-O', '-e' + str(stage / 'usr/bin/bandix-gui'), cwd=work, capture_output=True)
    dependency_line = next(line for line in result.stdout.splitlines() if line.startswith('shlibs:Depends='))
    dependencies = dependency_line.split('=', 1)[1].split(', ')
    dependencies += [f"libc6 (>= {info['glibc_baseline']})", *DEB_RUNTIME]
    dependencies = list(dict.fromkeys(dependencies))
    control = stage / 'DEBIAN'
    control.mkdir()
    size = sum(path.stat().st_size for path in stage.rglob('*') if path.is_file())
    (control / 'control').write_text(
        f"Package: bandix-gui\nVersion: {info['version']}-1\nArchitecture: {arch}\n"
        f"Maintainer: {metadata['maintainer']}\nSection: net\nPriority: optional\n"
        f"Homepage: {metadata['homepage']}\nInstalled-Size: {(size + 1023) // 1024}\n"
        f"Depends: {', '.join(dependencies)}\nRecommends: fonts-dejavu-core, xdg-utils\n"
        f"X-License: {metadata['license']}\nDescription: {metadata['summary']}\n"
        ' Monitor network traffic, manage device rate limits and quotas, and inspect\n'
        ' connections and DNS queries through an existing Bandix service.\n'
    )
    for name in ['postinst', 'postrm']:
        hook = control / name
        hook.write_text(CACHE_HOOK)
        hook.chmod(0o755)
    run('dpkg-deb', '--root-owner-group', '-Zxz', '--build', str(stage), str(output))
    shutil.rmtree(control)


def build_rpm(stage, work, output, info, metadata):
    top = work / 'rpmbuild'
    for name in ['BUILD', 'BUILDROOT', 'RPMS', 'SOURCES', 'SPECS', 'SRPMS']:
        (top / name).mkdir(parents=True, exist_ok=True)
    # Feed rpmbuild a source archive to keep shell/spec quoting independent of
    # the caller's output directory (which may contain spaces).
    run('tar', '-czf', str(top / 'SOURCES/payload.tar.gz'), '-C', str(stage), '.')
    requires = '\n'.join(f'Requires: {name}()(64bit)' for name in RPM_RUNTIME)
    hook = CACHE_HOOK.removeprefix('#!/bin/sh\n')
    spec = top / 'SPECS/bandix-gui.spec'
    spec.write_text(f'''Name: bandix-gui
Version: {info['version']}
Release: 1
Summary: {metadata['summary']}
License: {metadata['license']}
URL: {metadata['homepage']}
Packager: {metadata['maintainer']}
Source0: payload.tar.gz
BuildArch: {info['architecture']}
Requires: glibc >= {info['glibc_baseline']}
{requires}
Suggests: font(dejavusans)
Suggests: xdg-utils

%description
Monitor network traffic, manage device rate limits and quotas, and inspect
connections and DNS queries through an existing Bandix service.

%prep

%build

%install
mkdir -p "%{{buildroot}}"
tar -xzf "%{{SOURCE0}}" -C "%{{buildroot}}"

%post
{hook}
%postun
{hook}
%files
%defattr(-,root,root,-)
/usr/bin/bandix-gui
/usr/share/applications/bandix-gui.desktop
/usr/share/icons/hicolor/*/apps/bandix-gui.png
%doc /usr/share/doc/bandix-gui
''')
    run('rpmbuild', '-bb', '--target', info['architecture'], '--define', f'_topdir {top}',
        '--define', '_build_id_links none', '--define', 'debug_package %{nil}', str(spec))
    packages = list((top / 'RPMS').rglob('*.rpm'))
    if len(packages) != 1:
        raise RuntimeError(f'Expected one RPM, found {packages}')
    shutil.move(packages[0], output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--build-info', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--format', choices=['all', 'deb', 'rpm'], required=True)
    args = parser.parse_args()
    info = json.loads(args.build_info.read_text())
    metadata = json.loads((ROOT / 'packaging/linux/package.json').read_text())
    if not re.fullmatch(r'\d+\.\d+\.\d+', info['version']):
        raise ValueError('Native packages currently require a numeric x.y.z version')
    for value in metadata.values():
        if any(character in value for character in '\r\n%'):
            raise ValueError('Package metadata must be a single line without RPM macro syntax')
    work = args.output_dir.resolve() / 'native-packages'
    work.mkdir()
    stage = work / 'root'
    stage_files(stage, args.binary, info, metadata, work / 'abi.json')
    name = f"bandix-{info['version']}-linux-{info['architecture']}"
    if args.format in ['all', 'deb']:
        build_deb(stage, work, args.output_dir / f'{name}.deb', info, metadata)
    if args.format in ['all', 'rpm']:
        build_rpm(stage, work, args.output_dir / f'{name}.rpm', info, metadata)


if __name__ == '__main__':
    main()
