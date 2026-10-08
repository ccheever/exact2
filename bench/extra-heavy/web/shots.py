#!/usr/bin/env python3
"""shots.py <out-dir> [y …] — parity screenshots of both web builds (server.py running): for each feed offset y
(scrollTop, px) and each app, a headless Chrome page at 1366 x 940 is scrolled to y and captured through the
DevTools protocol over --remote-debugging-pipe (stdlib only). Writes <out-dir>/<app>-<y>.png. The Chrome it
starts is killed by its PID; its profile is a temporary directory removed afterwards."""
import base64, json, os, shutil, subprocess, sys, tempfile, time

CHROME = os.environ.get('BENCH_CHROME', '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome')
out = sys.argv[1]; ys = [int(y) for y in sys.argv[2:]] or [0, 3000, 8000, 20000, 60000]
os.makedirs(out, exist_ok=True)
FIND = """(() => { let b = null; for (const e of document.querySelectorAll('*')) { if (e.clientHeight < 300 || e.scrollHeight <= 20000) continue;
  const o = getComputedStyle(e).overflowY; if (o !== 'auto' && o !== 'scroll') continue; if (!b || e.scrollHeight > b.scrollHeight) b = e; } return b; })()"""

prof = tempfile.mkdtemp(prefix='xhw-shots-')
r_cmd, w_cmd = os.pipe(); r_out, w_out = os.pipe()
def child():
    os.dup2(r_cmd, 3); os.dup2(w_out, 4)
p = subprocess.Popen([CHROME, '--headless=new', '--remote-debugging-pipe', f'--user-data-dir={prof}', '--hide-scrollbars',
                      '--enable-unsafe-webgpu', '--no-first-run', 'about:blank'], pass_fds=(3, 4, r_cmd, w_out), preexec_fn=child,
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
os.close(r_cmd); os.close(w_out)
wf = os.fdopen(w_cmd, 'wb', buffering=0); rf = os.fdopen(r_out, 'rb', buffering=0)
buf = b''; nid = 0
def call(method, params=None, session=None):
    global buf, nid
    nid += 1; msg = {'id': nid, 'method': method, 'params': params or {}}
    if session: msg['sessionId'] = session
    wf.write(json.dumps(msg).encode() + b'\0')
    while True:
        while b'\0' not in buf:
            chunk = rf.read(1 << 20)
            if not chunk: raise SystemExit('chrome closed the pipe')
            buf += chunk
        line, buf = buf.split(b'\0', 1)
        m = json.loads(line)
        if m.get('id') == nid:
            if 'error' in m: raise RuntimeError(f'{method}: {m["error"]}')
            return m.get('result', {})

try:
    for y in ys:
        for app, port in (('exact2', 8811), ('expo', 8812)):
            t = call('Target.createTarget', {'url': 'about:blank'})['targetId']
            s = call('Target.attachToTarget', {'targetId': t, 'flatten': True})['sessionId']
            call('Emulation.setDeviceMetricsOverride', {'width': 1366, 'height': 940, 'deviceScaleFactor': 1, 'mobile': False}, s)
            call('Page.navigate', {'url': f'http://127.0.0.1:{port}/'}, s)
            time.sleep(6)
            call('Runtime.evaluate', {'expression': f'(() => {{ const s = {FIND}; if (s) s.scrollTop = {y}; return !!s; }})()'}, s)
            time.sleep(float(os.environ.get('XHW_WAIT', 4)))
            if os.environ.get('XHW_EVAL'):
                r = call('Runtime.evaluate', {'expression': os.environ['XHW_EVAL'], 'returnByValue': True, 'awaitPromise': True}, s)
                print(app, y, json.dumps(r.get('result', {}).get('value'))[:3000], flush=True)
                time.sleep(float(os.environ.get('XHW_AFTER', 0)))
            png = call('Page.captureScreenshot', {'format': 'png'}, s)['data']
            open(os.path.join(out, f'{app}-{y}.png'), 'wb').write(base64.b64decode(png))
            call('Target.closeTarget', {'targetId': t})
            print('shot', app, y, flush=True)
finally:
    p.kill(); p.wait()
    shutil.rmtree(prof, ignore_errors=True)
