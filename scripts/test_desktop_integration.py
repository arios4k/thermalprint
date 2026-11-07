"""Verify native label file recognition without changing the user's desktop settings."""
from pathlib import Path
import configparser
import os
import shutil
import subprocess
import tempfile
import tomllib
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent
MIME = 'application/x-thermalprint'


class DesktopIntegrationTests(unittest.TestCase):
    def test_desktop_and_packages_advertise_design_mime_type(self):
        desktop = configparser.ConfigParser(interpolation=None)
        desktop.read(ROOT / 'data/org.thermalprint.ThermalPrint.desktop')
        entry = desktop['Desktop Entry']
        self.assertEqual(entry['Exec'], 'thermalprint %f')
        self.assertIn(MIME, entry['MimeType'].split(';'))
        manifest = tomllib.loads((ROOT / 'Cargo.toml').read_text())
        assets = manifest['package']['metadata']['deb']['assets']
        self.assertIn(['data/org.thermalprint.ThermalPrint.xml', 'usr/share/mime/packages/', '644'], assets)
        rpm = (ROOT / 'packaging/rpm/thermalprint.spec.in').read_text()
        self.assertIn('%{_datadir}/mime/packages/org.thermalprint.ThermalPrint.xml', rpm)
        root = ET.parse(ROOT / 'data/org.thermalprint.ThermalPrint.xml').getroot()
        ns = {'mime': 'http://www.freedesktop.org/standards/shared-mime-info'}
        mime = root.find('mime:mime-type', ns)
        self.assertEqual(mime.attrib['type'], MIME)
        self.assertEqual(mime.find('mime:glob', ns).attrib['pattern'], '*.thermalprint')

    @unittest.skipUnless(shutil.which('update-mime-database') and shutil.which('xdg-mime'), 'requires shared-mime-info and xdg-utils')
    def test_desktop_recognizes_design_without_treating_all_json_as_labels(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            data = root / 'share'
            packages = data / 'mime/packages'
            packages.mkdir(parents=True)
            shutil.copy2(ROOT / 'data/org.thermalprint.ThermalPrint.xml', packages)
            env = dict(os.environ, XDG_DATA_HOME=str(data), XDG_CONFIG_HOME=str(root / 'config'))
            # Use the shared-mime-info backend consistently, independent of the desktop session.
            env['XDG_CURRENT_DESKTOP'] = ''
            env['DE'] = 'generic'
            subprocess.run(['update-mime-database', str(data / 'mime')], env=env, check=True, capture_output=True)
            for filename in ('label.thermalprint', 'label with spaces.thermalprint', 'LABEL.THERMALPRINT'):
                path = root / filename
                path.write_text('{"version": 1, "title": "Example"}')
                detected = subprocess.check_output(['xdg-mime', 'query', 'filetype', str(path)], env=env, text=True).strip()
                self.assertEqual(detected, MIME)
            path = root / 'other.json'
            path.write_text('{"title": "Not a label"}')
            detected = subprocess.check_output(['xdg-mime', 'query', 'filetype', str(path)], env=env, text=True).strip()
            self.assertNotEqual(detected, MIME)


if __name__ == '__main__':
    unittest.main()
