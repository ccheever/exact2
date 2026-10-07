#!/usr/bin/env python3
"""tpbin.py <tp.xml>… — inclusive ms/s per bucket (a sample counts once per bucket found anywhere in its stack), 16.5 s + 28 s window."""
import xml.etree.ElementTree as ET, collections, sys
B = [('MapKit (VectorKit/GeoServices/MapKit)', ('VectorKit', 'GeoServices', 'MapKit', 'MapsSupport')),
     ('AVKit / AVFoundation / CoreMedia', ('AVKit', 'AVFCore', 'AVFoundation', 'MediaToolbox', 'CoreMedia', 'VideoToolbox', 'AVFAudio')),
     ('WebKit', ('WebKit', 'WebCore', 'JavaScriptCore')),
     ('JPEG decode (image crate / zune)', ('image', 'zune')),
     ('ImageIO', ('ImageIO', 'AppleJPEG', 'CMPhoto')),
     ('Metal / wgpu', ('Metal', 'AGXMetal13_3', 'IOGPU', 'libexact_gpu.dylib')),
     ('CoreGraphics', ('CoreGraphics',)),
     ('CoreText', ('CoreText',)),
     ('exact2 host', ('ExactIOS', 'XHeavy Exact2')),  # the executable took the app's name; older builds are ExactIOS
     ('SwiftUI', ('SwiftUI', 'SwiftUICore', 'AttributeGraph', 'RenderBox')),
     ('Lottie', ('XHeavy',))]
for path in sys.argv[1:]:
    ids = {}; rows = []
    for ev, e in ET.iterparse(path, events=('end',)):
        i = e.get('id')
        if i: ids[i] = e
        if e.tag == 'row': rows.append(e)
    res = lambda e: ids.get(e.get('ref'), e) if e is not None and e.get('ref') else e
    S = []
    for row in rows:
        t = int(res(row.find('sample-time')).text) / 1e9
        w = int(res(row.find('weight')).text) / 1e6
        bt = row.find('tagged-backtrace'); bins = set(); syms = []
        if bt is not None:
            for f in res(bt).iter('frame'):
                f = res(f); b = res(f.find('binary')); syms.append(f.get('name') or '')
                if b is not None: bins.add(b.get('name'))
        S.append((t, w, bins, syms))
    t0 = min(s[0] for s in S) + 16.5
    c = collections.Counter(); tot = 0
    for t, w, bins, syms in S:
        if not (t0 <= t <= t0 + 28): continue
        tot += w
        for name, keys in B:
            if name.startswith('JPEG'):
                hit = any('5image' in s or 'zune_jpeg' in s for s in syms)
            else:
                hit = any(b in keys for b in bins)
            if hit: c[name] += w
    print(f'== {path}: total {tot/28:.0f} ms/s')
    for name, _ in B: print(f'{c[name]/28:7.1f} {name}')
