#!/usr/bin/env python3
"""linkprobe.py <macho> [dylib path] — append an LC_LOAD_DYLIB for the probe to a thin arm64 executable, so the
probe loads as an ordinary dependency instead of via DYLD_INSERT_LIBRARIES (which disables the app's prebuilt
launch closure and inflates cold start). Uses the header padding; refuses if there is no room. Re-sign after."""
import struct, sys

path = sys.argv[1]
name = (sys.argv[2] if len(sys.argv) > 2 else '@executable_path/Frameworks/probe.dylib').encode() + b'\0'
b = bytearray(open(path, 'rb').read())
magic, cpu, sub, ftype, ncmds, sizeofcmds, flags, res = struct.unpack_from('<IiiIIIII', b, 0)
if magic != 0xfeedfacf:
    sys.exit(f'{path}: not a thin 64-bit Mach-O (magic {magic:#x})')
hdr = 32
LC_SEGMENT_64, LC_LOAD_DYLIB = 0x19, 0xc
first = len(b)
off = hdr
for _ in range(ncmds):
    cmd, size = struct.unpack_from('<II', b, off)
    if cmd == LC_LOAD_DYLIB and name.rstrip(b'\0') in bytes(b[off:off + size]):
        print('already linked'); sys.exit(0)
    if cmd == LC_SEGMENT_64:
        nsects = struct.unpack_from('<I', b, off + 64)[0]
        for s in range(nsects):
            so = off + 72 + s * 80
            soff = struct.unpack_from('<I', b, so + 48)[0]
            if soff: first = min(first, soff)
    off += size
cmdsize = (24 + len(name) + 7) // 8 * 8
end = hdr + sizeofcmds
if end + cmdsize > first:
    sys.exit(f'{path}: no header room ({first - end} bytes free, need {cmdsize})')
lc = struct.pack('<IIIIII', LC_LOAD_DYLIB, cmdsize, 24, 2, 0x10000, 0x10000) + name
lc += b'\0' * (cmdsize - len(lc))
b[end:end + cmdsize] = lc
struct.pack_into('<II', b, 16, ncmds + 1, sizeofcmds + cmdsize)
open(path, 'wb').write(b)
print(f'linked {name.rstrip(bytes(1)).decode()} ({first - end - cmdsize} bytes of padding left)')
