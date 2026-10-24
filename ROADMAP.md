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
| Fields | Styled text, the selection, click functions, list fields |
| Visual effects | Complete |
| Sound | `play`, notes, `dial`, `beep`; no MACE |
| HyperCard import | 1.x and 2.x: structure, pictures, parts, text, scripts, icons, sounds, AddColor |
| User levels and stack protection | Levels, Protect Stack, passwords, private access |
| Script-modifiable menus | Complete, except submenus |
| Home, stacks in use, going between stacks | One stack window at a time |
| Printing, externals | Missing |

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
- Options and Patterns as menus.
- Delete Stack, which should respect `cantDelete`.

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

## Fields and text

**Today:** text per field (shared or per card for background fields) with
style runs of font, size and style, a font, size, style and alignment for
the whole field, `lockText`, `dontWrap`,
`showLines`, `wideMargins`, scrolling fields, `find` highlighting, the text
selection (`select`, `the selectedText` and friends, `the selection` as a
container), click functions in locked fields, and autoSelect list fields.

**Missing:**
- **How styled text looks.** Only egui's built-in fonts are available: one
  sans-serif and one monospaced (used for Courier and Monaco), with no bold
  weight, so bold is drawn twice slightly offset. Outline and shadow aren't
  drawn. Bundling or finding fonts like HyperCard's (Chicago, Geneva, New
  York, Times) would make styled stacks look closer to the original.
- **Copying styled text:** text copied from one field and pasted into
  another loses its styles.
- **List fields:** Command-click to select lines that aren't next to each
  other in a multipleLines field (Shift-click runs work), and
  `the selectedLine` reporting such selections.
- **The selection in the message box:** `the selection` and
  `the selectedText` only see fields.
- **`autoTab`, `fixedLineHeight`, `textHeight`** behaviour.

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
- **Menus:** the `textStyle` of menu items and `the markChar` beyond a
  check mark are accepted but not shown. Submenus and hierarchical menus
  (HyperCard 2.2's popup-style items) aren't supported.
- **Windows and palettes:** `palette`, `picture`, `close window`, and
  `the visible of window`.
- **`edit script` with a line**, and the **debugger**: `debug checkpoint`,
  breakpoints, stepping, and the message and variable watchers.
- **Other stacks:** see below.
- **Speech:** `speak` shells out to `spd-say`/`espeak`/`say`; `the voices`
  and `the speech` aren't real.

## Stacks

**Today:** a Home stack (made on first run in `~/.config/randomcard/`,
opened at startup, Go ▸ Home and `go home`), the message path stack →
stacks in use → Home, `start using`/`stop using` with `libraryStack` and
`releaseStack`, `the stacksInUse`, `go to stack "X"` and
`go to card 3 of stack "X"`, `send … to stack "X"`, and the name and script
of other stacks. Leaving a stack saves it; a stack without a file stays
loaded to come back to.

**Missing:**
- **Multiple stack windows:** `go to stack … in a new window` opens it in
  the same window, and `the stacks` lists only the current stack.
- **Objects in other stacks:** `field 1 of card 2 of stack "X"` and
  counting other stacks' cards need object references that name a stack;
  today only going there works.
- **History across stacks:** Back and Recent forget cards of stacks you've
  left.
- **Home as HyperCard had it:** its user-level card, search paths and stack
  list. RandomCard keeps the user level in Preferences instead.
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
- **A Home menu** belongs with Home stack support.
- **Printing** of cards and reports.

## Importing HyperCard stacks

**Today:** HyperCard 1.x and 2.x stacks, including MacBinary and
AppleSingle wrappers, private-access stacks, icons and sounds from the
resource fork and the resource chain, and AddColor's colors (HCcd and HCbg
resources) for buttons, fields and rectangles.

**Missing:**
- **MACE-compressed sounds** (MACE 3:1 and 6:1). The only decoders around
  are FFmpeg's (LGPL) and ports of it; using one means taking on its
  license, or writing a decoder from a description of the format.
- **AddColor pictures** (PICT resources or files) are imported but not
  drawn, and bevels show only on transparent parts. AddColor's own XCMD
  commands (`colorCard`, `addColor colorButton`…) aren't implemented, so
  scripts can't change colors.
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
4. ~~Using and authoring stacks.~~ Done.
5. ~~Selection and click functions.~~ Done.
6. ~~Styled text.~~ Done.
7. ~~Script-modifiable menus.~~ Done.
8. ~~Multiple stacks, Home, and `start using`.~~ Done, except stack windows
   and objects in other stacks.
9. ~~HyperCard 1.x import, MACE, AddColor.~~ Done, except MACE.
10. **Font metrics, printing, the debugger.**
