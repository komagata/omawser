#!/usr/bin/python3
"""Fictional executable-probe delay; installed only in the test box PATH."""
from pathlib import Path
import os
import sys
import time
home = Path.home()
if any('fixture space' in arg for arg in sys.argv[1:]):
    (home / 'probe-started').touch()
    while (home / 'probe-gate').exists():
        time.sleep(0.02)
os.execv('/usr/bin/test', ['test', *sys.argv[1:]])
