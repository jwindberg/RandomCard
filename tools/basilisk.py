"""Build a Basilisk II setup for installing and running HyperCard 2.4.1.

Basilisk II emulates a 68040 Macintosh (here a Quadra 650), which runs System
7.5 and the last HyperCard. This makes the disks and two configurations:

  install.prefs  boots System 7.5.3's Disk Tools 1 floppy, with the System
                 7.5.3 installer, the 7.5.5 update floppies
                 and the HyperCard 2.4.1 install disk, and a blank hard disk
                 to install onto
  run.prefs      boots the hard disk once everything is installed, with the
                 the Developer CD stacks

usage: basilisk.py APPLE_DIR OUT_DIR

To add a game or other disk later, put a .dsk in OUT_DIR/extras (to_dsk.py
turns a .sit or old disk image into one); run.sh mounts them all.

APPLE_DIR holds 7_5_3.sit, 7_5_5.sit, hypercard-2.4.1/HyperCard241.dsk,
Old_World_Mac_Roms and, optionally, Risk_II.img_.sit. Needs unar, and Python
with machfs.
"""
import os
import shutil
import subprocess
import sys

import machfs

HERE = os.path.dirname(os.path.abspath(__file__))
# The Quadra 650. (With the Quadra 900's ROM, System 7.1 and 7.5.3 both stop
# at startup with "unimplemented trap".)
ROM = 'Old_World_Mac_Roms/1MB ROMs/1993-02 - F1ACAD13 - Quadra, Centris 610,650,800.ROM'
HD_MB = 500


def unar(archive, into):
    if not os.path.isdir(into):
        subprocess.run(['unar', '-q', '-k', 'visible', '-o', into, archive], check=True)


def ndif(image, out):
    subprocess.run([sys.executable, os.path.join(HERE, 'ndif.py'), image, image + '.rsrc', out], check=True)


def main():
    apple, out = (os.path.abspath(a) for a in sys.argv[1:3])
    work = os.path.join(out, 'work')
    disks = os.path.join(out, 'disks')
    os.makedirs(work, exist_ok=True)
    os.makedirs(disks, exist_ok=True)
    unar(os.path.join(apple, '7_5_3.sit'), os.path.join(work, '753'))
    unar(os.path.join(apple, '7_5_5.sit'), os.path.join(work, '755'))
    src753 = os.path.join(work, '753', '7.5.3')

    # The System 7.5.3 installer: a compressed Disk Copy 6 image.
    ndif(os.path.join(src753, 'System 7.5.3 Net Install', 'Sys7.5 V7.5.3 CD Inst.img'),
         os.path.join(disks, 'System 7.5.3 Installer.dsk'))

    # The startup disk: 7.5.3's Disk Tools 1 floppy, a Disk Copy 4.2 image (an
    # 84-byte header before the disk). (Disk Tools PPC, beside the installer,
    # is cut down for PowerPC and won't start a 68k Mac.)
    images = os.path.join(src753, '7.5.3 Disk Images')
    # The folder's name is padded with non-breaking spaces.
    software = next(d for d in os.listdir(images) if d.strip('\u00a0 ') == 'System Software')
    tools = os.path.join(images, software, 'Disk Tools 1.image')
    data = open(tools, 'rb').read()
    open(os.path.join(disks, 'Disk Tools 7.5.3.dsk'), 'wb').write(data[84 : 84 + 1474560])

    # The 7.5.5 update floppies: Disk Copy 4.2 images, an 84-byte header
    # before the disk.
    floppies = os.path.join(work, '755', '7.5.5', 'System 7.5.5 Floppies')
    for n in (1, 2, 3):
        data = open(os.path.join(floppies, f'System 7.5.5 Update - {n}'), 'rb').read()
        open(os.path.join(disks, f'System 7.5.5 Update {n}.dsk'), 'wb').write(data[84 : 84 + 1474560])

    # HyperCard 2.4.1's install disk is a plain HFS image.
    shutil.copy2(os.path.join(apple, 'hypercard-2.4.1', 'HyperCard241.dsk'), os.path.join(disks, 'HyperCard 2.4.1.dsk'))

    # Extras: Risk II (ColorRisk 2.41) on its 800K floppy.
    extras = []
    risk = os.path.join(apple, 'Risk_II.img_.sit')
    if os.path.exists(risk):
        unar(risk, os.path.join(work, 'risk'))
        shutil.copy2(os.path.join(work, 'risk', 'Risk II.img'), os.path.join(disks, 'Risk II.dsk'))
        extras.append('Risk II.dsk')

    # A blank hard disk, already formatted, to install onto.
    hd = os.path.join(disks, 'Macintosh HD.dsk')
    if not os.path.exists(hd):
        blank = machfs.Volume()
        blank.name = 'Macintosh HD'
        with open(hd, 'wb') as f:
            f.write(blank.write(size=HD_MB * 1024 * 1024))

    shutil.copy2(os.path.join(apple, ROM), os.path.join(out, 'Quadra650.ROM'))
    common = [
        f'rom {out}/Quadra650.ROM',
        'modelid 14',
        'cpu 4',
        'fpu true',
        'ramsize 67108864',
        'screen win/1024/768',
        'displaycolordepth 8',
        'nocdrom true',
        'nosound false',
        'frameskip 0',
        'ignoresegv true',
        # Start the Mac straight away, not the settings window.
        'nogui true',
        # A folder shared with the Mac, as a disk named Unix (otherwise
        # Basilisk shares the whole Linux filesystem).
        f'extfs {out}/shared',
    ]

    os.makedirs(os.path.join(out, 'shared'), exist_ok=True)

    def prefs(name, order):
        # Disk Tools is a floppy-only system: it won't start from a hard disk.
        lines = common + [f'{"floppy" if d.startswith("Disk Tools") else "disk"} {disks}/{d}' for d in order]
        open(os.path.join(out, name), 'w').write('\n'.join(lines) + '\n')

    prefs('install.prefs', ['Disk Tools 7.5.3.dsk', 'Macintosh HD.dsk', 'System 7.5.3 Installer.dsk',
                            'System 7.5.5 Update 1.dsk', 'System 7.5.5 Update 2.dsk', 'System 7.5.5 Update 3.dsk',
                            'HyperCard 2.4.1.dsk'] + extras)
    # The Developer CD stacks, if tools/stacks_disk.py has made their disk.
    if os.path.exists(os.path.join(disks, 'Apple Stacks.dsk')):
        extras.append('Apple Stacks.dsk')
    # Once installed, the Mac needs only its hard disk and the extras.
    # (Risk II's floppy is only for copying ColorRisk onto the hard disk.)
    prefs('run.prefs', ['Macintosh HD.dsk'] + [e for e in extras if e != 'Risk II.dsk'])
    os.makedirs(os.path.join(out, 'extras'), exist_ok=True)
    scripts = {
        'install': 'exec BasiliskII --config install.prefs\n',
        # Every .dsk in extras/ is mounted too (tools/to_dsk.py makes them).
        'run': 'cp run.prefs .run.prefs\n'
               'for d in extras/*.dsk; do [ -f "$d" ] && echo "disk $PWD/$d" >> .run.prefs; done\n'
               'exec BasiliskII --config .run.prefs\n',
    }
    for name, body in scripts.items():
        path = os.path.join(out, f'{name}.sh')
        open(path, 'w').write('#!/bin/sh\ncd "$(dirname "$0")"\n' + body)
        os.chmod(path, 0o755)
    shutil.rmtree(work)
    print(f'made {out}: ./install.sh to install, then ./run.sh')


main()
