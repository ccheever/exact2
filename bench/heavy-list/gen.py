#!/usr/bin/env python3
"""Deterministic dataset + images for the heavy list benchmark (see SPEC.md).
Writes data/messages.json and data/images/*.jpg; prepare.sh runs it once and copies the output to the apps that
bundle it. Needs Pillow; 11.3.0 reproduces data.sha256 byte for byte (the JPEG bytes depend on its encoder)."""
import json, os, random
from PIL import Image, ImageDraw, ImageFilter

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'data')
IMG = os.path.join(OUT, 'images')
os.makedirs(IMG, exist_ok=True)
rng = random.Random(49975)

# ---- images: 24 avatars (256x256), 60 photos (mixed aspect, long side 1600), 20 link thumbs (600x400)
def photo(path, w, h, seed):
    r = random.Random(seed)
    im = Image.new('RGB', (w, h))
    d = ImageDraw.Draw(im)
    top = tuple(r.randrange(40, 220) for _ in range(3)); bot = tuple(r.randrange(20, 200) for _ in range(3))
    for y in range(h):
        t = y / max(1, h - 1)
        d.line([(0, y), (w, y)], fill=tuple(int(a + (b - a) * t) for a, b in zip(top, bot)))
    for _ in range(40):
        x, y, s = r.randrange(w), r.randrange(h), r.randrange(min(w, h) // 20, min(w, h) // 4)
        c = tuple(r.randrange(256) for _ in range(3))
        (d.ellipse if r.random() < .5 else d.rectangle)([x, y, x + s, y + s], fill=c)
    im = im.filter(ImageFilter.GaussianBlur(radius=max(1, min(w, h) // 300)))
    # sprinkle noise so JPEG decode is realistic, not a flat-color fast path
    px = im.load()
    for _ in range(w * h // 40):
        x, y = r.randrange(w), r.randrange(h); p = px[x, y]
        px[x, y] = tuple(max(0, min(255, v + r.randrange(-30, 30))) for v in p)
    im.save(path, 'JPEG', quality=85)

avatars = [f'avatar-{i:02d}.jpg' for i in range(24)]
for i, n in enumerate(avatars): photo(os.path.join(IMG, n), 256, 256, 1000 + i)
aspects = [(4, 3), (3, 4), (16, 9), (1, 1), (9, 16), (3, 2)]
photos = []
for i in range(60):
    aw, ah = aspects[i % len(aspects)]
    w, h = (1600, int(1600 * ah / aw)) if aw >= ah else (int(1600 * aw / ah), 1600)
    n = f'photo-{i:02d}.jpg'; photo(os.path.join(IMG, n), w, h, 2000 + i); photos.append((n, w, h))
thumbs = [f'link-{i:02d}.jpg' for i in range(20)]
for i, n in enumerate(thumbs): photo(os.path.join(IMG, n), 600, 400, 3000 + i)

# ---- text
words = ('the a we could maybe tomorrow coffee park camera jacket picnic weather train station platform '
         'schedule delay meeting notes draft review ship build test benchmark list scroll frame render '
         'layout text image photo album trip weekend dinner friends family project deadline launch '
         'design prototype feedback idea thanks great sounds good okay sure definitely later soon').split()
names = ['Ava', 'Ben', 'Chloé', 'Dmitri', 'Emeka', 'Fatima', 'Giulia', 'Hiro', 'Ingrid', 'José', 'Kofi', 'Léa',
         'Mateo', 'Nadia', 'Omar', 'Priya', 'Quinn', 'Rin', 'Sven', 'Tanvi', 'Uma', 'Viktor', 'Wen', 'Yara']
emoji = ['😀', '🎉', '☕️', '📸', '🚆', '🌧️', '🍕', '👍', '❤️', '🔥', '🙏', '😂', '🤔', '✨', '🧑🏽‍💻', '👨‍👩‍👧']
foreign = ['今日は駅で会いましょう。', 'مرحبا، سنلتقي في المحطة غداً.', '明天在车站见面吧。',
           'שלום, נתראה בתחנה מחר.', 'Увидимся завтра на станции.', 'नमस्ते, कल स्टेशन पर मिलते हैं।']
reactions_pool = ['👍', '❤️', '😂', '🎉', '🔥', '🙏', '😮', '👀']

def sentence(r):
    n = r.randrange(4, 18)
    s = ' '.join(r.choice(words) for _ in range(n))
    return s[0].upper() + s[1:] + r.choice(['.', '.', '!', '?'])

def runs_for(r, idx):
    """A paragraph as styled runs: plain, bold, italic, code, link, mention, tag, emoji."""
    runs = []
    for k in range(r.randrange(2, 9)):
        roll = r.random()
        if roll < .45: runs.append({'t': sentence(r) + ' '})
        elif roll < .55: runs.append({'t': ' '.join(r.choice(words) for _ in range(r.randrange(1, 4))) + ' ', 's': 'bold'})
        elif roll < .63: runs.append({'t': ' '.join(r.choice(words) for _ in range(r.randrange(1, 4))) + ' ', 's': 'italic'})
        elif roll < .70: runs.append({'t': r.choice(['build.mjs', 'list_viewport', 'onDelete', 'git rebase', 'scrollTop']) , 's': 'code'}); runs.append({'t': ' '})
        elif roll < .77: runs.append({'t': r.choice(['exact.dev/docs', 'expo.dev/blog', 'apple.com/swiftui']), 's': 'link'}); runs.append({'t': ' '})
        elif roll < .84: runs.append({'t': '@' + r.choice(names), 's': 'mention'}); runs.append({'t': ' '})
        elif roll < .90: runs.append({'t': '#' + r.choice(words), 's': 'tag'}); runs.append({'t': ' '})
        elif roll < .96: runs.append({'t': ''.join(r.choice(emoji) for _ in range(r.randrange(1, 4))) + ' '})
        else: runs.append({'t': r.choice(foreign) + ' '})
    runs[-1]['t'] = runs[-1]['t'].rstrip() or runs[-1]['t']
    return runs

messages = []
for i in range(10_000):
    r = random.Random(i * 7919 + 1)
    author = i % len(names)
    m = {
        'id': f'm{i}', 'index': i,
        'author': names[author], 'avatar': avatars[author],
        'minutesAgo': (10_000 - i) * 3 + r.randrange(0, 3),
        'paragraphs': [runs_for(r, i) for _ in range(r.choices([1, 2, 3, 4], [50, 30, 15, 5])[0])],
    }
    if r.random() < .22:
        k = r.choices([1, 2, 3, 4], [50, 25, 10, 15])[0]
        m['photos'] = [{'src': p[0], 'w': p[1], 'h': p[2]} for p in (photos[(i * 13 + j * 7) % len(photos)] for j in range(k))]
    if r.random() < .12:
        t = thumbs[i % len(thumbs)]
        m['link'] = {'thumb': t, 'title': sentence(r).rstrip('.!?'), 'description': ' '.join(sentence(r) for _ in range(3)), 'site': r.choice(['exact.dev', 'expo.dev', 'apple.com'])}
    if r.random() < .15 and i > 0:
        q = random.Random(i).randrange(max(0, i - 50), i)
        m['quote'] = {'id': f'm{q}', 'author': names[q % len(names)], 'excerpt': sentence(random.Random(q))}
    if r.random() < .35:
        m['reactions'] = [{'emoji': e, 'count': r.randrange(1, 40)} for e in r.sample(reactions_pool, r.randrange(1, 7))]
    messages.append(m)

json.dump({'version': 1, 'messages': messages}, open(os.path.join(OUT, 'messages.json'), 'w'), ensure_ascii=False, separators=(',', ':'))
print('messages', len(messages), 'images', len(os.listdir(IMG)))
