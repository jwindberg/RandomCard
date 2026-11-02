"""Turn an old Mac disk image, or a StuffIt archive holding one, into a plain
disk image (.dsk) that Basilisk II and Mini vMac can mount.

usage: to_dsk.py ARCHIVE_OR_IMAGE OUT_DIR

Handles a .sit (or anything unar opens) holding an image, Disk Copy 4.2
images (an 84-byte header), compressed Disk Copy 6 images (via ndif.py), and
plain HFS images. Writes one OUT_DIR/<name>.dsk per image found. Needs unar.
"""
import os
import shutil
import struct
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))


def is_hfs(data, at=0):
    return data[at + 1024 : at + 1026] == b'BD'


def convert(path, out_dir):
    data = open(path, 'rb').read()
    name = os.path.splitext(os.path.basename(path))[0]
    out = os.path.join(out_dir, name + '.dsk')
    rsrc = path + '.rsrc'
    # Disk Copy 4.2: a 63-byte name, sizes, checksums, and 0x0100 at 82.
    if len(data) > 84 and data[82:84] == b'\x01\x00' and data[0] < 64:
        size = struct.unpack('>I', data[64:68])[0]
        open(out, 'wb').write(data[84 : 84 + size])
    elif os.path.exists(rsrc) and b'bcem' in open(rsrc, 'rb').read():
        subprocess.run([sys.executable, os.path.join(HERE, 'ndif.py'), path, rsrc, out], check=True)
    elif is_hfs(data):
        shutil.copyfile(path, out)
    else:
        return None
    return out


def main():
    src, out_dir = sys.argv[1:3]
    os.makedirs(out_dir, exist_ok=True)
    made = []
    if convert_ok := (os.path.isfile(src) and not src.lower().endswith(('.sit', '.sea', '.hqx', '.bin', '.cpt', '.zip'))):
        made.append(convert(src, out_dir))
    if not convert_ok or made == [None]:
        made = []
        with tempfile.TemporaryDirectory() as tmp:
            subprocess.run(['unar', '-q', '-k', 'visible', '-o', tmp, src], check=True)
            for root, _, files in os.walk(tmp):
                for f in files:
                    if not f.endswith('.rsrc'):
                        made.append(convert(os.path.join(root, f), out_dir))
    made = [m for m in made if m]
    if not made:
        raise SystemExit(f'no disk image found in {src}')
    for m in made:
        print(m)


main()
