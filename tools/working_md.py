"""Write WORKING_STACKS.md: the stacks on Apple's Developer CDs that work fully
in RandomCard, from the results of the click_every_button test.

usage: working_md.py DEVCDS_DIR OUT.md RESULTS.tsv [MORE.tsv ...] [--fixes FIXES.tsv]
                     [--driven DRIVEN.tsv]

Results files are read in order, and a later row for the same stack replaces
an earlier one, so a recheck after a fix goes last. Names and descriptions
come from DEVCDS_DIR/INDEX.tsv. Links are relative to OUT.md.

FIXES.tsv, if given, lists stacks that work once fix steps are applied to our
copy: a path (relative to DEVCDS_DIR) and the fix steps, tab-separated. They
get a section of their own.

DRIVEN.tsv, if given, is the driven_checks test's results: stacks run through
hand-written steps (tests/driven) because clicking every button in card order
can't work for them. One that passes counts as working, marked ‡.
"""
import csv
import os
import sys
import urllib.parse

# Externals RandomCard answers as stubs: Mac details that mean nothing today,
# answered harmlessly (see XCMDS.md). Keep in step with src/runtime/externals.rs.
STUBS = ['BoardInfo', 'INITFinder', 'SpinAbout', 'SwitchXRoo']


def read(path):
    return list(csv.DictReader(open(path, encoding='utf-8'), delimiter='\t', quoting=csv.QUOTE_NONE))


def cell(s):
    return s.replace('|', '\\|').strip()


def stubs_called(path):
    """The stubs a stack's scripts name. Scripts are plain text in the file."""
    try:
        data = open(path, 'rb').read().lower()
    except OSError:
        return []
    return [s for s in STUBS if s.lower().encode() in data]


def main():
    args = sys.argv[1:]
    fixes = {}
    if '--fixes' in args:
        i = args.index('--fixes')
        for line in open(args[i + 1], encoding='utf-8'):
            if line.strip() and not line.startswith('#'):
                path, _, steps = line.rstrip('\n').partition('\t')
                fixes[path] = steps
        del args[i : i + 2]
    driven = {}
    if '--driven' in args:
        i = args.index('--driven')
        driven = {r['path']: r for r in read(args[i + 1]) if r['result'] == 'driven clean'}
        del args[i : i + 2]
    devcds, out_path, *results = args
    index = {r['path']: r for r in read(os.path.join(devcds, 'INDEX.tsv'))}
    latest = {}
    for f in results:
        for r in read(f):
            latest[r['path']] = r
    # A stack the click test failed but its driven steps pass counts, as ‡.
    for path, d in driven.items():
        if latest.get(path, {}).get('result') != 'every click clean':
            latest[path] = {'path': path, 'result': 'every click clean', 'clicks': f'{d["steps"]} steps', 'stopped': '0'}
    # A stack whose row says fix steps were applied (the click test's fixes
    # column) is listed with the reasons FIX_STEPS.tsv gives for its name.
    table = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'FIX_STEPS.tsv')
    reasons = {}
    for line in open(table, encoding='utf-8'):
        if line.strip() and not line.startswith('#'):
            f = line.rstrip('\n').split('\t')
            if len(f) >= 4:
                reasons.setdefault(f[0], [])
                if f[3] not in reasons[f[0]]:
                    reasons[f[0]].append(f[3])
    for r in latest.values():
        n = int(r.get('fixes') or 0)
        if n and r['path'] not in fixes:
            name = r['path'].rsplit('/', 1)[-1]
            fixes[r['path']] = f"{n} changes from FIX_STEPS.tsv: " + "; ".join(reasons.get(name, []))
    clean = [r for r in latest.values() if r['result'] == 'every click clean']
    working = [r for r in clean if r['path'] not in fixes]
    fixed = [r for r in clean if r['path'] in fixes]
    untested = [r for r in latest.values() if r['result'] == 'nothing to click']
    base = os.path.relpath(devcds, os.path.dirname(os.path.abspath(out_path)))
    stubbed = 0

    def row(r, extra=None):
        nonlocal stubbed
        info = index.get(r['path'], {})
        name = cell(info.get('name') or os.path.basename(r['path']))
        disc, _, inside = r['path'].partition('/')
        link = urllib.parse.quote(os.path.join(base, r['path']))
        discs = int(info.get('disc_count') or 1)
        where = f'{disc}: `{inside.rpartition("/")[0]}`' + (f' (and {discs - 1} more discs)' if discs > 1 else '')
        waits = ' (waited out)' if r.get('stopped', '0') != '0' else ''
        stubs = stubs_called(os.path.join(devcds, r['path']))
        mark = '*' if r['path'] in fixes else ''
        if r['path'] in driven:
            mark += '‡'
        about = cell(info.get('about', ''))
        if stubs:
            stubbed += 1
            mark += '†'
            about += f' (stubs: {", ".join(stubs)})'
        cells = [f'[{name}]({link}){mark}', f'{r["clicks"]}{waits}', about, cell(where)]
        if extra is not None:
            cells.insert(1, cell(extra))
        return '| ' + ' | '.join(cells) + ' |'

    key = lambda r: (index.get(r['path'], {}).get('name') or r['path']).lower()
    rows = [row(r) for r in sorted(working, key=key)]
    fixed_rows = [row(r, fixes[r['path']]) for r in sorted(fixed, key=key)]
    out = [
        '# Stacks that work fully', '',
        'Stacks from Apple\'s Developer CDs that open in RandomCard and survive a click on',
        'every scripted button on every card without a single script error. The more',
        'stacks on this list, the better RandomCard is doing; see BUGHUNT.md for what',
        'keeps the others off it.', '',
        'Made by the `click_every_button` test and `tools/working_md.py`. The stacks',
        'themselves live in `examples/apple/devcds` (unpacked by `tools/devcds.py`, not',
        'in git), so the links work once that\'s in place. "Waited out" means a click ran',
        'into the two-second limit, usually an animation or a loop waiting for a key',
        'that a headless run can\'t press; worth a look by hand.', '',
        '**Marks:**', '',
        '- **\\*** works fully once its fix steps are applied to our copy, such as renaming',
        '  a call to an old external to RandomCard\'s built-in. These are listed in their',
        '  own section below, with their fix steps. Fix steps are always small.',
        '- **†** works, but some Mac-only details answer empty: the stack calls a stub',
        '  (named after its description), such as INITFinder, which lists the Mac system',
        '  extensions running, and there are none on a computer today.',
        '- **‡** works, checked by running it through hand-written steps (in',
        '  `tests/driven`) instead of clicking every button in card order, which can\'t',
        '  work for it: a "Show" button that comes before the "Create" it needs.', '',
        f'**{len(clean)} stacks** work fully, of {len(latest)} tested so far: {len(working)} straight off the',
        f'disc and {len(fixed)} with fix steps. {stubbed} of them use stubs (†), and {len(driven)} are',
        'checked by driven steps (‡).', '',
        '| Stack | Clicks | What it is | Where |', '| --- | --- | --- | --- |',
    ]
    out += rows
    out += ['', '## Work with fix steps (*)', '']
    if fixed_rows:
        out += ['| Stack | Fix steps | Clicks | What it is | Where |', '| --- | --- | --- | --- | --- |'] + fixed_rows
    else:
        out += ['None yet.']
    out += ['', '## Open cleanly, nothing to click', '',
            f'These {len(untested)} open without errors but have no buttons with scripts, so',
            'the run proves little about them.', '',
            '| Stack | Clicks | What it is | Where |', '| --- | --- | --- | --- |']
    out += [row(r) for r in sorted(untested, key=key)]
    open(out_path, 'w', encoding='utf-8').write('\n'.join(out) + '\n')
    print(f'{len(clean)} working ({len(fixed)} with fix steps, {stubbed} with stubs), {len(untested)} with nothing to click')


main()
