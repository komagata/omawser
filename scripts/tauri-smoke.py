#!/usr/bin/env python3
"""Build and exercise the feature-gated native smoke against an isolated HTTP fixture."""
import http.server
import argparse
import os
from pathlib import Path
import subprocess
import signal
import tempfile
import threading

REPO = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--binary', type=Path, help='Use a prebuilt gui-smoke executable (no Cargo needed inside omabox).')
parser.add_argument('--timeout', type=int, default=90, help='Maximum seconds per smoke phase.')
args = parser.parse_args()
if args.binary:
    binary = args.binary.resolve(strict=True)
else:
    subprocess.run([os.environ.get('CARGO', str(Path.home() / '.cargo/bin/cargo')), 'build', '--offline', '--locked', '--manifest-path', str(REPO / 'src-tauri/Cargo.toml'), '--features', 'gui-smoke'], check=True)
    binary = REPO / 'src-tauri/target/debug/omawser'
with tempfile.TemporaryDirectory(prefix='omawser-tauri-smoke-', dir='/tmp') as tmp:
    root = Path(tmp)
    env = os.environ.copy()
    home = root / 'home'
    downloads = root / 'downloads'
    home.mkdir()
    downloads.mkdir()
    env['HOME'] = str(home)
    env['XDG_DOWNLOAD_DIR'] = str(downloads)
    for kind in ('CONFIG', 'DATA', 'CACHE', 'STATE'):
        directory = root / kind.lower()
        directory.mkdir()
        env[f'XDG_{kind}_HOME'] = str(directory)
    theme = root / 'state/omarchy/current/theme'
    theme.mkdir(parents=True)
    (theme / 'colors.toml').write_text('accent = "#123456"\nmode = "light"\n')
    env['OMA_SMOKE_ROOT'] = str(root)

    class Fixture(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            if self.path == '/always-fail' or (self.path == '/retry' and not (root / 'recover').exists()):
                self.connection.close()
                return
            if self.path == '/download':
                content = b'oma fixture download\n'
                self.send_response(200)
                self.send_header('Content-Type', 'application/octet-stream')
                self.send_header('Content-Disposition', 'attachment; filename=fixture.txt')
                self.send_header('Content-Length', str(len(content)))
                self.end_headers()
                self.wfile.write(content)
                return
            if self.path == '/redirect':
                self.send_response(302)
                self.send_header('Location', 'file:///etc/passwd')
                self.end_headers()
                return
            content = f'''<!doctype html><title>{self.path}</title><input id="typing" autofocus><style>body {{ min-height:4000px; }} #background-link {{ position:fixed; left:20px; top:100px; }}</style>
<script>
window.escapeCount = 0;
addEventListener('keydown', e => {{
 if(e.key === 'Escape') escapeCount++;
 if(e.ctrlKey && e.key.toLowerCase() === 'l') {{ e.preventDefault(); e.stopImmediatePropagation(); }}
}}, true);
</script><a id="background-link" href="/two">two</a>'''.encode()
            self.send_response(200)
            self.send_header('Content-Type', 'text/html')
            self.send_header('Content-Length', str(len(content)))
            self.end_headers()
            self.wfile.write(content)
        def log_message(self, *_):
            pass

    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Fixture)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        for phase in ('full', 'persistence'):
            env['OMA_SMOKE_PHASE'] = phase
            log_path = root / f'{phase}.log'
            print(f'native {phase} log: {log_path}', flush=True)
            timed_out = False
            with log_path.open('w') as log:
                process = subprocess.Popen([str(binary), '--gui-smoke', f'http://127.0.0.1:{server.server_port}/one'], cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                try:
                    process.wait(timeout=args.timeout)
                except subprocess.TimeoutExpired:
                    timed_out = True
                finally:
                    # This group belongs only to the launched test, including WebKit's children.
                    try:
                        os.killpg(process.pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait(timeout=5)
            output = log_path.read_text(errors='replace')
            print(output, flush=True)
            if timed_out or process.returncode or 'OMA_SMOKE_OK' not in output:
                raise SystemExit(f'native {phase} smoke failed (exit={process.returncode}, timeout={timed_out})')
    finally:
        server.shutdown()
