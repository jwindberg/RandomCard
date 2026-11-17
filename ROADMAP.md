# RandomCard roadmap

What's left for RandomCard, grouped by area. Work now follows three
tracks: HyperCard features still missing, HyperCard 2.4's own features,
and new features HyperCard never had, color first. Each section says what
exists today, what's missing, and where the work would go. The suggested
order at the end is shaped by what RandomCard's users ask for.

The drive to make every stack on Apple's Developer CDs work ended at 1,029
working (WORKING_STACKS.md; 1,059 at the 2026-11-14 regression run). Those stacks are now a regression suite: a
failing one matters only when it shows a missing HyperCard feature. Stacks
that assume the old Mac OS (colon paths, old system calls) are updated by
fix steps on our copies (FIX_STEPS.tsv), not by compatibility code.

## Design principles

RandomCard aims for HyperCard's functionality, not its 1987 appearance.

- **Modern around the card.** Menus, dialogs, Info windows, the script
  editor, the message box and scroll bars use standard egui widgets. They
  bring keyboard support, high-resolution displays, smooth scrolling and
  accessibility, and need less code than imitations of classic controls.
- **HyperCard's behavior wherever scripts or stacks can see it.** Part
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
  painting behave as in HyperCard.
- **Full color, in time.** HyperCard was black and white because the 1987
  Mac was. To stand beside modern software RandomCard aims for full color:
  painting with a color picker, colored parts and text, and images (see
  Color below). Black-and-white stacks keep looking exactly as they did:
  1-bit pictures and patterns become a special case of color, not a
  separate world.
- **Modern controls, as Apple would make HyperCard today.** Where a
  modern system has a standard control HyperCard lacked, RandomCard has it
  as a part, never a scripted imitation of one. Modern desktop and web
  design are both fair inspiration; the names follow HyperCard's patterns
  first, then official standards such as HTML's. Done: combo boxes (a
  field's `choices`) and sliders (the `slider` button style). Candidates:
  progress bars, steppers, toggle switches, segmented controls, date
  pickers, color wells and tabs.
- **A stack runs the way its author set it up.** Using a stack and authoring
  one are different activities. Opening a stack should behave as it would
  have in HyperCard, including its user level and protection.
- **The Tour shows everything that works.** A finished feature gets a way
  to try it in the Tour (`src/model/demo.rs`), tested in `demo_stack_runs`
  where a script can drive it.

## Status at a glance

| Area | State |
| --- | --- |
| Object model (stacks, backgrounds, cards, buttons, fields) | Complete |
| HyperTalk core (handlers, message path, chunks, control flow) | Complete |
| HyperTalk commands and functions | Most; gaps below |
| System messages | Mouse, navigation, keyboard, `idle`, `doMenu`; all commands interceptable |
| Browse, Button and Field tools | Working, with clipboard and undo |
| Paint tools | All 18 tools, the Tools palette with drawn icons, patterns, colors and Options, the Paint menu with Select, Pickup, Keep and Revert, scaling, Rotate, Slant, Distort and Perspective, and FatBits |
| Fields | Styled text, the selection, click functions, list fields |
| Visual effects | Complete |
| Sound | `play`, notes, `dial`, `beep`; no MACE |
| Movies | HyperCard 2.4's movie syntax, movie windows and controller, on macOS (AVFoundation), Windows (Media Foundation) and Linux (GStreamer) |
| HyperCard import | 1.x and 2.x: structure, pictures, parts, text, scripts, icons, sounds, AddColor |
| User levels and stack protection | Levels, Protect Stack, passwords, private access |
| Script-modifiable menus | Complete, except submenus |
| Home, stacks in use, going between stacks | One stack window at a time |
| Printing | Cards and text, as PDF; no report printing |
| Externals (XCMDs) | The common ones built in as features ([XCMDS.md](XCMDS.md)) |
| Color | Color painting, parts and text; AddColor from imported stacks, pictures included, on screen only; no Color Editor yet |

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
| 3 | Painting | The paint tools and the Tools palette |
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
- The Tools palette is a small window of its own, kept in front and
  acting on whichever stack is in front. It holds the tools in labeled
  groups, each icon showing its tool's setting (line size, polygon sides,
  Draw Filled); a tool's choices open under its row on a double-click.
  Below them, Patterns: the line and fill colors, the pattern and Edit…,
  each unfolding its choices (as HyperCard's Patterns menu dropped down);
  what the chosen tool doesn't use is greyed out. HyperCard's `tool
  window` and `pattern window` both name it.
  It's shown or hidden with View ▸ Tools Palette, Ctrl+T or Preferences,
  opens where it was last, and only appears at levels that allow tools.
  Preferences can dock it at the window's left instead, for desktops that
  can't keep a window in front and for small screens. There is no Tools
  menu.
- The message box stays at the bottom of the window. Screens are bigger now,
  so RandomCard can take the space.

**Done:**
- **Opening a stack behaves as in HyperCard.** Its `openStack` handler runs,
  and the user level it sets takes effect. Its protection limits what the
  user can raise the level to. `the userLevel` returns the level in effect.
  Imported HyperCard stacks keep their level limit, password and flags.
  Private-access stacks ask for their password and are decrypted (ported
  from HyperCardPreview, MIT licensed).
- **The interface follows the level.** Below Painting there are no
  palettes and no Paint, Options or Objects menu. Below Typing, fields can't be edited.
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
  session, whether the Tools palette is shown, whether the palettes are
  docked, and Power Keys. Settings are saved in
  `~/.config/randomcard/settings.json`.
- The demo Address Book sets Typing level on `openBackground` and restores
  the level when you leave.

- **Cursors that are the tools,** drawn over the card: the Brush as its
  shape at its real size in the fill color, the Eraser its square, the
  Spray its circle, and the Pencil and Bucket as their icons with the tip
  where paint lands; the crosshair for lines, shapes and selecting, and
  the I-beam for Text.
- **Spray size and density,** beyond HyperCard, whose spray was fixed: the
  Spray's options (8 to 32 pixels; Light, Medium, Heavy) and `the
  spraySize` and `the sprayDensity`.
- **Hex colors** in the color picker, beside Red, Green and Blue, to type
  or paste (`#F80` and names too); scripts take `#F80` as well.
- **Tool options, the same way for every tool.** Every tool with options
  has a corner mark; double-clicking anywhere on it opens them under its
  row, and again closes them. Each shows only the options it understands
  (the line size as a pull-down, Filled, Centered, Multiple, the sides,
  the brush), and the bottom row keeps only Grid and FatBits.
- **The Paint menu in the menu bar** while a paint tool is chosen, as in
  HyperCard, its picture commands greyed out until something is selected
  (checked in HyperCard 2.4.1). A script's `doMenu` still acts on the whole
  picture with nothing selected. HyperCard's Options menu is on the Tools
  palette as buttons; it's still a menu behind the scenes, for scripts'
  `doMenu` and stacks that change it.
- **Select, Pickup, Scale, Keep and Revert,** as HyperCard 2.4.1's manual
  describes them. Select takes the new element (what's changed since the
  last press on the card, with what it encloses), else tightens the
  selection or the layer; the Paint menu's commands work on the new
  element too. A moved or pasted selection floats over what was under it
  until you click elsewhere, so Pickup can take that and moving it again
  puts it back. Command-drag a selection's edge or corner to scale it
  (Shift on a corner keeps its proportions). The lasso hugs what's
  painted (Option-drag for all of it; Command-click for one shape), and
  the Select tool's Command- and Option-drags select tightly. Keep
  (Ctrl+K), switching layers and going to another card set what Revert
  goes back to; Revert with a selection restores only that part. Paint is
  still saved as you go, unlike HyperCard, which held it until Keep. Power
  keys S, P and R.
- **Rotate, Slant, Distort and Perspective,** as buttons under Grid and
  FatBits while the Select tool is chosen: handles on a rectangular
  selection's corners, dragged as often as you like until you click away;
  Undo goes back to before. `doMenu "Rotate"` and the rest choose them,
  and a script's `drag` with the Select tool selects a rectangle.

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
- **`the pattern` for lines and borders,** which are always solid.
  (Pattern editing is done: Edit… beside the pattern on the palette, or
  double-clicking the pattern, with squares to click and a pattern to take from the
  card; the stack keeps it.)
- (Done: Shift-constrained drawing, with straight strokes, lines and polygon
  sides at multiples of 15°, squares and circles, straight moves of a
  selection, and `drag … with shiftKey`. So are Options' Grid, Draw
  Centered, Draw Multiple and Power Keys, and `the grid`, `centered`,
  `multiple`, `multiSpace` and `powerKeys`.)
- **Dragging Rotate's and the others' handles from a script:** `doMenu
  "Rotate"` puts the handles on, but only the mouse drags them. How far
  apart Draw Multiple's copies are for each `multiSpace` is a guess, still
  to check in HyperCard 2.4.1. So is exactly how HyperCard's Perspective
  and Distort fill the shape; RandomCard keeps each row evenly spaced (a
  bilinear map), so lines survive.
- **Command-S for Select:** HyperCard's, but it's Save Stack in
  RandomCard, so Select has the menu and the S power key.

## Fields and text

**Today:** text per field (shared or per card for background fields) with
style runs of font, size and style, a font, size, style and alignment for
the whole field, `lockText`, `dontWrap`,
`showLines`, `wideMargins`, scrolling fields, `find` highlighting, the text
selection (`select`, `the selectedText` and friends, `the selection` as a
container), click functions in locked fields, autoSelect list fields, and
Cut, Copy, Paste and Clear Text, which keep fonts, sizes and styles when
text goes from one field to another (by menu, keyboard or `doMenu`).

**Missing:**
- **Sideways scrolling in a scrolling dontWrap field.** While typing, a
  dontWrap field scrolls sideways to follow the insertion point (HyperCard
  only cut long lines off), but a field that also has the scrolling style
  only scrolls up and down, so the insertion point can go out of view.
- **How styled text looks.** Only egui's built-in fonts are available: one
  sans-serif and one monospaced (used for Courier and Monaco), with no bold
  weight, so bold is drawn twice slightly offset. Outline and shadow aren't
  drawn. Text is spaced by HyperCard's metrics, so this affects only the
  look, not what fits. Open-licensed lookalike fonts could be bundled.
- **List fields:** Command-click to select lines that aren't next to each
  other in a multipleLines field (Shift-click runs work), and
  `the selectedLine` reporting such selections.
- **The selection in the message box:** `the selection` and
  `the selectedText` only see fields.
- **`autoTab`** behavior. Character positions within a word come from
  egui's font, so a click can land a character away from where it would
  have in HyperCard; line breaks and line heights match.

## Buttons

**Missing:**
- **Icon editor details:** it draws with the Line color and clears with
  a right-click, but has none of HyperCard's Special menu (Frame, Gray,
  Invert, Mirror, Rotate, Shadow), no undo, and new icons are always
  32×32.
- **Popup details:** the menu opens with the chosen item roughly over the
  button, but not to the pixel; the title is always drawn left-aligned;
  and `the selectedLine` can't be set (use `select line 2 of button 1`).

**Done beyond HyperCard:** icons of any size, in color (Add From File in
Button Info), and a settable `partNumber` to bring a part to the front.
Solitaire (`examples/Solitaire.rcstack`, made by `tools/make_solitaire.py`
from Byron Knoll's public-domain cards and BdRGames' CC0 back in
`reference/`) uses both: FreeCell, with Windows FreeCell's numbered deals,
Klondike (draw one or three) and Spider (one, two or four suits).

## HyperTalk

**Today:** see the README for what's implemented. All 268 scripts in the
sample stacks parse, and 3,009 of the 3,016 in Apple's developer stacks
(`examples/apple`). The other 7 hold slips in Apple's own scripts, such as
a missing `&` or a typed `whatof`. As in HyperCard, scripts are compiled a
handler at a time, so a broken handler fails only if it runs and the rest
of its script works.

**Missing:**
- **Odd forms in Apple's stacks:** `get lineClicked(4) of me` (a function's
  value followed by `of me`), seen once.
- **Keyboard gaps:** keyboard messages and interceptable commands are done
  (see the README). Still missing: `controlKey`; `keyDown` for editing keys
  such as Backspace and the arrows inside a field; a separate Enter key
  (keypad Enter arrives as Return, so Shift+Return stands in for it); and
  keyboard messages with tools other than Browse.
- **Menus:** the `textStyle` of menu items and `the markChar` beyond a
  check mark are accepted but not shown. Submenus and hierarchical menus
  (HyperCard 2.2's popup-style items) aren't supported.
- **Windows and palettes:** `picture`. (`palette`, `close window` and
  `the visible of window` are done.)
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
of other stacks, and their cards and parts (`field 1 of card 2 of stack "X"`). Leaving a stack saves it; a stack without a file stays
loaded to come back to. Stacks open in windows of their own (`go to stack
… in a new window`, File ▸ Open Stack in New Window…), each keeping its
card, history and undo; the front one is current, with `suspendStack` and
`resumeStack` as windows change places, `the stacks` lists them, and Close
Stack (⌘W) or `close window` closes one.

**Missing:**
- **Stack windows, what's left:** the first window, which holds the tools
  and message box, can't be closed (Close Stack there goes Home); dialogs
  such as `answer` appear in it whichever stack asked; a window behind
  shows its card as it last looked, so a script changing that stack shows
  only when it comes forward; a window's place on screen isn't kept with
  its stack or reported to scripts; and script editors and Info windows
  close when another stack comes forward, as they do on going to another
  stack.
- **History across stacks:** Back and Recent forget cards of stacks you've
  left.
- **Home as HyperCard had it:** its user-level card, search paths and stack
  list. RandomCard keeps the user level in Preferences instead.
- **Card size** changes (`set the rect of card window`). Opening a stack
  whose cards don't fit grows the window to fit (up to the screen), but a
  smaller stack doesn't shrink it.

## Editing and the UI

**Missing:**
- **Scaling cards to the screen.** HyperCard was made when every screen
  was small and much the same; today's run from a 720×720 HackberryPi to
  4K. A stack whose cards don't fit should be shown scaled down, and
  perhaps a small stack scaled up on a big screen, with clicks, drags and
  `the mouseLoc` still in the card's own coordinates. Needs thought first:
  when it happens (always, or a View menu choice such as Actual Size), how
  1-pixel paint stays crisp when scaled, and what scripts see. Notes from
  a first look (2026-11-15): egui can draw a layer scaled and maps the
  mouse to match, so most card code could stay in card coordinates; the
  places that read the raw pointer (drawn cursors, the polygon's rubber
  band, `the mouseLoc`) and fields being edited over the card would need
  converting. (Solitaire was made 672×480 instead, to fit small screens
  now.)
- **Native menus on macOS: what's left.** Keys are native (⌘ on a Mac,
  Ctrl elsewhere) and the Mac's menus are native, at the top of the screen,
  rebuilt as scripts change them. Still to do: Mac alternatives for Home
  and the Browse tool, whose ⌘H and ⌘Tab macOS keeps for itself; `hide
  menuBar` has no effect on a Mac; and opening a stack by double-clicking
  it in the Finder.
- **Undo for Info dialog changes** and for script edits; the undo history
  covers painting, parts and cards.
- **The system clipboard:** cut and copied parts and pictures stay inside
  RandomCard rather than going to the desktop clipboard.
- **Script editor:** syntax coloring, and Comment and Uncomment
  (Command-hyphen and Command-equal sign). Find, Find Again, Find
  Selection, Replace and Replace Again are done, with HyperCard's Whole
  Word, Case Sensitive and Wraparound Search, plus Go to Line.
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
- **AddColor:** bevels show only on transparent parts, and AddColor's
  changes show at once, not at the next `colorCard`.
- **XCMDs and XFCNs.** These are 68k/PowerPC code and can't run directly.
  The common ones are built in as features; see Externals below.
- **PICT resources:** bitmaps decode (src/import/pict.rs); QuickDraw's
  drawing opcodes (lines, shapes, text) don't yet. Palettes use it;
  `picture` windows will.
- **Checksum validation** of LIST and PAGE blocks, for clearer errors on
  damaged stacks.

## Externals

[XCMDS.md](XCMDS.md) lists every external found in real stacks and decides each
one: built into RandomCard, kept as an external, stubbed, or still unknown.
Nearly all have become RandomCard features. For the rare rest, the design:

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
- **Features first.** HyperCard ran on one kind of Mac, so a stack could
  carry 68000 code. RandomCard runs on x86, ARM and more, where compiled
  code in a stack would need a build for each of them. So what an
  external did becomes a RandomCard feature wherever possible, and
  nearly always it is.
- **Compiled externals, rarely.** For a feature that truly can't be part
  of RandomCard, compiled externals will be supported, written in Rust
  against the same interface. They should be the rare exception, and
  they're low priority: how they're packaged for each platform (inside
  a stack, beside it, or in `~/.config/randomcard/externals/`) is
  decided when the first one is needed. Many of Apple's own externals
  were shortcuts for features HyperCard lacked; the real need is
  custom work such as robots or unusual hardware.
- **An example, and a stack for it.** RandomCard ships one example
  external, with a stack of its own that shows how to write and use
  one. That's the only stack that uses an external: stacks that show
  ordinary features, the Tour among them, never do.
- **Imported stacks.** The importer lists the XCMDs and XFCNs in a
  stack's resource fork. A call to one that has no Rust version says
  "This stack uses the XCMD 'X', which RandomCard doesn't have", and
  Stack Info shows the list.

## Color

HyperCard came out on black-and-white Macs; color arrived late, through
the AddColor XCMD laid over 1-bit cards. RandomCard aims for full color,
as modern software has.

**Today:** AddColor from HyperCard 2.x stacks (colored buttons, fields,
rectangles, and color pictures from PICT resources and files) shows on
screen but doesn't print. Color pictures, colored parts and colored text
are RandomCard's own as well.

**The plan, in order.** On the Mac, a pattern was a mask: its black bits
drew in the foreground color and its white bits in the background color.
RandomCard does the same, so a pattern palette and a color palette work
together, and black on white is just the default.

1. ~~**Color pictures, with no visible change.**~~ Done: pictures hold
   any color with transparency (`Pixel` is RGBA). A black-and-white
   picture is kept at one byte a pixel and saved exactly as before; one
   with color is saved as a PNG. Printing shows colors as the nearer of
   black and white until step 5.
2. ~~**The color palette.**~~ Done: the Tools palette's line and fill
   color squares, each opening a picker, From the card and the recent
   colors. A
   pattern's set bits paint in the fill color and its clear bits white;
   the pencil, lines, borders and text use the line color. Scripts use
   `the penColor` and `the brushColor` (a name, `r,g,b` or `#rrggbb`).
   Tools stay pixel-exact, so the bucket fills one exact color. Invert
   gives each color its opposite, Trace Edges outlines in the paint's own
   color, and Darken and Lighten still sprinkle black or white. The
   Tour's Drawing card
   paints balloons in color. Still to come: patterns in color (step 7),
   and painting with transparency.
3. ~~**Images.**~~ Done: HyperCard 2.x's Import Paint and Export Paint,
   in the File menu with a paint tool and from scripts. Import Paint
   reads PNG, JPEG, GIF and BMP, and old MacPaint and PICT files, and
   leaves the picture selected; Export Paint saves the card as it looks,
   parts and all, as a PNG. An image copied in another program pastes as
   a paint selection, and a picture file dropped onto the card lands
   where it's dropped; either is scaled down to fit the card. Pictures
   copied in RandomCard go onto the system clipboard for other programs.
   The Tour's Picture Files.
4. **Colored parts and text.** ~~Fill, text and frame colors for buttons
   and fields in their Info windows and as properties~~ (done: `the
   foreColor`, `the backColor` and `the borderColor`, as
   HyperCard had none; a hilited part swaps its fill and text colors; the
   Tour's Colors on the Fields card). ~~A color in text style runs~~
   (done: `the foreColor` of a chunk, Style ▸ Text Color…, kept by Copy
   Text). ~~AddColor's commands for scripts~~ (done: every addColor
   function, on the color lists cards and backgrounds keep; the Tour's
   Color the House). AddColor stays its own layer rather than becoming
   part colors: its commands address items by their place in that list.
   ~~AddColor's color pictures~~ (done: PICT resources from the stack's
   fork and PICT files beside it are kept with the stack, stretched to
   their rects, their white see-through when marked transparent; opaque
   parts over a picture show it through their white, as AddColor did).
5. **Printing in color**, including AddColor: its colored buttons,
   fields, rectangles and pictures, laid behind the card as on screen.
6. **Color tools in place of 1-bit ones.** Some of HyperCard's paint
   commands were the best a black-and-white screen could do, and don't
   translate to grays and colors:
   - **Darken and Lighten** sprinkle black or white dots: speckles on a
     colored picture, not a darker or lighter one.
   - **Trace Edges** counts anything not white as paint, so on gray or a
     photo it outlines the whole selection as one block.
   - **The bucket** fills one exact color, so on a gradient, photo or scan
     it stops at the first slightly different pixel and leaves gaps.
   - **Tight selections** (Select, the lasso) treat white as empty; on a
     colored background nothing is, so they don't tighten.

   Possible replacements, today's color tools for the same jobs: Darker
   and Lighter, or a brightness amount (toward black or white, keeping
   the hue); Blend or Fade (mix the selection into what's under it, or
   into a color); Tint (a wash of color); Outline where the color changes;
   a tolerance for the bucket and Fill; a gradient fill; perhaps Blur and
   Sharpen. Invert, Transparent and the rest work as they are.

   Design questions: one press a step, as in HyperCard, or an amount (a
   slider, perhaps in the tool options row); in the Paint menu or a
   color section of the palette; and how far to go. A few well-chosen
   tools, not an image editor: RandomCard is for stacks, and photos can
   be edited elsewhere and imported. Old stacks' `doMenu "Darken"`,
   `"Lighten"` and `"Trace Edges"` keep working, with HyperCard's dots
   on black-and-white pictures so they look as their authors made them.
7. **Patterns in color.** A HyperCard pattern is 8×8 bits on or off; its
   on bits paint in the fill color and its off bits always in white
   (step 2). Color QuickDraw added full-color pixel patterns beside the
   old ones, and RandomCard needs the same, in steps:
   - **Two colors:** a color for the off bits too (navy stripes on
     yellow), beside the line and fill colors on the palette, with a
     property for scripts.
   - **Clear off bits,** so a pattern lays its stripes or dots over what's
     underneath.
   - **Full color:** any color in each cell, a tiny tile (brick, wood,
     gingham). The pattern editor becomes a small painting grid with the
     color picker.
   - **Perhaps larger tiles,** 16×16, as 8×8 is tight for detail in color.

   A stack's 40 patterns and `the pattern` as a number keep working, so
   old stacks see no difference; color patterns are kept with the stack,
   as edited patterns are. The palette's shape icons already show the
   current pattern, so they'd show color patterns too.

8. **The Color Editor.** HyperCard 2.4's Color Tools stack added a Color
   menu whose Open Coloring Tools turned HyperCard into an editor of
   AddColor's overlays: a color palette with tools for button, field,
   picture and rectangle overlays; Items (Place Rectangle, Place Picture
   with a preview and importing PICT files, Item Info, forward and back);
   Effects (color transitions for the stack, a background or a card); a
   PICT editor with color paint tools and cropping; and adding the
   `addColor` calls a stack needs to its scripts. RandomCard should have
   all of it as a feature of its own (its Editor and AddColorPalette
   XCMDs can't run), so a HyperCard color stack stays editable as one.
   The plan: a Color mode that edits a stack's AddColor overlays with
   RandomCard's own color palette, Info windows and paint tools, rather
   than a copy of Apple's interface. The Color Tools stack's install
   into Home and its On/Off button then work as they did.

A new release follows steps 1 and 2.

## Movies

HyperCard 2.4's movie syntax, from the documentation in its QuickTime
Tools stack: the Movie command, movie windows (`movie 1`, `window
"Intro"`), `play`, `pause`, `rewind` and the other movie messages, the
properties that make sense for today's movies (rate, currTime, startTime,
endTime, loop, soundVolume, the controller and more), and the messages
the card gets (openMovie, closeMovie, movieRateChanged, movieTimeChanged,
mouseDownInMovie). Movies play through the system's own player, so
RandomCard ships no codecs: the system decodes, in hardware where it can,
and has already licensed H.264 and HEVC. In the Tour's Movies card.

1. **Linux** (done): GStreamer, loaded when the first movie opens, so
   RandomCard runs without it and building needs nothing of it.
2. **macOS** (done): AVFoundation. `AVAssetReader` decodes the frames
   and a render synchronizer plays the sound and keeps time, so movies
   play without the main thread's event loop, tests included. Frames
   decode on the UI thread as they're drawn; if a large movie ever
   stutters, decoding moves to a thread of its own. macOS no longer
   decodes MPEG-1 or MPEG-2, or the classic QuickTime codecs.
3. **Windows** (written, to be tried on Windows): Media Foundation's
   Media Engine, in frame-server mode, hands over each picture through a
   Direct3D 11 texture (or a WIC bitmap, decoded in software, where
   there's no GPU that decodes video). It can't play most movies
   backward, so a negative rate steps back by seeking, without sound.
4. **Classic QuickTime movies, everywhere.** The movies in old stacks are
   in QuickTime's own early codecs: Graphics (SMC), Animation (RLE),
   Apple Video (road pizza) and Cinepak, often in 256 colors with a color
   table of their own. HyperCard 2.4's Simple Movie is one. GStreamer
   happens to decode some of them on Linux (RandomCard puts their colors
   right, which GStreamer gets wrong); macOS and Windows don't decode them
   at all. Decoders of our own for these simple codecs, in Rust, would play
   them the same on every system. This reverses the earlier "no classic
   codecs" decision for video, so it comes after steps 2 and 3. Music
   tracks and QuickTime VR stay out.

Still to come: resizing zoom windows by their grow box; `callBackTime`
and `callBackMsg` (a message at a time in the movie); `mouseUpInMovie`
and the other mouse messages; `the version of QuickTime`, as the movie
player's version; a frame step that knows the movie's frame rate rather
than assuming 30. Not coming: QuickTime VR, sprites, scenes, hotspots and
text tracks (`findText`), exporting, and music tracks.

## HyperCard 2.4.1's own stacks

The stacks that came with HyperCard 2.4.1 (`examples/apple/hypercard_241_stacks`,
from the installation disk; Apple's, so not in git) are the best test of
RandomCard there is. [HYPERCARD_241_STACKS.md](HYPERCARD_241_STACKS.md)
lists how each does, and the stacks that are clean stay clean: the
`hypercard_241_stacks_stay_clean` test checks them in every `cargo test`
wherever the stacks are. `tools/hc241.py` lists the stacks for a full
`click_every_button` run and writes the page and that list from the
results. They're Apple's own stacks, for the very version RandomCard
follows, using its newest features. The first pass (2026-11-16), clicking
every button with `click_every_button`: of 29 stacks, 10 work (the
HyperCard Tour, Language at a Glance, Help Extras, Audio Help, both
alternate Home stacks, Readymade Fields, Practice, About HyperCard and
PaintTest) and 19 have errors. Two files there aren't stacks: HyperCard
Fonts (a font suitcase) and Simple Movie (a QuickTime movie, which plays).

To do, roughly in order. Most errors come from a script that doesn't
parse, which takes all its handlers with it, so the syntax comes first.

1. **HyperTalk syntax not parsed yet:**
   - ~~`lock recent`~~ (done) and `lock error dialogs`, and its `unlock`
     (AppleScript Library)
   - `delete menuItem x from menu 3` (Audio Help's stack script, which
     Home's Take the Tour goes through)
   - `the number of bgs in this stack`
   - button families: `cd btn i of family 1` (New Features)
   - `selectedLine of me`, written as a property (Help stacks)
   - `find word 1 of it in fld "X"`, `find theType theText in theField`
   - `print (number of cards in bg "Body") cards`
   - windows by id: `set the zoomed of window id N`, `send "x" to window id N`
   - a property named by an expression: `set (TheProperty) of window X`
   - ~~`dial` as Phone Dialer writes it~~ (done: a bare `dial` goes to the
     stack's own dial handler, a handler that takes `dial` gets the number
     itself, and `the dialingTime`, `the dialingVolume` and the message
     box's size are in; Phone Dialer dials, and its Shrink Window works)
2. **Movies** (QuickTime Tools): `there is a movie X`, `the version of
   QuickTime`, `the number of windows`; tracks (`track 1 of movie 1`) give
   a clear error rather than stopping the script.
3. **Functions and properties:** `the destination` (HyperCard Help and the
   AppleScript Reference, on every navigation button), `the number of cd
   btns`, `textHeight` in Graph Maker, setting `the rect of card window`
   (see The card window in Suggested order), New Features' "The button has no line 0".
4. **Externals:** ~~Enviro~~ (done; with `lock recent`, the message box
   as `window "message"`, and AddColor pictures, Color Tools now runs
   clean, its Demo included), the palettes Power Tools, Art Bits, Background Art and Stack
   Templates use (`generalPalette`, `fieldPalette`, `hidePalette`), the
   Navigator palette (Readymade Buttons), the `picture` command, and
   `stackIsBeingUsed` and `tips`, which may be handlers in another stack.
5. **AppleScript and Apple events:** `there is a scriptingLanguage
   AppleScript`, `the version of scriptingLanguage "AppleScript"`,
   `request … of program`, `send … to program`, `request appleEvent
   sender`. Parsing them and answering that there's no AppleScript would
   let most of the Apple Event Primer and the three AppleScript stacks run;
   real support is the AppleEvents design question.
6. **The Color Editor** (the Color Tools stack): its Editor and
   AddColorPalette externals are HyperCard 2.4's color-editing tools, to
   be built in as a feature of RandomCard's own (Color, step 8).

Each uncertain behavior gets checked in the emulator first (what `lock
recent` does, what `the destination` returns). The emulator's QuickTime
is now 3.0.1, from the HyperCard 2.4.1 disk, so its movies play there to
compare.

## The web: REST and JSON

The goal: a stack as the front end of a web service, the way people use
React today, with REST as easy as the rest of HyperTalk. HyperCard 2.4 only
had `open URL` (a browser); these are RandomCard's additions. The library
is `src/web/` (`http.rs`, `json.rs`), with nothing of HyperTalk in it; the
language's side is `src/runtime/web.rs`.

1. ~~**GET and reading JSON objects.**~~ Done: `put url "…" into x` and
   `get url …`; `the result` empty when it worked, else "404 Not Found"
   or what went wrong, with the reply still readable; `the httpHeaders`,
   `the httpTimeout`, `the responseHeaders`; `urlEncode` and `urlDecode`;
   `key "name" of json x`, nesting. Rustls on Linux, the system's TLS on
   Windows and macOS. The ICNDB stack in `examples/` (the Internet Chuck
   Norris Database, api.chucknorris.io): a Connection card with the base
   URL, headers and timeout, a background `apiGet` every card shares, and
   a Random Joke card. The Tour stays offline: the web demos are
   example stacks, since the services they call aren't ours.
2. ~~**Arrays.**~~ Done: `element 2 of json x` (from 1, or `last`),
   nesting with keys; `the keys of json x` and `the elements of json x`,
   one a line; `the number of elements of json x`. Objects keep their key
   order. ICNDB's Random Joke card checks categories from the live list
   (`?category=dev,food` for any of several); Categories, Joke by ID and
   Search cards, with a search result opening on Joke by ID. With it,
   ~~Don't Save fields~~ (done: `the dontSave`, and a checkbox in Field
   Info): a field works as usual but its text isn't saved, so a stack
   opens without its last results. ICNDB marks its outputs this way. Still
   to decide: a message before saving, for scripts that tidy up; since
   stacks save themselves every few seconds, it would need to work on
   what's written, not on the cards on screen.
3. ~~**Every verb, and writing JSON.**~~ Done: each method by its own
   name, `post body to url U`, `put body to url U`, `patch … to url U`,
   `query … to url U` (the IETF QUERY method), `delete url U`, and
   `request M [with body] to url U` for any method; the reply in `it`,
   the status in `the result`. `set key "name" of json x to "Ada"` and
   `set element 2 of …` build and change JSON in a variable or field,
   making objects and arrays on the way; a value that's already JSON goes
   in as JSON, anything else as a string. Following the standards: methods
   sent exactly as written (RFC 9110 §9.1), a Content-Type with every body
   unless the script gives one (§8.3), `urlDecode` per RFC 3986 (`+` stays
   a plus), and JSON numbers kept exactly (RFC 8259 §6). ICNDB sends
   several categories as repeated names (`?category=dev&category=food`),
   as form encoding does, while scripts can still build any URL a service
   wants.
   The Tour's The Web card: JSON read and built on the card, offline,
   and a method, URL and body of your own to send (no address of ours).
4. ~~**PostCard**~~ Done: `examples/PostCard.rcstack`, a small Postman
   on a 640×480 card. Each card is a saved request: name, method (any,
   typed or from a menu), URL, params (one `name=value` a line, a name
   may repeat), headers and body; Send shows the status, time, size, the
   reply (Pretty lays out JSON) and its headers, none of them saved. New
   Request and Delete Request. Six examples on chucknorris.io and
   httpbin.org, QUERY among them. With it, `the responseStatus` (the
   status even when it worked), `the responseTime` in milliseconds, and
   `the pretty of json x`.

   Along the way, a **combo box**, which HyperCard never had: a field's
   `choices` (one a line, also in Field Info) put a ▼ inside its right
   edge; choosing one puts it into the field (then closeField), and the
   field can still be typed into, like HTML's `<input list>` with a
   `<datalist>`. The menu is at least the field's width, and typing
   narrows the choices to those containing what's typed. PostCard's and
   the Tour's Method use it. Popup buttons' menus now widen to fit their
   items.
5. ~~**Requests that don't freeze a handler.**~~ Done: `load url U with
   message "gotData"`, and `with message` after any web command; the
   script goes on, and the message comes to the object whose script asked
   (`on gotData theURL, reply`) with the result and the response
   properties set for that reply. The Tour's Send uses it. With it,
   `formEncode` and `formDecode` (HTML forms' encoding, `+` for a space),
   and the combo box narrows its choices as you type. Next: filling cards
   from data, one card per record.

   Swatches (`examples/Swatches.rcstack`, made by `tools/make_swatches.py`)
   uses all of it with The Color API (www.thecolorapi.com): a converter
   for designers and programmers, RGB and HSV taught with sliders and a
   color wheel the stack paints itself, color schemes, and WCAG contrast.

## Testing

- The importer is tested against HyperCardPreview's sample stacks when
  `RANDOMCARD_HC_SAMPLES` is set (see the README). Those samples contain no
  sound resources, so `snd` decoding is only tested with constructed data;
  real stacks with sounds should be added.
- `exercise_stack` runs a real stack with no UI: it opens the stack, goes to
  every card, clicks every visible button, and prints each script error
  once (`RANDOMCARD_EXERCISE=path/to/stack cargo test exercise_stack --
  --ignored --nocapture`). HIN Stack 1.4 runs with no errors. It doesn't
  type into dialogs or choose menu items yet.
- The UI has no automated tests; checks so far were manual.

## Suggested order

Done so far: keyboard messages, paint tools and patterns, clipboard and undo,
using and authoring stacks, the selection, styled text, scriptable menus,
Home and stacks in use, HyperCard 1.x import and AddColor, font metrics,
printing, the debugger, Find and Recent, popup buttons, the stack-testing
harness, and externals as built-in features (XCMDS.md).

The order follows what RandomCard's users ask for, so it will change with
their feedback. For now:

1. **Color, steps 1 and 2:** color pictures, then the color palette (see
   Color). Released as 0.2.0.
2. **Small, clearly good features,** between larger pieces. Done so far:
   Shift for straight lines and perfect shapes, the script editor's Find
   and Replace and Go to Line, copying styled text between fields, and
   Compact Stack (Delete Stack was already in).
3. **Color, steps 3 to 5:** images and colored parts and text (done,
   released as 0.3.0), then printing in color.
4. **HyperCard 2.4's own features:** audit them in the real HyperCard
   2.4.1 (Basilisk II, `examples/apple/emulator-2.4/run.sh`): movies
   from HyperTalk (started: see Movies; macOS and Linux done, Windows
   written), URLs, the new script editor, and its menus and tools.
5. **The card window:** card size changes (done: setting the rect,
   width or height of a card sizes every card, as Stack Info's Resize
   does, and sends sizeWindow; pictures keep what's painted beyond a
   smaller card), a card window smaller than its card, scrolled, the
   zoomed, and `moveWindow`.
6. **Multiple windows.** Done: stack windows (`go to stack … in a new
   window`, `the stacks`, suspendStack and resumeStack); the editor
   windows (script editors, the Variable and Message Watchers, and Info
   windows), each a real window of its own beside the card; and the Tools
   palette. Small alerts and dialogs stay in the main window, as in
   HyperCard.
7. **The rest of painting:** FatBits, the icon editor, Options, pattern
   editing, and the Tools palette with drawn icons, tool groups and
   Patterns are done, the Paint menu is back in the menu bar, and so are
   Select, Pickup, Scale, Keep and Revert, and Rotate, Slant, Distort and
   Perspective.
8. **History across stacks.** (Objects in other stacks are done: `field 1 of card 2 of stack "X"`.)
9. **Report printing, MACE sound, picture windows.**
10. **Needs a design first:** AppleEvents (`request`, `reply`): what they
    mean on today's systems. Home's User Preferences and search-path cards.

## Future

Ideas, not commitments: what HyperCard might have grown into. Each should
feel like HyperTalk (plain words, everything is text) rather than a
bolted-on API, and none should change how existing stacks behave. The
first three are favorites.

- **JSON and REST.** Under way; see "The web" below.
- **Databases.** SQLite from HyperTalk: `query "select …" from database
  "Addresses.db"`, with results as lines and items, and cards bound to
  rows.
- **Unicode and system fonts.** Text beyond MacRoman, emoji, and real
  fonts chosen from the system, for new stacks that don't need to look
  like 1990. Imported Japanese stacks (MacJapanese) are done; stacks from
  the Mac's other scripts (Chinese, Korean, Arabic, Hebrew, Cyrillic…)
  would be imported the same way, by their fonts' ids.
- **Markdown fields.** A field that holds Markdown and shows it
  formatted: `set the markdown of field "Notes" to true`, a true-or-false
  property like `lockText` and `dontWrap`, also set in the field's Info
  window. The field's text stays the Markdown source, so `put`, chunks,
  `find` and scripts work on plain text as always. A locked field shows the
  preview (headings, emphasis, lists, links, code, tables), and an unlocked
  one shows the preview until it's clicked into, then the source for
  editing, then the preview again. Links clicked in the preview send a
  message (`linkClicked`) the way click functions work in locked fields,
  rather than opening anything themselves. Follow CommonMark, with the
  GitHub tables and task lists people expect. Style runs don't apply to a
  Markdown field; its formatting comes from the Markdown.
- **Internationalization.** RandomCard's own text (menus, dialogs, Info
  windows, the Tour) translated, with American English as the source
  language; dates, times and numbers in the user's locale where scripts
  ask for them (`the long date`), right-to-left text and input methods in
  fields. HyperTalk's words stay English, so a stack's scripts run the same
  everywhere.
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
- **Resizable cards.** Cards that grow with the window, beyond HyperCard's
  fixed card sizes.
- **Media.** Recording sound (`record sound`), and playing common audio
  files. Video is under way (see Movies, including classic QuickTime's
  codecs). Not QuickTime's music tracks or QuickTime VR, which are as
  dead as the Mac they ran on.
- **More modern controls** (see the design principles). Sliders are done:
  `set the style of button "Red" to slider`, its contents its value, `the
  min`, `the max` and `the step` as HTML's range input names them, and
  sliderMoved and sliderChanged; in Button Info and the Tour's Channels
  card. Still for sliders: the arrow keys when one has the focus, and
  tick marks. Next candidates: progress bars, steppers, toggle switches,
  segmented controls, date pickers, color wells and tabs, each a part
  style with HyperTalk names.
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
