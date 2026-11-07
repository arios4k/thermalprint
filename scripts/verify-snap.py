#!/usr/bin/env python3
"""Inspect a built Snap without installing or executing its payload."""
import argparse
from pathlib import Path
import subprocess
import tomllib
import yaml

ROOT = Path(__file__).resolve().parent.parent
REQUIRED_FILES = {
    'bin/thermalprint', 'bin/thermalprint-launch',
    'usr/bin/lp', 'usr/bin/lpstat', 'usr/bin/lpoptions',
    'usr/bin/pdfinfo', 'usr/bin/pdftoppm',
    'usr/share/mime/packages/org.thermalprint.ThermalPrint.xml',
    'usr/share/doc/thermalprint/register-mime.sh',
    'usr/share/licenses/thermalprint/LICENSE',
    'usr/share/licenses/thermalprint/FONT-LICENSE.txt',
    'meta/gui/icon.png', 'meta/gui/thermalprint.desktop',
}
FORBIDDEN_INTERFACES = {'cups-control', 'raw-usb', 'system-files', 'personal-files', 'snapd-control'}


def validate_metadata(metadata, version):
    if metadata.get('name') != 'thermalprint' or str(metadata.get('version')) != version:
        raise ValueError('Snap name/version must match the ThermalPrint Cargo manifest')
    if metadata.get('base') != 'core24' or metadata.get('confinement') != 'strict':
        raise ValueError('Snap must use core24 and strict confinement')
    app = metadata.get('apps', {}).get('thermalprint', {})
    if app.get('command') != 'bin/thermalprint-launch':
        raise ValueError('Snap must use the tested launcher')
    if 'cups' not in app.get('plugs', []):
        raise ValueError('Missing printing-only cups interface')
    plugs = metadata.get('plugs', {})
    interfaces = set(app.get('plugs', []))
    for name, value in plugs.items():
        interfaces.add(value.get('interface', name) if isinstance(value, dict) else name)
    if interfaces & FORBIDDEN_INTERFACES:
        raise ValueError(f'Unexpected privileged interfaces: {interfaces & FORBIDDEN_INTERFACES}')


def verify(package):
    metadata = yaml.safe_load(subprocess.check_output(['unsquashfs', '-cat', str(package), 'meta/snap.yaml'], text=True))
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    validate_metadata(metadata, version)
    listing = subprocess.check_output(['unsquashfs', '-ll', str(package)], text=True)
    files = {line.split('squashfs-root/', 1)[1].split(' -> ', 1)[0] for line in listing.splitlines() if 'squashfs-root/' in line}
    missing = REQUIRED_FILES - files
    if missing:
        raise ValueError(f'Missing Snap payload files: {sorted(missing)}')
    for command in metadata['apps']['thermalprint'].get('command-chain', []):
        if command not in files:
            raise ValueError(f'Missing desktop/GPU command-chain helper: {command}')
    desktop = subprocess.check_output(['unsquashfs', '-cat', str(package), 'meta/gui/thermalprint.desktop'], text=True)
    if 'application/x-thermalprint;' not in desktop or '%f' not in desktop:
        raise ValueError('Snap desktop launcher must advertise saved label handling')
    print(f'Verified {package.name}: ThermalPrint {version}, strict core24, printing tools, desktop files, and licenses.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('package', type=Path)
    args = parser.parse_args()
    verify(args.package)
