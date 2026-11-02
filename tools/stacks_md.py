"""Turn the INDEX.tsv that the index_stacks test writes into STACKS.md: a
summary, the commonest problems, and every stack with what it is and how far
it works in RandomCard.

usage: stacks_md.py INDEX.tsv STACKS.md
"""
import collections
import csv
import re
import sys
import urllib.parse

ORDER = ['opens cleanly', 'opens; some scripts don\'t parse', 'errors on opening', 'doesn\'t import', 'crashed', 'hung']
LABEL = {
    'opens cleanly': 'Opens cleanly',
    'opens; some scripts don\'t parse': 'Opens; some scripts don\'t parse',
    'errors on opening': 'Errors on opening',
    'doesn\'t import': 'Doesn\'t import',
    'crashed': 'Crashed',
    'hung': 'Hung',
}


def short_disc(name):
    """'ADC Developer CD (1992-05) ("The Byte Stuff")_iso' -> 'The Byte Stuff (1992-05)'."""
    title = re.search(r'"([^"]+)"', name)
    date = re.search(r'\((\d{4}-\d{2}(?:, \d+)?)\)', name)
    vol = re.search(r'Volume (\d+(?:,\d)?)', name)
    if title:
        when = date.group(1) if date else (f'Vol. {vol.group(1)}' if vol else '')
        return f'{title.group(1)} ({when})' if when else title.group(1)
    return name.replace('ADC Developer CD ', '').replace('_iso', '').replace('_mdf', '').replace('_dmg', '')


def cell(s):
    return s.replace('|', '\\|').strip()


def main():
    rows = list(csv.DictReader(open(sys.argv[1], encoding='utf-8'), delimiter='\t', quoting=csv.QUOTE_NONE))
    counts = collections.Counter(r['works'] for r in rows)
    problems = collections.Counter(
        re.sub(r'"[^"]*"', '"…"', re.sub(r'line \d+', 'line N', r['first_error']))
        for r in rows if r['first_error']
    )
    out = ['# Stacks on Apple\'s Developer CDs', '',
           'Every distinct stack on the 1989–1994 Developer CDs (unpacked by `tools/devcds.py`),',
           'what it is, and how far it works in RandomCard. Identical copies on several discs',
           'are listed once. Made by the `index_stacks` test and `tools/stacks_md.py`.', '',
           '"Opens" means the stack imports and its openStack, openBackground and openCard',
           'handlers run without a script error. Clicking every button is a later, separate run.', '',
           '| Result | Stacks |', '| --- | --- |']
    for k in ORDER:
        if counts[k]:
            out.append(f'| {LABEL[k]} | {counts[k]} |')
    out += [f'| **All** | **{len(rows)}** |', '', '## Commonest problems', '', '| Problem | Stacks |', '| --- | --- |']
    for p, n in problems.most_common(15):
        out.append(f'| {cell(p)} | {n} |')
    out += ['', '## Every stack', '',
            'Each name links to the stack file, relative to this page. "Where" is the disc and',
            'folder it\'s in; identical copies on other discs are counted after it.', '',
            '| Stack | Result | Cards | What it is | Where |', '| --- | --- | --- | --- | --- |']
    for r in sorted(rows, key=lambda r: r['name'].lower()):
        discs = r['discs'].split('; ')
        disc, _, inside = r['path'].partition('/')
        folder = inside.rpartition('/')[0] or '(top level)'
        found = f'{short_disc(disc)}: `{folder}`' + (f' (and {len(discs) - 1} more discs)' if len(discs) > 1 else '')
        link = f'[{cell(r["name"])}]({urllib.parse.quote(r["path"])})'
        about = r['about'] or r['folder']
        result = LABEL.get(r['works'], r['works'])
        if r['works'] == 'errors on opening' and r['first_error']:
            result += f': {r["first_error"][:80]}'
        out.append(f'| {link} | {cell(result)} | {r["cards"]} | {cell(about)} | {cell(found)} |')
    open(sys.argv[2], 'w', encoding='utf-8').write('\n'.join(out) + '\n')
    print(f'{len(rows)} stacks')


main()
