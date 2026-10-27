#!/usr/bin/env python3
"""Check the required (not optional WEAK) glibc ABI of every bundled ELF."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys


def version_key(version):
    return tuple(int(part) for part in version.split('.'))


def glibc_versions(output):
    required, weak = set(), set()
    in_needs = False
    for line in output.splitlines():
        if line.startswith('Version '):
            in_needs = line.startswith('Version needs section')
        if not in_needs:
            continue
        match = re.search(r'Name: GLIBC_(\d+(?:\.\d+)+)\s+Flags: (\S+)', line)
        if match:
            (weak if match[2] == 'WEAK' else required).add(match[1])
        else:
            special = re.search(r'Name: (GLIBC_\S+)\s+Flags: (\S+)', line)
            if special and special[2] != 'WEAK':
                # E.g. GLIBC_ABI_DT_RELR cannot be compared as a numeric version.
                raise ValueError(f'Unsupported glibc ABI requirement: {special[1]}')
    return required, weak


def inspect_bundle(root):
    root = Path(root).resolve(strict=True)
    files, seen = [], set()
    for path in sorted(root.rglob('*')):
        if not path.is_file():
            continue
        resolved = path.resolve()
        if not resolved.is_relative_to(root):
            raise ValueError(f'Bundle symlink points outside its directory: {path}')
        if resolved in seen:
            continue
        seen.add(resolved)
        with path.open('rb') as source:
            if source.read(4) != b'\x7fELF':
                continue
        output = subprocess.check_output(
            ['readelf', '--wide', '--version-info', str(path)],
            text=True, env=dict(os.environ, LC_ALL='C'),
        )
        required, weak = glibc_versions(output)
        files.append({
            'path': str(path.relative_to(root)),
            'required_glibc': max(required, key=version_key) if required else None,
            'optional_glibc': sorted(weak, key=version_key),
        })
    versions = [entry['required_glibc'] for entry in files if entry['required_glibc']]
    if not versions:
        raise ValueError('No required glibc versions found in bundle')
    return {'minimum_glibc': max(versions, key=version_key), 'files': files}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--max-glibc', default='2.35')
    args = parser.parse_args()
    try:
        report = inspect_bundle(args.directory)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'Cannot verify glibc compatibility: {error}', file=sys.stderr)
        return 1
    report['glibc_baseline'] = args.max_glibc
    print(json.dumps(report, indent=2))
    failures = [entry for entry in report['files'] if entry['required_glibc']
                and version_key(entry['required_glibc']) > version_key(args.max_glibc)]
    for entry in failures:
        print(f"{entry['path']} requires glibc {entry['required_glibc']}, "
              f"exceeding release baseline {args.max_glibc}", file=sys.stderr)
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main())
