"""Put Mac files into Basilisk II's shared folder (its "Unix" disk), with their
resource forks and Finder types, so the running Mac sees them at once.

usage: to_shared.py SHARED_DIR FILE [TYPE CREATOR]

FILE is a file as unar unpacks it (its resource fork beside it as FILE.rsrc,
raw or AppleDouble). Disk images (.dsk, .img, .image) get Disk Copy's type,
so double-clicking one on the Mac mounts it. Otherwise give TYPE and CREATOR,
or the file gets '????'.

Basilisk keeps a file's resource fork in .rsrc/NAME and its Finder info in
.finf/NAME, beside it.
"""
import os
import shutil
import struct
import sys


def resource_fork(raw):
    if raw[:4] == b'\x00\x05\x16\x07':
        for i in range(struct.unpack('>H', raw[24:26])[0]):
            eid, off, length = struct.unpack('>III', raw[26 + 12 * i : 38 + 12 * i])
            if eid == 2:
                return raw[off : off + length]
        return b''
    return raw


def main():
    shared, src = sys.argv[1:3]
    name = os.path.basename(src)
    if len(sys.argv) > 4:
        kind, creator = sys.argv[3].encode('mac_roman'), sys.argv[4].encode('mac_roman')
    elif name.lower().endswith(('.dsk', '.img', '.image')):
        kind, creator = b'dImg', b'dCpy'
    else:
        kind, creator = b'????', b'????'
    for sub in ('.rsrc', '.finf'):
        os.makedirs(os.path.join(shared, sub), exist_ok=True)
    shutil.copyfile(src, os.path.join(shared, name))
    if os.path.exists(src + '.rsrc'):
        open(os.path.join(shared, '.rsrc', name), 'wb').write(resource_fork(open(src + '.rsrc', 'rb').read()))
    # FInfo (type, creator, flags, location, folder) then FXInfo: 32 bytes.
    finf = kind[:4].ljust(4) + creator[:4].ljust(4) + bytes(24)
    open(os.path.join(shared, '.finf', name), 'wb').write(finf)
    print(os.path.join(shared, name))


main()
