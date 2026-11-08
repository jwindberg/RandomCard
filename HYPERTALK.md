# HyperTalk in RandomCard

This is the reference for HyperTalk as RandomCard runs it: the language of
HyperCard 2.4.1, the old externals (XCMDs and XFCNs) now built in, and what
RandomCard adds. Every example in a `hypertalk` block is parsed by
RandomCard's own parser in the tests, so they're all real.

- What HyperCard 2.4.1 had that RandomCard still lacks is in
  [COMPLIANCE.md](COMPLIANCE.md), item by item.
- Every external found in old stacks, and what became of it, is in
  [XCMDS.md](XCMDS.md).
- The Tour (the stack RandomCard opens with) has a working example of nearly
  everything here: choose the Button tool and double-click a button to read
  its script.

Apple's own manual, the *HyperCard Script Language Guide* (1996), is still
the fullest description of the language. Where this page and the Guide
differ, the "Differences from HyperCard" section says why.

## Contents

1. [Scripts and handlers](#scripts-and-handlers)
2. [Values](#values)
3. [Expressions and operators](#expressions-and-operators)
4. [Chunks](#chunks)
5. [Objects](#objects)
6. [Commands](#commands)
7. [Functions](#functions)
8. [Properties](#properties)
9. [System messages](#system-messages)
10. [Old externals, built in](#old-externals-built-in)
11. [RandomCard's additions](#randomcards-additions)
12. [Differences from HyperCard](#differences-from-hypercard)

## Scripts and handlers

Every object (stack, background, card, button, field) has a script. A
script holds **handlers**: a message handler runs when its object gets that
message, and a function handler answers a call.

```hypertalk
on mouseUp
  -- A comment runs to the end of the line.
  put "Hello" into field "Greeting"
end mouseUp

function double n
  return n * 2
end double

on greet who, greeting
  if greeting is empty then put "Hello" into greeting
  answer greeting & "," && who & "!"
end greet
```

A long line continues onto the next after `¬` (Option-Return on a Mac
keyboard; type `¬` or paste it elsewhere):

```hypertalk
on mouseUp
  answer "This line is long, so it" && ¬
    "goes on to the next."
end mouseUp
```

### The message path

A message goes first to its target (the button clicked, say), then to the
card, the background, the stack, any stacks in use (`start using`), the
Home stack, and finally RandomCard itself, which carries out built-in
commands. A handler stops the message unless it says `pass`:

```hypertalk
on openCard
  put the short name of this card into field "Title"
  pass openCard
end openCard
```

Built-in commands go through the path too, so a script can intercept them:

```hypertalk
on doMenu what
  if what is "Delete Card" then
    answer "Cards in this stack can't be deleted."
  else
    pass doMenu
  end if
end doMenu
```

### Control structures

```hypertalk
on mouseUp
  if the hilite of button "Sound" then beep

  if x > 10 then
    put "big" into size
  else if x > 5 then
    put "medium" into size
  else
    put "small" into size
  end if

  repeat 3 times
    beep
  end repeat
  repeat with i = 1 to 10
    add i to total
  end repeat
  repeat with i = 10 down to 1
    put i into field "Countdown"
  end repeat
  repeat while the mouse is down
    put the mouseLoc into field "Where"
  end repeat
  repeat until x > 100
    add 10 to x
  end repeat
  repeat forever
    if the shiftKey is down then exit repeat
    if the optionKey is down then next repeat
  end repeat
end mouseUp
```

- `exit repeat` leaves a loop, `next repeat` goes on to its next turn.
- `exit mouseUp` (the handler's name) leaves the handler; `exit to
  HyperCard` stops every handler running.
- `return value` answers a function call; in a message handler, it sets
  `the result`.
- `pass mouseUp` sends the message on along the path.

### Variables and parameters

A variable comes into being when something is put into it, and lasts until
its handler ends. `global` shares one between handlers and scripts, and
keeps it until RandomCard quits. Variables typed in the message box are
globals.

```hypertalk
on mouseUp
  global gScore
  put 0 into gScore
  put the params into everything
  put param(1) into first
  put the paramCount into howMany
end mouseUp
```

`it` is the variable `get`, `answer`, `ask`, `read` and the web commands put
their result in.

### send and do

```hypertalk
on mouseUp
  send "mouseUp" to button "OK"
  send "refresh" to card "Index"
  do "put 1 + 2 into x"
  put value("3 * 4") into y
end mouseUp
```

## Values

Everything in HyperTalk is text. A value is a number, a true-or-false
value, a point or a rectangle when it looks like one:

| Kind | Looks like | Test |
| --- | --- | --- |
| number | `42`, `-3.5`, `1e6` | `x is a number`, `x is an integer` |
| true or false | `true`, `false` | `x is a logical` |
| point | `10,20` | `x is a point` |
| rectangle | `10,20,110,80` (left, top, right, bottom) | `x is a rect` |
| date | `11/8/2026`, `Sunday, November 8, 2026` | `x is a date` |

Constants:

| Constant | Value |
| --- | --- |
| `empty` | the empty text |
| `return`, `lineFeed` | the end of a line |
| `space`, `tab`, `comma`, `colon`, `quote` | those characters |
| `formFeed` | the form feed character |
| `pi` | 3.14159265358979323846 |
| `true`, `false` | true and false |
| `zero` … `ten` | 0 to 10 |
| `up`, `down` | for the mouse and keys |

`the numberFormat` sets how numbers are written as text (`"0.00"` for two
decimal places; the default is `"0.######"`).

## Expressions and operators

From first to last in precedence:

| Operators | Meaning |
| --- | --- |
| `( )` | grouping |
| `-`, `not`, `there is a` | negation, logical not, whether an object exists |
| `^` | power |
| `*`, `/`, `div`, `mod` | times, divide, whole-number divide, remainder |
| `+`, `-` | add, subtract |
| `&`, `&&` | join text; `&&` puts a space between |
| `<`, `>`, `<=` (`≤`), `>=` (`≥`), `contains`, `is in`, `is not in`, `is within`, `is not within`, `is a`, `is not a` | comparing |
| `=`, `is`, `<>` (`≠`), `is not` | equal and not |
| `and` | both |
| `or` | either |

Comparisons ignore case. Text that looks like numbers compares as numbers.

```hypertalk
on mouseUp
  put (3 + 4) * 2 into a
  put 17 div 5 && 17 mod 5 into b
  put "Hyper" & "Card" into c
  if field "Name" contains "Atkinson" then beep
  if the mouseLoc is within the rect of button 1 then beep
  if there is a card "Index" then go to card "Index"
  if x is not a number then answer "Please type a number."
end mouseUp
```

## Chunks

Any text, whether in a field, a variable or the message box, can be taken
apart by **chunks**: characters, words (separated by spaces), items
(separated by `the itemDelimiter`, a comma unless set) and lines.

```hypertalk
on mouseUp
  put char 1 of "HyperCard" into initial
  put word 2 to 3 of field "Title" into middle
  put item 2 of "red,green,blue" into color
  put line 1 of field "Notes" into firstLine
  put the last word of line 3 of field 1 into lastWord
  put the number of lines of field "Notes" into count
  put word 2 of item 3 of line 4 of field 2 into deep
  set the itemDelimiter to tab
  put item 3 of line 2 of field "Table" into cell
end mouseUp
```

Ordinals work with any chunk or object: `first` … `tenth`, `last`,
`middle` (`mid`), `any`.

`put` places text **into**, **before** or **after** a chunk, and `delete`
removes one:

```hypertalk
on mouseUp
  put "Dr. " before word 1 of field "Name"
  put return & "Another line" after field "Notes"
  put "blue" into item 3 of colors
  delete line 1 of field "Notes"
  delete char 1 to 3 of x
end mouseUp
```

`offset(part, whole)` finds text, and chunk text names the chunk itself:
`the foundChunk` is text like `char 6 to 15 of card field 1`, and scripts
may use it anywhere a chunk goes (`select the foundChunk`).

## Objects

| Object | Named by |
| --- | --- |
| stack | `stack "Addresses"`, `this stack` |
| background | `background "Main"`, `bkgnd 2`, `bg id 2390`, `this background` |
| card | `card "Index"`, `card 3`, `card id 4021`, `this card`, `next card`, `prev card`, `first card`, `recent card`, `any card`, `marked card 2`, `card 2 of background "People"` |
| button | `button "OK"`, `card button 2`, `bkgnd button id 5`, `btn 1` |
| field | `field "Notes"`, `card field 1`, `bkgnd field "Name"`, `fld id 7` |
| part | `part 3` (buttons and fields together, in order) |
| the message box | `the message box`, `msg` |
| a menu | `menu "Edit"`, `menuItem "Copy" of menu "Edit"` |
| a window | `window "Prompt"`, `the card window` |

`me` is the object whose script is running; `the target` is the object the
message was first sent to. Buttons and fields are card parts unless named
`bkgnd`/`bg`/`background`, except that `field` alone means a background
field when the card has none by that name, as in HyperCard.

A field's text is its value: `put field "Name" into n`, `put n into field
"Name"`.

## Commands

| Command | Example |
| --- | --- |
| add, subtract, multiply, divide | `add 1 to field "Count"`, `divide total by 3` |
| answer | `answer "Save first?" with "Cancel" or "Save"`; the button clicked goes into `it` |
| answer file | `answer file "Open which picture?" of type "png"` |
| ask | `ask "Your name?" with "Bill"`; `ask password "Password:"` |
| ask file | `ask file "Save the report as:" with "Report.txt"` |
| beep | `beep`, `beep 3` |
| choose | `choose brush tool`, `choose browse tool` |
| click | `click at 100,200`, `click at the loc of button 1 with shiftKey` |
| close | `close file "Notes.txt"`, `close printing`, `close window "Prompt"` |
| convert | `convert the date to seconds`, `convert x to dateItems` |
| create | `create menu "Tools"`, `create stack "New" with background "Main"` |
| debug | `debug checkpoint` (stops in the debugger at the next line) |
| delete | `delete line 2 of field 1`, `delete button "Old"`, `delete menu "Tools"`, `delete menuItem 2 of menu "Tools"` |
| dial | `dial "555-0187"` |
| disable, enable | `disable menuItem "Cut" of menu "Edit"`, `enable button "Next"` |
| do | `do "go to card" && n` |
| doMenu | `doMenu "New Card"` |
| drag | `drag from 10,10 to 100,100 with shiftKey` (paints with the current tool) |
| edit script | `edit script of button "OK"` |
| export paint | `export paint to file "Card.png"` |
| find | `find "Atkinson"`, `find whole "Bill Atkinson" in field "Name"`, `find chars "ink"`, `find word "card"`, `find string "per C"` |
| get | `get the name of this card` (into `it`) |
| go | `go to card "Index"`, `go next`, `go back`, `go home`, `go to stack "Addresses"` |
| hide, show | `hide button "Secret"`, `show field 2 at 10,10`, `hide menuBar`, `show 5 cards` |
| import paint | `import paint from file "Frog.png"` |
| lock, unlock | `lock screen`, `unlock screen with dissolve`, `lock messages`, `unlock messages` |
| mark, unmark | `mark card 3`, `mark cards where field "Done" is true`, `unmark all cards` |
| open | `open file "Notes.txt"`, `open printing`, `open "Report.pdf"` (in its own program) |
| palette | `palette "Tools", "330,110"` |
| play | `play "boing"`, `play "harpsichord" tempo 200 "c4q e g c5h"`, `play stop` |
| pop, push | `push this card`, `pop card` |
| print | `print this card`, `print 5 cards`, `print field "Notes"`, `print "Hello"` |
| put | `put 3 into x`, `put "!" after field 1`, `put "Undo,-,Cut" into menu "Edit"` |
| read | `read from file "Notes.txt" until eof`, `read from file f for 10` |
| reset | `reset menuBar` |
| save | `save this stack`, `save this stack as "Copy"` |
| select | `select line 2 of field 1`, `select text of field "Name"`, `select button 1`, `select empty` |
| send | `send "update" to card "Index"` |
| set | `set the hilite of button 1 to true`, `set the textStyle of word 2 of field 1 to bold` |
| sort | `sort lines of field 1`, `sort items of x descending numeric`, `sort cards by field "Name"`, `sort cards of this background by the short name of this card` |
| speak | `speak "Hello"` |
| start using, stop using | `start using stack "Library"` |
| stop sound | `stop sound` (every channel) |
| type | `type "Hello" with commandKey` |
| visual effect | `visual effect dissolve slowly`, `visual effect iris open to black` |
| wait | `wait 2 seconds`, `wait 10 ticks`, `wait until the mouseClick`, `wait while the sound is not "done"` |
| write | `write field "Notes" to file "Notes.txt"` |

```hypertalk
on mouseUp
  answer "Save first?" with "Cancel" or "Save"
  if it is "Cancel" then exit mouseUp
  ask "What's your name?" with "Bill"
  if it is not empty then put it into field "Name"
  visual effect dissolve slowly
  go to next card
  find whole "Bill Atkinson" in field "Name"
  sort cards of this background by field "Name"
  open file "Notes.txt"
  write field "Notes" to file "Notes.txt"
  close file "Notes.txt"
  lock screen
  set the hilite of button 1 to true
  unlock screen with dissolve
end mouseUp
```

Visual effects: `dissolve`, `wipe` and `scroll` (`left`, `right`, `up`,
`down`), `barn door` and `iris` (`open`, `close`), `venetian blinds`,
`checkerboard`, `zoom` (`open`, `close`, `in`, `out`), `shrink` and
`stretch` (`to top`, `to center`, `to bottom`), `push` (and a direction),
`plain`, each `slowly`, `fast` or `very fast`, `to black`, `to white`,
`to gray`, `to inverse` or `to card`.

## Functions

A function is called with parentheses, `max(3, 7)`, or with `the`,
`the date`. Functions of one value may be written `the sqrt of 16`.

| Kind | Functions |
| --- | --- |
| Math | `abs`, `annuity`, `atan`, `average`, `compound`, `cos`, `exp`, `exp1`, `exp2`, `ln`, `ln1`, `log2`, `max`, `min`, `random`, `round`, `sin`, `sqrt`, `sum`, `tan`, `trunc` |
| Text | `charToNum`, `numToChar`, `length`, `offset`, `value`, `textWidth`, `urlEncode`, `urlDecode`, `formEncode`, `formDecode` |
| Time | `the date` (`short`, `long`, `abbrev`), `the time` (`long`), `the seconds`, `the ticks`, `the dateItems` |
| Mouse | `the mouse`, `the mouseClick`, `the mouseLoc`, `the mouseH`, `the mouseV`, `the clickLoc`, `the clickH`, `the clickV`, `the clickChunk`, `the clickLine`, `the clickText` |
| Keys | `the shiftKey`, `the optionKey`, `the commandKey` (`cmdKey`), `the controlKey`, `the keysDown` |
| Selection and finding | `the selectedText`, `the selectedChunk`, `the selectedLine`, `the selectedField`, `the selectedLoc`, `the foundText`, `the foundChunk`, `the foundLine`, `the foundField`, `the selectedButton of card family 1` |
| Handlers | `the params`, `the paramCount`, `param(n)`, `the result`, `the target` |
| Sound and speech | `the sound`, `the speech`, `the voices` |
| The system | `the version`, `the systemVersion`, `the screenRect`, `the diskSpace`, `the heapSpace`, `the tool`, `the menus`, `the stacks`, `the stacksInUse`, `the windows` |

```hypertalk
on mouseUp
  put random(6) into die
  put round(2.5) && trunc(-2.5) into rounded
  put the long date into today
  put the number of words of field 1 into count
  put charToNum("A") into code
  put offset("Card", "HyperCard") into where
  convert today to dateItems
  if the shiftKey is down then answer the clickText
end mouseUp
```

`round` rounds a half to the even number, as HyperCard did: `round(2.5)`
is 2 and `round(3.5)` is 4. `random(n)` is a whole number from 1 to n.

## Properties

`the name of button 1`, `set the visible of field 2 to false`. Most work
on several kinds of object; `get`, `put` and `set` read and change them.

**Global:** `blindTyping`, `cursor`, `dragSpeed`, `editBkgnd`,
`itemDelimiter`, `language`, `lockMessages`, `lockRecent`, `lockScreen`,
`messageWatcher`, `numberFormat`, `powerKeys`, `soundChannel`,
`soundVolume`, `textArrows`, `traceDelay`, `userLevel`, `userModify`,
`variableWatcher`, and the web's `httpHeaders`, `httpTimeout`,
`responseHeaders`, `responseStatus`, `responseTime`.

**Painting:** `brush`, `centered`, `filled`, `grid`, `lineSize`,
`multiple`, `multiSpace`, `pattern`, `polySides`, `textFont`, `textSize`,
`textStyle`, `textAlign`, `textHeight`, and color: `penColor`,
`brushColor`.

**Stacks:** `name`, `size`, `freeSize`, `script`, `cantAbort`, `cantDelete`,
`cantModify`, `cantPeek`, `privateAccess`, `userLevel`.

**Backgrounds and cards:** `name`, `id`, `number`, `script`, `marked`,
`cantDelete`, `dontSearch`, `showPict`, `owner`, `rect`.

**Buttons:** `name`, `id`, `number`, `partNumber`, `rect`, `loc`, `left`,
`top`, `right`, `bottom`, `width`, `height`, `topLeft`, `bottomRight`,
`visible`, `style` (`transparent`, `opaque`, `rectangle`, `roundRect`,
`shadow`, `checkBox`, `radioButton`, `standard`, `default`, `oval`,
`popup`), `hilite`, `autoHilite`, `sharedHilite`, `family`, `showName`,
`enabled`, `icon`, `titleWidth`, `textFont`, `textSize`, `textStyle`,
`textAlign`, `textHeight`, `script`, `contents` (a popup's items), and
color: `foreColor`, `backColor`, `borderColor`.

**Fields:** the button properties that apply, and `lockText`, `dontWrap`,
`autoTab`, `wideMargins`, `showLines`, `sharedText`, `dontSearch`,
`autoSelect`, `multipleLines`, `fixedLineHeight`, `scroll`, `style`
(`transparent`, `opaque`, `rectangle`, `shadow`, `scrolling`), `text`,
`selectedText`, `selectedLine`, `selectedChunk`, and RandomCard's
`dontSave` and `choices`.

**Chunks of a field:** `textFont`, `textSize`, `textStyle`, `foreColor`,
`textWidth`.

```hypertalk
on mouseUp
  set the loc of button "Ball" to 100,100
  set the textStyle of line 1 of field "Notes" to bold
  set the style of button 1 to roundRect
  put the rect of this card into r
  set the userLevel to 5
  set the soundChannel to 2
  if the hilite of button "Loud" then set the soundVolume to 7
end mouseUp
```

Menus have `name`, `enabled`, `checkMark`, `markChar`, `commandChar`
(`cmdChar`) and `menuMessage` (`menuMsg`); windows a script opens have
`loc`, `rect`, `visible` and, for text windows, `text`.

## System messages

RandomCard sends these as HyperCard did:

| When | Messages |
| --- | --- |
| The mouse | `mouseDown`, `mouseStillDown`, `mouseUp`, `mouseDoubleClick`, `mouseEnter`, `mouseWithin`, `mouseLeave` |
| Keys | `keyDown`, `arrowKey`, `functionKey`, `commandKeyDown`, `controlKey`, `returnKey`, `enterKey`, `tabKey`, `returnInField`, `enterInField` |
| Fields | `openField`, `closeField`, `exitField` |
| Going places | `openCard`, `closeCard`, `openBackground`, `closeBackground`, `openStack`, `closeStack`, `resumeStack`, `suspendStack` |
| Making and removing | `newCard`, `deleteCard`, `newBackground`, `deleteBackground`, `newButton`, `deleteButton`, `newField`, `deleteField`, `newStack`, `deleteStack` |
| RandomCard itself | `startUp`, `resume`, `suspend`, `quit`, `idle`, `help` |
| Menus and palettes | `doMenu`, `openPalette`, `closePalette` |

Built-in commands go through the message path too (`on go`, `on doMenu`,
`on find`), so a script can change or stop any of them.

```hypertalk
on keyDown theKey
  if theKey is a number then pass keyDown
  beep
end keyDown

on arrowKey direction
  if direction is "left" then go prev card
  else if direction is "right" then go next card
end arrowKey

on idle
  put the time into field "Clock"
  pass idle
end idle
```

## Old externals, built in

HyperCard stacks extended the language with compiled externals, which no
longer run anywhere. RandomCard builds in the ones stacks depend on, by
their old names, so those stacks run unchanged; a few with prefixed names
(`KrsNewMenu`) are updated in our copies instead, by fix steps. Most are
also reachable through ordinary HyperTalk, given in the second column.
[XCMDS.md](XCMDS.md) lists every one found, with what it did and what
became of it.

| External | Built in as |
| --- | --- |
| AddColor (HyperCard 2.2–2.4's Color Tools) | `addColor` with all its functions: `colorCard`, `colorButton`, `addButton`, `addRect`, `addPict`, `getObjectClicked`, `moveToFront` and the rest |
| FileExists | `there is a file "Notes.txt"` |
| FileName, FilePath, FileN | `answer file` |
| FolderName, SelectDir, FolderPath | a dialog for choosing a folder |
| SFPutFile, NewFileName, FullSFPut | `ask file` |
| SysPrefs | the preferences file's path (in RandomCard's settings folder) |
| Dialog | `answer` |
| ListSelect | a list dialog for choosing lines |
| PopUpMenu, PopMenu | a popup menu at a point |
| Prompt | a "please wait" window: `Prompt 1, "Working…"` and `Prompt 3` |
| TEXWindow | a text window |
| TNPict | the `picture` window (pictures as PNG for now) |
| SelectLineXCMD | `select line … of field` |
| ZoomRect | `zoomRect rect1, rect2`: the Finder's zoom |
| PrintOut | `print` with text settings |
| StringWidth | `textWidth` |
| ProgressWindoid | a progress window |
| MakeSendMe4, MakeSendMe5 | a new message in the user's mail program |
| GetVol | the mounted volumes |
| FileCopy, EraseFile, FullMove, FullRename, GetDir, GetFInfo, GetBootVol, GetSysFolder, LeafName | files and folders (Frédéric Rinaldi's collection) |
| LineCount, FullSort, DateSort, GetPassword, GetMode, Switch, CapsLock, ClipInfo, ATalkZones, GlobalList, WindName, MacType | text, sorting and system details (Rinaldi) |
| Mousoid, Infoid, Promptoid, Notification | small floating windows (Rinaldi) |
| SendPS | PostScript, opened in the system's viewer to print |
| GetDocs, GetCreator | HyperCard 1.2's Documents stack |
| PlayMOD | songs: `play "Title Music"`, `play stop "Title Music"` |
| Main | ROT13, the Technical Notes stacks' easter egg |
| BoardInfo, INITFinder, SpinAbout, SwitchXRoo | stubs: Mac details that mean nothing today, answered harmlessly |

```hypertalk
on mouseUp
  if there is a file "Notes.txt" then answer "Found it."
  Prompt 1, "Counting the lines…"
  put LineCount("card field 1") into n
  Prompt 3
  put Switch("red,green,blue") into asLines
  zoomRect "0,0,10,10", "100,100,300,200"
end mouseUp
```

## RandomCard's additions

These go beyond HyperCard 2.4.1. Where HyperCard had a name for something,
RandomCard uses it.

### The web: REST and JSON

```hypertalk
on mouseUp
  put url "https://api.chucknorris.io/jokes/random" into reply
  if the result is not empty then
    answer "That didn't work:" && the result
    exit mouseUp
  end if
  put key "value" of json reply into field "Joke"

  set the httpHeaders to "Accept: application/json"
  set the httpTimeout to 10
  set key "name" of json body to "Ada"
  post body to url "https://httpbin.org/post"
  put the responseStatus && the responseTime into field "Status"

  load url "https://httpbin.org/get" with message "gotData"
end mouseUp

on gotData theURL, reply
  put the pretty of json reply into field "Reply"
end gotData
```

- **Requests:** `put url U into x` and `get url U` (GET); `post B to url U`,
  `put B to url U`, `patch B to url U`, `query B to url U` (the IETF QUERY
  method), `delete url U`, and `request M [with B] to url U` for any
  method. The reply goes into `it`, and `the result` is empty when it
  worked, else the status ("404 Not Found") or what went wrong.
- **Without waiting:** `with message "gotData"` after any of them (or `load
  url U with message …`): the handler goes on, and the message comes back
  to the same object with the URL and the reply.
- **Settings and answers:** `the httpHeaders`, `the httpTimeout`, `the
  responseHeaders`, `the responseStatus`, `the responseTime` (in
  milliseconds).
- **JSON:** `key "name" of json x`, `element 2 of json x` (from 1, or
  `last`), nested (`key "city" of key "address" of json x`); `the keys of
  json x` and `the elements of json x`, one a line; `the number of elements
  of json x`; `the pretty of json x`. `set key "name" of json x to "Ada"`
  builds and changes JSON in a variable or field.
- **Encoding:** `urlEncode`, `urlDecode`, `formEncode`, `formDecode`.

The `examples/` folder has two stacks built on these: ICNDB (a REST client
for one service) and PostCard (a small Postman).

### Fields

- **Combo boxes:** a field's `choices` (one a line) put a ▼ in it; picking
  one fills the field, which can still be typed into.
- **Don't Save:** `set the dontSave of field "Results" to true`: the field
  works as usual, but opens empty.
- **Colors:** `foreColor`, `backColor` and `borderColor` of buttons and
  fields, and `foreColor` of any chunk of a field.

### Painting and pictures

- `the penColor` and `the brushColor` paint in color.
- `import paint from file` reads PNG, JPEG, GIF, BMP, MacPaint and PICT;
  `export paint to file` writes PNG.

### Sound

- **Songs:** soundtracker modules (MOD, XM, S3M, IT) a stack carries play
  with `play "name"`, looping beside sound effects, until `play stop
  "name"`.
- `the soundVolume`, from 0 to 7.

### Text and the keyboard

- **Unicode:** text is Unicode throughout. `charToNum("花")` is 33457 and
  `numToChar(26413)` is 札.
- `textWidth(text [, font, size, style])` and `the textWidth of` a chunk:
  its width in pixels by HyperCard's own font measurements.
- `the keysDown`: the keys held now, by name (`a`, `shift`, `left`,
  `space`…), separated by commas.
- `flush keys`: drops keys typed but not yet handled.

### The system

`the osName` (`macos`, `linux` or `windows`), `the osVersion`, `the
pathSeparator`: for scripts that work with files on any system. `the
version` is 2.41, the version of HyperCard RandomCard follows.

## Differences from HyperCard

RandomCard keeps HyperCard's features and language, not the old Mac they
ran on:

- **Text is Unicode.** `charToNum` and `numToChar` use Unicode code points,
  not Mac Roman codes. Old stacks are converted when imported: Mac Roman
  text, and Japanese (MacJapanese) for stacks made with HyperCard J.
- **Paths are the system's:** `/Users/bill/Notes.txt`, not `Macintosh
  HD:Notes.txt`. Stacks that build Mac paths are updated by fix steps in
  our copies ([FIX_STEPS.tsv](FIX_STEPS.tsv)).
- **Compiled externals don't run.** The common ones are built in (see
  above); stacks that need others say "Can't understand".
- **AppleEvents, AppleScript and other programs** aren't there yet: see
  COMPLIANCE.md.
- **What's still missing** from HyperCard 2.4.1 (the `picture` command's
  windows, report printing, some window properties and a few messages) is
  listed in COMPLIANCE.md and on the roadmap.
