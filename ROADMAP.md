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
| Printing | Cards and text, as PDF; no report printing |
| Externals (XCMDs) | Missing |

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
  drawn. Text is spaced by HyperCard's metrics, so this affects only the
  look, not what fits. Open-licensed lookalike fonts could be bundled.
- **Copying styled text:** text copied from one field and pasted into
  another loses its styles.
- **List fields:** Command-click to select lines that aren't next to each
  other in a multipleLines field (Shift-click runs work), and
  `the selectedLine` reporting such selections.
- **The selection in the message box:** `the selection` and
  `the selectedText` only see fields.
- **`autoTab`** behaviour. Character positions within a word come from
  egui's font, so a click can land a character away from where it would
  have in HyperCard; line breaks and line heights match.

## Buttons

**Missing:**
- **Creating and editing icons** (an icon editor, or importing images).
  Icons can be imported and chosen, not drawn.
- **Popup details:** the menu opens with the chosen item roughly over the
  button, but not to the pixel; the title is always drawn left-aligned;
  and `the selectedLine` can't be set (use `select line 2 of button 1`).

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
- **`edit script` with a line.** The debugger is done; it lacks
  evaluating expressions in the message box while stopped (HyperCard ran
  them in the stopped handler's context), and checkpoints don't move when
  lines are added above them.
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

## Editing and the UI

**Missing:**
- **Undo for Info dialog changes** and for script edits; the undo history
  covers painting, parts and cards.
- **The system clipboard:** cut and copied parts and pictures stay inside
  RandomCard rather than going to the desktop clipboard.
- **Script editor:** find and replace, go to line, and syntax colouring,
  using egui's standard text-editing features.
- **A Home menu** belongs with Home stack support.
- **Report printing** (`open report printing`, Print Report…) and Page
  Setup. Printing makes US Letter PDFs; there's no choice of paper size,
  cards per page or card names yet. AddColor colors aren't printed.

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
  See item 4 of the suggested order: a framework for externals written in
  Rust, starting with versions of the common ones.
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

Done so far: keyboard messages, paint tools and patterns, clipboard and undo,
using and authoring stacks, the selection, styled text, scriptable menus,
Home and stacks in use, HyperCard 1.x import and AddColor, font metrics,
printing and the debugger.

Next, ranked by what people who built HyperCard stacks will need first:

1. ~~Find and Recent dialogs.~~ Done: Go ▸ Find… (Ctrl+F) readies `find ""`
   in the message box, and Go ▸ Recent (Ctrl+R) shows miniatures of the
   last 42 cards. Recent covers the current stack only, until history
   spans stacks.
2. ~~Popup buttons and the standard button style.~~ Done: popups import
   with their items, title width and chosen item, show their menu when
   clicked, and answer `the selectedText`, `selectedLine` and
   `selectedChunk`; `select line 2 of button 1` chooses an item. The Tour's
   Dice card has one.
3. **Real stacks as tests.** A harness that opens each stack in a folder,
   sends `openStack`, clicks every button, and reports script errors and
   unknown commands. Run against a collection of real stacks, it shows
   which commands, styles and XCMDs matter most, and picks the first
   built-in externals below.
4. **Externals: XCMDs and XFCNs in Rust.** A framework where anyone can add
   commands and functions in Rust, as HyperCard developers once did in
   Pascal, C or assembly:
   - **One interface.** An external is a Rust type with a name, saying
     whether it's a command or a function. It gets its arguments as text,
     and a context for calling back into RandomCard: evaluating an
     expression, sending a message, reading and setting globals and field
     text, and setting `the result`. These are the services HyperCard's
     XCMD callbacks offered.
   - **The same place in the message path.** As in HyperCard, externals are
     tried after the last handler (stack, stacks in use, Home), so a script
     handler with the same name always wins.
   - **Built in.** One Rust file each in `src/externals/`, registered by
     name. First come the ones HyperCard shipped or stacks commonly used:
     `FontExists`, `Palette` (the Navigator palette), `PictureWindow` (with
     a PICT decoder) and AddColor's commands. More come from the test
     results.
   - **Your own.** Written in Rust against a small SDK crate with the same
     interface, then either compiled into RandomCard (add a file, rebuild),
     or compiled to WebAssembly and installed in a stack, in Home, or in
     `~/.config/randomcard/externals/`. They'd be found along the same
     chain HyperCard searched for XCMD resources. A WebAssembly external
     needs no rebuild, runs on every platform, is sandboxed, and travels
     inside the stack as a real XCMD did. *To decide:* WebAssembly needs a
     runtime library (wasmi or wasmtime). Native dynamic libraries are the
     alternative, but they're specific to each platform and can't be
     sandboxed.
   - **Imported stacks.** The importer lists the XCMDs and XFCNs in a
     stack's resource fork. A call to one that has no Rust version says
     "This stack uses the XCMD 'X', which RandomCard doesn't have", and
     Stack Info shows the list.
5. **The script editor.** Find and replace, going to a line (`edit script
   of … at line`), and syntax colouring.
6. **FatBits and an icon editor.** Drawing custom icons and touching up
   pictures pixel by pixel.
7. **The rest of painting.** The Options and Patterns menus, editing
   patterns, Shift for straight lines and perfect shapes, and Pickup, Scale
   and Revert.
8. **Multiple stack windows.** `go to stack … in a new window`, and
   `the stacks`.
9. **Report printing, MACE sound, Compact Stack and Delete Stack.**
10. **Objects in other stacks.** First check what HyperCard allowed. Its
    scripts usually reached other stacks with `lock screen`, `go`, `get`
    and `go back`, and that idiom has to work across stacks either way.

## Future

Ideas, not commitments: what HyperCard might have grown into. Each should
feel like HyperTalk (plain words, everything is text) rather than a
bolted-on API, and none should change how existing stacks behave. The
first three are favourites.

- **JSON and REST.** Fetching and sending data from scripts, in
  HyperTalk's style:
  - `put url "https://api.example.com/items" into field "Data"`, and
    `post field "Form" to url "…"`, with `the result` holding the HTTP
    status and `the httpHeaders` for setting headers (tokens, content
    type).
  - JSON as chunk expressions, since everything is text:
    `put key "name" of item 3 of json field "Data" into field "Name"`,
    `the keys of json x`, `the number of items of json x`, and
    `set key "done" of json x to true`, which rewrites the text.
  - Requests that don't freeze a handler: `load url "…" with message
    "gotData"`, sending the message with the reply when it arrives.
  - Filling cards from data: one card per record, as HyperCard
    address books did by hand.
- **Databases.** SQLite from HyperTalk: `query "select …" from database
  "Addresses.db"`, with results as lines and items, and cards bound to
  rows.
- **Unicode and system fonts.** Text beyond MacRoman, emoji, and real
  fonts chosen from the system, for new stacks that don't need to look
  like 1990.
- **A web player.** egui runs in the browser (WebAssembly), so a stack
  could be published as a web page anyone can click through, the way
  HyperCard stacks were once passed around on floppies.
- **Stacks as web services.** A server mode where a stack answers
  requests: `on httpRequest path, params` returning a page or JSON, so a
  stack can be a small web app, built and tested in RandomCard.
- **Running stacks without a window.** `randomcard --run stack --send
  message` for scripting from the command line, cron jobs, and automated
  tests of stacks.
- **A shell and file system.** `do shell "…"` (opt-in, with permission
  per stack), listing folders, reading and writing files beyond
  `open file`, as replacements for the XCMDs stacks used for this.
- **Color and modern images.** A color paint layer alongside the 1-bit
  one, PNG and JPEG pictures on cards (pasted or dropped), resizable cards
  and high-resolution pictures.
- **Media.** Video playback in a card region, recording sound
  (`record sound`), and playing common audio files.
- **Plugins.** A safe stand-in for XCMDs: externals written as
  WebAssembly modules, sandboxed and cross-platform, called like
  commands and functions.
- **A scripting assistant.** Ask Claude to explain a handler, write one
  from a description, or find why a script fails, from the script editor.
- **Accessibility.** Screen readers announcing cards, buttons and
  fields (egui supports AccessKit), and keyboard navigation of buttons.
- **Collaboration.** A stack format that diffs well in git (a folder of
  cards and scripts), and stacks shared and edited by several people at
  once.
