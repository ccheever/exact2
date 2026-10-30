"""Alternate fresh copies of before/after executables at one path on the desktop."""
import argparse
import hashlib
import importlib
import json
from pathlib import Path
import shutil
import threading
import time

m = importlib.import_module('windows-measure')
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--before', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
before = args.before.resolve()
out = args.out.resolve()
out.mkdir(parents=True, exist_ok=False)
binary = m.ROOT/'bench/heavy-list/exact-heavylist/dist-windows/Heavy list.exe'
after = out/'after.exe'
assert before != binary, '--before must be a preserved executable'
shutil.copy2(binary, after)
(out/'images.json').write_text(json.dumps({arm: dict(path=str(path),
    sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    for arm, path in [('before', before), ('after', after)]}, indent=2)+'\n')
for arm in ['before', 'after']:
    (out/arm).mkdir()
m.wait_quiet(out)
stop = threading.Event()

def monitor():
    m.psutil.cpu_percent()
    while not stop.wait(5):
        m.record(out/'quiet.jsonl', m.quiet_sample())

observer = threading.Thread(target=monitor)
observer.start()
try:
    for number in range(1, 11):
        for arm in (['before', 'after'] if number % 2 else ['after', 'before']):
            shutil.copy2(before if arm == 'before' else after, binary)
            result = m.run('d3d11', number, out/arm, False, False)
            print(arm, number, result['dwmMs'], flush=True)
            time.sleep(5)
finally:
    stop.set()
    observer.join()
    shutil.copy2(after, binary)
