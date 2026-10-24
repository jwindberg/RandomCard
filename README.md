# RandomCard

A recreation of Apple's HyperCard in Rust: stacks of cards, backgrounds, buttons
and fields, MacPaint-style painting, and a HyperTalk interpreter with
HyperCard's message passing.

## Running

```sh
cargo run --release                 # opens your Home stack
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

**Home and other stacks.** As HyperCard came with one, RandomCard makes a
Home stack the first time it runs, at `~/.config/randomcard/Home.rcstack`,
and opens it at startup. Go ▸ Home (Ctrl+H) and `go home` return to it, and
its button takes you to the built-in Tour. Scripts can `go to stack "X"` or
`go to card 3 of stack "X"`. The stack you leave is saved first, and one
without a file stays loaded so you can come back to it.

**Importing HyperCard stacks.** File ▸ Import HyperCard Stack… (or
Open Stack…, or a path on the command line) reads original HyperCard 1.x
and 2.x stack files. MacBinary-wrapped files work too. The import brings in:

- cards in stack order, backgrounds, card and background names, marked
  cards, and the cantDelete, dontSearch and showPict flags;
- card and background pictures, decoded from HyperCard's WOBA compression;
- buttons and fields with their styles, flags, families, fonts, sizes and
  text styles;
- shared and per-card field text, and per-card hilites of background
  buttons;
- every script;
- button icons and sounds from the stack's resource fork;
- AddColor colors (HyperCard 2.x's color XCMD) for buttons, fields and
  rectangles. They're drawn as AddColor drew them: the white of the card
  takes the color, and black stays black.

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

Styled text comes across with its fonts, sizes and styles. What doesn't
come across: XCMDs and XFCNs, and PICT pictures (including AddColor's color
pictures, which are kept but not drawn). Private-access stacks import, since only the stack header is
encrypted; they ask for their password. Protection comes across too:
the user-level limit, password, Can't Modify and the other flags. Scripts
that use palettes or other windows parse, but those commands do nothing.

**User levels.** As in HyperCard, five levels decide what you can do:
Browsing, Typing (edit field text), Painting (paint tools and the Paint
menu), Authoring (the Button and Field tools, the Objects menu, Info
dialogs) and Scripting (scripts). RandomCard runs at Scripting by default,
like the full HyperCard. Pick another level in File ▸ Preferences…. Stacks
can lower it with `set the userLevel`. The demo's Address Book drops to
Typing while you're in it.

File ▸ Protect Stack… sets Can't Modify, Can't Delete, Can't Abort, Can't
Peek and Private Access, limits the user level, and sets a password, using
HyperCard's own password scheme. A password is then needed to open Protect
Stack or, through Preferences ▸ Unlock with Password…, to go above the
limit. Protection stops casual changes. It isn't encryption.

The tool panel sits on the left at Painting level and above. Hide or show
it with View ▸ Tool Panel or Ctrl+T. The message box opens at the bottom of
the window. Preferences are saved in `~/.config/randomcard/settings.json`.

**Tools.** Browse, Button, Field, and HyperCard's paint tools: Select,
Lasso, Pencil, Brush, Eraser, Line, Spray, Rect, Round Rect, Bucket, Oval,
Curve, Text, Regular Polygon and Polygon. Brush, spray, bucket and filled
shapes paint with the current pattern from a 40-pattern palette (imported
stacks bring their own patterns). There are 16 brush shapes, line sizes, a
Filled option, and the number of sides for regular polygons. The Paint menu
has Select All, Fill, Invert, Darken, Lighten, Trace Edges, Rotate,
Flip, Opaque and Transparent, applied to the selection or the whole
picture.

**Editing.** The Edit menu's Cut, Copy, Paste and Clear act on the
selected button or field (with the Button or Field tool) or the paint
selection, as in HyperCard ("Copy Button", "Paste Picture"), and scripts
can use them with `doMenu`. Pasted buttons and fields keep their text and
hilite. Objects ▸ Bring to Front / Send to Back reorder parts. Undo and
redo (Ctrl+Z, Ctrl+Shift+Z) cover painting, moving, resizing, creating,
pasting, reordering and deleting parts, and new, deleted, cut and pasted
cards, in one history of 30 steps. With the Button or Field tool you can drag to move, drag the
corner to resize, double-click for the Info dialog, Ctrl-drag to create, and
press Delete to remove. Paint tools draw on whichever layer you are editing.
Edit ▸ Background (Ctrl+B) switches to the background layer.

**Parts.** Button styles: transparent, opaque, rectangle, roundRect, shadow,
checkBox, radioButton, default and oval, with families, autoHilite and
enabled. Field styles: transparent, opaque, rectangle, shadow and scrolling,
with lockText, dontWrap, showLines, wideMargins, text font, size, style and
alignment. Locked fields with autoSelect are list fields: a click selects
the whole line, and with multipleLines Shift-click extends the selection.

**Styled text.** Text in a field can mix fonts, sizes and styles (bold,
italic, underline, condense, extend, group). While you edit a field, the
Font and Style menus restyle the selected text; with the text tool they set
the style for painted text. Styles stay with their text as you type around
them. Clicking grouped text in a locked field makes `the clickText` the
whole group. Only egui's built-in fonts are available, so Courier and Monaco
show as one monospaced font and the rest as one sans-serif font.

**HyperCard's text metrics.** Text is drawn in egui's fonts but measured by
the classic Mac bitmap fonts HyperCard used (Chicago, Geneva, New York,
Monaco, Courier, Helvetica, Times, Palatino): each word takes the width it
had in HyperCard, and each line its height, so fields wrap and scroll as
they did and imported stacks fit their fields. Unknown fonts measure as
Geneva, as they did on the Mac. Fields use HyperCard's text margins,
`textHeight` and `fixedLineHeight`, and showLines draws its lines on the
text's baselines. The metrics table in `src/model/metrics/data.rs` was made
with `tools/font_metrics.py`; it holds numbers only, not the fonts.

**HyperTalk.** Scripts live on buttons, fields, cards, backgrounds and the
stack. Messages travel the HyperCard message path (part → card → background
→ stack → stacks in use → Home) and can be passed on with `pass`.
`start using stack "Library"` puts a stack's script in the path (it gets a
`libraryStack` message), `stop using` takes it out (`releaseStack`), and
`the stacksInUse` lists them. Handlers in Home's script work in every stack.

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
  textStyle, textAlign, textHeight, fixedLineHeight, lockText, script, contents, marked, showPict,
  itemDelimiter, numberFormat, lockScreen, lockMessages, and more.
- Painting from scripts: `choose oval tool`, `set the pattern to 3`,
  `set the filled to true`, `drag from 10,10 to 100,80`, `click at` (a
  bucket fill, a pencil dot, or placing the text tool), and `type` with the
  text tool paints text in `the textFont`, `textSize` and `textStyle`.
  `the brush`, `the lineSize` and `the polySides` work too.
- Commands: put, get, set, add, subtract, multiply, divide, answer, ask
  (including password and file variants), go (including back and forth),
  find (normal, whole, word, chars and string), sort (lines, items or cards,
  numeric, text or dateTime, `by` an expression using `each`), mark and
  unmark, send, do, wait, visual effect, lock and unlock screen or messages,
  push and pop card, hide and show, delete, doMenu, choose tool, select
  (a part, `empty`, or text: `select line 2 of field 1`,
  `select after text of field "Notes"`),
  click at, type, convert, open/read/write/close file, beep, speak, and
  save stack.
- Functions: math (`abs`, `round`, `trunc`, `sqrt`, trig, `ln`, `exp`,
  `random`, `min`, `max`, `sum`, `average`, `annuity`, `compound`), text
  (`length`, `offset`, `charToNum`, `numToChar`, `value`), dates and times in
  short, long and abbreviated forms (`the seconds`, `the ticks`,
  `the dateItems`), mouse and keys (`the mouse`, `the mouseLoc`,
  `the clickLoc`, `the mouseClick`, `the shiftKey`), and `the result`,
  `the params`, `param(n)`, `the foundText`, and others.
- The selection: `the selectedText`, `selectedChunk`, `selectedLine`,
  `selectedField` and `selectedLoc`, and `the selection` as a container
  (`put "x" into the selection`). Clicking a button keeps the text
  selection, so its script can use it. Clicks in locked fields set
  `the clickText`, `clickChunk` and `clickLine`.
- Text properties of chunks and of the selection:
  `set the textStyle of word 2 of field 1 to bold`,
  `the textFont of the selection` ("mixed" when it varies). Setting a
  field's own textFont, textSize or textStyle restyles all its text.
- System messages: openStack, openBackground, openCard, closeCard, newCard,
  deleteCard, mouseDown, mouseUp, mouseStillDown, mouseEnter, mouseLeave,
  mouseWithin, mouseDoubleClick, openField, closeField, exitField, idle,
  arrowKey and doMenu. Menu commands go through `doMenu`, so scripts can
  intercept them.
- Menus: `create menu`, `put "A,-,B" into menu "X" with menuMessages …`,
  `put … after menuItem 2 of menu "Edit"`, `delete menu`/`menuItem`,
  `enable`/`disable` of menus and items, `the checkMark`, `the commandChar`
  (which also works as a shortcut), `the menuMessage` (sent instead of
  `doMenu` when the item is chosen), `the name` and `enabled`, `the menus`,
  `the number of menuItems of menu`, `there is a menu`, and
  `reset menuBar`. The demo stack adds a Tour menu this way.
- Keyboard messages: keyDown, returnInField, enterInField and tabKey go to
  the field being typed in; keyDown, returnKey, enterKey, tabKey,
  functionKey and commandKeyDown go to the card when no field is active.
  A handler that doesn't `pass` the message stops the key. Unhandled keys
  do what HyperCard did: Tab moves to the next unlocked field and selects
  its text, Return on the last line of an autoTab field moves on, typing
  with no field active goes into the message box (blind typing) and Return
  runs it, command keys do their menu items, and F1 undoes. Shift+Return
  stands in for the Enter key.
- Built-in commands are messages too: `on go`, `on find`, `on beep` and so
  on intercept the command, and `pass` lets it run. `choose` passes the
  tool's name and number; other commands pass the rest of the command as
  written (e.g. `to next card`). A command used inside its own handler runs
  directly.
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

**Printing.** File ▸ Print Card (Ctrl+P) and Print Stack…, and from
scripts `print card`, `print card 3`, `print all cards`, `print marked
cards`, `print 5 cards`, `print card from 0,0 to 512,300` (part of a card),
`print "text"` or `print field 1`, and `open printing` … `close printing`
to gather cards into one document. RandomCard makes a PDF and opens it in
your PDF viewer to print or save. Cards print at full size, two to a US
Letter page, pictures as crisp 1-bit images and buttons and fields as
vector shapes, with text laid out by HyperCard's metrics in the standard
PDF fonts (Helvetica, Times, Courier).

**The debugger.** As in HyperCard, a script stops at a checkpoint and shows
where it is in the script editor, with the line highlighted. Set a
checkpoint by clicking a line number in the editor or with Ctrl+D, or put
`debug checkpoint` in a script. While stopped: Step (F10) runs to the next
line of the handler, Step Into (F11) stops in handlers it calls, Trace runs
on showing each line at `the traceDelay`'s pace, Go (F5) runs to the next
checkpoint, and Abort (Shift+F5) stops. View ▸ Variable Watcher shows the
handler's locals and the globals, and you can change them; View ▸ Message
Watcher lists messages as they're sent (`show message watcher` works too).

## Keyboard

| Keys | Action |
| --- | --- |
| Ctrl+1 / 2 / 3 / 4 | First / previous / next / last card |
| ← → | Previous / next card (an `arrowKey` message) |
| Ctrl+N | New card |
| Ctrl+M | Message box |
| Ctrl+H | Home |
| Ctrl+B | Edit background |
| Ctrl+Alt+C / B / S | Card / background / stack script (Scripting level) |
| Ctrl+Tab | Browse tool |
| Ctrl+T | Show or hide the tool panel |
| Ctrl+Z, Ctrl+Shift+Z | Undo, redo |
| Ctrl+. | Abort running script |
| Ctrl+D | Set or clear a checkpoint (script editor) |
| F10 / F11 / F5 / Shift+F5 | Step / step into / go / abort (debugger) |
| Ctrl+Space | Hide or show the menu bar |
| Ctrl+S, Ctrl+O, Ctrl+Q | Save, open, quit |
| Ctrl+P | Print card |
| Ctrl+F | Find (in the message box, as HyperCard did) |
| Ctrl+R | Recent cards |

## Layout

```
src/
  model/       Stack, Background, Card, Part, Picture; JSON save/load; Tour and
               Home stacks
  import/      reader for original HyperCard stack files, WOBA decoder, AddColor
  print/       printing cards and text as PDF
  hypertalk/   lexer, parser, syntax tree, values and chunk expressions
  runtime/     shared World state, interpreter, objects and properties,
               commands, functions, dates; end-to-end tests
  paint/       raster painting: patterns, brushes, shapes, fills, selections,
               Paint menu operations, painted text
  sound/       note parsing, rendering, built-in voices and audio output
  ui/          egui app: menus, tool panel, card rendering, transitions,
               paint tools, info dialogs, script editor, user levels
```

## Not (yet) implemented

The main gaps are multiple open stacks and XCMDs. [ROADMAP.md](ROADMAP.md) lists the remaining work by area, with
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
