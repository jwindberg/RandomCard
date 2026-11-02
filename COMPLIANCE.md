# HyperCard 2.4.1 compliance

RandomCard aims to do everything the last HyperCard, 2.4.1 (1998), could do.
This file audits RandomCard against Apple's own documentation for it, item by
item. Every gap here is also on the roadmap (ROADMAP.md, "HyperCard 2.4.1
compliance").

## Sources

From the HyperCard 2.4.1 install disk (in `examples/apple/hypercard-2.4.1`, not
in git; text versions in its `text` folder):

- **HyperCard Script Language Guide** (1996, 680 pages): the HyperTalk manual.
  Appendix I, "HyperTalk Vocabulary", lists all 443 terms with their kind;
  Appendix H, "Syntax Summary", gives every command form.
- **HyperCard Reference** (2.3.5, 1996, 455 pages): the application: menus,
  tools, windows, color.
- **HyperCard Installation and New Features** (1998, 149 pages): what 2.4 and
  2.4.1 added.

## How it's checked

Three probes in `src/runtime/tests.rs`, re-runnable as RandomCard improves:

- `vocabulary_probe` evaluates every function, property and constant from
  Appendix I (properties against a suitable object).
- `command_forms_probe` parses a concrete example of every command form in
  Appendix H (written out by hand in `text/commands_concrete.txt`, since the
  PDF lost the italics that mark placeholders), and flags forms that parse to
  nothing.
- System messages are checked against the messages RandomCard sends.

Passing a probe means the term is there, not that every detail matches
HyperCard; the stacks in BUGHUNT.md test that.

## Results: the language (Script Language Guide)

| Kind | Checked | Work | Gaps |
| --- | --- | --- | --- |
| Command forms | 164 | 133 | 18 don't parse, 13 parse but do nothing |
| Functions, properties, constants | 210 | 173 | 37, a few of them the probe's own limits |
| System messages | 57 | 46 | 11 never sent |
| Keywords, chunks, ordinals, tools and the rest | 93 | not yet probed | |

### Gaps, by feature

**HyperCard 2 windows.** `the globalLoc` and `globalRect`, `the hBarLoc` and
`vBarLoc` (the card window's scroll), `the zoom`; `hide window` and `show
window` for named windows; the moveWindow and sizeWindow messages. A start was
made with TEXWindow's text windows (`the text of window`, `close window`).

**Palettes and picture windows.** The `palette` command and the openPalette
and closePalette messages; the `picture` command (Apple's Picture XCMD, built
into 2.x), with the openPicture, closePicture, mouseDownInPicture and
mouseUpInPicture messages and the `dithering` and `zoom` properties.

**AppleEvents and other programs.** `answer program`, `request … from
program`, `request appleEvent`, `reply`, the appleEvent message, `the
programs`, `the destination`, `the scriptingLanguage`; opening, printing and
closing documents with an application (`open "doc" with "TeachText"` parses
but does nothing). On a modern computer these map to running other programs
and to inter-program messages; what that means for RandomCard needs design.

**Report printing.** `open report printing with template`, `the
reportTemplates`, and the print text settings: `printMargins`,
`printTextAlign`, `printTextFont`, `printTextHeight`, `printTextSize`,
`printTextStyle`.

**Paint import and export.** `import paint from file`, `export paint to file`,
and `reset paint` (parses but does nothing).

**Error dialogs and Recent.** `lock error dialogs` and `unlock error dialogs`,
`the lockErrorDialogs` (a script handles its own errors instead of the error
dialog), and `lock recent` / `unlock recent`.

**Scripting environment properties.** `the scriptEditor`, `scriptTextFont`,
`scriptTextSize`, `the debugger`, `the environment` (development or player),
`the address`, `the stackSpace`, `the multiSpace`, `the longWindowTitles`, and
the message watcher's `hideIdle` and `hideUnused`. `the commands` and `the
properties` (lists of what's available) are also missing.

**Smaller items.**
- `show groups` and `hide groups` are accepted, but the gray underline under
  group-style text isn't drawn.
- `ask file … with default name`.
- `doMenu … with` modifier keys.
- `the selectedButton of family N` (the function form works), `the
  hilitedButton`, `the buttonCount`.
- `the dialingTime` and `dialingVolume`, for `dial`.
- `enterInField`, `enterKey`, `returnInField`, `returnKey`, `tabKey` and
  `help` used as commands parse but do nothing; each should do what its key or
  menu item does.
- `create stack … with background in a new window` parses but does nothing.

## Still to audit

- **What 2.4 and 2.4.1 added** (New Features manual): QuickTime movies driven
  from HyperTalk, linking to URLs, opening many files, the enhanced script
  editor, the improved Picture command, new syntax.
- **The application** (HyperCard Reference): menus, tools, dialogs, the color
  tools, and everything else that isn't the language.
- The 93 keywords, chunks, ordinals, tools and other terms.
