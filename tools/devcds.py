"""Fetch Apple's Developer CDs from the Internet Archive and unpack each one's
HFS volume, keeping resource forks as NAME.rsrc beside each file.

These are the stacks RandomCard is tested against: hundreds of Apple's own,
with their icons, sounds and pictures. Each disc goes in its own folder, with
a STACKS.txt listing its stacks; discs already unpacked are skipped. Put them
under examples/apple/devcds/, which git ignores.

usage:
  devcds.py list                      the discs it knows (1989-1994)
  devcds.py get OUT_DIR [MATCH...]    download and unpack (all, or names containing MATCH)
  devcds.py unpack IMAGE OUT_DIR      unpack one image already on disk

Needs Python with machfs (pip install machfs, for classic HFS volumes), and
curl, 7z and unar.
"""
import json
import os
import shutil
import struct
import subprocess
import sys
import urllib.parse
import urllib.request

import machfs

ITEM = 'Apple_Developer_Discs_1989-2009'
YEARS = ('1989/', '1990/', '1991/', '1992/', '1993/', '1994/')
SECTOR = 2048


def discs():
    with urllib.request.urlopen(f'https://archive.org/metadata/{ITEM}') as r:
        meta = json.load(r)
    files = [f for f in meta['files'] if f.get('source') == 'original' and f['name'].startswith(YEARS)]
    return sorted(files, key=lambda f: f['name'])


def disc_dir_name(name):
    base = os.path.basename(name)
    for ext in ('.rar', '.sit'):
        if base.endswith(ext):
            base = base[: -len(ext)]
    return base.replace("''", '"')


def raw_to_iso(data):
    """A raw CD image (.mdf: 2352-byte sectors, or 2448 with subchannel data)
    as plain 2048-byte sectors."""
    sync = b'\x00' + b'\xff' * 10 + b'\x00'
    if data[:12] != sync:
        return data
    size = next((n for n in (2352, 2448) if data[n : n + 12] == sync), 2352)
    out = bytearray()
    for i in range(0, len(data) - size + 1, size):
        s = data[i : i + 2352]
        mode = s[15]
        out += s[16:2064] if mode == 1 else s[24:2072]
    return bytes(out)


def hfs_volumes(data):
    """The HFS volumes in a disk image: from its partition map, or the whole image."""
    if data[:2] == b'ER' and data[512:514] == b'PM':
        count = struct.unpack('>I', data[516:520])[0]
        vols = []
        for i in range(1, count + 1):
            e = data[i * 512 : i * 512 + 512]
            if e[:2] != b'PM':
                break
            start, length = struct.unpack('>II', e[8:16])
            kind = e[48:80].split(b'\0')[0]
            if kind == b'Apple_HFS':
                vols.append(data[start * 512 : (start + length) * 512])
        return vols
    if data[1024:1026] == b'BD':
        return [data]
    return []


def safe(name):
    return name.replace('/', ':') or '_'


def walk(folder, out, stacks, counts, finder, root=None):
    root = root or out
    os.makedirs(out, exist_ok=True)
    for name, item in folder.items():
        path = os.path.join(out, safe(name))
        if isinstance(item, machfs.Folder):
            walk(item, path, stacks, counts, finder, root)
            continue
        finder.append((path, item.type.decode('mac_roman'), item.creator.decode('mac_roman')))
        with open(path, 'wb') as f:
            f.write(item.data)
        counts['files'] += 1
        if item.rsrc:
            with open(path + '.rsrc', 'wb') as f:
                f.write(item.rsrc)
        if item.type == b'STAK':
            stacks.append(os.path.relpath(path, root))


def unpack_hfs_plus(volume, out):
    """An HFS+ volume (some discs were re-imaged in it), through 7-Zip. Type
    codes don't come through, so stacks are found by their data."""
    os.makedirs(out, exist_ok=True)
    part = os.path.join(out, '.volume.hfs')
    with open(part, 'wb') as f:
        f.write(volume)
    run('7z', 'x', '-y', '-sns', f'-o{out}', part)
    os.remove(part)
    # Files keep the volume's permissions; make them readable.
    run('chmod', '-R', 'u+rwX', out)
    files, stacks = 0, []
    for dp, _, names in os.walk(out):
        for n in names:
            path = os.path.join(dp, n)
            if n in ('.journal', '.journal_info_block') or n.startswith('\0'):
                os.remove(path)
                continue
            if n.endswith(':rsrc'):
                os.replace(path, path[: -len(':rsrc')] + '.rsrc')
                continue
            files += 1
            with open(path, 'rb') as f:
                if f.read(8)[4:8] == b'STAK':
                    stacks.append(os.path.relpath(path, out))
    return files, stacks


def unpack_image(image, out):
    with open(image, 'rb') as f:
        data = raw_to_iso(f.read())
    vols = hfs_volumes(data)
    if not vols:
        raise SystemExit(f'no HFS volume found in {image}')
    stacks, counts, finder = [], {'files': 0}, []
    for n, v in enumerate(vols):
        dest = out if len(vols) == 1 else os.path.join(out, f'volume {n + 1}')
        if v[1024:1026] in (b'H+', b'HX'):
            files, found = unpack_hfs_plus(v, dest)
            counts['files'] += files
            stacks += [os.path.relpath(os.path.join(dest, s), out) for s in found]
            continue
        vol = machfs.Volume()
        vol.read(v)
        walk(vol, dest, stacks, counts, finder)
    # Each file's Mac type and creator, which HyperCard and the Finder use.
    with open(os.path.join(out, 'FINDER.tsv'), 'w') as f:
        for path, kind, creator in finder:
            f.write(f'{os.path.relpath(path, out)}\t{kind}\t{creator}\n')
    with open(os.path.join(out, 'STACKS.txt'), 'w') as f:
        f.write('\n'.join(sorted(stacks)) + '\n')
    return counts['files'], len(stacks)


def run(*cmd):
    subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL)


def get(out_root, matches):
    failed = []
    for f in discs():
        try:
            get_one(f, out_root, matches)
        except (subprocess.CalledProcessError, SystemExit, Exception) as e:
            print(f'FAILED {f["name"]}: {e}', flush=True)
            failed.append(f['name'])
    if failed:
        print(f'{len(failed)} failed; run again to retry them')


def get_one(f, out_root, matches):
    name = f['name']
    if matches and not any(m.lower() in name.lower() for m in matches):
        return
    dest = os.path.join(out_root, disc_dir_name(name))
    if os.path.exists(os.path.join(dest, 'STACKS.txt')):
        print(f'have   {name}')
        return
    work = os.path.join(out_root, '.work')
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(work)
    archive = os.path.join(work, os.path.basename(name))
    url = f'https://archive.org/download/{ITEM}/' + urllib.parse.quote(name)
    print(f'fetch  {name} ({int(f["size"]) // 1_000_000} MB)', flush=True)
    # archive.org drops long downloads now and then: retry, resuming.
    run('curl', '-s', '-L', '--fail', '--retry', '8', '--retry-delay', '30', '--retry-all-errors', '-C', '-', '-o', archive, url)
    if name.endswith('.sit'):
        # StuffIt archives hold files, not a disk image; unar keeps forks as AppleDouble.
        run('unar', '-q', '-k', 'visible', '-o', dest, archive)
        stacks = []
        with open(os.path.join(dest, 'STACKS.txt'), 'w') as s:
            s.write('')
        print(f'unpack {name}: unar (stacks not listed)')
    else:
        run('7z', 'x', '-y', f'-o{work}/x', archive)
        images = [os.path.join(dp, n) for dp, _, ns in os.walk(f'{work}/x') for n in ns
                  if n.lower().endswith(('.mdf', '.iso', '.dmg', '.cdr', '.img', '.toast', '.bin'))]
        image = max(images, key=os.path.getsize)
        if image.lower().endswith('.dmg'):
            # A compressed disk image: 7-Zip gives its raw volume (-tdmg, so
            # it doesn't try to open the classic HFS inside), or it's plain.
            try:
                run('7z', 'x', '-y', '-tdmg', f'-o{work}/dmg', image)
                parts = [os.path.join(dp, n) for dp, _, ns in os.walk(f'{work}/dmg') for n in ns]
                image = max(parts, key=os.path.getsize)
            except subprocess.CalledProcessError:
                pass
        files, stacks = unpack_image(image, dest)
        print(f'unpack {name}: {files} files, {stacks} stacks', flush=True)
    shutil.rmtree(work, ignore_errors=True)


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else 'list'
    if cmd == 'list':
        total = 0
        for f in discs():
            total += int(f['size'])
            print(f"{int(f['size']) // 1_000_000:6} MB  {f['name']}")
        print(f'{total / 1e9:.1f} GB')
    elif cmd == 'get':
        get(sys.argv[2], sys.argv[3:])
    elif cmd == 'unpack':
        files, stacks = unpack_image(sys.argv[2], sys.argv[3])
        print(f'{files} files, {stacks} stacks')


main()
