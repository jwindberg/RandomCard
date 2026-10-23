# RandomCard

A recreation of Apple's HyperCard in Rust: stacks of cards, backgrounds, buttons
and fields, MacPaint-style painting, and a HyperTalk interpreter with
HyperCard's message passing.

## Running

```sh
cargo run --release                 # opens the built-in tour stack
cargo run --release -- my.rcstack   # opens a stack file
cargo run --release -- "Home"       # imports an original HyperCard stack
cargo test                          # language, runtime and demo-stack tests
```

## What's there

**Stacks.** A stack holds backgrounds and cards. Each card belongs to a
background, and both layers have a picture plus buttons and fields. Background
fields keep separate text on each card unless `sharedText` is set. Background
buttons work the same way with `sharedHilite`. Stacks save as JSON
(`.rcstack`). Like HyperCard, changes are saved automatically once a stack has
a file.

**Importing HyperCard stacks.** File ▸ Import HyperCard Stack… (or
Open Stack…, or a path on the command line) reads original HyperCard 2.x
stack files. MacBinary-wrapped files work too. The import brings in:

- cards in stack order, backgrounds, card and background names, marked
  cards, and the cantDelete, dontSearch and showPict flags;
- card and background pictures, decoded from HyperCard's WOBA compression;
- buttons and fields with their styles, flags, families, fonts, sizes and
  text styles;
- shared and per-card field text, and per-card hilites of background
  buttons;
- every script;
- button icons and sounds from the stack's resource fork.

Text is converted from MacRoman. The result opens as an unsaved stack;
save it to keep it as a `.rcstack`.

Linux and Windows have no resource forks, so RandomCard looks where Mac
files usually carry them: a `Name.rsrc` file beside the stack, an
AppleDouble `._Name` file (also inside `__MACOSX/`), Netatalk's
`.AppleDouble/` and SheepShaver's `.rsrc/` folders, the stack file itself
if it's MacBinary or AppleSingle, and the real fork on macOS. Icons and
sounds are saved into the `.rcstack`. Sounds can be 8- or 16-bit, mono or
stereo, uncompressed or IMA4-compressed; MACE-compressed sounds are skipped.

Many stacks use HyperCard's built-in icons, which live in the HyperCard
application rather than the stack. As HyperCard did, RandomCard follows a
resource chain for icons and sounds the stack lacks: files named `Home`, `HyperCard`
or `HyperCard Player` beside the stack, then any resource files you put in
`~/.config/randomcard/resources/` (such as the resource fork of your own
copy of HyperCard). Only icons the stack's buttons use, and sounds its
scripts name in quotes, are copied.

What doesn't come across: other resources (XCMDs/XFCNs, color)
and styled runs within field text, which import as plain text. HyperCard 1.x stacks must first be converted with
HyperCard 2.x. Private-access stacks import, since only the stack header is
encrypted. Scripts that use menus, palettes or other windows parse, but
those commands do nothing.

**Tools.** Browse, Button, Field, and the paint tools: Select, Pencil, Brush,
Eraser, Line, Spray, Rect, Round Rect, Bucket and Oval, with a Filled option
and line sizes. With the Button or Field tool you can drag to move, drag the
corner to resize, double-click for the Info dialog, Ctrl-drag to create, and
press Delete to remove. Paint tools draw on whichever layer you are editing.
Edit ▸ Background (Ctrl+B) switches to the background layer.

**Parts.** Button styles: transparent, opaque, rectangle, roundRect, shadow,
checkBox, radioButton, default and oval, with families, autoHilite and
enabled. Field styles: transparent, opaque, rectangle, shadow and scrolling,
with lockText, dontWrap, showLines, wideMargins, text font, size, style and
alignment.

**HyperTalk.** Scripts live on buttons, fields, cards, backgrounds and the
stack. Messages travel the HyperCard message path (part → card → background
→ stack) and can be passed on with `pass`.

- Control flow: `on`/`function` handlers, `if`/`then`/`else` (single-line,
  block and `else if` chains), `repeat` (forever, N times, while, until,
  `with i = a [down] to b`), `next repeat`, `exit repeat`, `exit handler`,
  `exit to HyperCard`, `return`, `global`, and `pass`.
- Expressions: full HyperTalk operator precedence, including `&`, `&&`, `div`,
  `mod`, `^`, `contains`, `is in`, `is not in`, `is within`, `is a number`,
  and `there is a card "x"`. Unquoted literals evaluate to their own name, as
  in HyperCard.
- Chunks: `char`, `word`, `item` and `line`, with ranges (`char 2 to 5 of`),
  ordinals (`first`, `last`, `middle`, `any`), nesting, and `put` /
  `delete` into chunks.
- Object references: `card field "Name"`, `bg btn 3 of card 2`,
  `button id 12`, `next card`, `first card of background "X"`, `me`,
  `the target`, `msg`, and `this stack`, plus references held in variables.
- Properties: name (short and long), id, number, rect, loc, left, top,
  width, height, visible, style, hilite, autoHilite, textFont, textSize,
  textStyle, textAlign, lockText, script, contents, marked, showPict,
  itemDelimiter, numberFormat, lockScreen, lockMessages, and more.
- Commands: put, get, set, add, subtract, multiply, divide, answer, ask
  (including password and file variants), go (including back and forth),
  find (normal, whole, word, chars and string), sort (lines, items or cards,
  numeric, text or dateTime, `by` an expression using `each`), mark and
  unmark, send, do, wait, visual effect, lock and unlock screen or messages,
  push and pop card, hide and show, delete, doMenu, choose tool, select,
  click at, type, convert, open/read/write/close file, beep, speak, and
  save stack.
- Functions: math (`abs`, `round`, `trunc`, `sqrt`, trig, `ln`, `exp`,
  `random`, `min`, `max`, `sum`, `average`, `annuity`, `compound`), text
  (`length`, `offset`, `charToNum`, `numToChar`, `value`), dates and times in
  short, long and abbreviated forms (`the seconds`, `the ticks`,
  `the dateItems`), mouse and keys (`the mouse`, `the mouseLoc`,
  `the clickLoc`, `the mouseClick`, `the shiftKey`), and `the result`,
  `the params`, `param(n)`, `the foundText`, and others.
- System messages: openStack, openBackground, openCard, closeCard, newCard,
  deleteCard, mouseDown, mouseUp, mouseStillDown, mouseEnter, mouseLeave,
  mouseWithin, mouseDoubleClick, openField, closeField, exitField, idle,
  arrowKey and doMenu. Menu commands go through `doMenu`, so scripts can
  intercept them.
- Sound: `play "boing"`, `play "harpsichord" tempo 200 "c4e d e f g a b c5h"`
  (notes as a–g or MIDI numbers with `#`/`b`, octave, `w h q e s t x`
  durations, dots and triplets), `play stop`, `the sound`, `dial` (touch
  tones) and `beep`. Sounds come from the stack; HyperCard's harpsichord,
  flute and boing fall back to voices RandomCard synthesizes itself.
- Visual effects: dissolve, wipe, scroll/push, barn door, iris, zoom,
  venetian blinds and checkerboard, at four speeds.

Scripts run on their own thread, so a runaway `repeat forever` never freezes
the window. Ctrl+. stops it, like Command-period. Script errors show the line
and offer to open the script editor there.

## Keyboard

| Keys | Action |
| --- | --- |
| Ctrl+1 / 2 / 3 / 4 | First / previous / next / last card |
| ← → | Previous / next card (an `arrowKey` message) |
| Ctrl+N | New card |
| Ctrl+M | Message box |
| Ctrl+B | Edit background |
| Ctrl+Alt+C / B / S | Card / background / stack script |
| Ctrl+Tab | Browse tool |
| Ctrl+Z | Undo paint |
| Ctrl+. | Abort running script |
| Ctrl+Space | Hide or show the menu bar |
| Ctrl+S, Ctrl+O, Ctrl+Q | Save, open, quit |

## Layout

```
src/
  model/       Stack, Background, Card, Part, Picture; JSON save/load; demo stack
  import/      reader for original HyperCard 2.x stack files, WOBA decoder
  hypertalk/   lexer, parser, syntax tree, values and chunk expressions
  runtime/     shared World state, interpreter, objects and properties,
               commands, functions, dates; end-to-end tests
  sound/       note parsing, rendering, built-in voices and audio output
  ui/          egui app: menus, palette, card rendering, transitions,
               paint tools, info dialogs, script editor
```

## Not (yet) implemented

The main gaps are paint patterns and some paint tools, keyboard messages,
styled text in fields, scriptable menus, multiple open stacks, and
XCMDs. [ROADMAP.md](ROADMAP.md) lists the remaining work by area, with
notes on how to approach each piece and a suggested order.

## References

The language and object model follow HyperCard 2.x. These were used as
guides:

- [WyldCard](https://github.com/defano/wyldcard), Matt DeFano's Java
  reproduction of HyperCard. Its ANTLR HyperTalk grammar was the main
  reference for syntax.
- [ViperCard](https://github.com/moltenform/vipercard), a HyperCard
  reimplementation for the web.
- [HyperCardPreview](https://github.com/PierreLorenzi/HyperCardPreview),
  Pierre Lorenzi's HyperCard stack viewer (MIT License). The stack importer
  follows its file-format layouts and WyldCard's, `src/import/woba.rs`
  is a port of its WOBA bitmap decoder, and icon masks follow its method.

To test the importer against real stacks, point it at HyperCardPreview's
sample files:

```sh
git clone --depth 1 https://github.com/PierreLorenzi/HyperCardPreview
RANDOMCARD_HC_SAMPLES=HyperCardPreview/HyperCardCommon/HyperCardCommonTests \
  cargo test -- --include-ignored
```

Both are cloned into `reference/`, which is git-ignored, if you want to read
them alongside this code.

HyperCard and HyperTalk are trademarks of Apple Inc. This project is not
affiliated with Apple.
