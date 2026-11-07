"""Snap packaging checks; no snapd, Store credentials, or printer is required."""
import configparser
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

import yaml

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('verify_snap', ROOT / 'scripts/verify-snap.py')
verify_snap = importlib.util.module_from_spec(spec)
spec.loader.exec_module(verify_snap)


class SnapPackagingTests(unittest.TestCase):
    def test_recipe_is_confined_and_does_not_request_printer_administration(self):
        recipe = yaml.safe_load((ROOT / 'snap/snapcraft.yaml').read_text())
        self.assertEqual(recipe['base'], 'core24')
        self.assertEqual(recipe['confinement'], 'strict')
        self.assertEqual(recipe['grade'], 'stable')
        self.assertEqual(recipe['adopt-info'], 'thermalprint')
        app = recipe['apps']['thermalprint']
        self.assertIn('gnome', app['extensions'])
        self.assertIn('cups', app['plugs'])
        self.assertNotIn('cups-control', app['plugs'])
        part = recipe['parts']['thermalprint']
        self.assertEqual(part['plugin'], 'rust')
        self.assertEqual(part['rust-channel'], 'stable')
        self.assertTrue({'cups-client', 'poppler-utils'} <= set(part['stage-packages']))
        self.assertIn('cargo test --release --locked', part['override-build'])
        self.assertEqual(recipe['plugs']['cups-install']['default-provider'], 'cups')

    def run_launcher(self, server=None):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'bin').mkdir()
            binary = root / 'bin/thermalprint'
            binary.write_text('#!/usr/bin/python3\nimport json, os, sys\nprint(json.dumps({"args":sys.argv[1:], "server":os.environ["CUPS_SERVER"], "path":os.environ["PATH"], "data":os.environ["XDG_DATA_DIRS"], "browser":os.environ["BROWSER"]}))\n')
            binary.chmod(0o755)
            env = dict(os.environ, SNAP=str(root), XDG_DATA_DIRS='/existing/data')
            env.pop('CUPS_SERVER', None)
            env.pop('BROWSER', None)
            if server is not None:
                env['CUPS_SERVER'] = server
            args = ['label with spaces.thermalprint', 'literal;$value']
            result = subprocess.check_output(['sh', str(ROOT / 'snap/local/thermalprint-launch'), *args], env=env, text=True)
            output = json.loads(result)
            self.assertEqual(output['args'], args)
            self.assertEqual(output['browser'], 'gio open')
            self.assertTrue(output['path'].startswith(f'{root}/bin:{root}/usr/bin:'))
            self.assertEqual(output['data'], f'{root}/usr/share:/existing/data')
            return output

    def test_launcher_uses_cups_proxy_and_preserves_arguments(self):
        self.assertEqual(self.run_launcher()['server'], '/var/cups/cups.sock')

    def test_launcher_preserves_an_explicit_print_server(self):
        self.assertEqual(self.run_launcher('print.example:631')['server'], 'print.example:631')

    def test_verifier_rejects_unconfined_and_privileged_packages(self):
        metadata = {'name': 'thermalprint', 'version': '0.4.0', 'base': 'core24', 'confinement': 'strict',
                    'apps': {'thermalprint': {'command': 'bin/thermalprint-launch', 'plugs': ['cups']}}, 'plugs': {}}
        verify_snap.validate_metadata(metadata, '0.4.0')
        metadata['confinement'] = 'classic'
        with self.assertRaisesRegex(ValueError, 'strict'):
            verify_snap.validate_metadata(metadata, '0.4.0')
        metadata['confinement'] = 'strict'
        metadata['plugs']['innocent-looking-name'] = {'interface': 'cups-control'}
        with self.assertRaisesRegex(ValueError, 'privileged'):
            verify_snap.validate_metadata(metadata, '0.4.0')

    def test_host_mime_helper_refuses_to_run_inside_a_snap(self):
        with tempfile.TemporaryDirectory() as temporary:
            env = dict(os.environ, SNAP='/snap/thermalprint/current', XDG_DATA_HOME=temporary)
            result = subprocess.run(['sh', str(ROOT / 'snap/local/register-mime.sh')], env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('normal desktop terminal', result.stderr)
            self.assertEqual(list(Path(temporary).iterdir()), [])

    @unittest.skipUnless(shutil.which('update-mime-database') and shutil.which('xdg-mime'), 'requires desktop MIME tools')
    def test_optional_host_setup_registers_only_the_design_type_for_the_snap(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            snap_root = root / 'thermalprint/current'
            packages = snap_root / 'usr/share/mime/packages'
            packages.mkdir(parents=True)
            shutil.copy2(ROOT / 'data/org.thermalprint.ThermalPrint.xml', packages)
            data = root / 'data'
            applications = data / 'applications'
            applications.mkdir(parents=True)
            desktop_id = 'thermalprint_thermalprint.desktop'
            shutil.copy2(ROOT / 'data/org.thermalprint.ThermalPrint.desktop', applications / desktop_id)
            env = dict(os.environ, XDG_DATA_HOME=str(data), XDG_CONFIG_HOME=str(root / 'config'), XDG_CURRENT_DESKTOP='', DE='generic')
            env.pop('SNAP', None)
            subprocess.run(['sh', str(ROOT / 'snap/local/register-mime.sh'), str(snap_root)], env=env, check=True, capture_output=True)
            label = root / 'a label.thermalprint'
            label.write_text('{"version":1}')
            mime = subprocess.check_output(['xdg-mime', 'query', 'filetype', str(label)], env=env, text=True).strip()
            self.assertEqual(mime, 'application/x-thermalprint')
            default = subprocess.check_output(['xdg-mime', 'query', 'default', mime], env=env, text=True).strip()
            self.assertEqual(default, desktop_id)
            config = configparser.ConfigParser(interpolation=None)
            config.read(root / 'config/mimeapps.list')
            self.assertEqual(set(config['Default Applications']), {'application/x-thermalprint'})


if __name__ == '__main__':
    unittest.main()
