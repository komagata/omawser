#!/usr/bin/python3
"""Fictional long-lived browser: records argv without network or WebViews."""
import json
import os
from pathlib import Path
import sys
import time
with (Path.home() / 'launches.jsonl').open('a') as log:
    log.write(json.dumps({'argv': sys.argv, 'binary': __file__, 'pid': os.getpid()}) + '\n')
time.sleep(120)
