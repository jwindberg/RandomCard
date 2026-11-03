# Bug hunt

RandomCard's measure of success is how many real stacks work fully and
correctly. This file tracks what stops the stacks on Apple's Developer CDs
(1989–1994) from working, ranked by how many stacks each fix would rescue.
Stacks that already work fully are listed in [WORKING_STACKS.md](WORKING_STACKS.md).

## How stacks are tested

The stacks live in `examples/apple/devcds`, unpacked by `tools/devcds.py`
(not in git). Testing goes in levels:

1. **Imports:** the stack file reads.
2. **Parses:** every script compiles.
3. **Opens:** openStack, openBackground and openCard run without a script error.
   Levels 1–3 are the `index_stacks` test, which writes `INDEX.tsv`;
   `tools/stacks_md.py` turns that into `STACKS.md`.
4. **Clicks:** every scripted button on every card is clicked, and no click
   gives a script error. This is the `click_every_button` test, which reads a
   list of stacks and writes results beside it (`LEVEL4.list` gives
   `LEVEL4.tsv`). `tools/working_md.py` turns the results into WORKING_STACKS.md.

Some stacks can't pass level 4 however well RandomCard does: clicked in card
order, a "Show" button comes before the "Create" it needs. For those,
hand-written steps run it in a sensible order, as a person would use it:
one file per stack in `tests/driven`, naming its copies (`path:` lines) and the
message box lines to run. The `driven_checks` test runs them all and writes
`DRIVEN.tsv`; pass it to `working_md.py` with `--driven`, and a stack whose
steps run without an error counts as working, marked ‡. (`drive_stack` runs
steps for one stack, for trying them out.)

Level 4 runs with the disc folders read-only, so no stack's script can change
or delete anything in the collection.

To recheck after a fix, put the affected stacks' paths in a list, run level
4 on it, and pass the result to `working_md.py` after `LEVEL4.tsv`.

A typo in a stack's own script, one real HyperCard rejects too, is fixed in
the stack file itself, with the untouched file kept in
`examples/apple/devcds/ORIGINALS/` at the same path. Stack scripts that
only assume a Mac (colon paths, say) aren't bugs and get fix steps instead.

RandomCard converts a HyperCard stack in memory each time it opens one, so
none of the Developer CD stacks are saved in RandomCard's format yet. Later:
a conversion loop that imports every stack, applies its fix steps (the *
stacks), and saves it as a `.rcstack` into a "RandomCard Stacks" folder
mirroring the discs' folders, re-run whenever the importer improves.

## Where we are

1,703 distinct stacks.

| Level | Stacks |
| --- | --- |
| Open cleanly (levels 1–3) | 820 (748 at the first index) |
| Tested at level 4 | all 748 (320 without compiled externals, then the 428 with them), then 297 more whose scripts parse since #48 |
| **Work fully (level 4)** | **923** (45 use stubs, marked †; 3 checked by driven steps, marked ‡) |

1,192 of the 1,703 stacks contain compiled XCMDs or XFCNs, 885 distinct
names between them. Each one either becomes a RandomCard feature or, rarely,
stays an external; see XCMDS.md.

## Open bugs

Counts are stacks; "at open" counts come from level 3 over all 1,703, "on
click" counts from level 4 over the 320. On 2026-11-02 every failing stack
whose result came only from an older run (250) was rechecked: 9 had been
fixed along the way, and the counts below are from the recheck. A stack can have several bugs, and
fixing its first one can uncover the next.

| # | Bug | Stacks | Example | Notes |
| --- | --- | --- | --- | --- |
| 19 | Externals not built in (on click) | about 180 of the 428 with XCMDs | Outside the Rinaldi collection: ResCopy 5, QTPict 3, InfMgr 3, ProgressWindoid 3 | The XCMD stacks' biggest blocker. 127 of the failures are the Rinaldi collection, one demo stack per external (48 externals, US and French): each needs its own external built in. Each external becomes a feature or, rarely, stays an external; see XCMDS.md. |
| 17 | Fonts carried in a stack's resource fork are ignored | 374 (19 on the working list) | About Graphics Database Toolkit: body text in Garamond 16 drawn in Geneva, too wide to fit | Read FOND names, FONT/NFNT widths and line heights, and ideally draw the bitmaps themselves. Level 4 can't see this: it only catches script errors. |
| 31 | Broken on the disc: develop Issue 3 calls a handler its library doesn't offer | 6 copies | Card 115's resize button calls `resizeWindow`, which develop Index keeps in its own background; the library part (its stack script) offers `ISSUEresizeWindow`. Written for an older develop Index. Real HyperCard says "Can't understand" too. |
| 32 | Broken on the disc: stacks missing their own palettes or handlers | 5 | Developer Response Stack (2 copies, Titles Sampler): `palette "TitleNav"`, but its resource fork has no palettes at all (the Sampler beside it is a document, not a stack). 12 inch RGB Monitor BW and its MINIMUM copy: `palette "mainMenu"`, but the stack's resource fork has only "eddie1" (copied from the Monochrome stack without its palettes). Macintosh Classic.MINIMUM: its playSound buttons stayed when the sounds and the `playSound` handler were stripped. | Real HyperCard fails the same way. |
| 33 | `picture` command (HyperCard 2's picture windows) | 2 (Macintosh LC Color, Macintosh Classic BW) | Their "menu" button shows the "waitDLOG" picture, then goes to "Oct'90 Prod Trng:ATS Menu", which isn't on the disc (and one calls PlayMovie) | The PICT decoder that palettes brought (src/import/pict.rs) is the start of it. Even with `picture`, these two stop at the missing stack. |
| 35 | Broken on the disc: a damaged copy of a companion stack | 1 (•HC 2.0-6, Volume 07) | Its return arrows go to "•HC 2.0-2", whose copy on Volume 07 is typed as a stack but holds sound-like bytes (the Volume 04 copy is fine). It passed level 4 only while RandomCard couldn't find companions; HyperCard's search paths now find the damaged file. | Real HyperCard can't open it either. |
| 37 | Broken on the disc: Q & A Stack's Help button on its answers cards | 12 (Q & A Stack 4.0.3 to 4.1.1) | The button toggles `bg fld "help"`, but the "answers" background has no such field (checked in the raw block, not just after import). | Real HyperCard fails the same way. |
| 42 | Mac paths: `the long name of stack` has `/` paths, scripts look for `:` | 156 of the 970 tested parse colon paths; 4 Technical Notes Stacks hang | Technical Notes Stack 3.4.x: resetPrefs runs `repeat until last char of shortstackpath = ":"` over the stack's long name to find its folder; with no colon the loop never ends (the click test's 2-second stop used to hide it, and it left openStack half done). | Decided: each system's own paths, through Rust's `std::path`, and on macOS colon paths as HyperCard gave them (`Disk:Folder:Stack`, taken back in file commands too). New function `the pathSeparator` (":" on macOS, "\" on Windows, "/" elsewhere) lets a stack find its folder anywhere. On Linux and Windows the Technical Notes stacks need a fix step: `":"` in resetPrefs becomes `the pathSeparator`. |
| 41 | A click order that real HyperCard fails too | 1 (Guidebook Addresses) | Padlock unlocks the fields; Home then empties the globals (clearGlobals) before `go home`, and closeCard relocks through `toggle`, whose `if dataChanged` gets empty: "Expected true or false". Also needs RadioDialog, RadioDialogPlus and kSort (#19). | Like UserList (#8). |
| 8 | No such card (on click), what's left | 4 | UserList (2): the click test presses "Delete User", which deletes the card openStack pushed, before "Return" pops it; real HyperCard can't go to a deleted card either, so it's the click order (like ProgressWindoid, #24). Getting Started, "Finder"; OASIS Stack 1.1, "Desktop": not looked at. | |
| 5 | No such bkgnd field (on click), what's left | 2 | Version 1.2 nyheter, "Se även"; Welcome, "Install Button on Home Card" (sends to a handler in its era's Home stack) | Most of the 36 were fixed by #38. |
| 28 | `No such object ""` (on click) | 32; 25 now work fully | `select ""` is `select empty`, as in HyperCard. |
| 30 | Too much recursion (on click) | 23; 17 now work fully | The develop stacks' resize button presses Command-Shift-E, which shows HyperCard 2's size box, then clicks; that click drags the box. Without the size box the click went back to the button forever. `type` with those keys now arms it. |
| 18 | Companion stack missing from the disc | 4 | Button Ideas (Desperately Seeking Seven): no DateBook on that disc | Not RandomCard's fault: real HyperCard asks "Where is DateBook?" and fails the same way. Should be marked so they stop counting against us. |
| 6 | No such card button / bkgnd button (on click) | 11 | Help Index; Using, "Troubleshooting" | Help Index card 1 has an unnamed button that refers to another. |
| 7 | No such card field (on click) | 4 | Display Card 4•8, card 92 "Check my answers" | The training quizzes. |
| 9 | `goHome`, `goBack`, `exitStack`, `justThere`, `sendString`: Can't understand (on click) | 9 | Content Selector, "Home" | Handlers in a companion or Home stack; check whether the companion is on the disc (#18). |
| 10 | Can't set the hilite of a stack (on click) | 2 | •HC 2.0-3, "Goals & Objectives" | Probably `set the hilite of button X of stack Y`, a part in another stack. |
| 11 | Too much recursion (on click) | 1 | Téléphone, "Dial" | |
| 12 | No such stack (at open) | 164 | | Companion stacks, Home and help stacks. Sample which stacks they look for before fixing. |
| 13 | Can't understand / No such function (at open) | 191 | | Mostly externals not built in yet. |
| 14 | Scripts that don't parse | 149 of the 446 the index listed | play "Boing" e c d g3h. (unquoted notes, 30+); go to Help Index (unquoted names of two words, 53); there is a folder (42); messageName of window wName (22); print field "Name" with appName (13) | #48 fixed the other 297. The probe: `RANDOMCARD_DUMP_ERRORS=1` over the list. |
| 15 | The stack is damaged (bad first block) | 47 | | Very early stacks, or not stacks at all. |
| 16 | Clicks that ran into the 2-second limit | 24+ working stacks | Macintosh IIcx guided tours (211 of 583 clicks); Macintosh IIx (14) | Probably animations or loops waiting for a key; check by hand. |

## Fixed

| # | Bug | Stacks | Fix |
| --- | --- | --- | --- |
| 49 | Rinaldi externals not built in: MacType's six items, Switch, CapsLock, ClipInfo, ATalkZones, GlobalList, WindName (part of #19) | 25 (their demo stacks, US and French); all 25 now work fully | Built in (src/runtime/rinaldi.rs). MacType answers his six items (the system's name for the computer, as his list ended in 1990); Switch turns items to lines and back, by the itemDelimiter; CapsLock reads the last letter typed; ClipInfo gives TEXT or PICT for what was last copied; ATalkZones says there's no node address (no AppleTalk); GlobalList lists the globals, and declaring a global now makes it, as in HyperCard; WindName titles the window until another stack opens. Tour: Keys card (Caps Lock), Fields card "Switch", Debugging card "List Globals", "This Computer" and "Rename Window". |
| 48 | Scripts that don't parse: `top of window` with no window name | 297 of the 446 the index listed as not parsing; 192 newly working | SuperCard's name for the current window. Rinaldi's later demo stacks (about 270) share a stack script whose SuperCard branch says `return top of window`; HyperCard never runs it but compiles it, and RandomCard couldn't, so their stack scripts were dead and the index kept them from level 4. Bare `window` now means the card window. The 313 rechecked (with #49's) are in RINALDI2.tsv. |
| 47 | Rinaldi externals not built in: LineCount, FullSort, DateSort, GetPassword, Prompt, GetMode (part of #19) | 22 (their demo stacks, US and French, on several discs); all 22 now work fully | Built in (src/runtime/rinaldi.rs), with his answers to "!" and "?" and his error texts. LineCount counts a field's lines as RandomCard wraps them (the print code's layout, now shared). FullSort sorts lines or items by ASCII, case, international order, embedded number, date or time, ascending, descending, random or a given order, on any chunk, and keeps FullSortList for co-sorting. Prompt is a locked text window. GetPassword answers the number `ask password` does. Tour: Fields card, "Count Lines". |
| 46 | Broken on the disc: Technical Notes Stack's Select Folder button | 16 (Technical Notes Stack 4.0.2 to 4.1.5); 8 newly working, the others had passed without reaching it | Card 2's illustrations "Select Folder" button had `put cd fld "StackPictPath" into oldpath of cd id 82133`, which real HyperCard can't compile either; its twin, the index button, has `put cd fld "StackIndexPath" of cd id 82133 into oldpath`. Fixed in the stack files by putting the words in that order (same length, in the live CARD block; six files also hold a dead copy in a FREE block, left alone). Originals in `examples/apple/devcds/ORIGINALS/`. The same recheck found APDAlog Stack (Volume 07) now stopping at the BigPop external, and Technical Notes Stack 3.1 at a missing card button. |
| 44 | RandomCard's Home stack lacked HyperCard's own Home cards (was #40) | 4 (Stack Dump 1.2.2; the KL Launchers in Launch 1.2 and Launcher 1.3; Prototyping Toolkit 2.0's Start Here) | These stacks replace Home's script, and their getHomeInfo goes to card "User Preferences" of stack "Home" for the user's name, level and Power Keys, then to cards "stacks", "applications" and "documents" for the search paths. Home now has those cards (added to an existing Home when it loads). The level on them follows the user level, and the level set from Home's own card is kept in Settings. The folders on the Stacks card are searched for stacks, as in HyperCard, and `stack "Home"` names the Home stack whatever its file is called. |
| 45 | Broken on the disc: a typo in the stack's own script (was #26) | 3 (FullRename 1.4 (US), 1993-05, 1993-08, 1993-11) | `selectDir("Folder to rename"")` had an extra quote, which real HyperCard rejects too. Fixed in the stack file itself: the quote became a space, a one-byte change in a CARD block (no checksum). The untouched files are in `examples/apple/devcds/ORIGINALS/`. |
| 1 | Opening hung forever | 1 (SysEnv XFCN 1.0D13) | `repeat until the mouse is down` with an empty body never checked for ⌘-period; loops now check every pass. |
| 2 | Can't set the rect / loc of a stack (at open) | 27 | Scripts can read and set the card window's rect, loc and edges; setting moves the window. Not yet rechecked. |
| 3 | No such bkgnd field when sorting (on click) | 30 hit it; 9 now work fully | A card without the field sorted on sorts as empty, as in HyperCard. The rest moved on to #4 and #5. |
| 21 | FilePath, NewFileName, MacType, PopUpMenu, PopMenu not built in (on click) | 58 rechecked; 47 now work fully | Built in. Scripts' pop-up menus use popup buttons' menus. The Rinaldi demos moved on to their own externals (Switch, LeafName). |
| 22 | TEXWindow, then INITFinder, BoardInfo, SwitchXRoo not built in (on click) | 7 (Apple Bug Reporter) | TEXWindow opens a HyperCard 2 text window (`the text of window`, `close window`, TEXWindowClosing); the others are stubs. All 7 work, marked †. |
| 23 | MakeSendMe5, GetVol not built in (on click) | 7 (Q & A Stack 3.4 and 3.4.1, Scan Demo) | MakeSendMe5 (and MakeSendMe4) opens a new message in the user's mail program; GetVol lists the mounted volumes. All 7 work. |
| 24 | ProgressWindoid not built in (on click) | 3 (the ProgressWindoid demo) | Built in, with HyperCard 2's window commands for it: `show`, `hide` and `close window`, `set … of window`, and the fraction, text, loc, width, font and visible of a window. The click test fails these stacks on click order alone (Show before Create, Get after Close); driven in order (`tests/driven/ProgressWindoid.txt`), every button works, so all 3 count, marked ‡. |
| 25 | The Rinaldi collection's file externals (on click) | 64 demo copies; 63 now work fully | Built in (src/runtime/files.rs): FileCopy, EraseFile, FullMove, FullRename, GetDir (with Rinaldi's filters and layouts), GetFInfo, FullSFPut, GetBootVol, GetSysFolder, and the helpers the demos use: LeafName, FileN, FolderPath, SelectDir. Each answers "!" and "?" as Rinaldi's did. FileMaker (FileToRes, ResToFile) waits: it keeps files in a stack's resource fork. |
| 29 | `palette` command (on click) | 12 (the Info Island training stacks); 7 now work fully | HyperCard 2's palettes, built in: the stack's 'PLTE' resources and their 'PICT' pictures are read at import (a new PICT decoder, src/import/pict.rs) and kept in the stack; `palette name, point` opens one as a floating windoid whose buttons send their messages to the card. openPalette and closePalette are sent; `close`, `show` and `hide window`, the loc of a window, and the buttonCount, commands and hilitedButton properties work. Of the other 5, 3 are broken on the disc (#32) and 2 need `picture` (#33). |
| 34 | `There is no menu item "5" in menu "Windows"` (at open) | 8 (develop Find, Index, Table of Contents, Issues 1–5, on Volume 06); all 8 now work fully | The develop stacks build a Windows menu from `the windows`, expecting the card window under its stack's name, as HyperCard lists it. `the windows` now lists script windows, the card window by its stack's name, then Message, Tools and Patterns; `show window` takes the stack's name and "Patterns". Rechecked the 64 working stacks that read `the windows`: 63 still work, and the other was broken on the disc all along (#35). |
| 36 | Can't set the locktext of a window (on click) | 5 (C:F Registration Requests, 1993 discs); all 5 now work fully | Its zoom buttons show a field in a TEXWindow, then lock it. A text window's lockText now makes it read-only (still selectable, for copying). Zooming again sends "select" to the window: `send … to window` now works, "select" bringing it to the front and "close" closing it. |
| 20 | No such object (on click) | 21 (SpInside Macintosh, Technical Notes, HIN and APDALog stacks); all 21 now work fully | Stale: all 21 failed on card 3's "mask" button in the first XCMD run, and `select ""` (#28) had fixed them without a recheck. |
| 38 | `find … in field` and `pop card` (on click) | 49 rechecked (No such bkgnd field and No such card); 28 now work fully | `find … in bkgnd field "X"` looked the field up on the current card and then matched its id on every card; now the field is looked up on each card by its description, skipping cards without it, as HyperCard does (DateBook in every language, the idea stacks' Calendar buttons, the Catalog stacks). `push card` now saves the card's stack too, so `pop card` after a visit to another stack goes back there (DateBook's Return after the Address stack). |
| 39 | `startUp`, ZoomRect, and the click test's trips away (on click) | 6 (Can't find palette, Expected true or false); 2 now work fully | HyperCard sends startUp to the first card it shows, before openStack; RandomCard didn't, so the Prototyping Interactive Video Home stacks never set their globals. ZoomRect is built in (XCMDS.md). The click test now has a Home stack, so `go home` leaves the stack instead of going to card 1, and a click that leaves comes back with openStack, as a user returning would; before, stacks kept the globals a click had cleared. |
| 43 | The full recheck under the new click test (2026-11-02) | all 970 tested | Every tested stack rerun with a Home stack and real return trips. 103 that had passed failed: 48 on `fontExists` (Macintosh Reference, now built in), 37 on `send "startUp" to Home` (a bare Home is now the Home stack), 12 on Home's `getHomeInfo` (now built in: RandomCard's settings are always loaded). 93 pass again; the other 10 were never really working (#40, #42, and a few one-offs). The harness also leaked a copy of the stack per trip home (23 GB with 8 workers); fixed. |
| 27 | Found by the second index (2026-11-02): companions now found, and their scripts | 188 stacks tested, 89 now work fully | Parse: `hide scroll window`, `go cd "page" && n of stack "x" && y`, handler names with periods (`BuildQuery.old`), unquoted `with menuMessages`, `the card window`, `menu (expr)`, `show groups`. Run: the dynamic path from a library stack to the current stack (Apple's manual, "The Dynamic Path"); the scroll of the card window; `select` of a chunk held in a variable; XVersion. |
| 4 | `goWeekly`, `goSixMonthly`: Can't understand (on click) | 56 rechecked; 34 now work fully | Three fixes. Stacks are found as HyperCard's default search paths found them (the stack's folder, folders beside and above it, and the other floppies' folders), with names matched regardless of case. DateBook's XFCN dayMonthName is built in. And `click at` with the button or field tool selects the part instead of clicking it, as in HyperCard; DateBook's Extend button had been clicking itself forever. |
