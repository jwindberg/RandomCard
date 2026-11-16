"""HyperCard 2.4.1's own stacks as tests: list them for the click_every_button
test, and turn its results into HYPERCARD_241_STACKS.md and the list of clean
stacks the hypercard_241_stacks_stay_clean test keeps clean.

The stacks are the ones on HyperCard 2.4.1's installation disk, copied to
examples/apple/hypercard_241_stacks with their resource forks in .rsrc/
(Apple's, so not in git).

usage:
  hc241.py list STACKS_DIR > LEVEL4.list
      Every file there but the forks and Finder info, as absolute paths.
  hc241.py report STACKS_DIR LEVEL4.tsv
      Write HYPERCARD_241_STACKS.md and tests/hc241_clean.txt in the repo.

A full run, from the repo:
  python3 tools/hc241.py list examples/apple/hypercard_241_stacks > /tmp/hc241/LEVEL4.list
  RANDOMCARD_LEVEL4=/tmp/hc241/LEVEL4.list cargo test --release click_every_button -- --ignored
  python3 tools/hc241.py report examples/apple/hypercard_241_stacks /tmp/hc241/LEVEL4.tsv
"""
import csv
import datetime
import os
import sys

REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')

# Clean stacks too slow for every `cargo test` (none now: with no one
# watching, effects and waits don't pause): marked slow in the list, so the
# regular `cargo test` leaves them to RANDOMCARD_HC241_SLOW=1 and full runs.
SLOW = set()


def stack_files(top):
    for root, dirs, files in os.walk(top):
        dirs[:] = sorted(d for d in dirs if d not in ('.rsrc', '.finf'))
        for f in sorted(files):
            yield os.path.join(os.path.abspath(root), f)


def cell(s):
    return s.replace('|', '\\|').replace('\n', ' ').strip()


def report(top, tsv):
    top = os.path.abspath(top)
    rows = list(csv.DictReader(open(tsv, encoding='utf-8'), delimiter='\t', quoting=csv.QUOTE_NONE))
    for r in rows:
        r['name'] = os.path.relpath(r['path'], top)
    rows.sort(key=lambda r: r['name'].lower())
    clean = [r for r in rows if r['result'] in ('every click clean', 'nothing to click')]
    errors = [r for r in rows if r['result'] == 'errors' or r['result'] == 'crashed']
    others = [r for r in rows if r not in clean and r not in errors]
    today = datetime.date.today().isoformat()
    out = [
        "# HyperCard 2.4.1's own stacks",
        '',
        'The stacks that came with HyperCard 2.4.1, from its installation disk:',
        "Apple's own, for the version RandomCard follows, using its newest features.",
        "They live in `examples/apple/hypercard_241_stacks` (Apple's, so not in git).",
        '',
        'Each stack is opened, and every scripted button on every card clicked, by',
        "the `click_every_button` test; `tools/hc241.py` lists the stacks and writes",
        'this page from the results. A clean stack gets no script error on any',
        'click. "Stopped" counts clicks that ran into the two-second limit, usually',
        "a timed demo or an animation, which aren't errors.",
        '',
        'Clean stacks are kept clean: `tests/hc241_clean.txt` lists them, and the',
        '`hypercard_241_stacks_stay_clean` test, part of `cargo test` wherever the',
        'stacks are, fails if one of them gets an error; visual effects and `wait`',
        "don't pause, with no one watching, so all of them take seconds. A stack",
        'joins the list when this page is written again after a fix. What breaks',
        "the others is in the roadmap's \"HyperCard 2.4.1's own stacks\" section.",
        '',
        f'**{len(clean)} of {len(clean) + len(errors)} stacks are clean** ({today}).',
        '',
        '## Clean',
        '',
        '| Stack | Clicks | Stopped |',
        '| --- | --- | --- |',
    ]
    out += [f"| {cell(r['name'])} | {r['clicks']} | {r['stopped']} |" for r in clean]
    out += ['', '## With errors', '', '| Stack | Clicks | Distinct errors | First error |', '| --- | --- | --- | --- |']
    out += [f"| {cell(r['name'])} | {r['clicks']} | {r['distinct_errors']} | {cell(r['first_error'])} |" for r in errors]
    if others:
        out += ['', '## Not stacks', '', "Files beside the stacks: pictures, fonts and movies.", '']
        out += [f"- {r['name']}" for r in others]
    out.append('')
    open(os.path.join(REPO, 'HYPERCARD_241_STACKS.md'), 'w', encoding='utf-8').write('\n'.join(out))
    lines = ['# HyperCard 2.4.1 stacks that click clean, relative to examples/apple/hypercard_241_stacks.',
             '# Written by tools/hc241.py; the hypercard_241_stacks_stay_clean test keeps them clean.',
             '# "slow:" ones run only with RANDOMCARD_HC241_SLOW=1.']
    lines += [('slow: ' if r['name'] in SLOW else '') + r['name'] for r in clean]
    open(os.path.join(REPO, 'tests', 'hc241_clean.txt'), 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
    print(f'{len(clean)} clean, {len(errors)} with errors, {len(others)} not stacks')


def main():
    if len(sys.argv) == 3 and sys.argv[1] == 'list':
        for p in stack_files(sys.argv[2]):
            print(p)
    elif len(sys.argv) == 4 and sys.argv[1] == 'report':
        report(sys.argv[2], sys.argv[3])
    else:
        sys.exit(__doc__)


if __name__ == '__main__':
    main()
