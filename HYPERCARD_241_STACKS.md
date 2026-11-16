# HyperCard 2.4.1's own stacks

The stacks that came with HyperCard 2.4.1, from its installation disk:
Apple's own, for the version RandomCard follows, using its newest features.
They live in `examples/apple/hypercard_241_stacks` (Apple's, so not in git).

Each stack is opened, and every scripted button on every card clicked, by
the `click_every_button` test; `tools/hc241.py` lists the stacks and writes
this page from the results. A clean stack gets no script error on any
click. "Stopped" counts clicks that ran into the two-second limit, usually
a timed demo or an animation, which aren't errors.

Clean stacks are kept clean: `tests/hc241_clean.txt` lists them, and the
`hypercard_241_stacks_stay_clean` test, part of `cargo test` wherever the
stacks are, fails if one of them gets an error; visual effects and `wait`
don't pause, with no one watching, so all of them take seconds. A stack
joins the list when this page is written again after a fix. What breaks
the others is in the roadmap's "HyperCard 2.4.1's own stacks" section.

**11 of 29 stacks are clean** (2026-11-16).

## Clean

| Stack | Clicks | Stopped |
| --- | --- | --- |
| About HyperCard... | 16 | 0 |
| AppleScript Stacks/Language at a Glance | 81 | 0 |
| Color Tools | 19 | 0 |
| HyperCard Help/Help Extras | 6 | 0 |
| HyperCard Stacks/Audio Help | 23 | 0 |
| More Stacks/Alternate Home Stacks/Home (Browser)/Home | 28 | 1 |
| More Stacks/Alternate Home Stacks/Home (Small)/Home | 21 | 0 |
| More Stacks/Readymade Fields | 8 | 0 |
| PaintTest | 0 | 0 |
| Practice | 5 | 0 |
| Your Tour of HyperCard/HyperCard Tour | 189 | 1 |

## With errors

| Stack | Clicks | Distinct errors | First error |
| --- | --- | --- | --- |
| Addresses | 30 | 1 | going to card 3: Can't understand "setlocationcheckmark" |
| AppleScript Stacks/AppleScript Library | 14 | 6 | openStack: Script error in stack "AppleScript Library": line 36: Expected an object after "there is a", found "scriptinglanguage" |
| AppleScript Stacks/AppleScript Mail Merge | 29 | 2 | openStack: Script error in bkgnd id 2599: line 78: Expected "then", found "AppleScript" |
| AppleScript Stacks/AppleScript Text Controls | 13 | 11 | openStack: Script error in bkgnd id 2733: line 41: Expected "then", found "AppleScript" |
| AppleScript Stacks/HyperCard AppleScript Reference | 22 | 1 | card 1 button "Next": No such function "destination" |
| Home | 16 | 2 | openStack: No such function "stackisbeingused" |
| HyperCard Help/Apple Event Primer | 26 | 5 | card 4 button "Request": Script error in card button "Request": line 10: Expected end of line, found "of" |
| HyperCard Help/HyperCard Help | 47 | 1 | card 1 button "Home": No such function "destination" |
| HyperCard Help/HyperTalk Reference | 30 | 12 | card 1 button "Find Topic": Can't understand "gofindtopic" |
| HyperCard Stacks/Art Bits | 14 | 1 | card 1 button "Beasts": Can't understand "generalpalette" |
| HyperCard Stacks/Graph Maker | 20 | 1 | card 2 button "frame": No such property "textheight" |
| HyperCard Stacks/New Features | 14 | 3 | openStack: The button has no line 0 |
| HyperCard Stacks/Phone Dialer | 15 | 1 | card 1 button "Shrink Window": Can't set the rect of a card |
| HyperCard Stacks/QuickTime Tools | 173 | 2 | openStack: No such object "QuickTime" |
| More Stacks/Background Art | 20 | 1 | card 1 button "Books and notebooks": Can't understand "fieldpalette" |
| More Stacks/Power Tools | 68 | 4 | openStack: Can't understand "generalpalette" |
| More Stacks/Readymade Buttons | 85 | 3 | card 5 button "Show Navigator": Can't find palette "Navigator" |
| More Stacks/Stack Templates | 95 | 2 | openStack: Can't understand "hidepalette" |

## Not stacks

Files beside the stacks: pictures, fonts and movies.

- Flower.pic
- HyperCard Stacks/HyperCard Fonts
- HyperCard Stacks/Simple Movie
- HyperCard Stacks/Simple Movie.mov
