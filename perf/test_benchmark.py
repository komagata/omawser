import unittest
import argparse
import json
import os
from pathlib import Path
import subprocess
import socket
import sys
import urllib.error
import urllib.request
import benchmark

class BenchmarkTests(unittest.TestCase):
    def test_stat_handles_spaces_and_parentheses_in_comm(self):
        fields = ['S', '12'] + ['0'] * 9 + ['7', '9'] + ['0'] * 6 + ['123']
        parsed = benchmark.parse_stat('42 (name with ) spaces) ' + ' '.join(fields))
        self.assertEqual(parsed, {'pid': 42, 'ppid': 12, 'ticks': 16, 'starttime': 123, 'pgrp':0, 'session':0})

    def test_memory_units_and_partial_read(self):
        self.assertEqual(benchmark.parse_kib('Rss: 400 kB\nPss: 123 kB\nSwap: 9 kB\n'), {'Rss':400, 'Pss':123, 'Swap':9})
        self.assertEqual(benchmark.parse_kib('Name: browser\nVmRSS: 77 kB'), {'VmRSS':77})

    def test_aggregation_excludes_invalid_samples_and_keeps_range(self):
        samples = [dict(label='one', valid=True, startup_ms=v, idle_cpu_percent_one_core=v,
                        pss_kib=v, rss_kib=v, process_count=2,
                        scroll={'raf_median_ms':v, 'raf_p95_ms':v, 'long_frame_count':v}) for v in [10, 30, 20]]
        samples.append(dict(label='one', valid=False, startup_ms=999))
        aggregate = benchmark.aggregate(samples)['one']
        self.assertEqual(aggregate['valid_samples'], 3)
        self.assertEqual(aggregate['invalid_samples'], 1)
        self.assertEqual(aggregate['startup_ms'], {'median':20, 'min':10, 'max':30})
        self.assertEqual(aggregate['long_frame_count'], {'median':20, 'min':10, 'max':30})

    def test_cpu_tracks_pid_identity_and_one_core_units(self):
        before = {1:{'starttime':10,'ticks':100}, 2:{'starttime':20,'ticks':50}}
        after = {1:{'starttime':10,'ticks':150}, 2:{'starttime':99,'ticks':900}}
        value, complete = benchmark.cpu_delta(before, after, elapsed=2, hz=100)
        self.assertEqual(value, 25)
        self.assertFalse(complete)

    def test_ready_focus_is_diagnostic_but_measured_phases_require_foreground(self):
        page = dict(innerWidth=100, innerHeight=200, devicePixelRatio=1, visibilityState='hidden', focused=False)
        self.assertEqual(benchmark.page_issues(page, (100,200,1), require_foreground=False), [])
        self.assertIn('page was background/hidden', benchmark.page_issues(page, (100,200,1)))
        self.assertIn('page was unfocused', benchmark.page_issues(page, (100,200,1)))
        self.assertTrue(benchmark.page_issues({**page, 'visibilityState':'visible', 'focused':True, 'geometryChanged':True}, (100,200,1)))
        self.assertTrue(benchmark.page_issues({**page, 'visibilityState':'visible', 'focused':True}, (101,200,1)))

class ProcessTests(unittest.TestCase):
    def test_process_tree_memory_and_owned_cleanup(self):
        code = r'''import signal, subprocess, sys, time
child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)'])
def stop(*_):
    child.terminate()
    child.wait(timeout=3)
    sys.exit(0)
signal.signal(signal.SIGTERM,stop)
print(child.pid,flush=True)
time.sleep(30)
'''
        process = subprocess.Popen([sys.executable,'-c',code], start_new_session=True, stdout=subprocess.PIPE, text=True)
        outsider = subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)'],start_new_session=True)
        known = {}
        try:
            child_pid = int(process.stdout.readline())
            known, _ = benchmark.tree_snapshot(process.pid)
            self.assertIn(child_pid, known)
            self.assertIn(process.pid, known)
            self.assertEqual(len(known), 2)
            memory = benchmark.resources(known)
            self.assertTrue(memory['memory_complete'], memory['unreadable_processes'])
            self.assertGreater(memory['pss_kib'], 0)
            stale, _ = benchmark.tree_snapshot(process.pid, known[process.pid]['starttime']+1)
            self.assertEqual(stale, {})
            benchmark.stop_owned(process, known)
            self.assertIsNotNone(process.poll())
            self.assertFalse(Path('/proc',str(child_pid)).exists())
            self.assertIsNone(outsider.poll())
        finally:
            benchmark.stop_owned(process, known)
            process.stdout.close()
            outsider.terminate()
            outsider.wait(timeout=3)

class HarnessIntegrationTests(unittest.TestCase):
    def setUp(self):
        try:
            with socket.socket() as probe:
                probe.bind(('127.0.0.1',0))
        except PermissionError as error:
            self.skipTest(f'worker sandbox denies loopback sockets: {error}')

    def test_http_token_and_body_bound(self):
        server = benchmark.FixtureServer(5)
        try:
            with urllib.request.urlopen(server.url) as response:
                self.assertIn(b'Local scrolling fixture', response.read())
            with self.assertRaises(urllib.error.HTTPError) as rejected:
                urllib.request.urlopen(server.url + '-wrong')
            self.assertEqual(rejected.exception.code, 404)
            request = urllib.request.Request(server.url+'/ready', data=b'x'*(benchmark.MAX_BODY+1), method='POST')
            with self.assertRaises(urllib.error.HTTPError) as rejected:
                urllib.request.urlopen(request)
            self.assertEqual(rejected.exception.code, 413)
            self.assertFalse(server.events['ready'].is_set())
        finally:
            server.close()

    def test_sample_protocol_fresh_profile_and_cleanup(self):
        # Protocol stub exercises the host harness only; this is NOT browser evidence.
        stub = r'''import json, os, sys, urllib.request, time
from pathlib import Path
url=sys.argv[1]
roots=[os.environ[k] for k in ['XDG_CONFIG_HOME','XDG_CACHE_HOME','XDG_DATA_HOME']]
assert len(set(roots))==3 and all(not list(Path(p).iterdir()) for p in roots)
print(json.dumps(roots),flush=True)
page=dict(innerWidth=800,innerHeight=600,devicePixelRatio=1,visibilityState='visible',focused=True,userAgent='protocol-stub')
def post(name,payload):
    with urllib.request.urlopen(urllib.request.Request(url+'/'+name,data=json.dumps(payload).encode(),headers={'Content-Type':'application/json'})) as r: r.read()
post('ready',page)
post('calibrated',dict(page=page,intervals=[16,17,16]))
with urllib.request.urlopen(url+'/control') as r: assert json.load(r)['phase']=='scroll'
post('scroll',dict(page=page,before=page,intervals=[16,17,33],scroll_distance_px=4000,elapsed_ms=2000))
time.sleep(30)
'''
        args = argparse.Namespace(timeout=5, settle=0, idle_seconds=0.03)
        other = subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)'], start_new_session=True)
        try:
            result, geometry = benchmark.sample({'label':'stub','argv':[sys.executable,'-c',stub]},1,args,None)
            self.assertTrue(result['valid'], result['issues'])
            self.assertEqual(geometry, (800,600,1))
            self.assertEqual(result['scroll']['long_frame_count'], 1)
            self.assertEqual(result['scroll']['frame_count'], 3)
            self.assertGreater(result['pss_kib'], 0)
            self.assertGreater(result['startup_ms'], 0)
            self.assertTrue(all(not Path(root).exists() for root in json.loads(result['browser_log_tail'].strip())))
            self.assertIsNone(other.poll(), 'cleanup must not touch unrelated processes')
            self.assertTrue(all(not Path('/proc', str(pid)).exists() for pid in result['cpu_after']))
        finally:
            other.terminate()
            other.wait(timeout=3)

if __name__ == '__main__':
    unittest.main()
