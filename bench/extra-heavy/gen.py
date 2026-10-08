#!/usr/bin/env python3
"""Deterministic dataset + assets for the Extra Heavy feed benchmark (see SPEC.md). Seed 49975.

Writes data/feed.json (version 2: 19 kinds, the nested filmstrip/inbox interleaved; SPEC "Data") and data/{images,anim,video,svg}/, data/embed.html,
data/svgs.json. Fonts (data/fonts/) are an input fetched once by fetch-fonts.sh.
Run once; every app bundles the output (never regenerate per app).
"""
import base64, colorsys, json, math, os, random, re, subprocess, sys
from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(ROOT, 'data')
D = {k: os.path.join(OUT, k) for k in ('images', 'anim', 'video', 'svg')}
for d in D.values():
    os.makedirs(d, exist_ok=True)
rng = random.Random(49975)
N = 3000


def hexc(r, g, b):
    return '#%02x%02x%02x' % (round(r * 255), round(g * 255), round(b * 255))


def hsl(h, s, l):
    r, g, b = colorsys.hls_to_rgb(h % 1, l, s)
    return hexc(r, g, b)


# ---------------------------------------------------------------- images
def photo(path, w, h, seed, quality=85):
    """The heavy bench's synthetic photo: gradient + shapes + blur + noise (a realistic JPEG decode)."""
    r = random.Random(seed)
    im = Image.new('RGB', (w, h))
    d = ImageDraw.Draw(im)
    top = tuple(r.randrange(40, 220) for _ in range(3)); bot = tuple(r.randrange(20, 200) for _ in range(3))
    for y in range(h):
        t = y / max(1, h - 1)
        d.line([(0, y), (w, y)], fill=tuple(int(a + (b - a) * t) for a, b in zip(top, bot)))
    for _ in range(40):
        x, y, s = r.randrange(w), r.randrange(h), r.randrange(max(2, min(w, h) // 20), max(3, min(w, h) // 4))
        c = tuple(r.randrange(256) for _ in range(3))
        (d.ellipse if r.random() < .5 else d.rectangle)([x, y, x + s, y + s], fill=c)
    im = im.filter(ImageFilter.GaussianBlur(radius=max(1, min(w, h) // 300)))
    px = im.load()
    for _ in range(w * h // 40):
        x, y = r.randrange(w), r.randrange(h); p = px[x, y]
        px[x, y] = tuple(max(0, min(255, v + r.randrange(-30, 30))) for v in p)
    im.save(path, 'JPEG', quality=quality)


AVATARS = [f'avatar-{i:02d}.jpg' for i in range(24)]
SMALL = [f'small-{i:02d}.jpg' for i in range(48)]
ASPECTS = [(4, 3), (3, 4), (16, 9), (1, 1), (9, 16), (3, 2)]
PHOTOS = []
if '--no-images' not in sys.argv:
    for i, n in enumerate(AVATARS): photo(os.path.join(D['images'], n), 256, 256, 1000 + i)
    for i, n in enumerate(SMALL): photo(os.path.join(D['images'], n), 256, 256, 4000 + i)
for i in range(40):
    aw, ah = ASPECTS[i % len(ASPECTS)]
    w, h = (1600, int(1600 * ah / aw)) if aw >= ah else (int(1600 * aw / ah), 1600)
    n = f'photo-{i:02d}.jpg'
    if '--no-images' not in sys.argv:
        photo(os.path.join(D['images'], n), w, h, 2000 + i)
    PHOTOS.append({'src': n, 'w': w, 'h': h})


# ---------------------------------------------------------------- animated images
def gif(i):
    """12 frames, 240 × 240, 100 ms each: a sticker — a spinning star on a coloured disc."""
    frames = []
    hue = (i * 0.17) % 1
    for f in range(12):
        im = Image.new('RGB', (240, 240), (255, 255, 255))
        d = ImageDraw.Draw(im)
        rr, gg, bb = colorsys.hls_to_rgb(hue, 0.55, 0.75)
        d.ellipse([20, 20, 220, 220], fill=(int(rr * 255), int(gg * 255), int(bb * 255)))
        a0 = 2 * math.pi * f / 12
        pts = []
        for k in range(10):
            rad = 80 if k % 2 == 0 else 34
            a = a0 + k * math.pi / 5
            pts.append((120 + rad * math.sin(a), 120 - rad * math.cos(a)))
        d.polygon(pts, fill=(255, 236, 120))
        d.ellipse([112, 112, 128, 128], fill=(40, 40, 40))
        frames.append(im)
    frames[0].save(os.path.join(D['anim'], f'sticker-{i:02d}.gif'), save_all=True, append_images=frames[1:],
                   duration=100, loop=0, disposal=1, optimize=False)


def webp(i):
    """16 frames, 240 × 240, 80 ms each: an equaliser — 8 bars rising and falling (lossy, quality 80)."""
    frames = []
    hue = (0.55 + i * 0.13) % 1
    for f in range(16):
        im = Image.new('RGBA', (240, 240), (22, 24, 38, 255))
        d = ImageDraw.Draw(im)
        for k in range(8):
            h = 30 + 150 * (0.5 + 0.5 * math.sin(2 * math.pi * (f / 16 + k / 8) + i))
            rr, gg, bb = colorsys.hls_to_rgb((hue + k * 0.04) % 1, 0.6, 0.8)
            x = 22 + k * 25
            d.rounded_rectangle([x, 210 - h, x + 18, 210], radius=6, fill=(int(rr * 255), int(gg * 255), int(bb * 255), 255))
        frames.append(im)
    frames[0].save(os.path.join(D['anim'], f'eq-{i:02d}.webp'), save_all=True, append_images=frames[1:],
                   duration=80, loop=0, quality=80, method=4)


def kf(t, s, ease=True):
    k = {'t': t, 's': s}
    if ease:
        k['i'] = {'x': [0.42], 'y': [1]}; k['o'] = {'x': [0.58], 'y': [0]}
    return k


def static(v): return {'a': 0, 'k': v}


def tr(**over):
    t = {'ty': 'tr', 'p': static([0, 0]), 'a': static([0, 0]), 's': static([100, 100]), 'r': static(0),
         'o': static(100), 'sk': static(0), 'sa': static(0), 'nm': 'Transform'}
    t.update(over); return t


def layer(ind, name, shapes, ks):
    base = {'o': static(100), 'r': static(0), 'p': static([120, 120, 0]), 'a': static([0, 0, 0]), 's': static([100, 100, 100])}
    base.update(ks)
    return {'ddd': 0, 'ind': ind, 'ty': 4, 'nm': name, 'sr': 1, 'ks': base, 'ao': 0,
            'shapes': [{'ty': 'gr', 'nm': name, 'it': shapes + [tr()]}], 'ip': 0, 'op': 120, 'st': 0, 'bm': 0}


def rgba(h): return [int(h[1:3], 16) / 255, int(h[3:5], 16) / 255, int(h[5:7], 16) / 255, 1]


def lottie(i):
    """Bodymovin 5.7, 240 × 240, 60 fps, 120 frames (2 s loop), shape layers only:
    a bouncing ball, a rotating rounded square, a ring pulsing out."""
    c1, c2, c3 = hsl(i * 0.21, 0.7, 0.55), hsl(i * 0.21 + 0.33, 0.7, 0.5), hsl(i * 0.21 + 0.66, 0.6, 0.55)
    ball = layer(1, 'ball', [{'ty': 'el', 'd': 1, 's': static([44, 44]), 'p': static([0, 0]), 'nm': 'e'},
                             {'ty': 'fl', 'c': static(rgba(c1)), 'o': static(100), 'r': 1, 'nm': 'f'}],
                 {'p': {'a': 1, 'k': [kf(0, [120, 50, 0]), kf(60, [120, 176, 0]), kf(120, [120, 50, 0], False)]}})
    square = layer(2, 'square', [{'ty': 'rc', 'd': 1, 's': static([64, 64]), 'p': static([0, 0]), 'r': static(14), 'nm': 'r'},
                                 {'ty': 'fl', 'c': static(rgba(c2)), 'o': static(100), 'r': 1, 'nm': 'f'}],
                   {'p': static([60, 170, 0]), 'r': {'a': 1, 'k': [kf(0, [0], False), kf(120, [360], False)]}})
    ring = layer(3, 'ring', [{'ty': 'el', 'd': 1, 's': static([60, 60]), 'p': static([0, 0]), 'nm': 'e'},
                             {'ty': 'st', 'c': static(rgba(c3)), 'o': static(100), 'w': static(8), 'lc': 2, 'lj': 2, 'ml': 4, 'nm': 's'}],
                 {'p': static([180, 170, 0]),
                  's': {'a': 1, 'k': [kf(0, [60, 60, 100]), kf(120, [130, 130, 100], False)]},
                  'o': {'a': 1, 'k': [kf(0, [100]), kf(120, [0], False)]}})
    doc = {'v': '5.7.4', 'fr': 60, 'ip': 0, 'op': 120, 'w': 240, 'h': 240, 'nm': f'motion-{i:02d}', 'ddd': 0,
           'assets': [], 'layers': [ball, square, ring]}
    json.dump(doc, open(os.path.join(D['anim'], f'motion-{i:02d}.json'), 'w'), separators=(',', ':'))


GIFS = [f'sticker-{i:02d}.gif' for i in range(6)]
WEBPS = [f'eq-{i:02d}.webp' for i in range(6)]
LOTTIES = [f'motion-{i:02d}.json' for i in range(4)]
for i in range(6): gif(i); webp(i)
for i in range(4): lottie(i)

# ---------------------------------------------------------------- videos
CLIPS = [f'clip-{i:02d}.mp4' for i in range(4)]
if '--no-video' not in sys.argv and not all(os.path.exists(os.path.join(D['video'], c)) for c in CLIPS):
    subprocess.check_call(['xcrun', 'swift', os.path.join(ROOT, 'gen_video.swift'), D['video']])

# ---------------------------------------------------------------- SVGs
ICON_PATHS = {  # 24 × 24 viewBox, filled with the icon colour (#3C3C43)
    'heart': 'M12 21 L3.5 12.5 A5 5 0 0 1 12 5.5 A5 5 0 0 1 20.5 12.5 Z',
    'star': 'M12 2 L14.9 8.6 L22 9.3 L16.6 14 L18.2 21 L12 17.3 L5.8 21 L7.4 14 L2 9.3 L9.1 8.6 Z',
    'bell': 'M12 2 C8 2 6 5 6 9 L6 15 L4 18 L20 18 L18 15 L18 9 C18 5 16 2 12 2 Z M9.5 19.5 A2.5 2.5 0 0 0 14.5 19.5 Z',
    'bookmark': 'M6 2 L18 2 L18 22 L12 17 L6 22 Z',
    'share': 'M12 2 L17 7 L13.5 7 L13.5 14 L10.5 14 L10.5 7 L7 7 Z M4 11 L7 11 L7 19 L17 19 L17 11 L20 11 L20 22 L4 22 Z',
    'chat': 'M4 3 L20 3 A2 2 0 0 1 22 5 L22 15 A2 2 0 0 1 20 17 L9 17 L4 21 L4 17 A2 2 0 0 1 2 15 L2 5 A2 2 0 0 1 4 3 Z',
    'home': 'M12 2 L22 11 L19 11 L19 22 L14 22 L14 15 L10 15 L10 22 L5 22 L5 11 L2 11 Z',
    'search': 'M10 2 A8 8 0 1 1 9.99 2 Z M10 5 A5 5 0 1 0 10.01 5 Z M15.5 14 L22 20.5 L20.5 22 L14 15.5 Z',
    'camera': 'M8 4 L16 4 L17.5 7 L21 7 A1 1 0 0 1 22 8 L22 20 A1 1 0 0 1 21 21 L3 21 A1 1 0 0 1 2 20 L2 8 A1 1 0 0 1 3 7 L6.5 7 Z M12 9 A4.5 4.5 0 1 0 12.01 9 Z',
    'note': 'M9 3 L21 1 L21 16 A3 3 0 1 1 19 13.2 L19 5.5 L11 7 L11 18 A3 3 0 1 1 9 15.2 Z',
    'pin': 'M12 1 A8 8 0 0 1 20 9 C20 15 12 23 12 23 C12 23 4 15 4 9 A8 8 0 0 1 12 1 Z M12 6 A3 3 0 1 0 12.01 6 Z',
    'bolt': 'M13 1 L4 14 L11 14 L10 23 L20 9 L13 9 Z',
}
ICONS = list(ICON_PATHS)
SVGS = {}


def icon_svg(name):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">'
            f'<path d="{ICON_PATHS[name]}" fill="#3C3C43" fill-rule="evenodd"/></svg>')


def chart_svg(i):
    r = random.Random(7000 + i)
    color = hsl(r.random(), 0.6, 0.55)
    vals = [r.uniform(0.15, 1.0) for _ in range(12)]
    parts = ['<svg xmlns="http://www.w3.org/2000/svg" width="600" height="160" viewBox="0 0 600 160">']
    for y in (40, 80, 120):
        parts.append(f'<line x1="0" y1="{y}" x2="600" y2="{y}" stroke="#E5E5EA" stroke-width="1"/>')
    pts = []
    for k, v in enumerate(vals):
        x = 20 + k * 48; h = round(130 * v, 1)
        parts.append(f'<rect x="{x}" y="{150 - h}" width="28" height="{h}" rx="4" fill="{color}"/>')
        pts.append(f'{x + 14},{round(150 - h - 10 * r.random(), 1)}')
    parts.append(f'<polyline points="{" ".join(pts)}" fill="none" stroke="#FF9500" stroke-width="3" '
                 f'stroke-linejoin="round" stroke-linecap="round"/>')
    parts.append('<line x1="0" y1="150" x2="600" y2="150" stroke="#C7C7CC" stroke-width="2"/></svg>')
    return ''.join(parts)


def art_svg(i):
    r = random.Random(8000 + i)
    h = r.random()
    sky0, sky1 = hsl(h, 0.6, 0.72), hsl(h + 0.08, 0.7, 0.9)
    parts = ['<svg xmlns="http://www.w3.org/2000/svg" width="600" height="300" viewBox="0 0 600 300"><defs>',
             f'<linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{sky0}"/>'
             f'<stop offset="1" stop-color="{sky1}"/></linearGradient>',
             f'<linearGradient id="lake" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="{hsl(h + 0.5, 0.5, 0.45)}"/>'
             f'<stop offset="1" stop-color="{hsl(h + 0.55, 0.6, 0.6)}"/></linearGradient></defs>',
             '<rect x="0" y="0" width="600" height="300" fill="url(#sky)"/>',
             f'<circle cx="{r.randrange(80, 520)}" cy="{r.randrange(50, 110)}" r="{r.randrange(26, 44)}" fill="#FFD76A"/>']
    for layer_i, (base, light) in enumerate(((170, 0.42), (200, 0.32), (225, 0.24))):
        pts = ['0,300']
        x = 0
        while x <= 600:
            pts.append(f'{x},{base - r.randrange(0, 90 - layer_i * 20)}')
            x += r.randrange(60, 130)
        pts += ['600,' + str(base - 20), '600,300']
        parts.append(f'<polygon points="{" ".join(pts)}" fill="{hsl(h + 0.3, 0.35, light)}"/>')
    parts.append('<rect x="0" y="250" width="600" height="50" fill="url(#lake)"/>')
    for _ in range(5):
        tx = r.randrange(20, 580); ty = r.randrange(215, 245)
        parts.append(f'<rect x="{tx - 3}" y="{ty}" width="6" height="14" fill="#5B3A1E"/>'
                     f'<polygon points="{tx},{ty - 34} {tx - 14},{ty + 2} {tx + 14},{ty + 2}" fill="#2F6B3A"/>')
    parts.append('</svg>')
    return ''.join(parts)


for n in ICONS: SVGS[f'icon-{n}'] = icon_svg(n)
CHARTS = [f'chart-{i:02d}' for i in range(24)]
ARTS = [f'art-{i:02d}' for i in range(12)]
for i, n in enumerate(CHARTS): SVGS[n] = chart_svg(i)
for i, n in enumerate(ARTS): SVGS[n] = art_svg(i)
for n, s in SVGS.items():
    open(os.path.join(D['svg'], n + '.svg'), 'w').write(s)
json.dump(SVGS, open(os.path.join(OUT, 'svgs.json'), 'w'), separators=(',', ':'), sort_keys=True)

# ---------------------------------------------------------------- embed.html, the shader
EMBED = """<!doctype html>
<html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no">
<style>
  html, body { margin: 0; height: 100%; overflow: hidden; }
  body { font: 15px -apple-system, system-ui, sans-serif; color: #1c1c1e;
         background: hsl({{HUE}}, 70%, 96%); padding: 16px; box-sizing: border-box; }
  h1 { font-size: 20px; font-weight: 700; margin: 0 0 6px; }
  p { margin: 0 0 12px; color: #3c3c43; }
  .row { display: flex; align-items: flex-end; gap: 6px; height: 72px; }
  .bar { flex: 1; border-radius: 4px 4px 0 0; background: hsl({{HUE}}, 65%, 55%); }
  .spin { position: absolute; top: 16px; right: 16px; width: 28px; height: 28px; border-radius: 50%;
          border: 4px solid hsl({{HUE}}, 30%, 85%); border-top-color: hsl({{HUE}}, 65%, 50%);
          animation: spin 1s linear infinite; animation-play-state: {{PLAY}}; box-sizing: border-box; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style></head>
<body><div class="spin"></div>
<h1>{{TITLE}}</h1><p>Embedded page #{{N}} — a bundled HTML document in a web view.</p>
<div class="row">{{BARS}}</div>
</body></html>
"""
open(os.path.join(OUT, 'embed.html'), 'w').write(EMBED)

# ---------------------------------------------------------------- text
names = ['Ava', 'Ben', 'Chloé', 'Dmitri', 'Emeka', 'Fatima', 'Giulia', 'Hiro', 'Ingrid', 'José', 'Kofi', 'Léa',
         'Mateo', 'Nadia', 'Omar', 'Priya', 'Quinn', 'Rin', 'Sven', 'Tanvi', 'Uma', 'Viktor', 'Wen', 'Yara']
surnames = ['Okafor', 'Lindqvist', 'Moreau', 'Tanaka', 'Haddad', 'Rossi', 'Novak', 'Mensah', 'Kaur', 'Silva',
            'Cohen', 'Petrov', 'Nguyen', 'Garcia', 'Park', 'Ali']
words = ('the a we could maybe tomorrow coffee park camera jacket picnic weather train station platform '
         'schedule delay meeting notes draft review ship build test benchmark list scroll frame render '
         'layout text image photo album trip weekend dinner friends family project deadline launch '
         'design prototype feedback idea thanks great sounds good okay sure definitely later soon '
         'river bridge garden market museum sunset window harbor valley morning evening').split()
topics = ['Design', 'Travel', 'Food', 'Music', 'Cycling', 'Photography', 'Gardening', 'Books', 'Coffee', 'Trains']


def sentence(r, lo=4, hi=18):
    n = r.randrange(lo, hi)
    s = ' '.join(r.choice(words) for _ in range(n))
    return s[0].upper() + s[1:] + r.choice(['.', '.', '!', '?'])


def title(r, lo=2, hi=5):
    return ' '.join(w.capitalize() for w in (r.choice(words) for _ in range(r.randrange(lo, hi))))


INTL = {
    'ar': ['مرحباً بكم في المقهى الجديد! نفتح أبوابنا يوم السبت الساعة ٩ صباحاً.',
           'تم شحن طلبك رقم 4521 وسيصل خلال يومين إلى عنوانك في القاهرة.',
           'الطقس اليوم مشمس مع رياح خفيفة، ودرجة الحرارة 28 درجة مئوية.'],
    'he': ['הרכבת לתל אביב יוצאת בשעה 08:15 מרציף 3.',
           'תודה רבה על ההזמנה! נשמח לראותכם שוב בקרוב.',
           'הפגישה נדחתה ליום רביעי בגלל מזג האוויר.'],
    'cjk': ['今日は新しいカフェに行きました。コーヒーがとても美味しかったです。',
            '明天上午十点在会议室讨论新的设计方案。',
            '주말에 친구들과 함께 한강에서 자전거를 탔어요.'],
    'hi': ['नमस्ते! कल सुबह दस बजे स्टेशन पर मिलते हैं।',
           'यह किताब बहुत दिलचस्प है, आपको ज़रूर पढ़नी चाहिए।',
           'आज मौसम बहुत सुहावना है और हल्की बारिश हो रही है।'],
    'emoji': ['👨‍👩‍👧‍👦 🧑🏽‍💻 👩🏾‍🚀 🏳️‍🌈 ❤️‍🔥 🧑‍🤝‍🧑 🇯🇵 🇧🇷 🇮🇳 🫶🏻',
              '🏃🏻‍♀️‍➡️ 👨🏿‍🍳 🐻‍❄️ 🧔🏼‍♂️ 👩‍❤️‍👨 🏴‍☠️ 🇰🇪 🇺🇦 😶‍🌫️ 🙂‍↕️',
              '👩🏻‍🔬 🧑🏾‍🎨 🐦‍🔥 🍋‍🟩 👨‍👨‍👦 🏳️‍⚧️ 🇲🇽 🇪🇬 🇨🇦 🤝🏽'],
    'mixed': ['Order #4521 — تم الشحن إلى القاهرة ✅', 'Flight LY 008 → תל אביב, gate B12 ✈️',
              'Invoice ٣٤٥ paid via مصرف الراجحي at 14:30'],
}

PLACES = [('Ferry Building', 'San Francisco, CA', 37.7955, -122.3937), ('Palo Alto Caltrain', 'Palo Alto, CA', 37.4430, -122.1650),
          ('Grand Central', 'New York, NY', 40.7527, -73.9772), ("King's Cross", 'London', 51.5308, -0.1238),
          ('Gare du Nord', 'Paris', 48.8809, 2.3553), ('Tokyo Station', 'Tokyo', 35.6812, 139.7671),
          ('Seoul Station', 'Seoul', 37.5547, 126.9707), ('Opera House', 'Sydney', -33.8568, 151.2153),
          ('Tahrir Square', 'Cairo', 30.0444, 31.2357), ('Rothschild Blvd', 'Tel Aviv', 32.0636, 34.7747),
          ('CST Station', 'Mumbai', 18.9398, 72.8355), ('Zócalo', 'Mexico City', 19.4326, -99.1332),
          ('Avenida Paulista', 'São Paulo', -23.5614, -46.6559), ('Hauptbahnhof', 'Berlin', 52.5251, 13.3694),
          ('Centraal', 'Amsterdam', 52.3791, 4.9003), ('Termini', 'Rome', 41.9010, 12.5018),
          ('Galata Tower', 'Istanbul', 41.0256, 28.9741), ('Marina Bay', 'Singapore', 1.2834, 103.8607),
          ('Central', 'Hong Kong', 22.2819, 114.1582), ('V&A Waterfront', 'Cape Town', -33.9036, 18.4207),
          ('Union Station', 'Toronto', 43.6453, -79.3806), ('Union Station', 'Chicago', 41.8789, -87.6403),
          ('Pike Place', 'Seattle', 47.6097, -122.3422), ('Hallgrímskirkja', 'Reykjavík', 64.1417, -21.9266)]

FONTS = {  # key: (file, PostScript name, size)
    'bebas': ('BebasNeue-Regular.ttf', 'BebasNeue-Regular', 34),
    'pacifico': ('Pacifico-Regular.ttf', 'Pacifico-Regular', 24),
    'dmserif': ('DMSerifDisplay-Regular.ttf', 'DMSerifDisplay-Regular', 28),
    'abril': ('AbrilFatface-Regular.ttf', 'AbrilFatface-Regular', 26),
    'spacemono': ('SpaceMono-Regular.ttf', 'SpaceMono-Regular', 19),
    'lobster': ('Lobster-Regular.ttf', 'Lobster-Regular', 28),
    'elite': ('SpecialElite-Regular.ttf', 'SpecialElite-Regular', 21),
    'marker': ('PermanentMarker-Regular.ttf', 'PermanentMarker-Regular', 24),
    'crimson': ('CrimsonText-Italic.ttf', 'CrimsonText-Italic', 25),
    'inter': ('Inter-Regular.ttf', 'Inter-Regular', 22),
}
FONT_KEYS = list(FONTS)

# ---- code: generated TypeScript / Rust / Python, tokenised here (neither stack has a built-in highlighter)
KW = {'ts': 'const let function return if else for of async await export import from type interface new',
      'rs': 'fn let mut pub struct impl for in if else match return use self Some None',
      'py': 'def return if else for in import from class with as None True False lambda yield'}
KW = {k: set(v.split()) for k, v in KW.items()}
TOKEN = re.compile(r'(?P<comment>//.*|#.*)|(?P<string>"[^"]*"|\'[^\']*\')|(?P<number>\b\d+(\.\d+)?\b)|'
                   r'(?P<ident>[A-Za-z_][A-Za-z0-9_]*)|(?P<space>\s+)|(?P<punct>.)')


def tokenize(line, lang):
    out = []
    toks = list(TOKEN.finditer(line))
    for j, m in enumerate(toks):
        kind = m.lastgroup; t = m.group(0)
        if kind == 'ident':
            nxt = toks[j + 1].group(0) if j + 1 < len(toks) else ''
            if t in KW[lang]: kind = 'keyword'
            elif nxt == '(' or nxt == '!': kind = 'function'
            elif t[0].isupper(): kind = 'type'
            else: kind = 'plain'
        elif kind in ('space', 'punct'): kind = 'plain'
        if out and out[-1]['k'] == kind: out[-1]['t'] += t
        else: out.append({'t': t, 'k': kind})
    return out


def code_lines(r):
    lang = r.choice(['ts', 'rs', 'py'])
    a, b, c = r.choice(words), r.choice(words), r.choice(words)
    n1, n2 = r.randrange(2, 500), r.randrange(10, 99)
    if lang == 'ts':
        src = [f'// {sentence(r, 3, 6)}', f'import {{ {a.capitalize()}Store }} from "./{b}";', '',
               f'export async function load{a.capitalize()}(id: string) {{',
               f'  const {b} = await fetch("/api/{c}/" + id);', f'  if ({b}.status !== {n1}) return null;',
               f'  const items: {c.capitalize()}[] = await {b}.json();',
               f'  for (const it of items) it.score *= {n2 / 10};', '  return items.slice(0, 20);', '}']
    elif lang == 'rs':
        src = [f'// {sentence(r, 3, 6)}', f'pub struct {a.capitalize()} {{ id: u32, {b}: Vec<f32> }}', '',
               f'impl {a.capitalize()} {{', f'    pub fn {c}(&mut self, k: usize) -> Option<f32> {{',
               f'        let mut total = {n1}.0;', f'        for x in self.{b}.iter().take(k) {{ total += x; }}',
               f'        if total > {n2}.5 {{ Some(total) }} else {{ None }}', '    }', '}']
    else:
        src = [f'# {sentence(r, 3, 6)}', f'from {a} import {b.capitalize()}', '',
               f'class {c.capitalize()}Report:', f'    def summary(self, rows, limit={n1}):',
               f'        total = sum(r.{b} for r in rows)', f'        if total > {n2}:',
               f'            return "{a}: " + str(total)', '        return None']
    cut = r.randrange(6, len(src) + 1)
    return lang, [tokenize(l, lang) for l in src[:cut]]


def markdown(r):
    t = title(r, 2, 5)
    p1 = (f'{sentence(r)} We could **{r.choice(words)} {r.choice(words)}** and *{r.choice(words)}* '
          f'with `{r.choice(words)}()` — see [{r.choice(words)} notes](https://example.com/{r.choice(words)}). {sentence(r)}')
    items = [f'{sentence(r, 3, 8)[:-1]} **{r.choice(words)}**' if k == 0 else sentence(r, 3, 8)[:-1]
             for k in range(r.randrange(2, 5))]
    blocks = [f'## {t}', p1, '\n'.join('- ' + it for it in items)]
    if r.random() < 0.6:
        blocks.append('> ' + sentence(r, 6, 14))
    blocks.append(sentence(r) + ' ' + sentence(r))
    return '\n\n'.join(blocks)


# ---------------------------------------------------------------- rows
KINDS = ['photo', 'thumbs', 'shader', 'canvas', 'svg', 'video', 'map', 'markdown', 'code', 'intl',
         'typeface', 'carousel', 'motion', 'glass', 'live', 'thread', 'webview']
FIRST = ['photo', 'live', 'shader', 'carousel', 'motion', 'video', 'map', 'markdown', 'code', 'intl',
         'typeface', 'svg', 'glass', 'thumbs', 'thread', 'webview', 'canvas']
assert sorted(FIRST) == sorted(KINDS)
order = []
while len(order) < N:
    block = FIRST[:] if not order else rng.sample(KINDS, len(KINDS))
    order += block
order = order[:N]


def payload(kind, r, i):
    if kind == 'photo':
        return {'caption': sentence(r), 'photo': r.choice(PHOTOS)}
    if kind == 'thumbs':
        return {'title': f'{r.randrange(12, 60)} new photos in {title(r, 1, 3)}', 'thumbs': r.sample(SMALL, 12)}
    if kind == 'shader':
        h = r.random()
        return {'caption': sentence(r), 'photo': r.choice(PHOTOS), 'duoA': hsl(h, 0.7, 0.18), 'duoB': hsl(h + 0.45, 0.85, 0.78)}
    if kind == 'canvas':
        h = r.random()
        strokes = [{'p': [round(r.random(), 3) for _ in range(8)], 'color': hsl(h + k * 0.09, 0.65, 0.5),
                    'width': r.choice([2, 3, 4])} for k in range(6)]
        dots = [{'x': round(r.random(), 3), 'y': round(r.random(), 3), 'r': r.randrange(4, 18),
                 'color': hsl(h + 0.5 + k * 0.03, 0.7, 0.6)} for k in range(14)]
        return {'title': title(r, 2, 4), 'image': r.choice(SMALL), 'bg': hsl(h, 0.6, 0.96),
                'band': [hsl(h + 0.1, 0.8, 0.6), hsl(h + 0.35, 0.8, 0.55)], 'strokes': strokes, 'dots': dots}
    if kind == 'svg':
        return {'icons': r.sample(ICONS, 6), 'chart': r.choice(CHARTS), 'art': r.choice(ARTS)}
    if kind == 'video':
        return {'caption': sentence(r), 'video': r.choice(CLIPS)}
    if kind == 'map':
        name, addr, lat, lon = r.choice(PLACES)
        return {'place': name, 'address': addr, 'lat': lat, 'lon': lon}
    if kind == 'markdown':
        return {'md': markdown(r)}
    if kind == 'code':
        lang, lines = code_lines(r)
        ext = {'ts': 'ts', 'rs': 'rs', 'py': 'py'}[lang]
        return {'file': f'{r.choice(words)}_{r.choice(words)}.{ext}', 'lang': lang, 'lines': lines}
    if kind == 'intl':
        return {'blocks': [{'text': r.choice(INTL['ar']), 'dir': 'rtl'}, {'text': r.choice(INTL['he']), 'dir': 'rtl'},
                           {'text': r.choice(INTL['cjk']), 'dir': 'ltr'}, {'text': r.choice(INTL['hi']), 'dir': 'ltr'},
                           {'text': r.choice(INTL['emoji']), 'dir': 'ltr'}, {'text': r.choice(INTL['mixed']), 'dir': 'ltr'}]}
    if kind == 'typeface':
        return {'quote': sentence(r, 8, 20), 'by': f'{r.choice(names)} {r.choice(surnames)}',
                'font': FONT_KEYS[i // len(KINDS) % len(FONT_KEYS)], 'bg': hsl(r.random(), 0.7, 0.92)}
    if kind == 'carousel':
        return {'title': f'Trending in {r.choice(topics)}',
                'cards': [{'image': r.choice(SMALL), 'title': sentence(r, 3, 9)[:-1],
                           'meta': r.choice([f'${r.randrange(5, 199)}.{r.randrange(0, 99):02d}',
                                             f'{r.randrange(30, 50) / 10} ★ · {r.randrange(1, 99) / 10}k'])}
                          for _ in range(10)]}
    if kind == 'motion':
        return {'caption': sentence(r), 'gif': r.choice(GIFS), 'webp': r.choice(WEBPS), 'lottie': r.choice(LOTTIES)}
    if kind == 'glass':
        return {'photo': r.choice(PHOTOS), 'title': title(r, 2, 4), 'subtitle': sentence(r, 4, 8), 'rating': f'{r.randrange(35, 50) / 10} ★'}
    if kind == 'live':
        return {'title': f'{title(r, 1, 3)} drop', 'endsInSec': r.randrange(600, 86400),
                'rings': [round(r.uniform(0.25, 0.9), 2) for _ in range(3)],
                'wave': [r.randrange(4, 41) for _ in range(48)], 'updatedSec': r.randrange(0, 60)}
    if kind == 'thread':
        return {'text': ' '.join(sentence(r) for _ in range(r.randrange(8, 15)))}
    if kind == 'webview':
        return {'caption': sentence(r), 'title': title(r, 2, 4), 'hue': r.randrange(0, 360),
                'bars': [r.randrange(10, 101) for _ in range(12)]}
    raise ValueError(kind)


old_rows = []  # the 17-kind stream, exactly as generated before the nested kinds (2026-09-27)
minutes = 0
for i, kind in enumerate(order):
    r = random.Random(rng.randrange(1 << 30))
    author = r.choice(names)
    minutes += r.randrange(0, 5)
    row = {'id': f'r{i}', 'index': i, 'kind': kind, 'author': f'{author} {r.choice(surnames)}',
           'handle': '@' + (author.lower().replace('é', 'e') + str(r.randrange(1, 99))), 'avatar': r.choice(AVATARS),
           'minutesAgo': minutes}
    row.update(payload(kind, r, i))
    old_rows.append(row)

# ---------------------------------------------------------------- nested lists (SPEC kinds 18, 19)
# Their own stream (seed 49975 + 1,000,000), so the 17-kind stream above is untouched by them.
nrng = random.Random(49975 + 1_000_000)
FILM_COUNT, INBOX_COUNT, POOL = 2000, 1000, 997
FILM_STEPS = [s for s in range(5, 48) if math.gcd(s, 48) == 1]  # every small image once per 48 items


def message(r):
    n = f'{r.choice(names)} {r.choice(surnames)}'
    text = sentence(r, 4, 9) if r.random() < 0.5 else ' '.join(sentence(r, 10, 18) for _ in range(2))
    return {'name': n, 'avatar': r.choice(AVATARS), 'text': text}


MESSAGES = [message(nrng) for _ in range(POOL)]


def nested(kind, i, prev_minutes):
    r = random.Random(nrng.randrange(1 << 30))
    author = r.choice(names)
    row = {'id': f'r{i}', 'index': i, 'kind': kind, 'author': f'{author} {r.choice(surnames)}',
           'handle': '@' + (author.lower().replace('é', 'e') + str(r.randrange(1, 99))), 'avatar': r.choice(AVATARS),
           'minutesAgo': prev_minutes}
    if kind == 'filmstrip':
        row.update({'title': f'{FILM_COUNT:,} photos from {title(r, 1, 3)}', 'count': FILM_COUNT,
                    'img0': r.randrange(48), 'imgStep': r.choice(FILM_STEPS), 'num0': r.randrange(1000, 7000)})
    else:
        row.update({'title': f'#{r.choice(words)}-{r.choice(words)} · {INBOX_COUNT:,} messages', 'count': INBOX_COUNT,
                    'm0': r.randrange(POOL), 'mStep': r.randrange(1, POOL), 'clock0': r.randrange(1440)})
    return row


rows, k = [], 0
for i in range(N):
    kind = 'filmstrip' if i % 6 == 3 else 'inbox' if i % 12 == 7 else None
    if kind:
        rows.append(nested(kind, i, rows[-1]['minutesAgo'] if rows else 0))
    else:
        row = dict(old_rows[k]); k += 1
        row['id'], row['index'] = f'r{i}', i
        rows.append(row)
KINDS += ['filmstrip', 'inbox']
order = [r['kind'] for r in rows]

FAMILY = {'bebas': 'Bebas Neue', 'pacifico': 'Pacifico', 'dmserif': 'DM Serif Display', 'abril': 'Abril Fatface',
          'spacemono': 'Space Mono', 'lobster': 'Lobster', 'elite': 'Special Elite', 'marker': 'Permanent Marker',
          'crimson': 'Crimson Text Italic', 'inter': 'Inter'}
json.dump({'version': 2, 'kinds': KINDS, 'messages': MESSAGES,
           'fonts': {k: {'file': f, 'name': n, 'size': s, 'family': FAMILY[k]} for k, (f, n, s) in FONTS.items()},
           'rows': rows}, open(os.path.join(OUT, 'feed.json'), 'w'), ensure_ascii=False, separators=(',', ':'))
print('rows', len(rows), {k: order.count(k) for k in KINDS})
