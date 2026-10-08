#!/usr/bin/env python3
"""gen-assets.py — the exact2 app's generated inputs, from ../data (gen.py's output). Run by ../prepare.sh.

- assets/: the bundled files — ../data/images/*.jpg, anim/*.{gif,webp}, video/*.mp4 copied flat, and
  assets/fonts/ (the ten declared families' files and their licences; CrimsonText-Regular is not declared).
- data/feed.json and data/motion-0N.json: byte copies the data crate includes (`include_bytes!`).
- icon.png: app.json's 1024 px icon, ../data/images/avatar-00.jpg scaled up (bicubic).
- assets/embed/<id>.html and <id>-paused.html: embed.html filled in per webview row (SPEC 17).
  exact2's `iframe` has no `srcdoc` (LLP 1020 §302), so an HTML string becomes a bundled page.
- assets/still/<name>.png: frame 0 of each GIF/WebP, for BENCH_FREEZE. As on the web, an
  animated image cannot be paused; a frozen page shows its first frame as a still.
"""
import json, os, shutil
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, '..', 'data')
ASSETS = os.path.join(HERE, 'assets')
os.makedirs(ASSETS, exist_ok=True)

for sub, exts in (('images', ('.jpg',)), ('anim', ('.gif', '.webp')), ('video', ('.mp4',))):
    for f in sorted(os.listdir(os.path.join(DATA, sub))):
        if f.endswith(exts):
            shutil.copyfile(os.path.join(DATA, sub, f), os.path.join(ASSETS, f))
fonts = os.path.join(ASSETS, 'fonts'); os.makedirs(fonts, exist_ok=True)
for f in sorted(os.listdir(os.path.join(DATA, 'fonts'))):
    if (f.endswith('.ttf') and f != 'CrimsonText-Regular.ttf') or (f.startswith('LICENSE-') and f.endswith('.txt')):
        shutil.copyfile(os.path.join(DATA, 'fonts', f), os.path.join(fonts, f))
shutil.copyfile(os.path.join(DATA, 'feed.json'), os.path.join(HERE, 'data', 'feed.json'))
for i in range(4):
    n = f'motion-{i:02d}.json'
    shutil.copyfile(os.path.join(DATA, 'anim', n), os.path.join(HERE, 'data', n))
Image.open(os.path.join(DATA, 'images', 'avatar-00.jpg')).convert('RGB').resize((1024, 1024), Image.BICUBIC) \
    .save(os.path.join(HERE, 'icon.png'))

feed = json.load(open(os.path.join(DATA, 'feed.json')))
tpl = open(os.path.join(DATA, 'embed.html')).read()

out = os.path.join(ASSETS, 'embed'); os.makedirs(out, exist_ok=True)
n = 0
for r in feed['rows']:
    if r['kind'] != 'webview':
        continue
    bars = ''.join(f'<div class="bar" style="height:{v}%"></div>' for v in r['bars'])
    page = (tpl.replace('{{HUE}}', str(r['hue'])).replace('{{TITLE}}', r['title'])
            .replace('{{N}}', str(r['index'])).replace('{{BARS}}', bars))
    for play, suffix in (('running', ''), ('paused', '-paused')):
        open(os.path.join(out, f"{r['id']}{suffix}.html"), 'w').write(page.replace('{{PLAY}}', play))
    n += 1

still = os.path.join(ASSETS, 'still'); os.makedirs(still, exist_ok=True)
for f in sorted(os.listdir(os.path.join(DATA, 'anim'))):
    if f.endswith(('.gif', '.webp')):
        im = Image.open(os.path.join(DATA, 'anim', f)); im.seek(0)
        im.convert('RGBA').save(os.path.join(still, os.path.splitext(f)[0] + '.png'))
print(n, 'webview pages x2, stills, copies and icon done')
