"""Build an HFS disk image of the stacks on Apple's Developer CDs, for opening
them in the real HyperCard under an emulator, beside RandomCard.

usage: stacks_disk.py DEVCDS_DIR OUT.dsk

Takes every stack in DEVCDS_DIR/INDEX.tsv (made by the index_stacks test) and
copies the folder it's in, whole (companion stacks, pictures and documents
come along), keeping its path inside its disc. Each disc gets a top-level
folder named by its date and title. Files get their Mac type and creator from
the disc's FINDER.tsv (written by devcds.py), or STAK/WILD for stacks.

Needs Python with machfs.
"""
import csv
import os
import re
import sys

import machfs


def disc_folder(name):
    """'ADC Developer CD (1992-05) ("The Byte Stuff")_iso' -> '1992-05 The Byte Stuff'."""
    title = re.search(r'"([^"]+)"', name)
    date = re.search(r'\((\d{4}-\d{2})', name)
    vol = re.search(r'Volume (\d+)', name)
    when = date.group(1) if date else (f'Vol {vol.group(1)}' if vol else '')
    what = title.group(1) if title else re.sub(r'^ADC Developer CD |_(iso|mdf|dmg)$|\(\d{4}-\d{2}\)|[()\[\]]', '', name).strip()
    return f'{when} {what}'.strip()[:31].strip()


def finder_info(disc_dir):
    info = {}
    path = os.path.join(disc_dir, 'FINDER.tsv')
    if os.path.exists(path):
        for line in open(path, encoding='utf-8'):
            # Path, type, creator; type and creator are four characters each
            # and can themselves hold a tab (some Apple II files).
            line = line.rstrip('\n')
            p, kind, creator = line[:-10], line[-9:-5], line[-4:]
            info[p] = (kind.encode('mac_roman', 'replace')[:4], creator.encode('mac_roman', 'replace')[:4])
    return info


def mac_name(name):
    # devcds.py wrote a Mac '/' in a name as ':'.
    return name.replace(':', '/')[:31]


def main():
    devcds, out = sys.argv[1:3]
    rows = list(csv.DictReader(open(os.path.join(devcds, 'INDEX.tsv'), encoding='utf-8'), delimiter='\t', quoting=csv.QUOTE_NONE))
    folders = sorted({os.path.dirname(r['path']) for r in rows})
    vol = machfs.Volume()
    vol.name = 'Apple Stacks'
    infos = {}
    files = 0
    for folder in folders:
        disc, _, inside = folder.partition('/')
        if disc not in infos:
            infos[disc] = finder_info(os.path.join(devcds, disc))
        # Make the folder path on the disk.
        here = vol.setdefault(disc_folder(disc), machfs.Folder())
        for part in [p for p in inside.split('/') if p]:
            here = here.setdefault(mac_name(part), machfs.Folder())
        src = os.path.join(devcds, folder)
        for name in sorted(os.listdir(src)):
            path = os.path.join(src, name)
            if os.path.isdir(path) or (name.endswith('.rsrc') and os.path.exists(path[:-5])):
                continue
            f = machfs.File()
            f.data = open(path, 'rb').read()
            if os.path.exists(path + '.rsrc'):
                f.rsrc = open(path + '.rsrc', 'rb').read()
            rel = os.path.join(inside, name)
            if rel in infos[disc]:
                f.type, f.creator = infos[disc][rel]
            elif f.data[4:8] == b'STAK':
                f.type, f.creator = b'STAK', b'WILD'
            else:
                f.type, f.creator = b'????', b'????'
            here[mac_name(name)] = f
            files += 1
    size = sum(len(f.data) + len(f.rsrc) for f in iter_files(vol))
    # Room for HFS's allocation blocks (32K on a disk this size) and to spare.
    disk = (size * 3 // 2 // (1 << 20) + files * 64 // 1024 + 64) << 20
    disk = min(disk, (2 << 30) - (1 << 20))
    with open(out, 'wb') as fh:
        fh.write(vol.write(size=disk))
    print(f'{out}: {files} files in {len(folders)} folders, {disk >> 20} MB')


def iter_files(folder):
    for item in folder.values():
        if isinstance(item, machfs.File):
            yield item
        else:
            yield from iter_files(item)


main()
