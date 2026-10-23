# RandomCard roadmap

What's left to make RandomCard a faithful HyperCard 2.x, grouped by area.
Each section says what exists today, what's missing, and where the work
would go. The suggested order at the end ranks items by how much they
matter to people building or importing stacks.

## Design principles

RandomCard aims for HyperCard's functionality, not its 1987 appearance.

- **Modern around the card.** Menus, dialogs, Info windows, the script
  editor, the message box and scroll bars use standard egui widgets. They
  bring keyboard support, high-resolution displays, smooth scrolling and
  accessibility, and need less code than imitations of classic controls.
- **HyperCard's behaviour wherever scripts or stacks can see it.** Part
  rectangles, card size and coordinates are exact, because scripts move
  things by position and imported stacks are laid out to the pixel.
  Properties keep their HyperCard meaning even when the visuals are
  modern: `the scroll` of a field is still pixels scrolled, and `the style`
  still takes `shadow` or `roundRect`. Changes to text rendering shouldn't
  change how much text fits in a field.
- **Recognizably HyperCard on the card.** Card content can be cleaner
  (antialiased text, crisper outlines), but button and field styles stay
  visibly distinct, since stack authors chose them deliberately.
- **No compromise on semantics.** The message path, HyperTalk, and
  painting behave as in HyperCard. The paint layer stays black and white
  for now (imported pictures and patterns depend on it); colour would be
  an addition, not a change to the model.

## Status at a glance

| Area | State |
| --- | --- |
| Object model (stacks, backgrounds, cards, buttons, fields) | Complete |
| HyperTalk core (handlers, message path, chunks, control flow) | Complete |
| HyperTalk commands and functions | Most; gaps below |
| System messages | Mouse, navigation, `idle`, `doMenu`, `arrowKey`; keyboard missing |
| Browse, Button and Field tools | Working; no part clipboard or undo |
| Paint tools | 10 of 18 tools, black only |
| Fields | Plain text; no styled runs or text selection queries |
| Visual effects | Complete |
| Sound | `play`, notes, `dial`, `beep`; no MACE |
| HyperCard 2.x import | Structure, pictures, parts, text, scripts, icons, sounds |
| Multiple stacks, menus, printing, externals | Missing |

## Painting

**Today:** Select (rectangle), Pencil, Brush, Eraser, Line, Spray, Rect,
Round Rect, Bucket and Oval, in black, with a line size and a Filled option.
One level of undo. Code: `src/ui/paint.rs` and the paint handling in
`src/ui/mod.rs`.

**Missing:**
- **Patterns.** HyperCard's 40-pattern palette was central to its look: fills,
  brush strokes and filled shapes all used the current pattern. Stacks store
  their patterns in the STAK block (40 × 8 bytes at block offset 0x2C0), so
  the importer can bring them across; `the pattern` would select one. The raster functions would take
  an 8×8 pattern instead of a single pixel value.
- **Tools:** Lasso, Text (painted text), Curve, Regular Polygon and Polygon.
  Today `choose` maps these names to the nearest existing tool.
- **Paint menu:** Select All, Fill, Invert, Pickup, Darken, Lighten, Trace
  Edges, Rotate, Flip Horizontal/Vertical, Opaque, Transparent, Scale, and
  Revert. Most work on the current selection.
- **FatBits** (zoomed editing) and the `grid`, `centered` and `multiple`
  paint options.
- **Brush shapes** (HyperCard had 32), and `the brush`, `the lineSize`
  and `the filled` affecting script-driven painting.
- **Script-driven painting:** `drag from … to …` and `click at` with paint
  tools, which classic stacks used to draw.
- **Multi-level undo.**

## Fields and text

**Today:** plain text per field (shared or per card for background fields),
font, size, style and alignment for the whole field, `lockText`, `dontWrap`,
`showLines`, `wideMargins`, scrolling fields, and `find` highlighting.

**Missing:**
- **Styled text runs.** Per-character font, size and style
  (`set the textStyle of word 2 of field 1 to bold`). Needs a style-run model
  in `Part`/`Card`, rendering via multiple `LayoutJob` sections, and editing
  that preserves runs. The importer already skips HyperCard's style runs
  (`content_text` in `src/import/mod.rs`) and could convert them with the
  STBL block.
- **Selection functions:** `the selectedText`, `selectedChunk`,
  `selectedLine`, `selectedField`, `selectedLoc`, and `the selection` as a
  container. These return empty today (`src/runtime/builtins.rs`). The UI
  would publish the focused field's selection into `World`.
- **Click functions:** `the clickText`, `clickChunk` and `clickLine` for
  clicks in locked fields.
- **List fields:** `autoSelect` with `multipleLines` (whole-line selection
  and hiliting).
- **`autoTab`, `fixedLineHeight`, `textHeight`** behaviour.
- **`select` with chunks:** `select line 3 of field 1` focuses the field
  but doesn't place the selection.

## Buttons

**Missing:**
- **Popup buttons** (style 11; imported as rectangles today).
- **The "standard" style** (HyperCard 2.2+; imported as round rect).
- **Creating and editing icons** (an icon editor, or importing images).
  Icons can be imported and chosen, not drawn.
- **`titleWidth`** and `the selectedLine` / `selectedText` of popups.

## HyperTalk

**Today:** see the README for what's implemented. All 268 scripts in the
sample stacks parse.

**Missing:**
- **Keyboard messages:** `keyDown`, `returnKey`, `enterKey`, `tabKey`,
  `returnInField`, `enterInField`, `functionKey`, `commandKeyDown` and
  `controlKey` aren't sent, so scripts can't intercept typing. The UI would
  send them before applying a key, and only apply it if the message passes
  through to RandomCard (`default_handler` in `src/runtime/interp.rs`).
  This needs a synchronous round trip to the script thread for typing.
- **Built-in commands as messages.** In HyperCard, `go`, `find`, `answer`
  and the other commands travel the message path, so a script can intercept
  them with `on go`. Only `doMenu` and `arrowKey` do here.
- **Chunk properties:** `textStyle`, `textFont` and `textSize` of a chunk
  (depends on styled text).
- **Menus:** `create menu`, `put … into menu`, `delete menu`, `enable` and
  `disable menuItem`, `the checkMark`, `the menuMessage` and `reset menuBar`
  are accepted and do nothing. Needs a script-modifiable menu model that the
  UI renders.
- **Windows and palettes:** `palette`, `picture`, `close window`, and
  `the visible of window`.
- **`select`** of parts (as with the Button tool) and of chunks.
- **`edit script` with a line**, and the **debugger**: `debug checkpoint`,
  breakpoints, stepping, and the message and variable watchers.
- **Other stacks:** see below.
- **Speech:** `speak` shells out to `spd-say`/`espeak`/`say`; `the voices`
  and `the speech` aren't real.

## Stacks

**Missing:**
- **Multiple open stacks,** `go to stack` in a new window, and `the stacks`.
  Today one stack is open and opening another replaces it.
- **A Home stack** and HyperCard's message path beyond the stack
  (stack → Home → HyperCard), including `start using` and `stop using` for
  shared script libraries.
- **References to other stacks:** `card 1 of stack "X"` currently ignores
  the stack part, and objects in other stacks can't be counted or read.
- **User levels** (Browsing, Typing, Painting, Authoring, Scripting) and
  `the userLevel` actually restricting the UI.
- **Protect Stack:** passwords, `cantPeek`, `cantAbort`, private access.
  Imported values are kept but not enforced.
- **Card size** changes (`set the rect of card window`), and **Compact Stack**.
- **Recent cards** view (the Recent dialog) and a **Find dialog**
  (Ctrl+F) in the UI; `find` works from scripts and the message box.

## Editing and the UI

**Missing:**
- **Cut, Copy and Paste of buttons and fields** (cards have them).
- **Undo** for object edits (moving, resizing, deleting parts and cards).
- **Bring to Front / Send to Back** (only one step closer or farther now).
- **Font metrics:** text uses egui's fonts rather than HyperCard's bitmap
  fonts, which is intended. It runs slightly larger than the originals, so
  some imported fields show less text than they did; matching font sizes to
  HyperCard's text metrics would fix that without changing the look.
- **Script editor:** find and replace, go to line, and syntax colouring,
  using egui's standard text-editing features.
- **Font and Style menus**, and a **Home menu**.
- **Printing** of cards and reports.

## Importing HyperCard stacks

**Today:** HyperCard 2.x stacks, including MacBinary and AppleSingle
wrappers, private-access stacks, icons and sounds from the resource fork
and the resource chain.

**Missing:**
- **HyperCard 1.x stacks.** Their blocks use 12-byte headers (offsets shift
  by 4); HyperCardPreview handles both layouts and is the reference.
- **Styled text** (STBL style table and per-field style runs).
- **AddColor** color data (HCcd/HCbg resources).
- **MACE-compressed sounds** (MACE 3:1 and 6:1).
- **XCMDs and XFCNs.** These are 68k/PowerPC code and can't run directly.
  Common ones could be reimplemented natively by name (for example
  `Palette`, `PictureWindow`, text utilities), or flagged clearly when a
  script calls one.
- **PICT resources** and `picture`/`palette` windows.
- **Checksum validation** of LIST and PAGE blocks, for clearer errors on
  damaged stacks.

## Testing

- The importer is tested against HyperCardPreview's sample stacks when
  `RANDOMCARD_HC_SAMPLES` is set (see the README). Those samples contain no
  sound resources, so `snd` decoding is only tested with constructed data;
  real stacks with sounds should be added.
- Scripts in imported stacks are parsed but not all executed. A harness
  that sends `openStack`, then `mouseUp` to every button, and records script
  errors would show which language gaps matter most in practice.
- The UI has no automated tests; checks so far were manual.

## Suggested order

1. **Keyboard messages and interceptable commands.** Many classic stacks
   depend on `keyDown`, `returnInField` and `on go`.
2. **Paint patterns and the remaining paint tools.** The most visible gap
   for anyone drawing; imported stacks carry their patterns in the STAK
   block.
3. **Cut, Copy and Paste of parts, and undo for object edits.** Basic
   authoring comfort.
4. **Selection and click functions.** Common in scripts that react to
   clicks in fields.
5. **Styled text.** Needed for faithful imports and chunk properties.
6. **Script-modifiable menus.** Many stacks add their own menus.
7. **Multiple stacks, Home, and `start using`.** Larger architectural
   change to `World` and the message path.
8. **HyperCard 1.x import, MACE, AddColor.**
9. **Font metrics, user levels, printing, the debugger.**
