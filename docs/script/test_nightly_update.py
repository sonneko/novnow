"""Offline tests for the standalone my-novels nightly updater."""
import copy
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest import mock
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('nightly_update', ROOT / 'scripts/update-novnow-nightly.py')
updater = importlib.util.module_from_spec(spec)
spec.loader.exec_module(updater)


class FakeGitHub:
    def __init__(self, unsafe=False):
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode='w:gz') as archive:
            member = tarfile.TarInfo('novnow' if not unsafe else '../novnow')
            body = b'fake executable'
            member.size = len(body)
            archive.addfile(member, io.BytesIO(body))
        vsix = io.BytesIO()
        with zipfile.ZipFile(vsix, 'w') as archive:
            archive.writestr('extension/package.json', json.dumps({
                'publisher': 'sonneko', 'name': 'novnow', 'version': '0.1.1',
            }))
        self.files = {
            updater.BINARY_ASSET: buffer.getvalue(), updater.VSIX_ASSET: vsix.getvalue(),
            'manifest.json': json.dumps({'channel': 'nightly', 'sha': 'a' * 40, 'run': '123'}).encode(),
        }
        self.files['SHA256SUMS'] = ''.join(
            f'{hashlib.sha256(body).hexdigest()}  {name}\n' for name, body in self.files.items()
        ).encode()
        self.metadata = {'id': 1, 'tag_name': 'nightly', 'draft': False, 'assets': [
            {'id': index, 'name': name, 'size': len(body)}
            for index, (name, body) in enumerate(self.files.items(), 1)
        ]}
        self.after = None
        self.calls = 0

    def release(self):
        self.calls += 1
        return copy.deepcopy(self.after if self.calls > 1 and self.after else self.metadata)

    def download(self, asset_id, output):
        asset = next(asset for asset in self.metadata['assets'] if asset['id'] == asset_id)
        output.write_bytes(self.files[asset['name']])


class UpdaterTests(unittest.TestCase):
    def test_complete_pair_has_verified_source_and_executable(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            self.assertEqual(updater.fetch_bundle(directory, FakeGitHub()), 'a' * 40)
            self.assertEqual((directory / 'novnow').read_bytes(), b'fake executable')
            self.assertEqual((directory / 'novnow').stat().st_mode & 0o777, 0o755)
            self.assertEqual(updater.verify_vsix(directory), '0.1.1')

    def test_checksum_mismatch_does_not_extract_binary(self):
        github = FakeGitHub()
        body = github.files[updater.BINARY_ASSET]
        github.files[updater.BINARY_ASSET] = b'X' + body[1:]
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            with self.assertRaisesRegex(updater.UpdateError, 'Checksum mismatch'):
                updater.fetch_bundle(directory, github)
            self.assertFalse((directory / 'novnow').exists())

    def test_replaced_release_is_rejected_even_if_all_hashes_match(self):
        github = FakeGitHub()
        github.after = dict(github.metadata, id=2)
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(updater.UpdateError, 'changed during download'):
                updater.fetch_bundle(Path(tmp), github)

    def test_archive_traversal_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            with self.assertRaisesRegex(updater.UpdateError, 'Unsafe archive'):
                updater.fetch_bundle(directory, FakeGitHub(unsafe=True))
            self.assertFalse((directory / 'novnow').exists())

    def test_failed_vsix_install_keeps_current_and_success_keeps_previous(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            old, new = root / 'old set', root / 'new set'
            old.mkdir()
            new.mkdir()
            (root / 'current').symlink_to(old.name)
            failed = mock.Mock(side_effect=subprocess.CalledProcessError(1, ['code']))
            with self.assertRaises(subprocess.CalledProcessError):
                updater.activate(root, new, 'code', run=failed)
            self.assertEqual((root / 'current').resolve(), old)
            self.assertFalse((root / 'previous').exists())
            install = mock.Mock()
            updater.activate(root, new, 'code-insiders', 'Novel profile', run=install)
            self.assertEqual((root / 'current').resolve(), new)
            self.assertEqual((root / 'previous').resolve(), old)
            install.assert_called_once_with([
                'code-insiders', '--install-extension', str(new / updater.VSIX_ASSET),
                '--force', '--profile', 'Novel profile',
            ], check=True)

    def test_download_only_does_not_install_execute_or_switch(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'current').symlink_to('old')
            with mock.patch.object(updater, 'GitHub', return_value=FakeGitHub()), \
                 mock.patch.object(updater.platform, 'system', return_value='Linux'), \
                 mock.patch.object(updater.platform, 'machine', return_value='x86_64'), \
                 mock.patch.object(updater.shutil, 'which', return_value='/bin/gh'), \
                 mock.patch.object(updater.subprocess, 'run') as run, \
                 mock.patch.object(updater.subprocess, 'check_output') as execute:
                self.assertEqual(updater.main(['--install-dir', tmp, '--download-only']), 0)
                run.assert_not_called()
                execute.assert_not_called()
            self.assertEqual((root / 'current').readlink(), Path('old'))

    def test_incomplete_release_is_rejected(self):
        github = FakeGitHub()
        github.metadata['assets'].pop()
        with self.assertRaisesRegex(updater.UpdateError, 'missing required assets'):
            updater.selected_assets(github.metadata)
