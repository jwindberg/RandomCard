# RandomCard roadmap

What's left for RandomCard, grouped by area. Work now follows three
tracks: HyperCard features still missing, HyperCard 2.4's own features,
and new features HyperCard never had, color first. Each section says what
exists today, what's missing, and where the work would go. The suggested
order at the end is shaped by what RandomCard's users ask for.

The drive to make every stack on Apple's Developer CDs work ended at 1,029
working (WORKING_STACKS.md; 1,057 at the 2026-11-08 regression run). Those stacks are now a regression suite: a
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
| Paint tools | All 18 tools, patterns, Paint menu; no FatBits |
| Fields | Styled text, the selection, click functions, list fields |
| Visual effects | Complete |
| Sound | `play`, notes, `dial`, `beep`; no MACE |
| HyperCard import | 1.x and 2.x: structure, pictures, parts, text, scripts, icons, sounds, AddColor |
| User levels and stack protection | Levels, Protect Stack, passwords, private access |
| Script-modifiable menus | Complete, except submenus |
| Home, stacks in use, going between stacks | One stack window at a time |
| Printing | Cards and text, as PDF; no report printing |
| Externals (XCMDs) | The common ones built in as features ([XCMDS.md](XCMDS.md)) |
| Color | AddColor from imported stacks, on screen only; no color painting, parts or text yet |

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
  `the pattern` affecting lines and borders, which are always solid.
- **Keep and Revert.** HyperCard saved everything as you worked except
  painting: a card's or background's paint changes were held until Keep
  (Paint menu, Command-K), switching to or from the background, going to
  another card, or quitting; until then Revert threw them all away
  (unlike Undo, which takes back only the last change). RandomCard saves
  paint changes as they're made, so it has neither yet.
- **Paint menu:** Pickup and Scale; `the grid`, `centered` and
  `multiple` options (`the multiSpace` is kept, but nothing is drawn
  multiple yet). (Shift-constrained drawing is done: straight
  strokes, lines and polygon sides at multiples of 15°, squares and
  circles, straight moves of a selection, and `drag … with shiftKey`.)

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
- **Creating and editing icons** (an icon editor, or importing images).
  Icons can be imported and chosen, not drawn.
- **Popup details:** the menu opens with the chosen item roughly over the
  button, but not to the pixel; the title is always drawn left-aligned;
  and `the selectedLine` can't be set (use `select line 2 of button 1`).

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
- **Card size** changes (`set the rect of card window`).

## Editing and the UI

**Missing:**
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
- **AddColor pictures** (PICT resources or files) are imported but not
  drawn, and bevels show only on transparent parts. AddColor's changes
  show at once, not at the next `colorCard`.
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
rectangles and color pictures) shows on screen but doesn't print. Nothing
else can be colored.

**The plan, in order.** On the Mac, a pattern was a mask: its black bits
drew in the foreground color and its white bits in the background color.
RandomCard does the same, so a pattern palette and a color palette work
together, and black on white is just the default.

1. ~~**Color pictures, with no visible change.**~~ Done: pictures hold
   any color with transparency (`Pixel` is RGBA). A black-and-white
   picture is kept at one byte a pixel and saved exactly as before; one
   with color is saved as a PNG. Printing shows colors as the nearer of
   black and white until step 5.
2. ~~**The color palette.**~~ Done: in the tool panel under the
   patterns, Line and Fill swatches with a picker and an eyedropper each,
   and recent colors that set whichever of Line or Fill is chosen. A
   pattern's set bits paint in the fill color and its clear bits white;
   the pencil, lines, borders and text use the line color. Scripts use
   `the penColor` and `the brushColor` (a name, `r,g,b` or `#rrggbb`).
   Tools stay pixel-exact, so the bucket fills one exact color. Invert
   gives each color its opposite, Trace Edges outlines in the paint's own
   color, and Darken and Lighten still sprinkle black or white. The
   Tour's Drawing card
   paints balloons in color. Still to come: two-color patterns (a
   background color for the clear bits), and painting with transparency.
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
   foreColor`, `the backColor` and `the borderColor`, LiveCode's names, as
   HyperCard had none; a hilited part swaps its fill and text colors; the
   Tour's Colors on the Fields card). ~~A color in text style runs~~
   (done: `the foreColor` of a chunk, Style ▸ Text Color…, kept by Copy
   Text). ~~AddColor's commands for scripts~~ (done: every addColor
   function, on the color lists cards and backgrounds keep; the Tour's
   Color the House). AddColor stays its own layer rather than becoming
   part colors: its commands address items by their place in that list.
5. **Printing in color**, including AddColor.

A new release follows steps 1 and 2.

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
   2.4.1 (Basilisk II, `examples/apple/emulator-2.4/run.sh`): QuickTime
   from HyperTalk, URLs, the new script editor, and its menus and tools.
5. **The card window:** global rect and loc, scrolling (hBarLoc, vBarLoc),
   zoom, `moveWindow` and `sizeWindow`, and card size changes.
6. **Multiple windows.** `go to stack … in a new window`, and
   `the stacks`. Start with the tool windows: script editors, the Variable
   and Message Watchers, and Info windows are drawn inside the main window
   today, so stepping through a script covers the card being debugged.
   Each should be a real OS window (an egui viewport). Small alerts and
   dialogs stay in the main window, as in HyperCard.
7. **FatBits, an icon editor and the rest of painting:** the Options and
   Patterns menus, editing patterns, Pickup, Scale, and Keep and Revert.
8. **Objects in other stacks,** and history across stacks.
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
- **Media.** Video playback in a card region, recording sound
  (`record sound`), and playing common audio files. QuickTime music
  tracks too (note data for a General MIDI synthesizer, as Lost and Found
  plays its theme songs).
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
