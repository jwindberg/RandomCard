#!/usr/bin/env python3
"""Generate src/model/metrics/data.rs from classic Mac OS bitmap fonts.

Usage: font_metrics.py DIR > src/model/metrics/data.rs

DIR holds one .dfont per family (Chicago.dfont, Geneva.dfont, ...), such as
HyperCardPreview's HyperCardCommon/Resources. Only metrics are written out:
each size's ascent, descent, leading and character widths, and the family's
extra width per style. The fonts themselves aren't copied.
"""

import struct
import sys


def resources(fork):
    doff, moff, dlen, mlen = struct.unpack('>IIII', fork[:16])
    m = fork[moff:moff+mlen]
    tl_off, nl_off = struct.unpack('>HH', m[24:28])
    tl = m[tl_off:]
    n = struct.unpack('>H', tl[:2])[0] + 1
    out = []
    for i in range(n):
        kind, cnt, ref = struct.unpack('>4sHH', tl[2+i*8:10+i*8])
        for j in range(cnt+1):
            rid, name_off, attr_off = struct.unpack('>hHI', tl[ref+j*12:ref+j*12+8])
            off = attr_off & 0xffffff
            name = ''
            if name_off != 0xffff:
                p = nl_off + name_off
                name = m[p+1:p+1+m[p]].decode('mac_roman')
            l = struct.unpack('>I', fork[doff+off:doff+off+4])[0]
            out.append((kind.decode('mac_roman'), rid, name, fork[doff+off+4:doff+off+4+l]))
    return out

R = sys.argv[1]
FAMILIES = ["Chicago", "Courier", "Geneva", "Helvetica", "Monaco", "New York", "Palatino", "Times"]

def fixed(bits):
    """A 4.12 fixed-point fraction, in 4096ths; negatives come both ways."""
    if bits >> 15:
        return bits - 65536 if (bits >> 14) & 1 else -(bits & 0x7fff)
    return bits

def font_metrics(d):
    first, last = struct.unpack('>HH', d[2:6])
    kern = struct.unpack('>h', d[8:10])[0]
    rect_h = struct.unpack('>H', d[14:16])[0]
    asc, desc, lead, row_words = struct.unpack('>HHHH', d[18:26])
    n = last - first + 3
    bitimage = row_words * 2 * rect_h
    ow = 16 + struct.unpack('>H', d[16:18])[0] * 2
    # Some fonts leave out the table's final sentinel.
    n = min(n, (len(d) - ow) // 2)
    entries = [struct.unpack('>H', d[ow + 2*i: ow + 2*i + 2])[0] for i in range(n)]
    missing = entries[last - first + 1] & 0xff
    widths = []
    for c in range(32, 256):
        if first <= c <= last and entries[c - first] != 0xffff:
            widths.append(entries[c - first] & 0xff)
        else:
            widths.append(missing)
    return asc, desc, lead, widths

out = {}
styles = {}
for fam in FAMILIES:
    rs = resources(open(f"{R}/{fam}.dfont", 'rb').read())
    fond = next(d for k, i, n, d in rs if k == 'FOND')
    flags = struct.unpack('>H', fond[0:2])[0]
    if not (flags >> 13) & 1:
        p = [fixed(struct.unpack('>H', fond[0x1c + 2*i:0x1e + 2*i])[0]) for i in range(8)]
        styles[fam] = p
    cnt = struct.unpack('>H', fond[52:54])[0] + 1
    fonts = {(i): d for k, i, n, d in rs if k in ('NFNT', 'FONT')}
    for j in range(cnt):
        size, style, rid = struct.unpack('>HHH', fond[54 + 6*j: 60 + 6*j])
        if style == 0 and size > 0 and rid in fonts:
            out[(fam, size)] = font_metrics(fonts[rid])

print("// Generated from the classic Mac OS bitmap fonts' NFNT and FOND resources: only")
print("// their metrics, not the fonts. Widths are for MacRoman characters 32 to 255.")
print()
print("use super::{FamilyStyles, FontMetrics};")
print()
print("pub const FONTS: &[FontMetrics] = &[")
for (fam, size), (a, d, l, w) in sorted(out.items()):
    print(f'    FontMetrics {{ family: "{fam}", size: {size}, ascent: {a}, descent: {d}, leading: {l}, widths: &{w} }},'.replace("[", "[").replace(", ", ", "))
print("];")
print()
print("/// Extra width per style, in 4096ths of the size: bold, italic, underline,")
print("/// outline, shadow, condense, extend. Families not listed use whole pixels.")
print("pub const STYLES: &[FamilyStyles] = &[")
for fam, p in styles.items():
    vals = ", ".join(str(v) for v in p[1:8])
    print(f'    FamilyStyles {{ family: "{fam}", extra: [{vals}] }},')
print("];")
