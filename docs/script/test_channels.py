"""Channel publication preserves stable files; publication scripts fail closed."""
import importlib.util
import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('channel_pages', ROOT / 'scripts/channel-pages.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class Channels(unittest.TestCase):
    def test_nightly_preserves_stable_and_removes_stale_assets(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            previous, incoming, output = [base / name for name in ('previous', 'incoming', 'output')]
            for path, content in {
                previous / 'index.html': 'official website',
                previous / 'app/index.html': 'official app',
                previous / 'stable-build.json': '{"sha":"stable","run":1}',
                previous / 'nightly-app/index.html': 'old nightly',
                previous / 'nightly-app/old.wasm': 'stale',
                incoming / 'index.html': 'new nightly',
                incoming / 'new.wasm': 'new',
            }.items():
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            module.assemble('nightly', incoming, previous, output, 'new-sha', '2')
            for name in ('index.html', 'app/index.html', 'stable-build.json'):
                self.assertEqual((output / name).read_bytes(), (previous / name).read_bytes())
            self.assertFalse((output / 'nightly-app/old.wasm').exists())
            self.assertEqual((output / 'nightly-app/index.html').read_text(), 'new nightly')
            self.assertEqual(json.loads((output / 'nightly-build.json').read_text())['sha'], 'new-sha')

    def test_stable_resets_both_apps_to_identical_bytes(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            incoming = root / 'incoming'
            (incoming / 'app').mkdir(parents=True)
            (incoming / 'index.html').write_text('site')
            (incoming / 'app/index.html').write_text('release')
            (incoming / 'app/a.wasm').write_bytes(b'wasm')
            module.assemble('stable', incoming, root / 'unused', root / 'out', 'sha', '4')
            for path in (root / 'out/app').iterdir():
                self.assertEqual(path.read_bytes(), (root / 'out/nightly-app' / path.name).read_bytes())

    def test_missing_bootstrap_is_not_published_as_a_nightly_only_site(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            with self.assertRaises(ValueError):
                module.assemble('nightly', root, root / 'absent', root / 'out', 'sha', '1')
            self.assertFalse((root / 'out').exists())

    def test_workflows_share_non_cancellable_publication_and_no_registry_upload(self):
        publisher = (ROOT / '.github/workflows/channel-publish.yml').read_text()
        self.assertIn('group: novnow-channel-publication', publisher)
        self.assertIn('cancel-in-progress: false', publisher)
        self.assertIn('actions/deploy-pages@v4', publisher)
        for name in ('nightly.yml', 'deploy.yml'):
            self.assertIn('./.github/workflows/channel-publish.yml', (ROOT / '.github/workflows' / name).read_text())
        nightly = (ROOT / '.github/workflows/nightly.yml').read_text()
        self.assertNotIn('publish-crates', nightly)
        self.assertNotIn('pypi-publish', nightly)
        self.assertIn("github.event_name != 'pull_request'", nightly)
        script = (ROOT / 'scripts/publish-channel.sh').read_text()
        self.assertLess(script.index('sha256sum --check'), script.index('gh release delete'))
        self.assertIn('--prerelease --latest=false', script)
        self.assertIn('git push origin "$commit:refs/heads/gh-pages"', script)
        self.assertNotIn('git push --force', script)
        subprocess.run(['bash', '-n', str(ROOT / 'scripts/publish-channel.sh')], check=True)

    def test_worker_cache_cleanup_is_limited_to_registration_scope(self):
        worker = (ROOT / 'sw.js').read_text()
        self.assertIn('encodeURIComponent(self.registration.scope)', worker)
        self.assertIn('key.startsWith(SCOPE_CACHE)', worker)
        self.assertNotIn("key.startsWith('novnow-offline-')", worker)

    def test_publisher_commits_full_tree_without_switching_source_checkout(self):
        import os
        import shutil
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            remote, source, runner = [root / name for name in ('remote.git', 'source', 'runner')]
            runner.mkdir()
            def git(*args, cwd=None):
                return subprocess.check_output(['git', *args], cwd=cwd, stderr=subprocess.DEVNULL, text=True).strip()
            git('init', '--bare', str(remote))
            git('init', str(source))
            git('config', 'user.name', 'Test', cwd=source)
            git('config', 'user.email', 'test@example.invalid', cwd=source)
            git('remote', 'add', 'origin', str(remote), cwd=source)
            (source / 'source.txt').write_text('do not touch')
            git('add', '.', cwd=source)
            git('commit', '-m', 'source', cwd=source)
            sha = git('rev-parse', 'HEAD', cwd=source)
            git('push', 'origin', 'HEAD:refs/heads/main', cwd=source)
            (source / 'scripts').mkdir()
            shutil.copy(ROOT / 'scripts/channel-pages.py', source / 'scripts')
            site = root / 'payload/site'
            (site / 'app').mkdir(parents=True)
            (site / 'index.html').write_text('site')
            (site / 'app/index.html').write_text('app')
            env = dict(os.environ, RUNNER_TEMP=str(runner), GITHUB_OUTPUT=str(root / 'outputs'))
            subprocess.run(['bash', str(ROOT / 'scripts/publish-channel.sh'), 'stable', str(root / 'payload'), sha, '10'],
                           cwd=source, env=env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            self.assertEqual(git('rev-parse', 'HEAD', cwd=source), sha)
            self.assertEqual((source / 'source.txt').read_text(), 'do not touch')
            files = git('--git-dir=' + str(remote), 'ls-tree', '-r', '--name-only', 'gh-pages').splitlines()
            self.assertEqual(files, ['.nojekyll', 'app/index.html', 'index.html', 'nightly-app/index.html', 'stable-build.json'])
