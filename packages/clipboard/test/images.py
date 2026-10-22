# Writes fixture/images.ts: the clipboard fixture's images, as base64.
# - a 64 x 48 PNG carrying a text chunk (sanitizing drops it);
# - an APNG whose default image (red) is not part of its animation, so its
#   frame 0 is green and frame 1 blue (LLP 1133.006 D1);
# - a PNG whose header claims 9000 x 9000 pixels, over the 64,000,000 bound;
# - still 4 x 3 PNGs of the APNG's red and green: a host's sanitized PNG of
#   the APNG has the same SHA-256 as its sanitized green, not its red.
# usage: python3 packages/clipboard/test/images.py   (needs Pillow)
import base64, io, os, struct, zlib
from PIL import Image, PngImagePlugin

def chunk(t, d): return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
SIG = b'\x89PNG\r\n\x1a\n'

img = Image.new('RGB', (64, 48))
for x in range(64):
    for y in range(48): img.putpixel((x, y), (x * 4, y * 5, 128))
info = PngImagePlugin.PngInfo(); info.add_text('Comment', 'carried only if not sanitized')
b = io.BytesIO(); img.save(b, 'PNG', pnginfo=info); png = b.getvalue()

W, H = 4, 3
raw = lambda rgb: zlib.compress(b''.join(b'\x00' + bytes(rgb) * W for _ in range(H)))
fctl = lambda seq: chunk(b'fcTL', struct.pack('>IIIIIHHBB', seq, W, H, 0, 0, 1, 10, 0, 0))
apng = SIG + chunk(b'IHDR', struct.pack('>IIBBBBB', W, H, 8, 2, 0, 0, 0)) + chunk(b'acTL', struct.pack('>II', 2, 0))
apng += chunk(b'IDAT', raw((255, 0, 0)))
apng += fctl(0) + chunk(b'fdAT', struct.pack('>I', 1) + raw((0, 255, 0)))
apng += fctl(2) + chunk(b'fdAT', struct.pack('>I', 3) + raw((0, 0, 255)))
apng += chunk(b'IEND', b'')

huge = SIG + chunk(b'IHDR', struct.pack('>IIBBBBB', 9000, 9000, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(b'\0')) + chunk(b'IEND', b'')

green = SIG + chunk(b'IHDR', struct.pack('>IIBBBBB', W, H, 8, 2, 0, 0, 0)) + chunk(b'IDAT', raw((0, 255, 0))) + chunk(b'IEND', b'')
red = SIG + chunk(b'IHDR', struct.pack('>IIBBBBB', W, H, 8, 2, 0, 0, 0)) + chunk(b'IDAT', raw((255, 0, 0))) + chunk(b'IEND', b'')

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'fixture', 'images.ts')
with open(out, 'w') as f:
    f.write("// Written by packages/clipboard/test/images.py; never edited.\n")
    for name, data in [('png', png), ('apng', apng), ('huge', huge), ('green', green), ('red', red)]:
        f.write(f"export const {name} = '{base64.b64encode(data).decode()}';\n")
