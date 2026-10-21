#!/usr/bin/env python3
"""server.py <root> <results-dir> — the web pair's static server and collector (stdlib only; on bones run it
with /Library/Developer/CommandLineTools/usr/bin/python3, since /usr/bin/python3 waits on the Xcode licence).

Serves <root>/dist/exact at http://127.0.0.1:8811 and <root>/dist/expo at :8812 (build.sh) (extensionless paths fall back to
index.html), injects <root>/probe.js as the first script of every index.html, and answers:
  POST /__result?name=F   writes the body to <results-dir>/F
  POST /__mark?tag=T      returns {cpu: {renderer, gpu, browser, other, total} (s), rss: {… , total} (bytes)}
                          summed with `ps` over the process tree of the PID in <root>/chrome.pid (web.sh's)
Both origins send COOP same-origin + COEP credentialless, so the page is cross-origin isolated
(measureUserAgentSpecificMemory) while no-cors images (the map tiles) still load.
"""
import http.server, json, os, re, subprocess, sys, threading, urllib.parse

ROOT = os.path.abspath(sys.argv[1]); RESULTS = os.path.abspath(sys.argv[2]); os.makedirs(RESULTS, exist_ok=True)
PIDFILE = os.environ.get('CHROME_PIDFILE', os.path.join(ROOT, 'chrome.pid'))  # web.sh writes it beside itself
TYPES = {'.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.json': 'application/json',
         '.wasm': 'application/wasm', '.plan': 'application/octet-stream', '.mp4': 'video/mp4', '.png': 'image/png',
         '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg', '.gif': 'image/gif', '.webp': 'image/webp', '.svg': 'image/svg+xml',
         '.ttf': 'font/ttf', '.woff2': 'font/woff2', '.wgsl': 'text/plain'}


def ps_tree():
    try:
        pid = int(open(PIDFILE).read().split()[0])
    except Exception:
        return None
    rows = subprocess.run(['ps', '-A', '-o', 'pid=,ppid=,rss=,time=,command='], capture_output=True, text=True).stdout.splitlines()
    procs = {}
    for line in rows:
        p = line.split(None, 4)
        if len(p) < 5: continue
        t = p[3]; parts = t.replace('-', ':').split(':')
        secs = 0.0
        for x in parts: secs = secs * 60 + float(x)
        if '-' in t: secs += 0  # days never happen here
        procs[int(p[0])] = (int(p[1]), int(p[2]) * 1024, secs, p[4])
    tree = {pid}; changed = True
    while changed:
        changed = False
        for q, (pp, *_ ) in procs.items():
            if pp in tree and q not in tree: tree.add(q); changed = True
    cpu = {'renderer': 0.0, 'gpu': 0.0, 'browser': 0.0, 'other': 0.0}; rss = dict.fromkeys(cpu, 0)
    for q in tree:
        if q not in procs: continue
        _, r, s, cmd = procs[q]
        m = re.search(r'--type=([a-z-]+)', cmd)
        kind = 'browser' if q == pid else ('renderer' if m and m.group(1) == 'renderer' else 'gpu' if m and m.group(1) == 'gpu-process' else 'other')
        cpu[kind] += s; rss[kind] += r
    cpu['total'] = sum(cpu.values()); rss['total'] = sum(rss.values())
    return {'cpu': cpu, 'rss': rss, 'procs': len(tree)}


def handler(app):
    base = os.path.join(ROOT, 'dist', app)

    class H(http.server.BaseHTTPRequestHandler):
        protocol_version = 'HTTP/1.1'
        def log_message(self, *a): pass

        def send(self, code, body, ctype, extra=()):
            self.send_response(code)
            self.send_header('Content-Type', ctype); self.send_header('Content-Length', str(len(body)))
            self.send_header('Cross-Origin-Opener-Policy', 'same-origin'); self.send_header('Cross-Origin-Embedder-Policy', 'credentialless')
            self.send_header('Cache-Control', 'no-store')  # localhost: every load reads the current build
            for k, v in extra: self.send_header(k, v)
            self.end_headers()
            if self.command != 'HEAD': self.wfile.write(body)

        def do_POST(self):
            u = urllib.parse.urlparse(self.path); qs = urllib.parse.parse_qs(u.query)
            body = self.rfile.read(int(self.headers.get('Content-Length') or 0))
            if u.path == '/__result':
                name = os.path.basename(qs.get('name', ['result.json'])[0])
                open(os.path.join(RESULTS, name), 'wb').write(body)
                return self.send(200, b'{}', 'application/json')
            if u.path == '/__mark':
                return self.send(200, json.dumps(ps_tree()).encode(), 'application/json')
            self.send(404, b'', 'text/plain')

        def do_HEAD(self): self.do_GET()

        def do_GET(self):
            path = urllib.parse.unquote(urllib.parse.urlparse(self.path).path)
            if path == '/__probe.js': return self.send(200, open(os.path.join(ROOT, 'probe.js'), 'rb').read(), 'text/javascript')
            f = os.path.normpath(os.path.join(base, path.lstrip('/')))
            if not f.startswith(base): return self.send(403, b'', 'text/plain')
            if os.path.isdir(f): f = os.path.join(f, 'index.html')
            if not os.path.isfile(f) and not os.path.splitext(f)[1]: f = os.path.join(base, 'index.html')
            if not os.path.isfile(f): return self.send(404, b'not found', 'text/plain')
            data = open(f, 'rb').read(); ext = os.path.splitext(f)[1].lower(); ctype = TYPES.get(ext, 'application/octet-stream')
            if ext == '.html' and os.path.basename(f) == 'index.html' and os.path.dirname(f) == base:
                data = re.sub(rb'(<head[^>]*>|<html[^>]*>)', lambda m: m.group(1) + b'<script src="/__probe.js"></script>', data, count=1)
            rng = self.headers.get('Range')
            m = re.match(r'bytes=(\d*)-(\d*)$', rng or '')
            if m and (m.group(1) or m.group(2)):
                a = int(m.group(1)) if m.group(1) else len(data) - int(m.group(2)); b = int(m.group(2)) if m.group(1) and m.group(2) else len(data) - 1
                b = min(b, len(data) - 1)
                return self.send(206, data[a:b + 1], ctype, [('Content-Range', f'bytes {a}-{b}/{len(data)}'), ('Accept-Ranges', 'bytes')])
            self.send(200, data, ctype, [('Accept-Ranges', 'bytes')])
    return H


if __name__ == '__main__':
    servers = [http.server.ThreadingHTTPServer(('127.0.0.1', port), handler(app)) for app, port in (('exact', 8811), ('expo', 8812))]
    for s in servers[1:]: threading.Thread(target=s.serve_forever, daemon=True).start()
    print('serving', ROOT, '-> exact :8811, expo :8812; results', RESULTS, flush=True)
    servers[0].serve_forever()
