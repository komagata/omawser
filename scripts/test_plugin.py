#!/usr/bin/env python3
"""Run only inside an isolated omabox; exercises the real Omarchy shell IPC."""
import json
from pathlib import Path
import shutil
import subprocess
import time
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ID = 'komagata.omawser'


def command(*args, check=True):
    result = subprocess.run(args, text=True, capture_output=True, timeout=12)
    if check and result.returncode:
        raise AssertionError(f'{args!r} exited {result.returncode}:\n{result.stdout}\n{result.stderr}')
    return result


def wait_for(predicate, message, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.04)
    raise AssertionError(message)


class PluginTest(unittest.TestCase):
    def test_service_lifecycle(self):
        self.assertTrue((ROOT / 'manifest.json').is_file(), 'root plugin manifest is missing')
        home = Path.home()
        self.assertEqual(str(home), '/home/sbx', 'Run this test inside omabox only')
        plugins = home / '.config/omarchy/plugins'
        plugins.mkdir(parents=True, exist_ok=True)
        installed = plugins / ID
        staging = Path(tempfile.mkdtemp(prefix='plugin-fixture-', dir=home))
        command('git', 'init', '--quiet', str(staging))
        # The QML entrypoint itself has these characters, not only a symlink target.
        payload = "fixture space ; $dollar 'quote #percent%"
        source = staging / payload
        source.mkdir()
        manifest = json.loads((ROOT / 'manifest.json').read_text())
        manifest['entryPoints']['service'] = payload + '/plugin/Service.qml'
        (staging / 'manifest.json').write_text(json.dumps(manifest))
        shutil.copytree(ROOT / 'plugin', source / 'plugin')
        shutil.copytree(ROOT / 'scripts', source / 'scripts', ignore=shutil.ignore_patterns('__pycache__'))
        binary = source / 'src-tauri/target/release/omawser'
        executable = home / 'fictional-browser'
        binary.parent.mkdir(parents=True)
        binary.symlink_to(executable)
        launches = home / 'launches.jsonl'
        launches.unlink(missing_ok=True)
        gate = home / 'probe-gate'
        probe_started = home / 'probe-started'
        bin_dir = home / '.local/bin'
        bin_dir.mkdir(parents=True, exist_ok=True)
        shutil.copy(ROOT / 'test/plugin/test-probe.py', bin_dir / 'test')
        (bin_dir / 'test').chmod(0o755)
        self.assertEqual(shutil.which('test'), str(bin_dir / 'test'),
                         'box PATH must resolve the fictional executable probe')
        profile = home / '.local/share/omawser/webkit/fictional-cookie'
        profile.parent.mkdir(parents=True)
        profile.write_text('fictional retained data')
        staging.rename(installed)
        source = installed / payload
        binary = source / 'src-tauri/target/release/omawser'

        def ipc(method, check=True):
            return command('omarchy-shell', ID, method, check=check)

        def status():
            result = ipc('status', check=False)
            try:
                return json.loads(result.stdout)
            except ValueError:
                return {}

        def state(value):
            return wait_for(lambda: status().get('state') == value, 'expected state ' + value)

        def count():
            return len(launches.read_text().splitlines()) if launches.exists() else 0

        def enable():
            command('omarchy-shell', 'shell', 'rescanPlugins')
            wait_for(lambda: any(plugin.get('id') == ID for plugin in json.loads(
                command('omarchy-shell', 'shell', 'listPlugins').stdout)),
                'plugin discovery did not finish')
            command('omarchy', 'plugin', 'enable', ID)
            stable_since = None

            def registered_stably():
                nonlocal stable_since
                if status().get('state') in ('unbuilt', 'ready'):
                    if stable_since is None:
                        stable_since = time.monotonic()
                    return time.monotonic() - stable_since >= 0.5
                stable_since = None
                return False

            wait_for(registered_stably, 'service IPC did not settle after installation')

        def disable():
            command('omarchy', 'plugin', 'disable', ID)
            wait_for(lambda: ipc('status', check=False).returncode != 0, 'IPC survived disable')

        try:
            enable()
            state('unbuilt')
            self.assertIn('build', status()['message'].lower())
            self.assertEqual(count(), 0, 'enable must not launch the browser')
            self.assertEqual(ipc('open').stdout.strip(), 'checking')
            state('unbuilt')
            self.assertEqual(count(), 0)
            # Build outside the watched clone; its preinstalled symlink preserves argv.
            shutil.copy(ROOT / 'test/plugin/browser.py', executable)
            executable.chmod(0o755)
            self.assertEqual(ipc('refresh').stdout.strip(), 'checking')
            state('ready')
            self.assertEqual(count(), 0, 'refresh must not launch the browser')
            self.assertEqual(ipc('open').stdout.strip(), 'checking')
            wait_for(lambda: count() == 1, 'open did not launch exactly once')
            wait_for(lambda: status().get('lastAction') == 'launch-requested', 'missing acknowledgement')
            record = json.loads(launches.read_text().splitlines()[0])
            self.assertEqual(len(record['argv']), 1)
            self.assertEqual(Path(record['argv'][0]).resolve(), binary.resolve())
            self.assertIn(payload, record['argv'][0])
            self.assertEqual(Path(record['binary']).resolve(), binary.resolve())
            # Every open probes again; a removed build must not use cached readiness.
            executable.chmod(0o644)
            ipc('open')
            state('unbuilt')
            self.assertEqual(count(), 1)
            executable.chmod(0o755)
            # The PATH fixture gates the real executable test, giving deterministic overlap.
            gate.touch()
            probe_started.unlink(missing_ok=True)
            ipc('open')
            wait_for(probe_started.exists, 'probe fixture not used; check box HOME/.local/bin on PATH')
            self.assertEqual(ipc('open').stdout.strip(), 'busy')
            self.assertEqual(ipc('refresh').stdout.strip(), 'busy')
            gate.unlink()
            wait_for(lambda: count() == 2, 'accepted open did not launch')
            state('ready')
            gate.touch()
            probe_started.unlink(missing_ok=True)
            ipc('open')
            wait_for(probe_started.exists, 'probe did not start')
            disable()
            gate.unlink()
            time.sleep(0.3)
            self.assertEqual(count(), 2, 'disabled service launched late')
            self.assertTrue(Path('/proc/' + str(record['pid'])).exists(), 'detached app did not survive disable')
            enable()
            state('ready')
            self.assertEqual(count(), 2, 're-enable launched automatically')
            # A stalled probe is bounded and does not launch later.
            gate.touch()
            ipc('open')
            state('failed')
            gate.unlink()
            wait_for(lambda: not status().get('busy'), 'timed-out probe did not drain')
            self.assertEqual(ipc('refresh').stdout.strip(), 'checking')
            state('ready')
            self.assertEqual(count(), 2, 'timeout recovery launched without open')
            gate.touch()
            probe_started.unlink(missing_ok=True)
            ipc('open')
            wait_for(probe_started.exists, 'removal probe did not start')
            command('omarchy', 'plugin', 'remove', ID, '--yes')
            gate.unlink()
            wait_for(lambda: ipc('status', check=False).returncode != 0, 'IPC survived removal')
            time.sleep(0.3)
            self.assertEqual(count(), 2, 'removed service launched late')
            self.assertFalse(installed.exists())
            self.assertEqual(profile.read_text(), 'fictional retained data')
            self.assertTrue(Path('/proc/' + str(record['pid'])).exists())
        finally:
            gate.unlink(missing_ok=True)
            command('omarchy', 'plugin', 'disable', ID, check=False)
            if installed.exists():
                shutil.rmtree(installed)
            command('omarchy-shell', 'shell', 'rescanPlugins', check=False)


if __name__ == '__main__':
    unittest.main()
