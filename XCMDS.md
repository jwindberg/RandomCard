# Externals (XCMDs and XFCNs)

HyperCard stacks extended HyperTalk with externals: compiled code stored in
a stack's resource fork (or in Home, or in HyperCard itself). An **XCMD** is
an external command (`PrintOut "text"`), an **XFCN** an external function
(`get SysPrefs(name)`). Real HyperCard ran their 68000 or PowerPC code on
the Mac; RandomCard can't run that code.

This document lists every external found in the stacks we test against,
what it did, and what RandomCard does about it. It's the plan for roadmap
item 4, and it grows as more stacks are scanned.

## Decisions

Each external gets one of four decisions:

- **Built in.** RandomCard does this itself, as a HyperTalk command,
  function or property, or as part of the app. The external's own name
  keeps working, so old stacks run unchanged. This is the choice wherever
  it's possible. Almost nothing HyperCard's externals did was truly tied to
  the hardware; they were features HyperCard should have had.
- **External.** Stays an external (roadmap item 4), only when a feature
  truly can't be part of RandomCard. This should be very rare, and it's low
  priority. HyperCard ran on one kind of Mac, so an external was one piece
  of 68000 code; RandomCard runs on x86, ARM and more, so an external would
  need building for each of them. None of the externals found so far needs
  this.
- **Stub.** Obsolete on modern systems (memory checks, Mac system details,
  32-bit QuickDraw tests). It answers harmlessly, so stacks that ask keep
  running.
- **Unknown.** We can't document or specify it yet: no documentation, and
  its behavior isn't clear from the stacks that use it. Needs research
  before a decision.

The **Status** column says whether it's planned or done. HIN Stack 1.4's
externals were done first, since it uses no others.

## Same name, different external

Externals were written by many people, and some names were used by more than
one. A built-in has to answer the way the stack's own external did:

| Name | The versions | How RandomCard tells them apart now |
| --- | --- | --- |
| FileExists | Apple DTS's answers a Mac error code (0 when the file is there); the others answer true or false. | Guesses from whether the stack's scripts compare the answer with 0. |
| PopMenu | Appropriate Technology's takes (top, left, checked, items) and answers "true,item,menu"; another takes (point, lines) and answers a line number. | By the number of arguments. |

Guessing from scripts or arguments is fragile. Each stack carries its
external's compiled code in its resource fork, so the reliable way is to
identify the external by that code (a checksum of the XCMD or XFCN resource,
or the version and author strings most contain) and answer as that version
did. Add to this table whenever another one turns up.

## Names and prefixes

Some externals carry their author's prefix (Krs and the like). Decided
(2026-11-02): RandomCard stays proper. A built-in gets one clean name
(`NewMenu`, not `KrsNewMenu`), and the interpreter doesn't keep the old names
as aliases. RandomCard doesn't promise to run every old stack unchanged;
instead each such issue gets a written fix step, applied to our converted
copy of the stack (never the original on the disc). The fix steps are listed
in one place, each with the old form, the new form, why, and the stacks that
need it. Fix steps stay mild, like removing a prefix from a couple of calls; a stack that would need a drastic rewrite shows a gap in RandomCard to close instead. The same goes for name collisions above: a stack carrying the other
version of an external gets the fix step for that version.

## Externals found

First scan, 2026-10-31: the 31 Apple developer stacks in `examples/apple`
(Technical Notes, Q & A, Qstack, SpInside Macintosh, Human Interface Notes,
the Developer Stack and the 1990 Developer Conference stack). They hold 180
distinct externals. Names that differ only in case (`KRSGetInfo` and
`KrsGetInfo`) are counted once, since HyperTalk ignores case. More than half
come from the Developer Stack, Apple's library of sample externals; most of
those are never called by its own scripts.

| Decision | Externals |
| --- | --- |
| Built in | 119 |
| External | 0 |
| Stub | 38 |
| Unknown | 23 (12 of them HyperKRS) |

"Found in" counts the stacks that carry the external in their resource fork,
and how many of those stacks' scripts call it. Many stacks carry externals
they never call.

### Files and folders

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| FileExists | XFCN | 27 stacks, called in 26 | Whether a file exists. Two different versions exist: one answers 0 or not 0, the other true or false. | Built in | As `there is a file`, answering in whichever style the stack expects | Done |
| KrsFileExists | XFCN | 1 stack, called in 1 | Whether a file exists (from HyperKRS). | Built in | Same as FileExists | Planned |
| FileName | XFCN | 26 stacks, called in 24 | Shows a file dialog and returns the chosen file's path. | Built in | As `answer file` | Done |
| KrsGetFile | XCMD | 2 stacks, called in 1 | Shows a file dialog for one file type (HyperKRS). | Built in | As `answer file … of type …` | Planned |
| FolderName | XFCN | 17 stacks, called in 17 | Shows a dialog to choose a folder and returns its path. | Built in | As a folder dialog (`answer folder`) | Done |
| folderPath | XFCN | 2 stacks, called in 2 | Shows a dialog to choose a folder (by David Fry). | Built in | As a folder dialog | Planned |
| SFPutFile | XFCN | 25 stacks, called in 11 | Shows a Save dialog and returns the chosen path. | Built in | As `ask file` | Done |
| DeleteFile | XFCN | 4 stacks, called in 4 | Deletes a file. | Built in | As a new `delete file` command | Planned |
| RemoveFile | XCMD | 2 stacks, called in 2 | Deletes a file. | Built in | As `delete file` | Planned |
| FileCopy | XCMD | 2 stacks, called in 2 | Copies a file to a folder (by Frédéric Rinaldi). | Built in | As a new `copy file` command | Planned |
| MoveFile | XFCN | 3 stacks, called in 2 | Moves a file to a folder (by Dewey Williams). | Built in | As a new `move file` command | Planned |
| FileSize | XFCN | 2 stacks, called in 2 | A file's size. | Built in | As a file property | Planned |
| FileType | XFCN | 3 stacks, called in 3 | A file's Mac type and creator codes. | Built in | From the file name's extension, since modern files have no type codes | Planned |
| GetFInfo | XFCN | 2 stacks, called in 2 | A file's Finder information: type, creator, dates (by Frédéric Rinaldi). | Built in | As file properties; type and creator from the extension | Planned |
| FolderContents | XFCN | 2 stacks, called in 2 | Lists the files in a folder. | Built in | As a list of a folder's files | Planned |
| TruncFile | XFCN | 2 stacks, called in 2 | The file name at the end of a path. | Built in | As a function on paths | Planned |
| defaultFolder | XCMD | 2 stacks, called in 2 | Sets the folder file dialogs open in. | Built in | As a property for file dialogs | Planned |
| DiskSpaceAvail | XFCN | 3 stacks, called in 3 | Free space on a disk. | Built in | As `the diskSpace` of a volume | Planned |
| VolumeList | XFCN | 2 stacks, called in 2 | Lists the mounted disks. | Built in | As a list of mounted volumes | Planned |
| SystemFolder | XFCN | 2 stacks, called in 2 | The path of the System Folder. | Built in | As the folder for application data on each system | Planned |
| SysPrefs | XFCN | 22 stacks, called in 22 | Finds or creates a file in the Preferences folder, and returns its path. | Built in | With RandomCard's settings folder (`~/.config/randomcard`) | Done |
| CompressFiles | XCMD | 2 stacks, called in 2 | Compresses files into an AppleLink package. | Built in | As commands for zip archives; AppleLink's own format isn't documented | Planned |
| UncompressFiles | XCMD | 2 stacks, called in 2 | Expands an AppleLink package. | Built in | As CompressFiles | Planned |
| CompressedFileInfo | XFCN | 2 stacks, called in 2 | Describes an AppleLink package. | Built in | As CompressFiles | Planned |
| addStringResource | XCMD | 2 stacks, called in 2 | Writes a string into a file's resource fork. | Stub | Resource forks don't exist on modern systems | Planned |
| CopyFile | XFCN | 1 stack, called in 1 | Copies a file. | Built in | As `copy file` | Planned |
| DeleteFile2 | XCMD | 1 stack, called in 1 | Deletes a file (another version). | Built in | As `delete file` | Planned |
| RenameFile | XFCN | 1 stack, called in 1 | Renames a file. | Built in | As a new `rename file` command | Planned |
| FileLength | XFCN | 1 stack, called in 1 | A file's size. | Built in | As a file property | Planned |
| FileModDate | XFCN | 1 stack, called in 1 | When a file was last changed. | Built in | As a file property | Planned |
| FileCreator | XFCN | 1 stack, called in 1 | A file's Mac creator code. | Built in | From the file name's extension, as FileType | Planned |
| ChangeFileType | XFCN | 1 stack, called in 0 | Changes a file's Mac type and creator codes. | Stub | Modern files have no type codes | Planned |
| SetFile | XFCN | 1 stack, called in 0 | Changes a file's Finder information. | Stub | As ChangeFileType | Planned |
| FileVisFlag | XFCN | 1 stack, called in 1 | Whether a file is hidden in the Finder. | Stub | Answers visible | Planned |
| Files | XFCN | 1 stack, called in 1 | Lists the files of one type in a folder. | Built in | As FolderContents, by extension | Planned |
| GetVolume | XFCN | 1 stack, called in 0 | The current folder. | Built in | As defaultFolder | Planned |
| SetVolume | XCMD | 1 stack, called in 0 | Sets the current folder. | Built in | As defaultFolder | Planned |
| Pathitems | XFCN | 1 stack, called in 0 | Splits a path into its parts. | Built in | As a function on paths | Planned |
| Lastpathitem | XFCN | 1 stack, called in 0 | The last part of a path. | Built in | As TruncFile | Planned |
| NewFileName | XFCN | 1 stack, called in 1 | A Save dialog with a prompt and default name. | Built in | As `ask file` | Done |
| StdFile | XCMD | 1 stack, called in 0 | The standard Open and Save dialogs. | Built in | As `answer file` and `ask file` | Planned |
| ImportPict | XCMD | 1 stack, called in 1 | Imports a picture file onto cards. | Built in | As importing images (see Color in the roadmap) | Planned |
| import | XCMD | 1 stack, called in 1 | Imports text or pictures from a file. | Unknown | Its exact behavior isn't clear from the stack | Research |
| CombineFiles | XCMD | 1 stack, called in 0 | Combines several files into one. | Built in | As a command that joins files | Planned |
| ReadCat | XFCN | 1 stack, called in 0 | Reads a disk catalog (not called). | Unknown | Needs research | Research |
| deprotect | XCMD | 1 stack, called in 1 | Removes a stack's protection. | Unknown | Needs research into what it changed | Research |

### Dialogs, menus and windows

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| Dialog | XFCN | 9 stacks, called in 9 | An alert with chosen buttons and font: `dialog(msg, "OK", 1, Chicago, Plain, 12)`. | Built in | As `answer` | Done |
| ListSelect | XFCN | Rinaldi collection (9 demo stacks); Qstack, called in 2 | A dialog for choosing one line, a run of lines or any lines from a list, by Frédéric Rinaldi. | Built in | src/runtime/rinaldi.rs, with a list dialog (src/ui/list_dialog.rs); a stack's own DLOG isn't used | Done |
| XScrollBox | XFCN | 3 stacks, called in 3 | A dialog with a scrolling list to choose from (Dartmouth, 1989). | Built in | As the list-choosing dialog | Planned |
| PopUpMenu | XFCN | 3 stacks, called in 3 | A popup menu at a point, returning the chosen item's number. | Built in | As a popup menu function, using popup buttons' menus | Done |
| MakeSendMe5 | XCMD | Q & A stacks | Sends an AppleLink message (to, cc, subject, text); MakeSendMe4 is the older version. | Built in | A new message in the user's mail program | Done |
| GetVol | XFCN | 3 Developer CD stacks | The mounted volumes, one per line. | Built in | The computer's mounted volumes, as paths | Done |
| ProgressWindoid | XFCN | 3 Developer CD stacks | A floating window with a label over a progress bar (by Chris Thorman, Apple, 1990), managed through HyperCard 2's window commands and properties. | Built in | As a progress window, with `show`/`hide`/`close window` and `set … of window` | Done |
| FileCopy, EraseFile, FullMove, FullRename | XCMD | Rinaldi collection | Copy, delete, move and rename files and folders. | Built in | src/runtime/files.rs, by the same names | Done |
| GetDir, GetFInfo, FullSFPut, GetBootVol, GetSysFolder | XFCN | Rinaldi collection | List a folder (with filters and layouts), a file's details, a Save dialog, the startup disk, the System Folder. | Built in | src/runtime/files.rs; Mac type and creator answer "????" | Done |
| LineCount, FullSort, DateSort, GetPassword, GetMode | XFCN | Rinaldi collection | A field's lines as wrapped; sorting lines or items any way (with FullSortList for co-sorting); sorting lines by date; a hidden password, as `ask password`'s number; the screen's mode and size. | Built in | src/runtime/rinaldi.rs | Done |
| Switch, CapsLock, ClipInfo, ATalkZones, GlobalList, WindName | XFCN, XCMD | Rinaldi collection | Items to lines and back; whether Caps Lock is on; the clipboard's types; AppleTalk zones; the globals; the window's title. | Built in | src/runtime/rinaldi.rs (no AppleTalk: ATalkZones answers its error) | Done |
| Mousoid, Infoid, Promptoid, Notification | XCMD | Rinaldi collection | Windoids showing the mouse's position, or what's under it; Prompt as a HyperCard 2 window; waiting for the user when HyperCard is in the background. | Built in | src/runtime/rinaldi.rs; the UI keeps the windoids current (src/ui/windows.rs). Promptoid's icon isn't shown | Done |
| SendPS | XCMD | Rinaldi collection (9 demo stacks) | Sent PostScript, text or a file, to a LaserWriter over AppleTalk. | Built in | src/runtime/rinaldi.rs: opens the PostScript in the system's viewer to print from there, as printing does with its PDFs | Done |
| GetDocs, GetCreator | XCMD, XFCN | HyperCard 1.2's Documents stack (11 copies, several languages) | A card for each file in Home's document folders; the program for a creator code. | Built in | src/runtime/home_docs.rs: today's files are ???? unless a stack (STAK, WILD) or a program (APPL); GetCreator names HyperCard for WILD and the system's choice for the rest | Done |
| ClipToScrap, ScrapToClip | XCMD | Rinaldi collection (ScrapXCMDs, 9 stacks) | Added the clipboard to the Mac's Scrapbook file, and took a page back. | Won't do: no system today has a Scrapbook, and its file lived in a resource fork | — | Won't do |
| LeafName, FileN, FolderPath, SelectDir | XFCN | Rinaldi demos | The last part of a path; an Open dialog; folder dialogs. | Built in | | Done |
| FileToRes, ResToFile (FileMaker) | XCMD | Rinaldi collection | Keep a file in a stack's resource fork, and make it again. | Built in, later | Needs somewhere in a stack to keep files | Planned |
| PopMenu | XFCN | 5 Developer CD stacks | Appropriate Technology's popup menu: (top, left, checked, items), answering "true,item,menu". Another PopMenu takes (point, lines) and answers a line number. | Built in | With PopUpMenu's menu; hierarchical menus are shown flat | Done |
| FilePath | XFCN | Rinaldi collection | An Open dialog, answering the file's path. | Built in | As FileName | Done |
| MacType | XFCN | Rinaldi collection | The computer, system version, processor, FPU, color and keyboard. | Built in | src/runtime/rinaldi.rs; the computer is the system's name | Done |
| PopUpMenu1 | XFCN | 1 stack, called in 1 | A popup menu (another version). | Built in | As PopUpMenu | Planned |
| HPopUpMenu | XFCN | 2 stacks, called in 2 | A popup menu (by Guy de Picciotto). | Built in | As PopUpMenu | Planned |
| BigPop | XFCN | 20 stacks, called in 7 | A popup menu for long lists: `BigPop <menu list>[,<top>,<left>]`. | Built in | As PopUpMenu | Planned |
| fixDumbPop | XFCN | 1 stack, called in 1 | A popup menu (a variant). | Built in | As PopUpMenu | Planned |
| KRSPopUp | XFCN | 2 stacks, called in 1 | A popup menu (HyperKRS). | Built in | As PopUpMenu | Planned |
| KRSNewMenu | XFCN | 1 stack, called in 1 | Adds a menu to the menu bar (HyperKRS). | Built in | As `create menu`, which RandomCard has | Planned |
| KrsDeleteMenu | XFCN | 1 stack, called in 1 | Removes a menu (HyperKRS). | Built in | As `delete menu` | Planned |
| KrsEnableMenu | XCMD | 1 stack, called in 1 | Enables or disables a menu item (HyperKRS). | Built in | As `enable`/`disable menuItem` | Planned |
| KrsCheckMenu | XCMD | 1 stack, called in 0 | Checks a menu item (HyperKRS). | Built in | As `the checkMark` | Planned |
| KrsChangeMenu | XCMD | 1 stack, called in 0 | Renames a menu item (HyperKRS). | Built in | As `set the name of menuItem` | Planned |
| KrsShowMenu | XCMD | 1 stack, called in 1 | Shows a menu (HyperKRS). | Built in | As the menus RandomCard draws | Planned |
| Prompt | XCMD | 2 stacks, called in 2 | Shows a "please wait" message (`prompt 1, "…"`) and hides it (`prompt 3`) (by Frédéric Rinaldi). | Built in | src/runtime/rinaldi.rs; a locked text window named "Prompt" | Done |
| TEXWindow | XCMD | 2 stacks, called in 2 | Shows text in a window of its own. | Built in | As a text window (with roadmap item 8, windows) | Done |
| TNPict | XCMD | 9 stacks, called in 9 | Shows a picture file in a window (Technical Notes stacks). | Built in | As HyperCard 2's `picture` command | Part done: no PICT pictures yet |
| SelectLineXCMD | XCMD | 8 stacks, called in 8 | Selects a line in a field. | Built in | As `select line … of field`, which RandomCard has | Done |
| SpinAbout | XCMD | 8 stacks, called in 8 | An animated About box (by Andrew Shebanow, Apple, 1989). | Stub | A bit of fun from 1989; it can do nothing | Done |
| ZoomRect | XCMD | 3 stacks (Prototyping Interactive Video, Prototyping Toolkit 2.0), called in 3 | Draws the Finder's zoom: a trail of rectangles from one rect to another. | Built in | As `zoomRect rect1, rect2`, drawn over the card | Done |
| Alert | XCMD | 1 stack, called in 1 | An alert with chosen icon, buttons and sound. | Built in | As `answer` | Planned |
| DialogList | XFCN | 1 stack, called in 1 | A dialog for choosing from a list. | Built in | As the list-choosing dialog | Planned |
| DoList | XCMD | 1 stack, called in 1 | A dialog for choosing from a list, with chosen buttons. | Built in | As the list-choosing dialog | Planned |
| Popup | XFCN | 1 stack, called in 1 | A popup menu over a rectangle. | Built in | As PopUpMenu | Planned |
| NewMenu | XFCN | 1 stack, called in 1 | Adds a menu to the menu bar. | Built in | As `create menu` | Planned |
| DeleteMenu | XFCN | 1 stack, called in 1 | Removes a menu. | Built in | As `delete menu` | Planned |
| EnableMenu | XCMD | 1 stack, called in 1 | Enables or disables a menu item. | Built in | As `enable`/`disable menuItem` | Planned |
| CheckMenu | XCMD | 1 stack, called in 1 | Checks a menu item. | Built in | As `the checkMark` | Planned |
| ChangeMenu | XCMD | 1 stack, called in 0 | Changes a menu item. | Built in | As setting a menu item's name | Planned |
| ShowMenu | XCMD | 1 stack, called in 1 | Shows a menu. | Built in | As the menus RandomCard draws | Planned |
| MenuBar | XFCN | 1 stack, called in 1 | Whether the menu bar is showing. | Built in | As `the visible of menuBar` | Planned |
| Progress | XCMD | 1 stack, called in 1 | Shows a progress message, like Prompt. | Built in | As a status message | Planned |
| DragOn | XCMD | 1 stack, called in 1 | Lets the user drag a button or field around. | Built in | As dragging parts from a script | Planned |
| DispPICT | XCMD | 1 stack, called in 0 | Shows a picture (not called). | Built in | As HyperCard 2's `picture` command | Planned |
| DrawPict | XCMD | 1 stack, called in 1 | Draws a picture into a rectangle. | Built in | As the `picture` command | Planned |
| TitleBar | XCMD | 1 stack, called in 1 | Shows or hides the card window's title bar. | Stub | The window keeps its system title bar | Planned |
| Color | XCMD | 1 stack, called in 0 | Colors text or the screen with named colors (not called). | Built in | Part of full color (see Color in the roadmap) | Planned |
| MungeMCTB | XCMD | 1 stack, called in 1 | Changes the menu bar's colors. | Stub | Menus use the system's colors | Planned |
| AboutMe | XFCN | 1 stack, called in 0 | An About box (not called). | Stub | Does nothing | Planned |
| BarButton | XCMD | 1 stack, called in 1 | Not clear from the stack. | Unknown | Needs research | Research |
| ThumbBtn | XFCN | 1 stack, called in 0 | A scroll thumb made from a button (not called). | Unknown | Needs research | Research |

### Printing

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| PrintOut | XCMD | 9 stacks, called in 9 | Prints text in a chosen font, size and style. | Built in | As `print` with text settings | Done |
| PrintDoc | XCMD | 3 stacks, called in 3 | Prints a document, with a prompt. | Built in | As `print` | Planned |
| PrintClip | XCMD | 1 stack, called in 0 | Prints the clipboard (not called). | Stub | Copy into a field and `print` instead | Planned |
| ResetPrinter | XCMD | 1 stack, called in 0 | Resets the printer (not called). | Stub | Means nothing with PDF printing | Planned |

### Text and values

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| StringWidth | XFCN | 8 stacks, called in 8 | The width of text in pixels. | Built in | From RandomCard's HyperCard text metrics | Done |
| isnum | XFCN | 26 stacks, called in 13 | Whether a value is a number. | Built in | As `is a number`, except that empty counts (so a script asking until it gets a number stops on Cancel) | Done |
| Main | XFCN | 9 stacks, called in 9 | ROT13: `main("Ner jr ybfg va gur jrrqf?")` is "Are we lost in the weeds?", an easter egg. | Built in | As a small text function, so the Technical Notes stacks run | Done |
| String2Index | XFCN | 2 stacks, called in 2 | Turns a cross-reference into an index (SpInside Macintosh). | Unknown | Specific to SpInside Macintosh; needs reading its scripts | Research |
| eval | XFCN | 17 stacks, called in 4 | Not clear: no calls found that pass arguments. | Unknown | Possibly like `value()`; needs research | Research |
| trans | XFCN | 17 stacks, called in 4 | Not clear: no calls found that pass arguments. | Unknown | Needs research | Research |
| newTexFcn | XFCN | 22 stacks, called in 0 | Not called by any script; probably a helper for TEXWindow. | Unknown | Needs research | Research |
| ChangeCase | XFCN | 1 stack, called in 1 | Changes text to upper, lower or title case. | Built in | As text functions | Planned |
| upper | XFCN | 1 stack, called in 1 | Text in upper case. | Built in | As a text function | Planned |
| lower | XFCN | 1 stack, called in 1 | Text in lower case. | Built in | As a text function | Planned |
| initialCaps | XFCN | 1 stack, called in 0 | Text with each word capitalized (not called). | Built in | As a text function | Planned |
| FindString | XFCN | 1 stack, called in 1 | Finds an item in a list. | Built in | As `itemOffset`-style functions | Planned |
| LastOffset | XFCN | 1 stack, called in 0 | The last place one text occurs in another (not called). | Built in | As a text function | Planned |
| FormatNum | XFCN | 1 stack, called in 0 | Formats a number (not called). | Built in | As number formatting | Planned |
| FormatNumPadded | XFCN | 1 stack, called in 0 | Formats a number to a width (not called). | Built in | As number formatting | Planned |
| FormatPhoneNum | XFCN | 1 stack, called in 0 | Formats a phone number (not called). | Built in | As number formatting with a pattern | Planned |
| zipCheck | XFCN | 1 stack, called in 0 | Checks a US ZIP code against its state (not called). | Built in | Low priority: needs a table of ZIP codes | Planned |
| Strip | XFCN | 1 stack, called in 1 | Removes characters from text. | Built in | As a text function | Planned |
| StripNum | XFCN | 1 stack, called in 0 | Removes digits from text (not called). | Built in | As a text function | Planned |
| SortItems | XFCN | 1 stack, called in 1 | Sorts the items of a list. | Built in | As `sort items` | Planned |
| SortReals | XFCN | 1 stack, called in 0 | Sorts numbers (not called). | Built in | As `sort … numeric` | Planned |
| SortRealsII | XFCN | 1 stack, called in 0 | Sorts numbers (another version, not called). | Built in | As `sort … numeric` | Planned |
| sumInt | XFCN | 1 stack, called in 0 | Adds up numbers (not called). | Built in | As `sum()` | Planned |
| CStoHCDateConversion | XFCN | 1 stack, called in 0 | Converts dates between formats (not called). | Built in | As `convert` | Planned |
| FontName | XFCN | 1 stack, called in 0 | Lists fonts (not called). | Built in | As the fonts RandomCard has | Planned |
| FontSize | XFCN | 1 stack, called in 0 | Lists a font's sizes (not called). | Built in | As the sizes RandomCard offers | Planned |
| InKey | XFCN | 1 stack, called in 0 | Which key is down (not called). | Built in | As the key functions | Planned |
| Interpolate | XFCN | 1 stack, called in 0 | Not clear (not called). | Unknown | Needs research | Research |
| Stripper | XCMD | 1 stack, called in 0 | Not clear (not called). | Unknown | Needs research | Research |

### System information

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| SysEnv | XFCN | 28 stacks, called in 15 | The Mac's system environment: machine, system version and so on. | Stub | Answers machine, system version and processor in the format stacks read | Done |
| sysEnvironment | XFCN | 4 stacks, called in 3 | The system environment (another version). | Stub | As SysEnv | Done |
| TotalRAM | XFCN | 4 stacks, called in 4 | Total memory. | Stub | Answers the real amount | Done |
| MaxApplZone | XFCN | 2 stacks, called in 1 | Grows HyperCard's memory zone. | Stub | Means nothing today | Planned |
| BoardInfo | XFCN | 2 stacks, called in 2 | The NuBus cards installed. | Stub | Answers none | Done |
| SCSIInfo | XFCN | 3 stacks, called in 2 | The SCSI devices attached. | Stub | Answers none | Planned |
| INITFinder | XFCN | 3 stacks, called in 3 | The system extensions (INITs) installed. | Stub | Answers none | Done |
| SwitchXRoo | XCMD | 3 stacks, called in 3 | Changes the monitor's color depth (by Bill Steinberg, 1990). | Stub | Monitors are always in full color now | Done |
| monitorCount | XFCN | 1 stack, called in 1 | How many monitors are connected. | Built in | As the screens RandomCard can see | Planned |
| monitorConfig | XFCN | 1 stack, called in 1 | A monitor's settings. | Built in | As each screen's size and depth | Planned |
| ApplMemory | XFCN | 1 stack, called in 1 | Free memory for HyperCard. | Stub | Answers a generous amount | Planned |
| ApplSize | XFCN | 1 stack, called in 1 | HyperCard's memory size. | Stub | As ApplMemory | Planned |
| ApplicationList | XFCN | 1 stack, called in 1 | The applications running. | Stub | Answers RandomCard alone | Planned |
| MFIsRunning | XFCN | 1 stack, called in 1 | Whether MultiFinder is running. | Stub | Answers true: every modern system multitasks | Planned |
| MultiFinder | XFCN | 1 stack, called in 1 | Whether MultiFinder is running. | Stub | As MFIsRunning | Planned |
| IsRunning | XFCN | 1 stack, called in 1 | Whether an application is running. | Stub | Answers false | Planned |
| numberOfDAs | XFCN | 1 stack, called in 1 | How many desk accessories are installed. | Stub | Answers 0 | Planned |
| GetDANames | XCMD | 1 stack, called in 0 | The desk accessories installed (not called). | Stub | Answers none | Planned |
| ScreenSize | XFCN | 1 stack, called in 1 | The screen's size. | Built in | As `the screenRect` | Planned |
| thePixel | XFCN | 1 stack, called in 1 | Whether the pixel at a point is black (by Steve Maller, 1988). | Built in | From the card's picture | Planned |
| Ejector | XCMD | 1 stack, called in 1 | Ejects a floppy disk. | Stub | Does nothing | Planned |
| ShutDown | XCMD | 1 stack, called in 0 | Shuts the Mac down (not called). | Stub | Does nothing | Planned |
| doRestart | XCMD | 1 stack, called in 0 | Restarts the Mac (not called). | Stub | Does nothing | Planned |
| OSErr | XCMD | 1 stack, called in 1 | Shows the message for a Mac OS error code. | Stub | Shows the code | Planned |

### AppleLink (Apple's online service, closed in 1997)

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| MakeSendMe4 | XCMD | 17 stacks, called in 4 | Makes an AppleLink message file. | Stub | AppleLink is gone; email or the web could replace it | Planned |
| MakeSendMe5 | XCMD | 19 stacks, called in 6 | Makes an AppleLink message file (by Steve Maller). | Stub | As MakeSendMe4 | Planned |
| readLinkAddresses | XFCN | 2 stacks, called in 2 | Reads the AppleLink address book. | Stub | As MakeSendMe4 | Planned |
| writeLinkAddresses | XCMD | 2 stacks, called in 2 | Writes the AppleLink address book. | Stub | As MakeSendMe4 | Planned |

### HyperKRS (full-text search with its own index files)

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| KRSWordSearch | XFCN | 2 stacks, called in 2 | Searches a HyperKRS index for words. | Unknown | The index file format isn't documented; RandomCard's `find` may serve instead | Research |
| KRSBuildQuery | XFCN | 1 stack, called in 1 | Builds a HyperKRS search query. | Unknown | As KRSWordSearch | Research |
| KRSqueryCheck | XCMD | 1 stack, called in 1 | Checks a query's syntax. | Unknown | As KRSWordSearch | Research |
| KRSCountChars | XFCN | 2 stacks, called in 2 | Counts characters for the index. | Unknown | As KRSWordSearch | Research |
| KRSFindDesc | XFCN | 1 stack, called in 1 | Finds a search result's description. | Unknown | As KRSWordSearch | Research |
| KrsFirstDesc | XFCN | 1 stack, called in 1 | The first result's description. | Unknown | As KRSWordSearch | Research |
| KrsGetCardList | XFCN | 1 stack, called in 1 | The cards a search found. | Unknown | As KRSWordSearch | Research |
| KRSGetIDs | XFCN | 1 stack, called in 1 | Card IDs from the index. | Unknown | As KRSWordSearch | Research |
| KrsGetTitles | XFCN | 2 stacks, called in 2 | Titles of found cards (two versions, KrsGetTitles and KRSGetTitles). | Unknown | As KRSWordSearch | Research |
| KrsNumHits | XFCN | 1 stack, called in 1 | How many matches a search found. | Unknown | As KRSWordSearch | Research |
| KrsGetInfo | XCMD | 2 stacks, called in 2 | Reads settings of an index (two versions). | Unknown | As KRSWordSearch | Research |
| KrsSetInfo | XCMD | 2 stacks, called in 2 | Changes settings of an index (two versions). | Unknown | As KRSWordSearch | Research |

### Applications and resources

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| SubLaunch | XFCN | 1 stack, called in 1 | Opens an application, or a document with an application. | Built in | As `open` | Planned |
| GetAppIcon | XFCN | 1 stack, called in 1 | Copies an application's icon into the stack. | Stub | Modern icons don't fit HyperCard's 32-pixel icons | Planned |
| GetDocIcon | XFCN | 1 stack, called in 1 | Copies a document's icon into the stack. | Stub | As GetAppIcon | Planned |
| KillResID | XCMD | 1 stack, called in 1 | Deletes an icon or other resource from a stack. | Built in | As removing a stack's icons and sounds | Planned |
| Rescopy | XCMD | 1 stack, called in 1 | Copies resources between stacks. | Built in | As copying a stack's icons and sounds | Planned |
| Resources | XFCN | 1 stack, called in 0 | Lists a file's resources (not called). | Built in | As listing a stack's icons and sounds | Planned |
| UpdateResFork | XCMD | 1 stack, called in 1 | Saves changes to a resource fork. | Stub | RandomCard saves stacks itself | Planned |
| ClipToPICTRes | XCMD | 1 stack, called in 0 | Saves the clipboard as a picture resource (not called). | Stub | Resource forks don't exist on modern systems | Planned |

### Sound, speech and MIDI

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| speak | XCMD | 1 stack, called in 1 | Speaks text aloud. | Built in | As `speak`, which HyperTalk has | Planned |
| Talk | XCMD | 1 stack, called in 1 | Speaks text aloud, with pitch and speed. | Built in | As `speak` | Planned |
| HyperSND | XCMD | 1 stack, called in 0 | Plays sounds (not called). | Built in | As `play` | Planned |
| SoundCapToRes | XCMD | 1 stack, called in 1 | Records sound into the stack. | Built in | As recording sound (see Media in the roadmap's Future) | Planned |
| InitMidi | XCMD | 1 stack, called in 0 | Starts MIDI (not called). | Built in | As MIDI commands, through the system's MIDI support | Planned |
| ResetMidi | XCMD | 1 stack, called in 0 | Resets MIDI (not called). | Built in | As InitMidi | Planned |
| RxMidi | XFCN | 1 stack, called in 0 | Receives MIDI data (not called). | Built in | As InitMidi | Planned |
| TxMidi | XCMD | 1 stack, called in 0 | Sends MIDI data (not called). | Built in | As InitMidi | Planned |

### Serial ports

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| CommInit | XCMD | 1 stack, called in 0 | Opens a serial port (not called). | Built in | As serial port commands, opened and read like files | Planned |
| CommRead | XFCN | 1 stack, called in 0 | Reads from a serial port (not called). | Built in | As CommInit | Planned |
| CommWrite | XCMD | 1 stack, called in 0 | Writes to a serial port (not called). | Built in | As CommInit | Planned |
| SendSerial | XCMD | 1 stack, called in 0 | Sends text out of a serial port (not called). | Built in | As CommInit | Planned |

## Stacks scanned

| Stack | Externals | Notes |
| --- | --- | --- |
| Human Interface Notes Stacks/HIN Stack 1.4 | 14: Dialog, FileExists, FileName, isnum, Main, newTexFcn, PrintOut, SelectLineXCMD, SFPutFile, SpinAbout, StringWidth, SysEnv, SysPrefs, TNPict |  |
| Human Interface Notes Stacks/HIN Stack 2.0.3 | 14: Dialog, FileExists, FileName, isnum, Main, newTexFcn, PrintOut, SelectLineXCMD, SFPutFile, SpinAbout, StringWidth, SysEnv, SysPrefs, TNPict |  |
| Q & A Stacks/Q & A Stack 3.1 | 4: BigPop, eval, isnum, trans |  |
| Q & A Stacks/Q & A Stack 3.2.2 | 9: BigPop, eval, FileName, isnum, MakeSendMe4, MakeSendMe5, SFPutFile, SysEnv, trans |  |
| Q & A Stacks/Q & A Stack 3.4 | 12: BigPop, eval, FileExists, FileName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 3.4.1 | 12: BigPop, eval, FileExists, FileName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 4.0 | 23: BigPop, eval, FileExists, FileName, isnum, KRSBuildQuery, KRSCountChars, KRSFindDesc, KrsGetFile, KRSGetIDs, KRSGetInfo, KRSGetTitles, KRSPopUp, KRSSetInfo, KRSWordSearch, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans, TruncFile |  |
| Q & A Stacks/Q & A Stack 4.0.1 | 13: BigPop, eval, FileExists, FileName, FolderName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 4.0.2 | 13: BigPop, eval, FileExists, FileName, FolderName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 4.0.2 (Vol IX) | 13: BigPop, eval, FileExists, FileName, FolderName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 4.0.3 | 13: BigPop, eval, FileExists, FileName, FolderName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 4.0.4 | 13: BigPop, eval, FileExists, FileName, FolderName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 4.0.5 | 13: BigPop, eval, FileExists, FileName, FolderName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 4.0.6 | 13: BigPop, eval, FileExists, FileName, FolderName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 4.0.7 | 13: BigPop, eval, FileExists, FileName, FolderName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 4.0.8 | 13: BigPop, eval, FileExists, FileName, FolderName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 4.0.9 | 13: BigPop, eval, FileExists, FileName, FolderName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 4.1 | 13: BigPop, eval, FileExists, FileName, FolderName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Q & A Stacks/Q & A Stack 4.1.1 | 13: BigPop, eval, FileExists, FileName, FolderName, isnum, MakeSendMe4, MakeSendMe5, newTexFcn, SFPutFile, SysEnv, SysPrefs, trans |  |
| Qstack/Qstack 2.0 | 33: addStringResource, BoardInfo, CompressedFileInfo, CompressFiles, defaultFolder, DeleteFile, DiskSpaceAvail, FileCopy, FileExists, FileSize, FileType, FolderContents, folderPath, GetFInfo, INITFinder, ListSelect, MakeSendMe5, MoveFile, PopUpMenu, PrintDoc, Prompt, readLinkAddresses, RemoveFile, SCSIInfo, SwitchXRoo, SysEnv, sysEnvironment, SystemFolder, TEXWindow, TotalRAM, UncompressFiles, VolumeList, writeLinkAddresses |  |
| Qstack/Qstack 2.1 | 35: addStringResource, BoardInfo, CompressedFileInfo, CompressFiles, defaultFolder, DeleteFile, DiskSpaceAvail, FileCopy, FileExists, FileSize, FileType, FolderContents, folderPath, GetFInfo, HPopUpMenu, INITFinder, ListSelect, MakeSendMe5, MoveFile, PopUpMenu, PopUpMenu1, PrintDoc, Prompt, readLinkAddresses, RemoveFile, SCSIInfo, SwitchXRoo, SysEnv, sysEnvironment, SystemFolder, TEXWindow, TotalRAM, UncompressFiles, VolumeList, writeLinkAddresses |  |
| Qstack/Qstack Ver. 1.0 | 12: fixDumbPop, INITFinder, MakeSendMe4, MakeSendMe5, monitorConfig, monitorCount, PrintDoc, SCSIInfo, SFPutFile, SysEnv, sysEnvironment, TotalRAM |  |
| SpInside Macintosh Stacks/SpInside Macintosh 1.2 | 35: BigPop, Dialog, FileExists, FileName, isnum, KrsChangeMenu, KrsCheckMenu, KRSCountChars, KrsDeleteMenu, KrsEnableMenu, KrsFileExists, KrsFirstDesc, KrsGetCardList, KrsGetFile, KrsGetInfo, KrsGetTitles, KRSNewMenu, KrsNumHits, KRSPopUp, KRSqueryCheck, KrsSetInfo, KrsShowMenu, KRSWordSearch, Main, MaxApplZone, PrintOut, SelectLineXCMD, SFPutFile, SpinAbout, String2Index, StringWidth, SysEnv, TNPict, TruncFile, XScrollBox |  |
| SpInside Macintosh Stacks/SpInside Macintosh 2.0.3 | 19: BigPop, Dialog, FileExists, FileName, FolderName, isnum, Main, MaxApplZone, newTexFcn, PrintOut, SelectLineXCMD, SFPutFile, SpinAbout, String2Index, StringWidth, SysEnv, SysPrefs, TNPict, XScrollBox |  |
| Technical Notes Stacks/Technical Notes Stack 3.1 | 8: BigPop, Dialog, FileName, isnum, Main, PrintOut, SysEnv, TNPict |  |
| Technical Notes Stacks/Technical Notes Stack 3.4.2 | 15: Dialog, FileExists, FileName, FolderName, isnum, Main, newTexFcn, PrintOut, SelectLineXCMD, SFPutFile, SpinAbout, StringWidth, SysEnv, SysPrefs, TNPict |  |
| Technical Notes Stacks/Technical Notes Stack 4.0.1 | 15: Dialog, FileExists, FileName, FolderName, isnum, Main, newTexFcn, PrintOut, SelectLineXCMD, SFPutFile, SpinAbout, StringWidth, SysEnv, SysPrefs, TNPict |  |
| Technical Notes Stacks/Technical Notes Stack 4.1.4 | 15: Dialog, FileExists, FileName, FolderName, isnum, Main, newTexFcn, PrintOut, SelectLineXCMD, SFPutFile, SpinAbout, StringWidth, SysEnv, SysPrefs, TNPict |  |
| Unpacked_Archives/Dev Conf Stack | 11: ApplicationList, ApplMemory, ApplSize, DeleteFile, DiskSpaceAvail, FileExists, MFIsRunning, SwitchXRoo, sysEnvironment, TotalRAM, XScrollBox | Resource fork in AppleDouble form |
| Unpacked_Archives/Developer Stack 1.3r | 103: AboutMe, Alert, BarButton, ChangeCase, ChangeFileType, ChangeMenu, CheckMenu, ClipToPICTRes, Color, CombineFiles, CommInit, CommRead, CommWrite, CopyFile, CStoHCDateConversion, DeleteFile, DeleteFile2, DeleteMenu, deprotect, DialogList, DispPICT, DoList, doRestart, DragOn, DrawPict, Ejector, EnableMenu, FileCreator, FileExists, FileLength, FileModDate, FileName, Files, FileType, FileVisFlag, FindString, FontName, FontSize, FormatNum, FormatNumPadded, FormatPhoneNum, GetAppIcon, GetDANames, GetDocIcon, GetVolume, HPopUpMenu, HyperSND, import, ImportPict, initialCaps, InitMidi, InKey, Interpolate, IsRunning, KillResID, LastOffset, Lastpathitem, lower, MenuBar, MoveFile, MultiFinder, MungeMCTB, NewFileName, NewMenu, numberOfDAs, OSErr, Pathitems, Popup, PopUpMenu, PrintClip, Progress, ReadCat, RenameFile, Rescopy, ResetMidi, ResetPrinter, Resources, RxMidi, ScreenSize, SendSerial, SetFile, SetVolume, ShowMenu, ShutDown, SortItems, SortReals, SortRealsII, SoundCapToRes, speak, StdFile, Strip, StripNum, Stripper, SubLaunch, sumInt, Talk, thePixel, ThumbBtn, TitleBar, TxMidi, UpdateResFork, upper, zipCheck | Apple's library of sample externals; resource fork in AppleDouble form |
| Unpacked_Archives/Technical Notes Stack 4.0.1.ƒ/Technical Notes Stack 4.0.1 | 15: Dialog, FileExists, FileName, FolderName, isnum, Main, newTexFcn, PrintOut, SelectLineXCMD, SFPutFile, SpinAbout, StringWidth, SysEnv, SysPrefs, TNPict | Resource fork in AppleDouble form |

## Other findings

- **The DTS stacks call each other.** Scripts call handlers that live in the
  other stacks of the set (`findChapter`, `goChapter`, `findQA`,
  `findTechNote`), so they work together as a group, not one at a time.
- **HyperTalk gaps.** The scan also found scripts RandomCard couldn't
  parse, unrelated to externals: `do` followed by a command
  (`do ShowCategory CardNum`), `there is a window "…"`, `answer … with "No",
  "Yes"` with commas, `doMenu` with an unquoted item (`domenu copy button`),
  variables named `times` or `to`, and `size of this stack`. RandomCard
  handles them all now; see ROADMAP.md under HyperTalk for what's left.
