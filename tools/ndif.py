"""Expand a compressed Disk Copy 6 (NDIF) image into a raw disk image.

usage: ndif.py IMAGE RSRC OUT [PART ...]

IMAGE is the image's data fork, RSRC its resource fork (raw, or AppleDouble as
unar writes it), OUT the raw disk image to write. A segmented image (a .smi
and its .part files) continues its data in the parts' data forks, in order. The 'bcem' resource maps the
image: a 128-byte header (the number of 512-byte sectors at offset 68, the
number of entries at 124), then 12-byte entries of start sector (3 bytes),
type (1 byte), offset and length in the data fork. Types: 0x00 zeros, 0x02
stored, 0x80 and 0x83 ADC-compressed, 0xFF the end.
"""
import struct
import sys


def resource_fork(raw):
    """The resource fork, from an AppleDouble file or as it is."""
    if raw[:4] == b'\x00\x05\x16\x07':
        count = struct.unpack('>H', raw[24:26])[0]
        for i in range(count):
            eid, off, length = struct.unpack('>III', raw[26 + 12 * i : 38 + 12 * i])
            if eid == 2:
                return raw[off : off + length]
        raise SystemExit('no resource fork in the AppleDouble file')
    return raw


def resource(fork, kind):
    data_off, map_off, _, map_len = struct.unpack('>IIII', fork[:16])
    m = fork[map_off : map_off + map_len]
    type_list = struct.unpack('>H', m[24:26])[0]
    for i in range(struct.unpack('>h', m[type_list : type_list + 2])[0] + 1):
        t, _, ref = struct.unpack('>4sHH', m[type_list + 2 + 8 * i : type_list + 10 + 8 * i])
        if t == kind:
            entry = m[type_list + ref : type_list + ref + 12]
            off = data_off + (struct.unpack('>I', entry[4:8])[0] & 0xFFFFFF)
            length = struct.unpack('>I', fork[off : off + 4])[0]
            return fork[off + 4 : off + 4 + length]
    raise SystemExit(f'no {kind.decode()} resource')


def adc(data, size):
    """Apple Data Compression."""
    out = bytearray()
    i = 0
    while i < len(data) and len(out) < size:
        c = data[i]
        if c & 0x80:
            n = (c & 0x7F) + 1
            out += data[i + 1 : i + 1 + n]
            i += 1 + n
            continue
        if c & 0x40:
            n = (c & 0x3F) + 4
            back = struct.unpack('>H', data[i + 1 : i + 3])[0] + 1
            i += 3
        else:
            n = ((c >> 2) & 0x0F) + 3
            back = ((c & 0x03) << 8 | data[i + 1]) + 1
            i += 2
        for _ in range(n):
            out.append(out[-back])
    return bytes(out[:size])


def main():
    image = b''.join(open(p, 'rb').read() for p in [sys.argv[1]] + sys.argv[4:])
    bcem = resource(resource_fork(open(sys.argv[2], 'rb').read()), b'bcem')
    sectors = struct.unpack('>I', bcem[68:72])[0]
    count = struct.unpack('>I', bcem[124:128])[0]
    entries = []
    for i in range(count):
        e = bcem[128 + 12 * i : 140 + 12 * i]
        start = int.from_bytes(e[0:3], 'big')
        kind = e[3]
        off, length = struct.unpack('>II', e[4:12])
        entries.append((start, kind, off, length))
    out = bytearray(sectors * 512)
    for (start, kind, off, length), nxt in zip(entries, entries[1:] + [(sectors, 0xFF, 0, 0)]):
        size = (nxt[0] - start) * 512
        if kind == 0xFF:
            break
        if kind == 0x02:
            chunk = image[off : off + length]
        elif kind in (0x80, 0x83):
            chunk = adc(image[off : off + length], size)
        elif kind == 0x00:
            continue
        else:
            raise SystemExit(f'unknown chunk type {kind:#x} at sector {start}')
        out[start * 512 : start * 512 + len(chunk)] = chunk
    open(sys.argv[3], 'wb').write(out)
    print(f'{sys.argv[3]}: {sectors * 512 // 1024}K')


main()
