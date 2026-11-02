"""Build a Mini vMac setup for running the real HyperCard beside RandomCard.

It makes one hard disk image holding System 6.0.8, HyperCard 1.2.5 (with its
Home stack), HyperCard Player 2.1 and folders of Apple's stacks, all taken
from the Developer CDs that tools/devcds.py unpacks, and puts it in a folder
with Mini vMac and its ROM. Mini vMac emulates a Mac Plus: 512x342, black
and white, which is what these stacks were made for.

usage: minivmac.py DEVCDS_DIR MINIVMAC_DIR OUT_DIR

  DEVCDS_DIR    where tools/devcds.py unpacked the discs
  MINIVMAC_DIR  a folder with the "Mini vMac" program and "vMac.ROM"
  OUT_DIR       where the setup goes; run "Mini vMac" there with the disk

Needs Python with machfs (pip install machfs).
"""
import os
import shutil
import struct
import sys

import machfs

# (disc folder, path on the disc, name on the emulated disk)
CONTENTS = [
    ('ADC Developer CD (Volume 03,5 - "WWDC 1990")_mdf', 'Applications/HyperCard 1.2.5', 'HyperCard 1.2.5'),
    ('ADC Developer CD (1993-03) ("Other People\'s Memory")_iso',
     'Development Platforms/HyperCard Related/HyperCard Player 2.1', 'HyperCard Player 2.1'),
    ('ADC Developer CD (Volume 04 - "Discy Business")_mdf',
     'Jul 90 Developer Essentials/SpInside:TNStack:Q&A:Human I:F', 'Apple Stacks 1990'),
    ('ADC Developer CD (Volume 04 - "Discy Business")_mdf', 'Info Island/HC 2.0 training stacks', 'HC 2.0 Training'),
]
SYSTEM = ('ADC Developer CD (1993-03) ("Other People\'s Memory")_iso', 'System Software/U.S. 6.0.8/System Tools.image')


def disk_copy_image(path):
    """The HFS volume in a Disk Copy 4.2 image (an 84-byte header, then the disk)."""
    with open(path, 'rb') as f:
        data = f.read()
    size = struct.unpack('>I', data[64:68])[0]
    return data[84 : 84 + size]


def resource_types(fork):
    """The resource types in a resource fork."""
    if len(fork) < 16:
        return set()
    try:
        map_off, map_len = struct.unpack('>II', fork[4:12])
        m = fork[map_off : map_off + map_len]
        type_list = struct.unpack('>H', m[24:26])[0]
        count = struct.unpack('>H', m[type_list : type_list + 2])[0] + 1
        return {m[type_list + 2 + 8 * i : type_list + 6 + 8 * i] for i in range(count)}
    except struct.error:
        return set()


def finder_info(disc_dir):
    """Type and creator by path, from the disc's FINDER.tsv if it has one."""
    info = {}
    path = os.path.join(disc_dir, 'FINDER.tsv')
    if os.path.exists(path):
        for line in open(path, encoding='utf-8'):
            # Path, type, creator; type and creator are four characters each
            # and can themselves hold a tab (some Apple II files).
            line = line.rstrip('\n')
            p, kind, creator = line[:-10], line[-9:-5], line[-4:]
            info[p] = (kind.encode('mac_roman'), creator.encode('mac_roman'))
    stacks = os.path.join(disc_dir, 'STACKS.txt')
    listed = set(open(stacks, encoding='utf-8').read().split('\n')) if os.path.exists(stacks) else set()
    return info, listed


def load_folder(disc_dir, rel, info, stacks):
    """A folder unpacked by devcds.py, as a machfs folder with forks and types."""
    folder = machfs.Folder()
    src = os.path.join(disc_dir, rel)
    for name in sorted(os.listdir(src)):
        path = os.path.join(src, name)
        sub = os.path.join(rel, name)
        if os.path.isdir(path):
            folder[name.replace(':', '/')] = load_folder(disc_dir, sub, info, stacks)
            continue
        if name.endswith('.rsrc') and os.path.exists(path[:-5]):
            continue
        f = machfs.File()
        f.data = open(path, 'rb').read()
        if os.path.exists(path + '.rsrc'):
            f.rsrc = open(path + '.rsrc', 'rb').read()
        if sub in info:
            f.type, f.creator = info[sub]
        elif sub in stacks or name in stacks:
            f.type, f.creator = b'STAK', b'WILD'
        elif b'CODE' in resource_types(f.rsrc):
            # An application; HyperCard's own creator is WILD.
            f.type, f.creator = b'APPL', b'WILD' if 'hypercard' in name.lower() else b'????'
        else:
            f.type, f.creator = b'????', b'????'
        folder[name.replace(':', '/')] = f
    return folder


def size_of(item):
    if isinstance(item, machfs.File):
        return len(item.data) + len(item.rsrc) + 1024
    return sum(size_of(i) for i in item.values())


def main():
    devcds, minivmac, out = sys.argv[1:4]
    system = machfs.Volume()
    system.read(disk_copy_image(os.path.join(devcds, *SYSTEM)))

    vol = machfs.Volume()
    vol.name = 'HyperCard HD'
    vol['System Folder'] = system['System Folder']
    for disc, rel, name in CONTENTS:
        disc_dir = os.path.join(devcds, disc)
        info, stacks = finder_info(disc_dir)
        folder = load_folder(disc_dir, rel, info, stacks)
        # HyperCard looks for Home beside itself; some discs kept it a folder down.
        if 'Home' not in folder:
            for sub in [f for f in folder.values() if isinstance(f, machfs.Folder)]:
                if 'Home' in sub:
                    folder['Home'] = sub.pop('Home')
                    break
        vol[name] = folder
        print(f'added {name}')

    size = (size_of(vol) * 3 // 2 // (1024 * 1024) + 16) * 1024 * 1024
    os.makedirs(out, exist_ok=True)
    disk = os.path.join(out, 'HyperCard HD.dsk')
    with open(disk, 'wb') as f:
        f.write(vol.write(size=size, bootable=True))
    for name in ('Mini vMac', 'vMac.ROM'):
        shutil.copy2(os.path.join(minivmac, name), os.path.join(out, name))
    print(f'{disk}: {size // (1024 * 1024)} MB')
    print(f'run: cd "{out}" && ./"Mini vMac" "HyperCard HD.dsk"')


main()
