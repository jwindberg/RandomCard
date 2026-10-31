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
  keeps working, so old stacks run unchanged. This is the first choice when
  the job is general and can be specified clearly.
- **External.** Stays an external, written in Rust against RandomCard's
  externals framework (roadmap item 4), because it's specialized, tied to
  one platform, or not worth growing the language for.
- **Stub.** Obsolete on modern systems (memory checks, Mac system details,
  32-bit QuickDraw tests). It answers harmlessly, so stacks that ask keep
  running.
- **Unknown.** We can't document or specify it yet: no documentation, and
  its behavior isn't clear from the stacks that use it. Needs research
  before a decision.

The **Status** column says whether it's planned or done.

## Externals found

First scan, 2026-10-31: the 31 Apple developer stacks in `examples/apple`
(Technical Notes, Q & A, Qstack, SpInside Macintosh, Human Interface Notes,
the Developer Stack and the 1990 Developer Conference stack). They hold 80
distinct externals:

| Decision | Externals |
| --- | --- |
| Built in | 46 |
| External | 4 |
| Stub | 14 |
| Unknown | 16 (12 of them HyperKRS) |

"Found in" counts the stacks that carry the external in their resource fork,
and how many of those stacks' scripts call it. Many stacks carry externals
they never call.

### Files and folders

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| FileExists | XFCN | 24 stacks, called in 24 | Whether a file exists. Two different versions exist: one answers 0 or not 0, the other true or false. | Built in | As `there is a file`, answering in whichever style the stack expects | Planned |
| KrsFileExists | XFCN | 1 stack, called in 1 | Whether a file exists (from HyperKRS). | Built in | Same as FileExists | Planned |
| FileName | XFCN | 24 stacks, called in 22 | Shows a file dialog and returns the chosen file's path. | Built in | As `answer file` | Planned |
| KrsGetFile | XCMD | 2 stacks, called in 1 | Shows a file dialog for one file type (HyperKRS). | Built in | As `answer file … of type …` | Planned |
| FolderName | XFCN | 16 stacks, called in 16 | Shows a dialog to choose a folder and returns its path. | Built in | As a folder dialog (`answer folder`) | Planned |
| folderPath | XFCN | 2 stacks, called in 2 | Shows a dialog to choose a folder (by David Fry). | Built in | As a folder dialog | Planned |
| SFPutFile | XFCN | 24 stacks, called in 10 | Shows a Save dialog and returns the chosen path. | Built in | As `ask file` | Planned |
| DeleteFile | XFCN | 2 stacks, called in 2 | Deletes a file. | Built in | As a new `delete file` command | Planned |
| RemoveFile | XCMD | 2 stacks, called in 2 | Deletes a file. | Built in | As `delete file` | Planned |
| FileCopy | XCMD | 2 stacks, called in 2 | Copies a file to a folder (by Frédéric Rinaldi). | Built in | As a new `copy file` command | Planned |
| MoveFile | XFCN | 2 stacks, called in 2 | Moves a file to a folder (by Dewey Williams). | Built in | As a new `move file` command | Planned |
| FileSize | XFCN | 2 stacks, called in 2 | A file's size. | Built in | As a file property | Planned |
| FileType | XFCN | 2 stacks, called in 2 | A file's Mac type and creator codes. | Built in | From the file name's extension, since modern files have no type codes | Planned |
| GetFInfo | XFCN | 2 stacks, called in 2 | A file's Finder information: type, creator, dates (by Frédéric Rinaldi). | Built in | As file properties; type and creator from the extension | Planned |
| FolderContents | XFCN | 2 stacks, called in 2 | Lists the files in a folder. | Built in | As a list of a folder's files | Planned |
| TruncFile | XFCN | 2 stacks, called in 2 | The file name at the end of a path. | Built in | As a function on paths | Planned |
| defaultFolder | XCMD | 2 stacks, called in 2 | Sets the folder file dialogs open in. | Built in | As a property for file dialogs | Planned |
| DiskSpaceAvail | XFCN | 2 stacks, called in 2 | Free space on a disk. | Built in | As `the diskSpace` of a volume | Planned |
| VolumeList | XFCN | 2 stacks, called in 2 | Lists the mounted disks. | Built in | As a list of mounted volumes | Planned |
| SystemFolder | XFCN | 2 stacks, called in 2 | The path of the System Folder. | Built in | As the folder for application data on each system | Planned |
| SysPrefs | XFCN | 21 stacks, called in 21 | Finds or creates a file in the Preferences folder, and returns its path. | Built in | With the system's preferences folder (`~/.config` on Linux, Application Support on a Mac) | Planned |
| CompressFiles | XCMD | 2 stacks, called in 2 | Compresses files into an AppleLink package. | External | Its file format isn't documented; a Rust external could use zip instead | Planned |
| UncompressFiles | XCMD | 2 stacks, called in 2 | Expands an AppleLink package. | External | As CompressFiles | Planned |
| CompressedFileInfo | XFCN | 2 stacks, called in 2 | Describes an AppleLink package. | External | As CompressFiles | Planned |
| addStringResource | XCMD | 2 stacks, called in 2 | Writes a string into a file's resource fork. | Stub | Resource forks don't exist on modern systems | Planned |

### Dialogs, menus and windows

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| Dialog | XFCN | 8 stacks, called in 8 | An alert with chosen buttons and font: `dialog(msg, "OK", 1, Chicago, Plain, 12)`. | Built in | As `answer` | Planned |
| ListSelect | XFCN | 2 stacks, called in 2 | A dialog for choosing one line from a list (by Frédéric Rinaldi). | Built in | As a new list-choosing dialog | Planned |
| XScrollBox | XFCN | 2 stacks, called in 2 | A dialog with a scrolling list to choose from (Dartmouth, 1989). | Built in | As the list-choosing dialog | Planned |
| PopUpMenu | XFCN | 2 stacks, called in 2 | A popup menu at a point, returning the chosen item. | Built in | As a popup menu function, using popup buttons' menus | Planned |
| PopUpMenu1 | XFCN | 1 stack, called in 1 | A popup menu (another version). | Built in | As PopUpMenu | Planned |
| HPopUpMenu | XFCN | 1 stack, called in 1 | A popup menu (by Guy de Picciotto). | Built in | As PopUpMenu | Planned |
| BigPop | XFCN | 20 stacks, called in 7 | A popup menu for long lists: `BigPop <menu list>[,<top>,<left>]`. | Built in | As PopUpMenu | Planned |
| fixDumbPop | XFCN | 1 stack, called in 1 | A popup menu (a variant). | Built in | As PopUpMenu | Planned |
| KRSPopUp | XFCN | 2 stacks, called in 1 | A popup menu (HyperKRS). | Built in | As PopUpMenu | Planned |
| KRSNewMenu | XFCN | 1 stack, called in 1 | Adds a menu to the menu bar (HyperKRS). | Built in | As `create menu`, which RandomCard has | Planned |
| KrsDeleteMenu | XFCN | 1 stack, called in 1 | Removes a menu (HyperKRS). | Built in | As `delete menu` | Planned |
| KrsEnableMenu | XCMD | 1 stack, called in 1 | Enables or disables a menu item (HyperKRS). | Built in | As `enable`/`disable menuItem` | Planned |
| KrsCheckMenu | XCMD | 1 stack, called in 0 | Checks a menu item (HyperKRS). | Built in | As `the checkMark` | Planned |
| KrsChangeMenu | XCMD | 1 stack, called in 0 | Renames a menu item (HyperKRS). | Built in | As `set the name of menuItem` | Planned |
| KrsShowMenu | XCMD | 1 stack, called in 1 | Shows a menu (HyperKRS). | Built in | As the menus RandomCard draws | Planned |
| Prompt | XCMD | 2 stacks, called in 2 | Shows a "please wait" message (`prompt 1, "…"`) and hides it (`prompt 3`) (by Frédéric Rinaldi). | Built in | As a status message | Planned |
| TEXWindow | XCMD | 2 stacks, called in 2 | Shows text in a window of its own. | Built in | As a text window (with roadmap item 8, windows) | Planned |
| TNPict | XCMD | 8 stacks, called in 8 | Shows a picture file in a window (Technical Notes stacks). | Built in | As HyperCard 2's `picture` command | Planned |
| SelectLineXCMD | XCMD | 7 stacks, called in 7 | Selects a line in a field. | Built in | As `select line … of field`, which RandomCard has | Planned |
| SpinAbout | XCMD | 7 stacks, called in 7 | An animated About box (by Andrew Shebanow, Apple, 1989). | Stub | A bit of fun from 1989; it can do nothing | Planned |

### Printing

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| PrintOut | XCMD | 8 stacks, called in 8 | Prints text in a chosen font, size and style. | Built in | As `print` with text settings | Planned |
| PrintDoc | XCMD | 3 stacks, called in 3 | Prints a document, with a prompt. | Built in | As `print` | Planned |

### Text and values

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| StringWidth | XFCN | 7 stacks, called in 7 | The width of text in pixels. | Built in | From RandomCard's HyperCard text metrics | Planned |
| isnum | XFCN | 25 stacks, called in 12 | Whether a value is a number. | Built in | As `is a number` | Planned |
| Main | XFCN | 8 stacks, called in 8 | ROT13: `main("Ner jr ybfg va gur jrrqf?")` is "Are we lost in the weeds?", an easter egg. | External | A good first Rust external to ship as an example | Planned |
| String2Index | XFCN | 2 stacks, called in 2 | Turns a cross-reference into an index (SpInside Macintosh). | Unknown | Specific to SpInside Macintosh; needs reading its scripts | Research |
| eval | XFCN | 17 stacks, called in 4 | Not clear: no calls found that pass arguments. | Unknown | Possibly like `value()`; needs research | Research |
| trans | XFCN | 17 stacks, called in 4 | Not clear: no calls found that pass arguments. | Unknown | Needs research | Research |
| newTexFcn | XFCN | 21 stacks, called in 0 | Not called by any script; probably a helper for TEXWindow. | Unknown | Needs research | Research |

### System information

| Name | Kind | Found in | What it did | Decision | RandomCard | Status |
| --- | --- | --- | --- | --- | --- | --- |
| SysEnv | XFCN | 27 stacks, called in 14 | The Mac's system environment: machine, system version and so on. | Stub | Answers in its format with values describing the modern system, once the format is known | Planned |
| sysEnvironment | XFCN | 3 stacks, called in 2 | The system environment (another version). | Stub | As SysEnv | Planned |
| TotalRAM | XFCN | 3 stacks, called in 3 | Total memory. | Stub | Answers the real amount | Planned |
| MaxApplZone | XFCN | 2 stacks, called in 1 | Grows HyperCard's memory zone. | Stub | Means nothing today | Planned |
| BoardInfo | XFCN | 2 stacks, called in 2 | The NuBus cards installed. | Stub | Answers none | Planned |
| SCSIInfo | XFCN | 3 stacks, called in 2 | The SCSI devices attached. | Stub | Answers none | Planned |
| INITFinder | XFCN | 3 stacks, called in 3 | The system extensions (INITs) installed. | Stub | Answers none | Planned |
| SwitchXRoo | XCMD | 2 stacks, called in 2 | Changes the monitor's color depth (by Bill Steinberg, 1990). | Stub | Monitors are always in full color now | Planned |
| monitorCount | XFCN | 1 stack, called in 1 | How many monitors are connected. | Built in | As the screens RandomCard can see | Planned |
| monitorConfig | XFCN | 1 stack, called in 1 | A monitor's settings. | Built in | As each screen's size and depth | Planned |

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
| Unpacked_Archives/Dev Conf Stack | 0: none | unpack requires a buffer of 4 bytes |
| Unpacked_Archives/Developer Stack 1.3r | 0: none | unpack requires a buffer of 4 bytes |
| Unpacked_Archives/Technical Notes Stack 4.0.1.ƒ/Technical Notes Stack 4.0.1 | 0: none | unpack requires a buffer of 4 bytes |

## Other findings

- **The DTS stacks call each other.** Scripts call handlers that live in the
  other stacks of the set (`findChapter`, `goChapter`, `findQA`,
  `findTechNote`), so they work together as a group, not one at a time.
- **HyperTalk gaps.** The scan also found scripts RandomCard can't parse
  yet, unrelated to externals: `do` followed by a command
  (`do ShowCategory CardNum`), `there is a window "…"`, `answer … with "No",
  "Yes"` with commas, `doMenu` with an unquoted item (`domenu copy button`),
  a variable named `times` or `to`, and `size of this stack`. They're listed
  in ROADMAP.md under HyperTalk.
