"""Write examples/Solitaire.rcstack: solitaire card games: FreeCell,
Klondike and Spider.

usage: python3 -I tools/make_solitaire.py   (from the repository's top folder)

The card faces are Byron Knoll's Vector Playing Cards 1.3 (public domain),
read from reference/Playing Cards/PNG-cards-1.3 and made into color icons.
The back is drawn here after Bas de Reuver's (BdRGames) design 1, which is
CC0. The games' scripts are here, readable; in the stack, choose Objects ▸
Card Script on a game's card, and Stack Script for what they share.
"""
import re
import base64
import io
import json
import os

from PIL import Image, ImageDraw

# The table is laid out in these units, then scaled by SCALE: at 0.8 it's
# 672×480 and fits a 720×720 screen. The scripts' numbers are marked §n
# and scaled the same way, so the slots and the cards placed in them agree.
SCALE = 0.8
WIDTH, HEIGHT = 840, 600
CARD_W, CARD_H = 80, 116


def sc(n):
    """A length in the layout's units, scaled and rounded."""
    return int(n * SCALE + 0.5)


PX_W, PX_H = sc(CARD_W), sc(CARD_H)
FACES = 'reference/Playing Cards/PNG-cards-1.3'
FELT = [21, 96, 54]
SLOT = [16, 78, 43]
SLOT_EDGE = [96, 160, 116]
WHITE = [255, 255, 255]
BACK_ICON = 40100
next_id = 1


def new_id():
    global next_id
    next_id += 1
    return next_id - 1


def rect(l, t, r, b):
    """A rect in the layout's units, scaled."""
    return rect_px(sc(l), sc(t), sc(l) + sc(r - l), sc(t) + sc(b - t))


def rect_px(l, t, r, b):
    """A rect already scaled."""
    return {'left': l, 'top': t, 'right': r, 'bottom': b}


def card_rect(x, y):
    """A card's rect at (x, y), both already scaled."""
    return rect_px(x, y, x + PX_W, y + PX_H)


def text_size(n):
    return max(10, sc(n))


def scaled(script):
    """The script with each §n replaced by n scaled."""
    return re.sub(r'§(\d+)', lambda m: str(sc(int(m.group(1)))), script)


def button(name, r, script='', style='RoundRect', **extra):
    p = {'id': new_id(), 'kind': 'Button', 'name': name, 'rect': r if isinstance(r, dict) else rect(*r),
         'script': script.strip('\n'), 'style': style}
    if 'text_size' in extra:
        extra['text_size'] = text_size(extra['text_size'])
    p.update(extra)
    return p


def field(name, r, text='', size=12, style='Transparent', lock=True, align='Left', bold=False, **extra):
    p = {'id': new_id(), 'kind': 'Field', 'name': name, 'rect': rect(*r), 'script': '', 'style': style,
         'contents': text, 'text_size': text_size(size), 'text_style': 'bold' if bold else 'plain', 'text_align': align,
         'lock_text': lock, 'dont_wrap': False}
    p.update(extra)
    return p


def label(name, r, text, size=12, **extra):
    return field(name, r, text, size, **extra)


def picture():
    return {'width': sc(WIDTH), 'height': sc(HEIGHT), 'data': ''}


RANKS = 'A23456789TJQK'
SUITS = 'CDHS'
RANK_FILES = {'A': 'ace', 'T': '10', 'J': 'jack', 'Q': 'queen', 'K': 'king'}
SUIT_FILES = {'C': 'clubs', 'D': 'diamonds', 'H': 'hearts', 'S': 'spades'}


def face_file(code):
    rank, suit = RANK_FILES.get(code[0], code[0]), SUIT_FILES[code[1]]
    # The court cards in their traditional dress, and the plain ace of
    # spades (the ornate one may not be Byron Knoll's to give away).
    alt = '2' if code[0] in 'JQK' or code == 'AS' else ''
    return os.path.join(FACES, f'{rank}_of_{suit}{alt}.png')


def png_picture(im):
    out = io.BytesIO()
    im.save(out, 'PNG', optimize=True)
    return {'width': im.width, 'height': im.height, 'data': '', 'png': base64.b64encode(out.getvalue()).decode()}


def icon_picture(path):
    return png_picture(Image.open(path).convert('RGBA').resize((PX_W, PX_H), Image.LANCZOS))


def card_back(color=(30, 68, 149)):
    """Bas de Reuver's design 1: a white card, a colored rounded frame, and a
    diamond lattice inside it. Drawn four times over and scaled down."""
    s = 4
    w, h = CARD_W * s, CARD_H * s
    sx, sy = w / 250, h / 350
    im = Image.new('RGBA', (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle((0, 0, w - 1, h - 1), radius=10 * sx, fill=(255, 255, 255, 255), outline=(0, 0, 0, 255),
                        width=max(1, round(sx)))
    d.rounded_rectangle((15 * sx, 15 * sy, 235 * sx, 335 * sy), radius=10 * sx, outline=color + (255,),
                        width=round(3 * sx))
    left, top, right, bottom = 20 * sx, 20 * sy, 230 * sx, 330 * sy
    cell_w, cell_h = 10 * sx, 20 * sy
    lattice = Image.new('RGBA', (w, h), (0, 0, 0, 0))
    ld = ImageDraw.Draw(lattice)
    y = top
    while y < bottom:
        x = left
        while x < right:
            ld.line((x, y, x + cell_w, y + cell_h), fill=color + (255,), width=round(1.5 * sx))
            ld.line((x + cell_w, y, x, y + cell_h), fill=color + (255,), width=round(1.5 * sx))
            x += cell_w
        y += cell_h
    mask = Image.new('L', (w, h), 0)
    ImageDraw.Draw(mask).rectangle((left, top, right, bottom), fill=255)
    im.paste(lattice, (0, 0), Image.composite(lattice, Image.new('RGBA', (w, h)), mask))
    d.rectangle((left, top, right, bottom), outline=color + (255,), width=max(1, round(sx)))
    return png_picture(im.resize((PX_W, PX_H), Image.LANCZOS))


# ----- What every game shares: the stack script -----

STACK_SCRIPT = r'''
-- Solitaire: what every game shares. Each game's own rules are in its
-- card's script.
--
-- A card is a button whose icon is its face (named AS, TH, 2C: rank,
-- then suit) or the Back. While you play, a game is in globals; it's also
-- kept in hidden fields on its card, so a game in progress is saved with
-- the stack.

function rankOf c
  if c is empty then return 0
  return offset(char 1 of c, "A23456789TJQK")
end rankOf

function isRed c
  return char 2 of c is in "DH"
end isRed

-- A deck shuffled by game number, the way Windows FreeCell shuffled: its
-- random numbers and its swaps, so a number always deals the same game.
-- The deck is lines; so is what comes back, in the order dealt.
function shuffled deck, n
  put n into seed
  put empty into dealt
  put the number of lines of deck into remaining
  repeat while remaining > 0
    put (214013 * seed + 2531011) mod 2147483648 into seed
    put (seed div 65536) mod remaining + 1 into k
    put line k of deck & return after dealt
    put line remaining of deck into line k of deck
    delete line remaining of deck
    subtract 1 from remaining
  end repeat
  return dealt
end shuffled

-- A card put in its place, face up (its own icon) or down (the Back),
-- in front of the cards it covers.
on placeCard b, face, x, y
  set the rect of card button b to x & "," & y & "," & (x + §80) & "," & (y + §116)
  if the icon of card button b is not face then set the icon of card button b to face
  show card button b
  set the partNumber of card button b to 999
end placeCard

-- The whole game on one line, for Undo and for saving: lists separated
-- by |, each of lines separated by /.
function packLines v, n
  put empty into packed
  repeat with k = 1 to n
    put line k of v after packed
    if k < n then put "/" after packed
  end repeat
  return packed
end packLines

function unpackLines v, n
  put empty into unpacked
  set the itemDelimiter to "/"
  repeat with k = 1 to n
    put item k of v into line k of unpacked
  end repeat
  set the itemDelimiter to ","
  return unpacked
end unpackLines

-- ----- The record: wins, losses and streaks, on each game's card -----

function recordText
  put field "Won" + 0 into won
  put field "Lost" + 0 into lost
  put "Won" && won & ", lost" && lost into r
  if won + lost > 0 then put " (" & round(won * 100 / (won + lost)) & "%)" after r
  put ". Streak" && field "Streak" + 0 & ", best" && field "Best" + 0 & "." after r
  return r
end recordText

on showRecord
  put recordText() into field (the short name of this card && "Record") of card "Games"
end showRecord

-- Before leaving a game for another: one with moves made and not won
-- counts as lost, and ends the streak.
on abandonGame moves
  if moves > 0 and field "Finished" is empty then
    add 1 to field "Lost"
    put 0 into field "Streak"
    showRecord
  end if
end abandonGame

-- A win counts once, even after Undo and winning again; then the cards
-- celebrate.
on wonGame cardList
  if field "Finished" is not "won" then
    put "won" into field "Finished"
    add 1 to field "Won"
    add 1 to field "Streak"
    if field "Streak" > field "Best" then put field "Streak" into field "Best"
    showRecord
  end if
  play "harpsichord" tempo 240 "c4e e g c5q"
  celebrate cardList
end wonGame

-- The cards fly into a ring around the news, a card at a time. A click
-- skips to the end.
on celebrate cardList
  put "You won!" & return & recordText() into field "Banner"
  show field "Banner"
  show button "Play Again"
  set the partNumber of field "Banner" to 999
  set the partNumber of button "Play Again" to 999
  put the number of items of cardList into howMany
  put false into skip
  if howMany > 60 then put 1 into frames else put 3 into frames
  repeat with k = 1 to howMany
    put item k of cardList into c
    put (k - 1) * 2 * pi / howMany - pi / 2 into angle
    put §420 + round(§260 * cos(angle)) into x
    put §334 + round(§200 * sin(angle)) into y
    show card button c
    set the partNumber of card button c to 999
    if the mouseClick then put true into skip
    if skip is not true then
      put the loc of card button c into startLoc
      repeat with f = 1 to frames
        put item 1 of startLoc + round((x - item 1 of startLoc) * f / frames) into sx
        put item 2 of startLoc + round((y - item 2 of startLoc) * f / frames) into sy
        set the loc of card button c to sx & "," & sy
        wait 1 tick
      end repeat
    end if
    set the loc of card button c to x & "," & y
  end repeat
  set the partNumber of field "Banner" to 999
  set the partNumber of button "Play Again" to 999
end celebrate
'''

# The mouse, the same in every game; each game's script supplies whereIs,
# pickUp, dropPlace, tryMove, clickMove, layout and checkWin.
DRAG = r'''
-- ----- The mouse (the same in every game) -----

-- Press a card and drag it, with the cards on it, to where it goes; or
-- just click it to send it to the best place. The game says what can be
-- picked up (pickUp), where a drop lands (dropPlace), and how a move is
-- made (tryMove, clickMove).
on mouseDown
  put the short name of the target into c
  if not isCardName(c) then pass mouseDown
  put whereIs(c) into src
  put pickUp(c, src) into moving
  if moving is empty then exit mouseDown
  -- Pick the cards up: in front, keeping where each sits from the mouse.
  put the number of items of moving into howMany
  put empty into starts
  repeat with k = 1 to howMany
    put item k of moving into m
    set the partNumber of card button m to 999
    put the topLeft of card button m into line k of starts
  end repeat
  put the mouseLoc into start
  put false into dragged
  repeat while the mouse is down
    put the mouseLoc into pt
    put item 1 of pt - item 1 of start into dx
    put item 2 of pt - item 2 of start into dy
    if abs(dx) + abs(dy) > 4 then put true into dragged
    if dragged then
      repeat with k = 1 to howMany
        put line k of starts into tl
        set the topLeft of card button (item k of moving) to (item 1 of tl + dx) & "," & (item 2 of tl + dy)
      end repeat
    end if
  end repeat
  if dragged then
    put item 1 of moving into lead
    put dropPlace(the loc of card button lead) into dest
    if dest is empty then put "same" into problem
    else put tryMove(moving, src, dest) into problem
  else
    put clickMove(moving, src) into problem
  end if
  layout
  if problem is not empty and problem is not "same" then showStatus problem
  checkWin
end mouseDown

-- Edit ▸ Undo undoes a move here.
on doMenu what
  if what is "Undo" then undoMove
  else pass doMenu
end doMenu
'''

# ----- FreeCell -----

FREECELL_SCRIPT = r'''
-- FreeCell, with the same numbered deals as Windows FreeCell: game 1 here
-- is game 1 there.
--   gCols    8 lines, a column each, its cards bottom to top by commas
--   gCells   4 lines, the free cells
--   gHomes   4 lines, the top card on each foundation
--   gHistory a line per move, the game before it, for Undo

on openCard
  global gCols
  loadGame
  if gCols is empty then newGame random(32000)
  else layout
  pass openCard
end openCard

-- ----- Dealing -----

-- Deal game n as Windows FreeCell does: aces first in the deck (clubs,
-- diamonds, hearts, spades), its shuffle, and the cards dealt across the
-- columns a row at a time.
on newGame n
  global gCols, gCells, gHomes, gGame, gMoves, gHistory
  put n into gGame
  put empty into field "Finished"
  put empty into gCols
  put empty into gCells
  put empty into gHomes
  put empty into gHistory
  put 0 into gMoves
  put empty into deck
  repeat with i = 0 to 51
    put i div 4 + 1 into r
    put i mod 4 + 1 into s
    put char r of "A23456789TJQK" & char s of "CDHS" into line i + 1 of deck
  end repeat
  put shuffled(deck, n) into dealt
  repeat with k = 1 to 52
    put line k of dealt into c
    put (k - 1) mod 8 + 1 into col
    if line col of gCols is empty then put c into line col of gCols
    else put "," & c after line col of gCols
  end repeat
  saveGame
  layout
end newGame

on undoMove
  global gHistory, gMoves
  if gHistory is empty then
    showStatus "Nothing to undo."
    exit undoMove
  end if
  put the number of lines of gHistory into n
  unpackState line n of gHistory
  delete line n of gHistory
  subtract 1 from gMoves
  saveGame
  layout
end undoMove

-- ----- The cards -----

function isCardName c
  if the length of c is not 2 then return false
  return offset(char 1 of c, "A23456789TJQK") > 0 and offset(char 2 of c, "CDHS") > 0
end isCardName

-- Whether c can go on under in a column: one rank lower, the other color.
function fitsOn c, under
  return rankOf(under) = rankOf(c) + 1 and isRed(c) is not isRed(under)
end fitsOn

-- The foundation c can go on, or 0: an ace on an empty one, or the next
-- card of the same suit.
function homeFor c
  global gHomes
  repeat with h = 1 to 4
    put line h of gHomes into pileTop
    if pileTop is empty then put rankOf(c) = 1 into ok
    else put char 2 of pileTop = char 2 of c and rankOf(c) = rankOf(pileTop) + 1 into ok
    if ok then return h
  end repeat
  return 0
end homeFor

-- Where card c is: "col,n,i" (column n, i from the bottom), "cell,n" or
-- "home,n".
function whereIs c
  global gCols, gCells, gHomes
  repeat with n = 1 to 8
    put line n of gCols into pile
    repeat with i = 1 to the number of items of pile
      if item i of pile is c then return "col," & n & "," & i
    end repeat
  end repeat
  repeat with n = 1 to 4
    if line n of gCells is c then return "cell," & n
    if line n of gHomes is c then return "home," & n
  end repeat
  return empty
end whereIs

-- Whether the cards of pile from i up are a run: each one rank lower than
-- the last, alternating colors.
function isRun pile, i
  put the number of items of pile into n
  repeat with k = i to n - 1
    put k + 1 into j
    if not fitsOn(item j of pile, item k of pile) then return false
  end repeat
  return true
end isRun

-- What pressing card c picks up: it and the run on it, or nothing.
function pickUp c, src
  global gCols
  if src is empty or item 1 of src is "home" then return empty
  if item 1 of src is "cell" then return c
  put line (item 2 of src) of gCols into pile
  put item 3 of src into i
  if not isRun(pile, i) then
    showStatus "Only a run (each card one lower, alternating colors) moves together."
    return empty
  end if
  put the number of items of pile into n
  return item i to n of pile
end pickUp

-- How many cards can move at once: one per free cell, plus one, and
-- doubled for each empty column (not counting the one moved to).
function maxMove toEmpty
  global gCols, gCells
  put 0 into freeCells
  repeat with n = 1 to 4
    if line n of gCells is empty then add 1 to freeCells
  end repeat
  put 0 into spaces
  repeat with n = 1 to 8
    if line n of gCols is empty then add 1 to spaces
  end repeat
  if toEmpty and spaces > 0 then subtract 1 from spaces
  return (freeCells + 1) * 2 ^ spaces
end maxMove

-- ----- Moves -----

-- Why moving these cards from src to dest isn't allowed, or empty if it is.
function moveProblem moving, src, dest
  global gCols, gCells
  put the number of items of moving into howMany
  put item 1 of moving into lead
  put item 2 of dest into n
  if item 1 of dest is "home" then
    if howMany > 1 then return "One card at a time goes to a foundation."
    if homeFor(lead) = 0 then return "That card can't go to a foundation yet."
    return empty
  end if
  if item 1 of dest is "cell" then
    if howMany > 1 then return "A free cell holds one card."
    if item 1 of src is "cell" then return "same"
    if firstEmptyCell() = 0 then return "The free cells are full."
    return empty
  end if
  if item 1 of src is "col" and item 2 of src = n then return "same"
  put line n of gCols into pile
  if pile is empty then
    put maxMove(true) into most
    if howMany > most then return "There's room to move" && most && "cards to an empty column."
    return empty
  end if
  if not fitsOn(lead, last item of pile) then return "That card doesn't go there."
  put maxMove(false) into most
  if howMany > most then return "There's room to move" && most && "cards at once."
  return empty
end moveProblem

function firstEmptyCell
  global gCells
  repeat with n = 1 to 4
    if line n of gCells is empty then return n
  end repeat
  return 0
end firstEmptyCell

-- Make a move if it's allowed, then play what's safe to the foundations.
-- Returns why not, or empty.
function tryMove moving, src, dest
  global gHistory, gMoves
  put moveProblem(moving, src, dest) into problem
  if problem is not empty then return problem
  -- A card for the foundations or a free cell goes to whichever fits.
  if item 1 of dest is "home" then put "home," & homeFor(item 1 of moving) into dest
  if item 1 of dest is "cell" then put "cell," & firstEmptyCell() into dest
  put the number of lines of gHistory + 1 into n
  put packState() into line n of gHistory
  add 1 to gMoves
  moveCards moving, src, dest
  layout
  autoPlay
  saveGame
  return empty
end tryMove

on moveCards moving, src, dest
  global gCols, gCells, gHomes
  if item 1 of src is "col" then
    put item 2 of src into n
    put item 3 of src into i
    if i = 1 then put empty into line n of gCols
    else
      put i - 1 into j
      put item 1 to j of line n of gCols into line n of gCols
    end if
  else
    put empty into line (item 2 of src) of gCells
  end if
  put item 2 of dest into n
  if item 1 of dest is "col" then
    if line n of gCols is empty then put moving into line n of gCols
    else put "," & moving after line n of gCols
  else if item 1 of dest is "cell" then
    put moving into line n of gCells
  else
    put moving into line n of gHomes
  end if
end moveCards

-- Whether a card can go to its foundation without being missed: low
-- cards always, others once both foundations of the other color are up
-- to one below it.
function safeHome c
  global gHomes
  if c is empty then return false
  if homeFor(c) = 0 then return false
  put rankOf(c) into r
  if r <= 2 then return true
  put 0 into ready
  repeat with h = 1 to 4
    put line h of gHomes into pileTop
    if pileTop is not empty and isRed(pileTop) is not isRed(c) and rankOf(pileTop) >= r - 1 then add 1 to ready
  end repeat
  return ready = 2
end safeHome

on autoPlay
  global gCols, gCells
  repeat
    put false into moved
    repeat with n = 1 to 8
      put last item of line n of gCols into c
      if safeHome(c) then
        moveCards c, "col," & n & "," & the number of items of line n of gCols, "home," & homeFor(c)
        put true into moved
      end if
    end repeat
    repeat with n = 1 to 4
      put line n of gCells into c
      if safeHome(c) then
        moveCards c, "cell," & n, "home," & homeFor(c)
        put true into moved
      end if
    end repeat
    if not moved then exit repeat
    wait 6 ticks
    layout
  end repeat
end autoPlay

-- A click without a drag: the card (and any run on it) goes to the best
-- place it can: a foundation, another column, a free cell, an empty column.
function clickMove moving, src
  global gCols
  put item 1 of moving into lead
  if the number of items of moving = 1 and homeFor(lead) > 0 then return tryMove(moving, src, "home,1")
  repeat with n = 1 to 8
    if line n of gCols is not empty then
      if moveProblem(moving, src, "col," & n) is empty then return tryMove(moving, src, "col," & n)
    end if
  end repeat
  if the number of items of moving = 1 and item 1 of src is "col" then
    if firstEmptyCell() > 0 then return tryMove(moving, src, "cell,1")
  end if
  repeat with n = 1 to 8
    if line n of gCols is empty then
      if moveProblem(moving, src, "col," & n) is empty then return tryMove(moving, src, "col," & n)
    end if
  end repeat
  return "That card has nowhere to go."
end clickMove

-- Where cards dropped with their top card's center at pt go.
function dropPlace pt
  put item 1 of pt into x
  put item 2 of pt into y
  put (x - §36) div §96 + 1 into n
  if x < §36 or n > 8 then return empty
  if y < §172 then
    if n <= 4 then return "cell," & n
    return "home," & (n - 4)
  end if
  return "col," & n
end dropPlace

-- ----- Showing the game -----

function slotLeft n
  return §44 + (n - 1) * §96
end slotLeft

-- Put every card where the game says it is, in front of the ones it
-- covers. A long column squeezes up to fit.
on layout
  global gCols, gCells, gHomes
  lock screen
  repeat with n = 1 to 8
    put line n of gCols into pile
    put the number of items of pile into howMany
    put §26 into gap
    if howMany > 1 then put min(§26, (§590 - §182 - §116) div (howMany - 1)) into gap
    repeat with i = 1 to howMany
      placeCard item i of pile, item i of pile, slotLeft(n), §182 + (i - 1) * gap
    end repeat
  end repeat
  repeat with n = 1 to 4
    if line n of gCells is not empty then placeCard line n of gCells, line n of gCells, slotLeft(n), §46
    put line n of gHomes into pileTop
    if pileTop is not empty then
      put char 2 of pileTop into suit
      put rankOf(pileTop) - 1 into below
      repeat with r = 1 to below
        hide card button (char r of "A23456789TJQK" & suit)
      end repeat
      placeCard pileTop, pileTop, slotLeft(n + 4), §46
    end if
  end repeat
  hide field "Banner"
  hide button "Play Again"
  showStatus
  unlock screen
end layout

on showStatus msg
  global gGame, gMoves, gHomes
  if msg is empty then
    put 52 into remaining
    repeat with h = 1 to 4
      subtract rankOf(line h of gHomes) from remaining
    end repeat
    put "Game" && gGame & "   Moves:" && gMoves & "   Cards left:" && remaining into msg
  end if
  put msg into field "Status"
end showStatus

on checkWin
  global gHomes, gGame, gMoves
  repeat with h = 1 to 4
    if rankOf(line h of gHomes) < 13 then exit checkWin
  end repeat
  if gMoves = 1 then put "move" into plural else put "moves" into plural
  showStatus "You won game" && gGame && "in" && gMoves && plural & "!"
  put empty into cardList
  repeat with r = 1 to 13
    repeat with s = 1 to 4
      put char r of "A23456789TJQK" & char s of "CDHS" & "," after cardList
    end repeat
  end repeat
  delete last char of cardList
  wonGame cardList
end checkWin

-- ----- Keeping the game -----

on loadGame
  global gGame, gMoves, gHistory
  unpackState field "State"
  put field "Game" into gGame
  put field "Moves" into gMoves
  put field "History" into gHistory
end loadGame

on saveGame
  global gGame, gMoves, gHistory
  put packState() into field "State"
  put gGame into field "Game"
  put gMoves into field "Moves"
  put gHistory into field "History"
end saveGame

function packState
  global gCols, gCells, gHomes
  return packLines(gCols, 8) & "|" & packLines(gCells, 4) & "|" & packLines(gHomes, 4)
end packState

on unpackState s
  global gCols, gCells, gHomes
  set the itemDelimiter to "|"
  put item 1 of s into cols
  put item 2 of s into cells
  put item 3 of s into homes
  set the itemDelimiter to ","
  put unpackLines(cols, 8) into gCols
  put unpackLines(cells, 4) into gCells
  put unpackLines(homes, 4) into gHomes
end unpackState
''' + DRAG

FREECELL_RULES = (
    'Get all 52 cards onto the four foundations at the top right, each suit from ace up to king. Columns '
    'build down in alternating colors: a red 6 goes on a black 7. Any card can go in an empty column, and '
    'the four free cells at the top left hold one card each. You can move a run at once if the free cells '
    'and empty columns would let you move it a card at a time. Game numbers match Windows FreeCell\'s; of '
    'its first 32,000 deals, only game 11982 can\'t be won.')

# ----- Klondike -----

KLONDIKE_SCRIPT = r'''
-- Klondike, the solitaire most people call Solitaire.
--   gKCols    7 lines, a column each, bottom to top; a card face down
--             has a - before it (-AS)
--   gKStock   the cards left to deal, by commas, the next one last
--   gKWaste   the cards dealt, the top one last
--   gKHomes   4 lines, the top card on each foundation
--   gKHistory a line per move, the game before it, for Undo

on openCard
  global gKCols
  loadGame
  if gKCols is empty then newGame random(32000)
  else layout
  pass openCard
end openCard

-- ----- Dealing -----

-- Deal game n: the shuffle is FreeCell's, so a number always deals the
-- same; the columns get one to seven cards, the top one face up.
on newGame n
  global gKCols, gKStock, gKWaste, gKHomes, gKGame, gKMoves, gKHistory
  put n into gKGame
  put empty into field "Finished"
  put empty into gKCols
  put empty into gKWaste
  put empty into gKHomes
  put empty into gKHistory
  put 0 into gKMoves
  put empty into deck
  repeat with i = 0 to 51
    put i div 4 + 1 into r
    put i mod 4 + 1 into s
    put char r of "A23456789TJQK" & char s of "CDHS" into line i + 1 of deck
  end repeat
  put shuffled(deck, n) into dealt
  put 0 into k
  repeat with row = 1 to 7
    repeat with col = row to 7
      add 1 to k
      put line k of dealt into c
      if col > row then put "-" before c
      if line col of gKCols is empty then put c into line col of gKCols
      else put "," & c after line col of gKCols
    end repeat
  end repeat
  put empty into gKStock
  repeat with j = 52 down to k + 1
    put line j of dealt & "," after gKStock
  end repeat
  delete last char of gKStock
  saveGame
  layout
end newGame

-- The stock: the next card (or three) to the waste, face up; when it's
-- empty, the waste turns over to be dealt again.
on dealStock
  global gKStock, gKWaste, gKHistory, gKMoves
  if gKStock is empty and gKWaste is empty then exit dealStock
  put the number of lines of gKHistory + 1 into n
  put packState() into line n of gKHistory
  add 1 to gKMoves
  if gKStock is empty then
    repeat with k = the number of items of gKWaste down to 1
      put item k of gKWaste & "," after gKStock
    end repeat
    delete last char of gKStock
    put empty into gKWaste
  else
    repeat drawCount() times
      if gKStock is empty then exit repeat
      put last item of gKStock into c
      delete last item of gKStock
      if gKWaste is empty then put c into gKWaste
      else put "," & c after gKWaste
    end repeat
  end if
  saveGame
  layout
end dealStock

on mouseUp
  if the short name of the target is "Stock" then dealStock
  else pass mouseUp
end mouseUp

on undoMove
  global gKHistory, gKMoves
  if gKHistory is empty then
    showStatus "Nothing to undo."
    exit undoMove
  end if
  put the number of lines of gKHistory into n
  unpackState line n of gKHistory
  delete line n of gKHistory
  subtract 1 from gKMoves
  saveGame
  layout
end undoMove

-- ----- The cards -----

function isCardName c
  if the length of c is not 2 then return false
  return offset(char 1 of c, "A23456789TJQK") > 0 and offset(char 2 of c, "CDHS") > 0
end isCardName

function fitsOn c, under
  return rankOf(under) = rankOf(c) + 1 and isRed(c) is not isRed(under)
end fitsOn

function homeFor c
  global gKHomes
  repeat with h = 1 to 4
    put line h of gKHomes into pileTop
    if pileTop is empty then put rankOf(c) = 1 into ok
    else put char 2 of pileTop = char 2 of c and rankOf(c) = rankOf(pileTop) + 1 into ok
    if ok then return h
  end repeat
  return 0
end homeFor

-- Where card c is: "col,n,i", "waste", "home,n" or "stock".
function whereIs c
  global gKCols, gKWaste, gKHomes, gKStock
  repeat with n = 1 to 7
    put line n of gKCols into pile
    repeat with i = 1 to the number of items of pile
      if item i of pile is c or item i of pile is "-" & c then return "col," & n & "," & i
    end repeat
  end repeat
  if "," & c & "," is in "," & gKWaste & "," then return "waste"
  repeat with n = 1 to 4
    if line n of gKHomes is c then return "home," & n
  end repeat
  return "stock"
end whereIs

-- What pressing card c picks up: it and the cards on it in a column, the
-- top of the waste, or a foundation's top card.
function pickUp c, src
  global gKCols, gKWaste
  if item 1 of src is "waste" then
    if c is last item of gKWaste then return c
    return empty
  end if
  if item 1 of src is "home" then return c
  if item 1 of src is not "col" then return empty
  put line (item 2 of src) of gKCols into pile
  put item 3 of src into i
  if char 1 of item i of pile is "-" then return empty
  put the number of items of pile into n
  return item i to n of pile
end pickUp

-- ----- Moves -----

function moveProblem moving, src, dest
  global gKCols
  put item 1 of moving into lead
  put item 2 of dest into n
  if item 1 of dest is "home" then
    if the number of items of moving > 1 then return "One card at a time goes to a foundation."
    if item 1 of src is "home" then return "same"
    if homeFor(lead) = 0 then return "That card can't go to a foundation yet."
    return empty
  end if
  if item 1 of dest is not "col" then return "same"
  if item 1 of src is "col" and item 2 of src = n then return "same"
  put line n of gKCols into pile
  if pile is empty then
    if rankOf(lead) is not 13 then return "Only a king can go in an empty column."
    return empty
  end if
  if not fitsOn(lead, last item of pile) then return "That card doesn't go there."
  return empty
end moveProblem

function tryMove moving, src, dest
  global gKHistory, gKMoves
  put moveProblem(moving, src, dest) into problem
  if problem is not empty then return problem
  if item 1 of dest is "home" then put "home," & homeFor(item 1 of moving) into dest
  put the number of lines of gKHistory + 1 into n
  put packState() into line n of gKHistory
  add 1 to gKMoves
  moveCards moving, src, dest
  layout
  autoPlay
  saveGame
  return empty
end tryMove

-- Move cards, turning up the card they uncover.
on moveCards moving, src, dest
  global gKCols, gKWaste, gKHomes
  if item 1 of src is "col" then
    put item 2 of src into n
    put item 3 of src into i
    if i = 1 then put empty into line n of gKCols
    else
      put i - 1 into j
      put item 1 to j of line n of gKCols into line n of gKCols
      put last item of line n of gKCols into under
      if char 1 of under is "-" then put char 2 to 3 of under into last item of line n of gKCols
    end if
  else if item 1 of src is "waste" then
    delete last item of gKWaste
  else if item 1 of src is "home" then
    put item 2 of src into h
    put line h of gKHomes into c
    if rankOf(c) = 1 then put empty into line h of gKHomes
    else put char (rankOf(c) - 1) of "A23456789TJQK" & char 2 of c into line h of gKHomes
  end if
  put item 2 of dest into n
  if item 1 of dest is "col" then
    if line n of gKCols is empty then put moving into line n of gKCols
    else put "," & moving after line n of gKCols
  else
    put moving into line n of gKHomes
  end if
end moveCards

-- As in FreeCell: low cards, and others once both foundations of the
-- other color are up to one below.
function safeHome c
  global gKHomes
  if c is empty or char 1 of c is "-" then return false
  if homeFor(c) = 0 then return false
  put rankOf(c) into r
  if r <= 2 then return true
  put 0 into ready
  repeat with h = 1 to 4
    put line h of gKHomes into pileTop
    if pileTop is not empty and isRed(pileTop) is not isRed(c) and rankOf(pileTop) >= r - 1 then add 1 to ready
  end repeat
  return ready = 2
end safeHome

on autoPlay
  global gKCols, gKWaste
  repeat
    put false into moved
    repeat with n = 1 to 7
      put last item of line n of gKCols into c
      if safeHome(c) then
        moveCards c, "col," & n & "," & the number of items of line n of gKCols, "home," & homeFor(c)
        put true into moved
      end if
    end repeat
    put last item of gKWaste into c
    if safeHome(c) then
      moveCards c, "waste", "home," & homeFor(c)
      put true into moved
    end if
    if not moved then exit repeat
    wait 6 ticks
    layout
  end repeat
end autoPlay

-- A click: to a foundation if it can, else onto a column, else a king to
-- an empty column.
function clickMove moving, src
  global gKCols
  put item 1 of moving into lead
  if the number of items of moving = 1 and item 1 of src is not "home" and homeFor(lead) > 0 then
    return tryMove(moving, src, "home,1")
  end if
  repeat with n = 1 to 7
    if line n of gKCols is not empty then
      if moveProblem(moving, src, "col," & n) is empty then return tryMove(moving, src, "col," & n)
    end if
  end repeat
  repeat with n = 1 to 7
    if line n of gKCols is empty then
      if moveProblem(moving, src, "col," & n) is empty then return tryMove(moving, src, "col," & n)
    end if
  end repeat
  return "That card has nowhere to go."
end clickMove

function dropPlace pt
  put item 1 of pt into x
  put item 2 of pt into y
  put (x - §84) div §96 + 1 into n
  if x < §84 or n > 7 then return empty
  if y < §172 then
    if n >= 4 then return "home," & (n - 3)
    return empty
  end if
  return "col," & n
end dropPlace

-- ----- Showing the game -----

function slotLeft n
  return §92 + (n - 1) * §96
end slotLeft

on layout
  global gKCols, gKStock, gKWaste, gKHomes
  lock screen
  repeat with k = 1 to the number of items of gKStock
    hide card button (item k of gKStock)
  end repeat
  if gKStock is empty then hide button "Stock" else show button "Stock"
  -- The waste: its top card, or its top three fanned when dealing three.
  put the number of items of gKWaste into n
  put n - drawCount() + 1 into firstShown
  if firstShown < 1 then put 1 into firstShown
  repeat with k = 1 to n
    put item k of gKWaste into c
    if k < firstShown then hide card button c
    else placeCard c, c, slotLeft(2) + (k - firstShown) * §16, §46
  end repeat
  -- The columns: cards face down close together, face up further apart,
  -- all squeezed up if a column runs long.
  repeat with col = 1 to 7
    put line col of gKCols into pile
    put 0 into downs
    put 0 into ups
    repeat with i = 1 to the number of items of pile
      if char 1 of item i of pile is "-" then add 1 to downs else add 1 to ups
    end repeat
    put downs * §10 + max(0, ups - 1) * §26 into wanted
    put 1 into scale
    if wanted > §292 then put §292 / wanted into scale
    put §182 into y
    repeat with i = 1 to the number of items of pile
      put item i of pile into c
      if char 1 of c is "-" then
        put char 2 to 3 of c into c
        placeCard c, "Back", slotLeft(col), y
        add round(§10 * scale) to y
      else
        placeCard c, c, slotLeft(col), y
        add round(§26 * scale) to y
      end if
    end repeat
  end repeat
  repeat with h = 1 to 4
    put line h of gKHomes into pileTop
    if pileTop is not empty then
      put char 2 of pileTop into suit
      put rankOf(pileTop) - 1 into below
      repeat with r = 1 to below
        hide card button (char r of "A23456789TJQK" & suit)
      end repeat
      placeCard pileTop, pileTop, slotLeft(h + 3), §46
    end if
  end repeat
  hide field "Banner"
  hide button "Play Again"
  showStatus
  unlock screen
end layout

on showStatus msg
  global gKGame, gKMoves, gKStock
  if msg is empty then
    put "Game" && gKGame & "   Moves:" && gKMoves & "   Stock:" && the number of items of gKStock into msg
  end if
  put msg into field "Status"
end showStatus

on checkWin
  global gKHomes, gKGame, gKMoves
  repeat with h = 1 to 4
    if rankOf(line h of gKHomes) < 13 then exit checkWin
  end repeat
  showStatus "You won game" && gKGame && "in" && gKMoves && "moves!"
  put empty into cardList
  repeat with r = 1 to 13
    repeat with s = 1 to 4
      put char r of "A23456789TJQK" & char s of "CDHS" & "," after cardList
    end repeat
  end repeat
  delete last char of cardList
  wonGame cardList
end checkWin

-- Draw Three, checked, deals three cards at a time.
function drawCount
  if the hilite of button "Draw Three" then return 3
  return 1
end drawCount

-- ----- Keeping the game -----

on loadGame
  global gKGame, gKMoves, gKHistory
  unpackState field "State"
  put field "Game" into gKGame
  put field "Moves" into gKMoves
  put field "History" into gKHistory
end loadGame

on saveGame
  global gKGame, gKMoves, gKHistory
  put packState() into field "State"
  put gKGame into field "Game"
  put gKMoves into field "Moves"
  put gKHistory into field "History"
end saveGame

function packState
  global gKCols, gKStock, gKWaste, gKHomes
  return packLines(gKCols, 7) & "|" & gKStock & "|" & gKWaste & "|" & packLines(gKHomes, 4)
end packState

on unpackState s
  global gKCols, gKStock, gKWaste, gKHomes
  set the itemDelimiter to "|"
  put item 1 of s into cols
  put item 2 of s into gKStock
  put item 3 of s into gKWaste
  put item 4 of s into homes
  set the itemDelimiter to ","
  put unpackLines(cols, 7) into gKCols
  put unpackLines(homes, 4) into gKHomes
end unpackState
''' + DRAG

KLONDIKE_RULES = (
    'Get all 52 cards onto the four foundations at the top right, each suit from ace up to king. Columns '
    'build down in alternating colors, and only a king can go in an empty column; turning a card over is '
    'automatic. Click the stock at the top left to deal a card (or three, with Draw Three) onto the waste; '
    'when it runs out, click again to turn the waste over. You can move a face-up card with the cards on it.')

# ----- Spider -----

SPIDER_SCRIPT = r'''
-- Spider: two decks, in one, two or four suits.
--   gSCols    10 lines, a column each, bottom to top; each card is a
--             button s1 to s104, with a - before it while face down
--   gSFaces   104 lines: the face each button shows (AS, TH)
--   gSStock   the buttons left to deal, by commas
--   gSDone    the kings of the runs taken off, by commas
--   gSHistory a line per move, the game before it, for Undo

on openCard
  global gSCols
  loadGame
  if gSCols is empty then newGame random(32000)
  else layout
  pass openCard
end openCard

-- ----- Dealing -----

-- Deal game n: 104 cards in the suits chosen, shuffled by number; ten
-- columns, the first four of six cards and the rest of five, the top one
-- face up; fifty left in the stock.
on newGame n
  global gSCols, gSFaces, gSStock, gSDone, gSGame, gSMoves, gSHistory
  put n into gSGame
  put empty into field "Finished"
  put empty into gSCols
  put empty into gSDone
  put empty into gSHistory
  put 0 into gSMoves
  put item suitCount() of "S,SH,,CDHS" into suits
  put empty into deck
  repeat with deckNo = 1 to 8 div the length of suits
    repeat with s = 1 to the length of suits
      repeat with r = 1 to 13
        put char r of "A23456789TJQK" & char s of suits & return after deck
      end repeat
    end repeat
  end repeat
  delete last char of deck
  put shuffled(deck, n) into gSFaces
  put 0 into k
  repeat with col = 1 to 10
    if col <= 4 then put 6 into tall else put 5 into tall
    repeat with i = 1 to tall
      add 1 to k
      put "s" & k into c
      if i < tall then put "-" before c
      if line col of gSCols is empty then put c into line col of gSCols
      else put "," & c after line col of gSCols
    end repeat
  end repeat
  put empty into gSStock
  repeat with j = k + 1 to 104
    put "s" & j & "," after gSStock
  end repeat
  delete last char of gSStock
  saveGame
  layout
end newGame

-- The stock deals a card face up onto every column, which must all have
-- cards.
on dealStock
  global gSCols, gSStock, gSHistory, gSMoves
  if gSStock is empty then exit dealStock
  repeat with n = 1 to 10
    if line n of gSCols is empty then
      showStatus "Every column needs a card before the stock deals."
      exit dealStock
    end if
  end repeat
  put the number of lines of gSHistory + 1 into h
  put packState() into line h of gSHistory
  add 1 to gSMoves
  repeat with n = 1 to 10
    put item 1 of gSStock into c
    delete item 1 of gSStock
    put "," & c after line n of gSCols
  end repeat
  layout
  takeRuns
  saveGame
  layout
end dealStock

on mouseUp
  if the short name of the target is "Stock" then dealStock
  else pass mouseUp
end mouseUp

on undoMove
  global gSHistory, gSMoves
  if gSHistory is empty then
    showStatus "Nothing to undo."
    exit undoMove
  end if
  put the number of lines of gSHistory into n
  unpackState line n of gSHistory
  delete line n of gSHistory
  subtract 1 from gSMoves
  saveGame
  layout
end undoMove

-- ----- The cards -----

function isCardName c
  if char 1 of c is not "s" then return false
  put the length of c into lastChar
  put char 2 to lastChar of c into n
  return n is a number
end isCardName

-- The face a card button shows: AS, TH.
function faceOf b
  global gSFaces
  if char 1 of b is "-" then delete char 1 of b
  put the length of b into lastChar
  return line (char 2 to lastChar of b) of gSFaces
end faceOf

-- Where card b is: "col,n,i", or "stock" for the rest.
function whereIs b
  global gSCols
  repeat with n = 1 to 10
    put line n of gSCols into pile
    repeat with i = 1 to the number of items of pile
      if item i of pile is b or item i of pile is "-" & b then return "col," & n & "," & i
    end repeat
  end repeat
  return "stock"
end whereIs

-- Whether the cards of pile from i up are a run of one suit, each one
-- rank lower than the last.
function isRun pile, i
  put the number of items of pile into n
  repeat with k = i to n - 1
    put faceOf(item k of pile) into a
    put faceOf(item (k + 1) of pile) into c
    if char 1 of item (k + 1) of pile is "-" then return false
    if rankOf(c) is not rankOf(a) - 1 or char 2 of c is not char 2 of a then return false
  end repeat
  return true
end isRun

function pickUp b, src
  global gSCols
  if item 1 of src is not "col" then return empty
  put line (item 2 of src) of gSCols into pile
  put item 3 of src into i
  if char 1 of item i of pile is "-" then return empty
  if not isRun(pile, i) then
    showStatus "Only a run of one suit moves together."
    return empty
  end if
  put the number of items of pile into n
  return item i to n of pile
end pickUp

-- ----- Moves -----

function moveProblem moving, src, dest
  global gSCols
  if item 1 of dest is not "col" then return "same"
  put item 2 of dest into n
  if item 2 of src = n then return "same"
  put line n of gSCols into pile
  if pile is empty then return empty
  if rankOf(faceOf(last item of pile)) is not rankOf(faceOf(item 1 of moving)) + 1 then
    return "That card doesn't go there."
  end if
  return empty
end moveProblem

function tryMove moving, src, dest
  global gSHistory, gSMoves
  put moveProblem(moving, src, dest) into problem
  if problem is not empty then return problem
  put the number of lines of gSHistory + 1 into n
  put packState() into line n of gSHistory
  add 1 to gSMoves
  moveCards moving, src, dest
  layout
  takeRuns
  saveGame
  return empty
end tryMove

-- Move cards, turning up the card they uncover.
on moveCards moving, src, dest
  global gSCols
  put item 2 of src into n
  put item 3 of src into i
  if i = 1 then put empty into line n of gSCols
  else
    put i - 1 into j
    put item 1 to j of line n of gSCols into line n of gSCols
    turnUp n
  end if
  put item 2 of dest into n
  if line n of gSCols is empty then put moving into line n of gSCols
  else put "," & moving after line n of gSCols
end moveCards

on turnUp n
  global gSCols
  put last item of line n of gSCols into under
  if char 1 of under is "-" then
    delete char 1 of under
    put under into last item of line n of gSCols
  end if
end turnUp

-- A run from king to ace of one suit comes off the table.
on takeRuns
  global gSCols, gSDone
  repeat with n = 1 to 10
    put line n of gSCols into pile
    put the number of items of pile into howMany
    if howMany >= 13 then
      put howMany - 12 into i
      if rankOf(faceOf(item i of pile)) = 13 and isRun(pile, i) and char 1 of item i of pile is not "-" then
        if gSDone is empty then put item i of pile into gSDone
        else put "," & item i of pile after gSDone
        repeat with k = i + 1 to howMany
          hide card button (item k of pile)
        end repeat
        if i = 1 then put empty into line n of gSCols
        else
          put i - 1 into j
          put item 1 to j of pile into line n of gSCols
          turnUp n
        end if
        wait 10 ticks
        layout
      end if
    end if
  end repeat
end takeRuns

-- A click: onto a card one rank higher of the same suit, else of any
-- suit, else to an empty column.
function clickMove moving, src
  global gSCols
  put faceOf(item 1 of moving) into lead
  repeat with tier = 1 to 3
    repeat with n = 1 to 10
      put line n of gSCols into pile
      if tier < 3 and pile is empty then next repeat
      if tier = 3 and pile is not empty then next repeat
      if tier = 1 and char 2 of faceOf(last item of pile) is not char 2 of lead then next repeat
      if moveProblem(moving, src, "col," & n) is empty then return tryMove(moving, src, "col," & n)
    end repeat
  end repeat
  return "That card has nowhere to go."
end clickMove

function dropPlace pt
  put item 1 of pt into x
  put (x - §10) div §82 + 1 into n
  if x < §10 or n > 10 then return empty
  return "col," & n
end dropPlace

-- ----- Showing the game -----

function slotLeft n
  return §11 + (n - 1) * §82
end slotLeft

on layout
  global gSCols, gSStock, gSDone
  lock screen
  repeat with k = 1 to the number of items of gSStock
    hide card button (item k of gSStock)
  end repeat
  if gSStock is empty then hide button "Stock" else show button "Stock"
  repeat with col = 1 to 10
    put line col of gSCols into pile
    put 0 into downs
    put 0 into ups
    repeat with i = 1 to the number of items of pile
      if char 1 of item i of pile is "-" then add 1 to downs else add 1 to ups
    end repeat
    put downs * §8 + max(0, ups - 1) * §24 into wanted
    put 1 into scale
    if wanted > §300 then put §300 / wanted into scale
    put §46 into y
    repeat with i = 1 to the number of items of pile
      put item i of pile into b
      if char 1 of b is "-" then
        delete char 1 of b
        placeCard b, "Back", slotLeft(col), y
        add round(8 * scale) to y
      else
        placeCard b, faceOf(b), slotLeft(col), y
        add round(§24 * scale) to y
      end if
    end repeat
  end repeat
  -- Each run taken off shows its king at the bottom left.
  repeat with k = 1 to the number of items of gSDone
    put item k of gSDone into b
    placeCard b, faceOf(b), §11 + (k - 1) * §24, §476
  end repeat
  hide field "Banner"
  hide button "Play Again"
  showStatus
  unlock screen
end layout

on showStatus msg
  global gSGame, gSMoves, gSStock, gSDone
  if msg is empty then
    put "Game" && gSGame & "   Moves:" && gSMoves & "   Deals:" && the number of items of gSStock div 10 into msg
    put "   Runs:" && the number of items of gSDone after msg
  end if
  put msg into field "Status"
end showStatus

on checkWin
  global gSDone, gSGame, gSMoves
  if the number of items of gSDone < 8 then exit checkWin
  showStatus "You won game" && gSGame && "in" && gSMoves && "moves!"
  put empty into cardList
  repeat with k = 1 to 104
    put "s" & k into b
    set the icon of card button b to faceOf(b)
    put b & "," after cardList
  end repeat
  delete last char of cardList
  wonGame cardList
end checkWin

-- How many suits new games deal: the radio button chosen.
function suitCount
  if the hilite of button "Two Suits" then return 2
  if the hilite of button "Four Suits" then return 4
  return 1
end suitCount

-- ----- Keeping the game -----

on loadGame
  global gSGame, gSMoves, gSHistory, gSFaces
  unpackState field "State"
  put field "Game" into gSGame
  put field "Moves" into gSMoves
  put field "History" into gSHistory
  put field "Faces" into gSFaces
end loadGame

on saveGame
  global gSGame, gSMoves, gSHistory, gSFaces
  put packState() into field "State"
  put gSGame into field "Game"
  put gSMoves into field "Moves"
  put gSHistory into field "History"
  put gSFaces into field "Faces"
end saveGame

function packState
  global gSCols, gSStock, gSDone
  return packLines(gSCols, 10) & "|" & gSStock & "|" & gSDone
end packState

on unpackState s
  global gSCols, gSStock, gSDone
  set the itemDelimiter to "|"
  put item 1 of s into cols
  put item 2 of s into gSStock
  put item 3 of s into gSDone
  set the itemDelimiter to ","
  put unpackLines(cols, 10) into gSCols
end unpackState
''' + DRAG

SPIDER_RULES = (
    'Build eight runs from king down to ace, each of one suit; a finished run comes off the table on its '
    'own. Any card can go on a card one rank higher, whatever its suit, but only a run of one suit moves '
    'together. Any card or run can go in an empty column. Click the stock at the bottom right to deal a card '
    'onto every column, which all need a card first. Choose one, two or four suits for new games.')


STACK_SCRIPT, FREECELL_SCRIPT, KLONDIKE_SCRIPT, SPIDER_SCRIPT = (
    scaled(STACK_SCRIPT), scaled(FREECELL_SCRIPT), scaled(KLONDIKE_SCRIPT), scaled(SPIDER_SCRIPT))


def ask_game(abandon):
    return r'''
on mouseUp
  global ''' + abandon + r'''
  ask "Play which game? (1 to 1000000)" with ''' + abandon.replace('Moves', 'Game') + r'''
  if it is empty then exit mouseUp
  if it is not a number or it < 1 or it > 1000000 or it is not trunc(it) then
    answer "Games are numbered from 1 to 1000000."
    exit mouseUp
  end if
  put it into n
  abandonGame ''' + abandon + r'''
  newGame n
end mouseUp
'''


def build():
    global next_id
    next_id = 1
    table = {'id': new_id(), 'name': 'Table', 'script': '', 'picture': picture(), 'show_pict': True,
             'parts': [], 'colors': [{'kind': 'rect', 'rect': rect(0, 0, WIDTH, HEIGHT), 'color': FELT,
                                      'bevel': 0}]}
    plain = {'id': new_id(), 'name': 'Plain', 'script': '', 'picture': picture(), 'show_pict': True,
             'parts': []}
    cards = []
    icons = {}

    def card(name, bg, script=''):
        c = {'id': new_id(), 'name': name, 'script': script.strip('\n'), 'background': bg['id'],
             'picture': picture(), 'show_pict': True, 'parts': []}
        cards.append(c)
        return c['parts']

    faces = [r + s for r in RANKS for s in SUITS]
    for i, code in enumerate(faces):
        icons[str(40001 + i)] = {'name': code, 'picture': icon_picture(face_file(code))}
    icons[str(BACK_ICON)] = {'name': 'Back', 'picture': card_back()}
    slot = dict(show_name=False, auto_hilite=False, back_color=SLOT, border_color=SLOT_EDGE)

    def game_card(name, script, rules, moves_global, game_global, options=()):
        """A game's card: its bar of buttons, its status, its hidden fields
        for keeping the game and its record."""
        p = card(name, table, script)
        bar = [
            ('New Game', 24, 112, f'on mouseUp\n  global {moves_global}\n  abandonGame {moves_global}\n  newGame random(32000)\nend mouseUp'),
            ('Game #…', 118, 196, ask_game(moves_global)),
            ('Restart', 202, 270, f'on mouseUp\n  global {game_global}, {moves_global}\n  abandonGame {moves_global}\n  newGame {game_global}\nend mouseUp'),
            ('Undo', 276, 334, 'on mouseUp\n  undoMove\nend mouseUp'),
            ('Rules', 340, 398, 'on mouseUp\n  answer field "Rules"\nend mouseUp'),
            ('Games', 404, 468, 'on mouseUp\n  visual effect dissolve fast\n  go to card "Games"\nend mouseUp'),
        ]
        for bname, l, r, s in bar:
            p.append(button(bname, (l, 10, r, 34), s, text_size=12))
        p.extend(options)
        # The status gets the bar's room that an option in it doesn't use.
        status_left = 590 if any(o['rect']['top'] < 40 for o in options) else 478
        p.append(field('Status', (status_left, 12, 816, 32), '', 12, align='Right', bold=True, fore_color=WHITE,
                       dont_save=True))
        for fname in ('State', 'Game', 'Moves', 'History', 'Finished', 'Faces'):
            p.append(field(fname, (0, 0, 10, 10), '', 9, visible=False))
        for fname in ('Won', 'Lost', 'Streak', 'Best'):
            p.append(field(fname, (0, 0, 10, 10), '0', 9, visible=False))
        p.append(field('Rules', (0, 0, 10, 10), rules, 9, visible=False))
        return p

    def finish(p, buttons):
        """The cards, then the win banner in front of them."""
        for b, icon in buttons:
            p.append(button(b, card_rect(sc(24), sc(182)), '', 'Transparent', icon=icon,
                            show_name=False, auto_hilite=False, visible=False))
        p.append(field('Banner', (290, 284, 550, 364), '', 18, style='Rectangle', align='Center', bold=True,
                       back_color=[255, 250, 230], visible=False, dont_save=True))
        p.append(button('Play Again', (360, 372, 480, 398), 'on mouseUp\n  newGame random(32000)\nend mouseUp',
                        visible=False))

    # ----- Games (the contents) -----
    p = card('Games', plain)
    p.append(label('Title', (40, 24, 800, 84), 'Solitaire', 44, align='Center', bold=True))
    p.append(label('Subtitle', (40, 86, 800, 110), 'Card games for one, made with RandomCard', 15,
                   align='Center', text_style='italic'))
    games = [
        ('FreeCell', 'Every card dealt face up, four free cells, and nearly every deal can be won. Numbered '
                     'deals match Windows FreeCell\'s.'),
        ('Klondike', 'The solitaire most people call Solitaire: seven columns, face-down cards to uncover, and '
                     'the stock dealt one or three at a time.'),
        ('Spider', 'Two decks in ten columns: build runs from king to ace, in one, two or four suits. One suit '
                   'is gentle; four is hard.'),
    ]
    for i, (name, text) in enumerate(games):
        y = 140 + i * 110
        p.append(button(name, (60, y, 240, y + 40),
                        f'on mouseUp\n  visual effect dissolve fast\n  go to card "{name}"\nend mouseUp',
                        text_size=16, text_style='bold', back_color=FELT, fore_color=WHITE, border_color=FELT))
        p.append(label(f'{name} Record', (60, y + 46, 240, y + 90), 'No games yet.', 12, align='Center'))
        p.append(label(f'{name} About', (270, y + 4, 780, y + 90), text, 13))
    p.append(button('About', (60, 540, 180, 570), 'on mouseUp\n  go to card "About"\nend mouseUp'))

    # ----- FreeCell -----
    p = game_card('FreeCell', FREECELL_SCRIPT, FREECELL_RULES, 'gMoves', 'gGame')
    for n in range(4):
        p.append(button(f'Cell {n + 1}', card_rect(sc(44) + n * sc(96), sc(46)), '', 'RoundRect', **slot))
        p.append(button(f'Foundation {n + 1}', card_rect(sc(44) + (n + 4) * sc(96), sc(46)), '', 'RoundRect',
                        **slot))
    for n in range(8):
        p.append(button(f'Column {n + 1}', card_rect(sc(44) + n * sc(96), sc(182)), '', 'RoundRect', **slot))
    finish(p, [(code, 40001 + i) for i, code in enumerate(faces)])

    # ----- Klondike -----
    draw3 = button('Draw Three', (478, 13, 584, 31), 'on mouseUp\n  layout\nend mouseUp', 'CheckBox',
                   fore_color=WHITE, text_size=12, auto_hilite=True)
    p = game_card('Klondike', KLONDIKE_SCRIPT, KLONDIKE_RULES, 'gKMoves', 'gKGame', [draw3])
    p.append(button('Stock Place', card_rect(sc(92), sc(46)), 'on mouseUp\n  dealStock\nend mouseUp',
                    'RoundRect', **slot))
    for n in range(4):
        p.append(button(f'Foundation {n + 1}', card_rect(sc(92) + (n + 3) * sc(96), sc(46)), '', 'RoundRect',
                        **slot))
    for n in range(7):
        p.append(button(f'Column {n + 1}', card_rect(sc(92) + n * sc(96), sc(182)), '', 'RoundRect', **slot))
    p.append(button('Stock', card_rect(sc(92), sc(46)), '', 'Transparent', icon=BACK_ICON,
                    show_name=False, auto_hilite=False))
    finish(p, [(code, 40001 + i) for i, code in enumerate(faces)])

    # ----- Spider -----
    radios = []
    for i, (rname, n) in enumerate([('One Suit', 1), ('Two Suits', 2), ('Four Suits', 4)]):
        x = 300 + i * 110
        radios.append(button(rname, (x, 570, x + 104, 590), '', 'Radio', family=1, fore_color=WHITE,
                             text_size=12, auto_hilite=True, hilite=(n == 1)))
    p = game_card('Spider', SPIDER_SCRIPT, SPIDER_RULES, 'gSMoves', 'gSGame', radios)
    for n in range(10):
        p.append(button(f'Column {n + 1}', card_rect(sc(11) + n * sc(82), sc(46)), '', 'RoundRect', **slot))
    p.append(button('Stock', card_rect(sc(WIDTH) - sc(11) - PX_W, sc(476)), '', 'Transparent', icon=BACK_ICON,
                    show_name=False, auto_hilite=False))
    finish(p, [(f's{k}', BACK_ICON) for k in range(1, 105)])

    # ----- About -----
    p = card('About', plain)
    p.append(label('Title', (40, 30, 800, 70), 'About Solitaire', 28, bold=True))
    p.append(label('Credits', (40, 80, 800, 220),
                   'The card faces are Byron Knoll\'s Vector Playing Cards (version 1.3), which he released into '
                   'the public domain. The back follows Bas de Reuver\'s (BdRGames) design 1, CC0.\n\n'
                   'FreeCell\'s numbered deals follow the ones Jim Horne wrote for Windows FreeCell, so game 1 '
                   'here is the same deal as game 1 there. Klondike and Spider shuffle the same way, so their '
                   'game numbers deal the same each time too.', 13))
    p.append(label('Shows Label', (40, 234, 800, 256), 'What Solitaire shows about RandomCard', 15, bold=True))
    p.append(field('Shows', (40, 260, 800, 500),
                   'Each card is a button whose icon is the card\'s face, or its back: icons of any size, in '
                   'color, changed by the script as cards turn over.\n'
                   'Dragging is HyperTalk: repeat while the mouse is down, moving the cards to the mouseLoc, '
                   'and setting their partNumber to bring them to the front.\n'
                   'Each game\'s rules are its card\'s own script, in plain HyperTalk, and what the games share '
                   '(the shuffle, the record, the win) is in the stack script: choose Objects ▸ Card Script or '
                   'Stack Script to read them.\n'
                   'A game in progress is kept in hidden fields, so it\'s saved with the stack, and Undo goes '
                   'back to the start of the deal.', 13, style='Scrolling'))
    p.append(button('Games', (40, 540, 160, 570), 'on mouseUp\n  go to card "Games"\nend mouseUp'))

    stack = {'format': 1, 'name': 'Solitaire', 'script': STACK_SCRIPT.strip('\n'), 'width': sc(WIDTH),
             'height': sc(HEIGHT), 'next_id': next_id, 'backgrounds': [table, plain], 'cards': cards,
             'icons': icons}
    return stack


if __name__ == '__main__':
    stack = build()
    with open('examples/Solitaire.rcstack', 'w') as f:
        json.dump(stack, f, indent=2, ensure_ascii=False)
    print('wrote examples/Solitaire.rcstack:', len(stack['cards']), 'cards,', len(stack['icons']), 'icons')
