#!/usr/bin/env python3
"""Linux browser benchmark. JS rAF is a scheduling proxy, not input-to-photon."""
import argparse
import contextlib
import http.server
import json
import math
import os
from pathlib import Path
import secrets
import signal
import statistics
import subprocess
import tempfile
import threading
import time

MAX_BODY = 128 * 1024


def parse_stat(text):
    pid, rest = text.split(' (', 1)
    fields = rest.rsplit(') ', 1)[1].split()
    return {'pid': int(pid), 'ppid': int(fields[1]),
            'ticks': int(fields[11]) + int(fields[12]), 'starttime': int(fields[19]),
            'pgrp':int(fields[2]), 'session':int(fields[3])}


def parse_kib(text):
    values = {}
    for line in text.splitlines():
        parts = line.split()
        if len(parts) == 3 and parts[2] == 'kB':
            values[parts[0].rstrip(':')] = int(parts[1])
    return values


def tree_snapshot(root_pid, root_starttime=None):
    """Two snapshots only for CPU. Sum unique process identities, never thread RSS."""
    all_stats = {}
    unreadable_stats = []
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            all_stats[int(entry.name)] = parse_stat((entry / 'stat').read_text())
        except (OSError, ValueError, IndexError):
            unreadable_stats.append(int(entry.name))
    root = all_stats.get(root_pid)
    if root_starttime is not None and root is not None and root['starttime'] != root_starttime:
        return {}, unreadable_stats
    # A new session also identifies children orphaned when a launcher exits.
    owned = {pid for pid, record in all_stats.items() if record['session'] == root_pid}
    if root_pid in all_stats:
        owned.add(root_pid)
    while True:
        children = {pid for pid, record in all_stats.items() if record['ppid'] in owned}
        expanded = owned | children
        if expanded == owned:
            break
        owned = expanded
    records = {pid: all_stats[pid] for pid in owned}
    return records, unreadable_stats


def resources(records):
    pss, rss, failures = 0, 0, []
    for pid, identity in records.items():
        try:
            status = parse_kib(Path(f'/proc/{pid}/status').read_text())
            rollup = parse_kib(Path(f'/proc/{pid}/smaps_rollup').read_text())
            current = parse_stat(Path(f'/proc/{pid}/stat').read_text())
            if current['starttime'] != identity['starttime']:
                raise ValueError('process identity changed')
            rss += status['VmRSS']
            pss += rollup['Pss']
        except (OSError, ValueError, KeyError) as error:
            failures.append({'pid':pid, 'error':type(error).__name__ + ': ' + str(error)})
    return {'pss_kib':pss, 'rss_kib':rss, 'process_count':len(records),
            'unreadable_processes':failures, 'memory_complete':not failures and bool(records)}


def cpu_delta(before, after, elapsed, hz):
    complete = bool(before) and before.keys() == after.keys()
    ticks = 0
    for pid, record in after.items():
        prior = before.get(pid)
        if prior is None or prior['starttime'] != record['starttime'] or record['ticks'] < prior['ticks']:
            complete = False
            continue
        ticks += record['ticks'] - prior['ticks']
    return ticks / hz / elapsed * 100, complete


def page_issues(page, geometry, require_foreground=True):
    issues = []
    if require_foreground:
        if page.get('visibilityState') != 'visible' or page.get('hiddenObserved'):
            issues.append('page was background/hidden')
        if not page.get('focused') or page.get('focusLost'):
            issues.append('page was unfocused')
        if page.get('geometryChanged'):
            issues.append('viewport/DPR changed during measured phase')
    actual = (page.get('innerWidth'), page.get('innerHeight'), page.get('devicePixelRatio'))
    if actual != tuple(geometry):
        issues.append(f'viewport/DPR mismatch: {actual} != {tuple(geometry)}')
    return issues


def distribution(values):
    return {'median':statistics.median(values), 'min':min(values), 'max':max(values)} if values else None


def percentile(values, quantile):
    ordered = sorted(values)
    index = (len(ordered)-1) * quantile
    lower = math.floor(index)
    upper = math.ceil(index)
    return ordered[lower] + (ordered[upper]-ordered[lower]) * (index-lower)


def aggregate(samples):
    result = {}
    for label in dict.fromkeys(sample['label'] for sample in samples):
        matching = [sample for sample in samples if sample['label'] == label]
        valid = [sample for sample in matching if sample['valid']]
        summary = {'valid_samples':len(valid), 'invalid_samples':len(matching)-len(valid)}
        for key in ['startup_ms', 'idle_cpu_percent_one_core', 'pss_kib', 'rss_kib', 'process_count']:
            summary[key] = distribution([sample[key] for sample in valid])
        for key in ['raf_median_ms', 'raf_p95_ms', 'long_frame_count', 'frame_count',
                    'scroll_distance_px', 'elapsed_ms', 'nominal_frame_ms']:
            summary[key] = distribution([sample['scroll'][key] for sample in valid if key in sample['scroll']])
        result[label] = summary
    return result


class FixtureServer:
    def __init__(self, timeout):
        self.timeout = timeout
        self.token = '/' + secrets.token_urlsafe(32)
        self.events = {name:threading.Event() for name in ['ready', 'calibrated', 'scroll']}
        self.payloads = {}
        self.received_at = {}
        self.scroll = threading.Event()
        self.closing = threading.Event()
        fixture = Path(__file__).with_name('fixture.html').read_bytes()
        owner = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def setup(self):
                super().setup()
                self.connection.settimeout(5)

            def log_message(self, *_args):
                pass

            def reply(self, code, body=b'', kind='application/json'):
                self.send_response(code)
                self.send_header('Content-Type', kind)
                self.send_header('Content-Length', str(len(body)))
                self.send_header('Cache-Control', 'no-store')
                self.send_header('Content-Security-Policy', "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'")
                self.end_headers()
                with contextlib.suppress(OSError):
                    self.wfile.write(body)

            def do_GET(self):
                if self.path == owner.token:
                    self.reply(200, fixture, 'text/html; charset=utf-8')
                elif self.path == owner.token + '/control':
                    # The browser sends one request; no idle polling timers.
                    if owner.scroll.wait(owner.timeout) and not owner.closing.is_set():
                        self.reply(200, b'{"phase":"scroll"}')
                    else:
                        self.reply(408)
                else:
                    self.reply(404)

            def do_POST(self):
                name = self.path.removeprefix(owner.token + '/')
                if self.path != owner.token + '/' + name or name not in owner.events:
                    self.reply(404)
                    return
                try:
                    length = int(self.headers.get('Content-Length', '0'))
                    if not 0 < length <= MAX_BODY:
                        self.reply(413)
                        return
                    payload = json.loads(self.rfile.read(length))
                    received = time.monotonic()
                    if not isinstance(payload, dict):
                        raise ValueError('expected JSON object')
                except (ValueError, OSError):
                    self.reply(400)
                    return
                if owner.events[name].is_set():
                    self.reply(409)
                    return
                owner.payloads[name] = payload
                owner.received_at[name] = received
                owner.events[name].set()
                self.reply(200, b'{}')

        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.server.daemon_threads = True
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.url = f'http://127.0.0.1:{self.server.server_port}{self.token}'

    def wait(self, phase, process):
        deadline = time.monotonic() + self.timeout
        while not self.events[phase].wait(min(0.1, max(0, deadline-time.monotonic()))):
            if process.poll() is not None:
                raise RuntimeError(f'browser exited {process.returncode} before {phase}')
            if time.monotonic() >= deadline:
                raise TimeoutError(f'timed out waiting for {phase}')
        return self.payloads[phase]

    def close(self):
        self.closing.set()
        self.scroll.set()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)


def stop_owned(process, known):
    """Kill only our new process group and positively identified descendants."""
    root_identity = known.get(process.pid, {}).get('starttime')
    if root_identity is None:
        # No identity was ever captured; the Popen child alone is still ours.
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)
        return
    current, _ = tree_snapshot(process.pid, root_identity)
    known.update(current)

    def signal_group(sig):
        # Only signal a group while a known member still has its original identity.
        for pid, record in known.items():
            try:
                current = parse_stat(Path(f'/proc/{pid}/stat').read_text())
                if current['starttime'] == record['starttime'] and current['pgrp'] == process.pid:
                    with contextlib.suppress(ProcessLookupError):
                        os.killpg(process.pid, sig)
                    return
            except (OSError, ValueError):
                pass

    def signal_descendants(sig):
        for pid, record in known.items():
            try:
                if parse_stat(Path(f'/proc/{pid}/stat').read_text())['starttime'] == record['starttime']:
                    os.kill(pid, sig)
            except (OSError, ValueError):
                pass

    signal_group(signal.SIGTERM)
    signal_descendants(signal.SIGTERM)
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        pass
    current, _ = tree_snapshot(process.pid, root_identity)
    known.update(current)
    signal_group(signal.SIGKILL)
    signal_descendants(signal.SIGKILL)
    process.wait(timeout=3)


def sample(engine, run, args, geometry):
    result = {'label':engine['label'], 'run':run, 'valid':False, 'issues':[]}
    known = {}
    process = None
    server = None
    try:
        server = FixtureServer(args.timeout)
        with tempfile.TemporaryDirectory(prefix='omawser-benchmark-') as profile:
            env = os.environ.copy()
            for variable, directory in [('XDG_CONFIG_HOME','config'), ('XDG_CACHE_HOME','cache'), ('XDG_DATA_HOME','data')]:
                location = Path(profile, directory)
                location.mkdir()
                env[variable] = str(location)
            # Preserve XDG_STATE_HOME: the app reads the actual theme, no theme writes.
            result['argv'] = engine['argv'] + [server.url]
            result['cwd'] = engine.get('cwd')
            with open(Path(profile, 'browser.log'), 'wb') as log:
                try:
                    started = time.monotonic()
                    process = subprocess.Popen(result['argv'], cwd=engine.get('cwd'), env=env,
                                               start_new_session=True, stdout=log, stderr=log)
                    root_record = parse_stat(Path(f'/proc/{process.pid}/stat').read_text())
                    known[process.pid] = root_record
                    result['ready_page'] = server.wait('ready', process)
                    result['startup_ms'] = (server.received_at['ready']-started)*1000
                    calibration = server.wait('calibrated', process)
                    result['calibration'] = calibration
                    actual = tuple(result['ready_page'].get(key) for key in ['innerWidth','innerHeight','devicePixelRatio'])
                    expected = actual if geometry is None else geometry
                    result['issues'].extend(page_issues(result['ready_page'], expected, require_foreground=False))
                    result['issues'].extend(page_issues(calibration['page'], expected))
                    if result['issues']:
                        raise ValueError('invalid page conditions; no resource/scroll comparison')
                    geometry = expected
                    time.sleep(args.settle)
                    before_start = time.monotonic()
                    before, unavailable_before = tree_snapshot(process.pid, root_record['starttime'])
                    before_end = time.monotonic()
                    known.update(before)
                    cpu_start = (before_start+before_end)/2
                    time.sleep(args.idle_seconds)
                    after_start = time.monotonic()
                    after, unavailable_after = tree_snapshot(process.pid, root_record['starttime'])
                    after_end = time.monotonic()
                    cpu_end = (after_start+after_end)/2
                    result['proc_scan_seconds'] = {'before':before_end-before_start, 'after':after_end-after_start}
                    result.update(resources(after))
                    known.update(after)
                    result['idle_cpu_elapsed_seconds'] = cpu_end-cpu_start
                    result['idle_cpu_percent_one_core'], result['cpu_complete'] = cpu_delta(
                        before, after, cpu_end-cpu_start, os.sysconf('SC_CLK_TCK'))
                    result['proc_scan_unreadable_pids'] = {'before':unavailable_before, 'after':unavailable_after}
                    result['cpu_before'] = before
                    result['cpu_after'] = after
                    if not result['memory_complete']:
                        result['issues'].append('partial memory sum: process data unreadable')
                    if not result['cpu_complete']:
                        result['issues'].append('process tree changed: partial CPU delta')
                    server.scroll.set()
                    scroll = server.wait('scroll', process)
                    for page in [scroll['before'], scroll['page']]:
                        result['issues'].extend(page_issues(page, geometry))
                    nominal = statistics.median(calibration['intervals'])
                    intervals = scroll['intervals']
                    if not intervals or nominal <= 0:
                        raise ValueError('missing frame intervals')
                    scroll.update(raf_median_ms=statistics.median(intervals), raf_p95_ms=percentile(intervals, 0.95),
                                  nominal_frame_ms=nominal, frame_count=len(intervals),
                                  long_frame_threshold_ms=nominal*1.5,
                                  long_frame_count=sum(value > nominal*1.5 for value in intervals))
                    result['scroll'] = scroll
                    if scroll['scroll_distance_px'] <= 0:
                        result['issues'].append('no actual scrolling')
                    result['valid'] = not result['issues']
                finally:
                    if process is not None:
                        stop_owned(process, known)
                    log.flush()
                    result['browser_log_tail'] = Path(profile, 'browser.log').read_bytes()[-8192:].decode('utf-8', 'replace')
    except (OSError, ValueError, RuntimeError, TimeoutError, KeyError, statistics.StatisticsError) as error:
        result['issues'].append(type(error).__name__ + ': ' + str(error))
    finally:
        if server is not None:
            server.close()
    return result, geometry


def load_engines(filename):
    config = json.loads(Path(filename).read_text())
    engines = config['engines']
    if not isinstance(engines, list) or not engines:
        raise ValueError('engines must be a nonempty array')
    labels = set()
    for engine in engines:
        if not isinstance(engine.get('label'), str) or not engine['label'] or engine['label'] in labels:
            raise ValueError('engine labels must be nonempty and unique')
        labels.add(engine['label'])
        argv = engine.get('argv')
        if not isinstance(argv, list) or not argv or any(not isinstance(arg, str) or not arg or '\0' in arg for arg in argv):
            raise ValueError('argv must be a nonempty array of nonempty strings; never shell text')
        if 'cwd' in engine and not isinstance(engine['cwd'], str):
            raise ValueError('cwd must be a string')
    return engines


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', required=True, help='JSON: {"engines":[{"label":"name","argv":["binary","args"],"cwd":"/path"}]}')
    parser.add_argument('--output', default='benchmark-results.json')
    parser.add_argument('--runs', type=int, default=3)
    parser.add_argument('--order', choices=['interleaved','sequential'], default='interleaved')
    parser.add_argument('--settle', type=float, default=2, help='idle settling seconds after frame calibration')
    parser.add_argument('--idle-seconds', type=float, default=3)
    parser.add_argument('--timeout', type=float, default=30, help='maximum seconds per page phase')
    parser.add_argument('--viewport', nargs=3, type=float, metavar=('WIDTH','HEIGHT','DPR'), help='expected inner viewport; otherwise first valid page establishes it')
    args = parser.parse_args()
    numbers = [args.settle, args.idle_seconds, args.timeout] + (args.viewport or [])
    if any(not math.isfinite(value) for value in numbers) or (args.viewport and any(value <= 0 for value in args.viewport)):
        parser.error('timings and viewport must be finite; viewport values must be positive')
    if args.runs < 1 or args.settle < 0 or args.idle_seconds <= 0 or args.timeout <= args.settle+args.idle_seconds+2:
        parser.error('positive runs/idle required; timeout must exceed settle + idle + 2 seconds')
    try:
        engines = load_engines(args.config)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    geometry = tuple(args.viewport) if args.viewport else None
    schedule = [(engine, run) for run in range(1,args.runs+1) for engine in engines]
    if args.order == 'sequential':
        schedule = [(engine, run) for engine in engines for run in range(1,args.runs+1)]
    samples = []
    report = {'schema_version':1, 'metric_note':'JS rAF scheduling proxy; not input-to-photon or compositor performance',
              'cache_policy':'fresh XDG profiles each sample, warm/uncontrolled OS cache',
              'settings':vars(args), 'samples':samples}
    for engine, run in schedule:
        print(f"Running {engine['label']} {run}/{args.runs}", flush=True)
        result, geometry = sample(engine, run, args, geometry)
        samples.append(result)
        report['expected_viewport'] = geometry
        report['aggregates'] = aggregate(samples)
        Path(args.output).write_text(json.dumps(report, indent=2) + '\n')
        print(f"  valid={result['valid']} issues={result['issues']}", flush=True)
    print(json.dumps(report['aggregates'], indent=2))
    return 0 if all(item['valid'] for item in samples) else 1


if __name__ == '__main__':
    raise SystemExit(main())
