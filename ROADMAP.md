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
- **A stack runs the way its author set it up.** Using a stack and authoring
  one are different activities. Opening a stack should behave as it would
  have in HyperCard, including its user level and protection.

## Status at a glance

| Area | State |
| --- | --- |
| Object model (stacks, backgrounds, cards, buttons, fields) | Complete |
| HyperTalk core (handlers, message path, chunks, control flow) | Complete |
| HyperTalk commands and functions | Most; gaps below |
| System messages | Mouse, navigation, keyboard, `idle`, `doMenu`; all commands interceptable |
| Browse, Button and Field tools | Working, with clipboard and undo |
| Paint tools | All 18 tools, patterns, Paint menu; no FatBits |
| Fields | Plain text; no styled runs or text selection queries |
| Visual effects | Complete |
| Sound | `play`, notes, `dial`, `beep`; no MACE |
| HyperCard 2.x import | Structure, pictures, parts, text, scripts, icons, sounds |
| User levels and stack protection | Levels, Protect Stack, passwords, private access |
| Multiple stacks, menus, printing, externals | Missing |

## Using and authoring stacks

HyperCard separated using a stack from building one, and stacks depended on
it. An address book let people add, edit and delete addresses without being
able to move a field, add a button or read a script. RandomCard now does the
same.

**HyperCard's model.** Five user levels, each unlocking more:

| Level | Name | Adds |
| --- | --- | --- |
| 1 | Browsing | Clicking buttons and navigating; fields can't be typed in |
| 2 | Typing | Typing and editing text in fields |
| 3 | Painting | The paint tools, and the Paint, Options and Patterns menus |
| 4 | Authoring | The Button and Field tools, the Objects menu and Info dialogs |
| 5 | Scripting | Opening and editing scripts |

The level was a setting of HyperCard itself (kept in the Home stack), and
stacks commonly set it on opening, such as `set the userLevel to 2` in
`openStack`. **Protect Stack** limited the highest level a user could reach
and could require a password to change that; it also covered
`cantModify` (no changes saved), `cantDelete`, `cantPeek` (no peeking at
scripts or button outlines), `cantAbort` (no Command-period) and private
access (a password to open the stack at all). Stacks could also hide the
menu bar or replace HyperCard's menus with their own.

**Decisions:**
- RandomCard behaves like the purchased HyperCard: it runs at Scripting
  level by default, and otherwise stacks behave as they did in HyperCard.
- The tool panel stays docked on the left. It can be shown or hidden (View ▸
  Tool Panel, Ctrl+T, or Preferences), and it only appears at levels that
  allow tools. There are no tear-off palettes and no Tools menu.
- The message box stays at the bottom of the window. Screens are bigger now,
  so RandomCard can take the space.

**Done:**
- **Opening a stack behaves as in HyperCard.** Its `openStack` handler runs,
  and the user level it sets takes effect. Its protection limits what the
  user can raise the level to. `the userLevel` returns the level in effect.
  Imported HyperCard stacks keep their level limit, password and flags.
  Private-access stacks ask for their password and are decrypted (ported
  from HyperCardPreview, MIT licensed).
- **The interface follows the level.** Below Painting there is no tool
  panel and no Paint or Objects menu. Below Typing, fields can't be edited.
  Script buttons, script shortcuts and the error alert's Script button need
  Scripting. A Can't Modify stack is browsed at Browsing level, and its
  changes aren't saved.
- **Menus:** File, Edit, Go, View, Paint while a paint tool is active, and
  Objects at Authoring and above. Items the level doesn't allow are hidden
  or disabled.
- **Protect Stack** (File menu): Can't Modify, Can't Delete, Can't Abort,
  Can't Peek, Private Access, the level limit, and a password. The password
  uses HyperCard's own hash, so imported passwords keep working. With a
  password set, Protect Stack asks for it before opening. The new limit
  applies as soon as the dialog closes. Protect Stack is always in the menu,
  so a stack can't lock its author out.
- **Preferences** (File menu): RandomCard's own user level, capped by the
  stack's limit, with "Unlock with Password…" to lift the limit for the
  session, and whether the tool panel is shown. Settings are saved in
  `~/.config/randomcard/settings.json`.
- The demo Address Book sets Typing level on `openBackground` and restores
  the level when you leave.

**Still to do:**
- Font and Style menus while editing text; Options and Patterns as menus.
- Delete Stack, which should respect `cantDelete`.
- Stacks hiding the menu bar or adding their own menus (item 7 below).

**What protection can and can't do.** It keeps a stack's users from
changing it by accident or casually. It isn't security: a `.rcstack` file
is JSON that anyone can edit in a text editor, much as HyperCard's
protection could be worked around. Real secrecy would need the stack file
to be encrypted (as HyperCard's private access did for its header).

## Painting

**Today:** all 18 of HyperCard's paint tools, a 40-pattern palette (imported
stacks bring their own), 16 brush shapes, the Paint menu operations, painted
text, multi-level undo and redo, and script-driven painting with `drag`,
`click at` and `type`. Code: `src/paint/` and `src/ui/painting.rs`.

**Missing:**
- **FatBits** (zoomed pixel editing). Needs the card view to scale.
- **Pattern editing** (double-clicking a pattern to change it) and
  `the pattern` affecting lines and borders, which are always black.
- **Paint menu:** Pickup, Scale, and Revert; `the grid`, `centered` and
  `multiple` options; Shift-constrained drawing (straight lines, squares,
  circles).
- **A font and style menu** for the text tool. Today scripts or the message
  box set `the textFont`, `textSize` and `textStyle` for painted text.

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
- **Keyboard gaps:** keyboard messages and interceptable commands are done
  (see the README). Still missing: `controlKey`; `keyDown` for editing keys
  such as Backspace and the arrows inside a field; a separate Enter key
  (keypad Enter arrives as Return, so Shift+Return stands in for it); and
  keyboard messages with tools other than Browse.
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
- **Card size** changes (`set the rect of card window`), and **Compact Stack**.
- **Recent cards** view (the Recent dialog) and a **Find dialog**
  (Ctrl+F) in the UI; `find` works from scripts and the message box.

## Editing and the UI

**Missing:**
- **Undo for Info dialog changes** and for script edits; the undo history
  covers painting, parts and cards.
- **The system clipboard:** cut and copied parts and pictures stay inside
  RandomCard rather than going to the desktop clipboard.
- **Font metrics:** text uses egui's fonts rather than HyperCard's bitmap
  fonts, which is intended. It runs slightly larger than the originals, so
  some imported fields show less text than they did; matching font sizes to
  HyperCard's text metrics would fix that without changing the look.
- **Script editor:** find and replace, go to line, and syntax colouring,
  using egui's standard text-editing features.
- **Font and Style menus:** part of the menu restructuring in "Using and
  authoring stacks". A **Home menu** belongs with Home stack support.
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

1. ~~Keyboard messages and interceptable commands.~~ Done.
2. ~~Paint patterns and the remaining paint tools.~~ Done, except FatBits.
3. ~~Cut, Copy and Paste of parts, and undo for object edits.~~ Done.
4. ~~Using and authoring stacks.~~ Done, except Font and Style menus.
5. **Selection and click functions.** Common in scripts that react to
   clicks in fields.
6. **Styled text.** Needed for faithful imports and chunk properties.
7. **Script-modifiable menus.** Many stacks add their own menus; builds on
   the menus from item 4.
8. **Multiple stacks, Home, and `start using`.** Larger architectural
   change to `World` and the message path.
9. **HyperCard 1.x import, MACE, AddColor.**
10. **Font metrics, printing, the debugger.**
