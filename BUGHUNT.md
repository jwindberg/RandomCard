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

Level 4 runs with the disc folders read-only, so no stack's script can change
or delete anything in the collection.

To recheck after a fix, put the affected stacks' paths in a list, run level
4 on it, and pass the result to `working_md.py` after `LEVEL4.tsv`.

RandomCard converts a HyperCard stack in memory each time it opens one, so
none of the Developer CD stacks are saved in RandomCard's format yet. Later:
a conversion loop that imports every stack, applies its fix steps (the *
stacks), and saves it as a `.rcstack` into a "RandomCard Stacks" folder
mirroring the discs' folders, re-run whenever the importer improves.

## Where we are

1,703 distinct stacks.

| Level | Stacks |
| --- | --- |
| Open cleanly (levels 1–3) | 748 |
| Tested at level 4 | all 748 (320 without compiled externals, then the 428 with them) |
| **Work fully (level 4)** | **429** (16 use stubs, marked †) |

1,192 of the 1,703 stacks contain compiled XCMDs or XFCNs, 885 distinct
names between them. Each one either becomes a RandomCard feature or, rarely,
stays an external; see XCMDS.md.

## Open bugs

Counts are stacks; "at open" counts come from level 3 over all 1,703, "on
click" counts from level 4 over the 320. A stack can have several bugs, and
fixing its first one can uncover the next.

| # | Bug | Stacks | Example | Notes |
| --- | --- | --- | --- | --- |
| 19 | Externals not built in (on click) | about 180 of the 428 with XCMDs | Outside the Rinaldi collection: TeXWindow 7, ResCopy 5, MakeSendMe5 4, QTPict 3 | The XCMD stacks' biggest blocker. 127 of the failures are the Rinaldi collection, one demo stack per external (48 externals, US and French): each needs its own external built in. Each external becomes a feature or, rarely, stays an external; see XCMDS.md. |
| 20 | No such object (on click) | 21 | | Not looked at yet. |
| 17 | Fonts carried in a stack's resource fork are ignored | 374 (19 on the working list) | About Graphics Database Toolkit: body text in Garamond 16 drawn in Geneva, too wide to fit | Read FOND names, FONT/NFNT widths and line heights, and ideally draw the bitmaps themselves. Level 4 can't see this: it only catches script errors. |
| 5 | No such bkgnd field (on click), not sorting | 15 | Catalog, "New Button" (HyperCard British 1.2.2); Background Ideas, "Calendar" | Catalog, Téléphone, Katalog and the like, card 1; the idea stacks' Calendar buttons, after goWeekly was fixed. |
| 18 | Companion stack missing from the disc | 4 | Button Ideas (Desperately Seeking Seven): no DateBook on that disc | Not RandomCard's fault: real HyperCard asks "Where is DateBook?" and fails the same way. Should be marked so they stop counting against us. |
| 6 | No such card button / bkgnd button (on click) | 11 | Help Index; Using, "Troubleshooting" | Help Index card 1 has an unnamed button that refers to another. |
| 7 | No such card field (on click) | 4 | Display Card 4•8, card 92 "Check my answers" | The training quizzes. |
| 8 | No such card (on click) | 3 | UserList, "Return"; OASIS Stack 1.1, "Desktop" | |
| 9 | `goHome`, `goBack`, `exitStack`, `justThere`, `sendString`: Can't understand (on click) | 9 | Content Selector, "Home" | Handlers in a companion or Home stack; check whether the companion is on the disc (#18). |
| 10 | Can't set the hilite of a stack (on click) | 2 | •HC 2.0-3, "Goals & Objectives" | Probably `set the hilite of button X of stack Y`, a part in another stack. |
| 11 | Too much recursion (on click) | 1 | Téléphone, "Dial" | |
| 12 | No such stack (at open) | 164 | | Companion stacks, Home and help stacks. Sample which stacks they look for before fixing. |
| 13 | Can't understand / No such function (at open) | 191 | | Mostly externals not built in yet. |
| 14 | Scripts that don't parse | 410 | | Run the parse-failure probe across all stacks. |
| 15 | The stack is damaged (bad first block) | 47 | | Very early stacks, or not stacks at all. |
| 16 | Clicks that ran into the 2-second limit | 24+ working stacks | Macintosh IIcx guided tours (211 of 583 clicks); Macintosh IIx (14) | Probably animations or loops waiting for a key; check by hand. |

## Fixed

| # | Bug | Stacks | Fix |
| --- | --- | --- | --- |
| 1 | Opening hung forever | 1 (SysEnv XFCN 1.0D13) | `repeat until the mouse is down` with an empty body never checked for ⌘-period; loops now check every pass. |
| 2 | Can't set the rect / loc of a stack (at open) | 27 | Scripts can read and set the card window's rect, loc and edges; setting moves the window. Not yet rechecked. |
| 3 | No such bkgnd field when sorting (on click) | 30 hit it; 9 now work fully | A card without the field sorted on sorts as empty, as in HyperCard. The rest moved on to #4 and #5. |
| 21 | FilePath, NewFileName, MacType, PopUpMenu, PopMenu not built in (on click) | 58 rechecked; 47 now work fully | Built in. Scripts' pop-up menus use popup buttons' menus. The Rinaldi demos moved on to their own externals (Switch, LeafName). |
| 22 | TEXWindow, then INITFinder, BoardInfo, SwitchXRoo not built in (on click) | 7 (Apple Bug Reporter) | TEXWindow opens a HyperCard 2 text window (`the text of window`, `close window`, TEXWindowClosing); the others are stubs. All 7 work, marked †. |
| 4 | `goWeekly`, `goSixMonthly`: Can't understand (on click) | 56 rechecked; 34 now work fully | Three fixes. Stacks are found as HyperCard's default search paths found them (the stack's folder, folders beside and above it, and the other floppies' folders), with names matched regardless of case. DateBook's XFCN dayMonthName is built in. And `click at` with the button or field tool selects the part instead of clicking it, as in HyperCard; DateBook's Extend button had been clicking itself forever. |
