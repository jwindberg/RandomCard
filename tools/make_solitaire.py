"""Write examples/Solitaire.rcstack: solitaire card games, starting with
FreeCell.

usage: python3 -I tools/make_solitaire.py   (from the repository's top folder)

The card faces are Byron Knoll's Vector Playing Cards 1.3 (public domain),
read from reference/Playing Cards/PNG-cards-1.3 and made into color icons.
The games' scripts are here, readable; in the stack, choose Objects ▸ Card
Script on a game's card.
"""
import base64
import io
import json
import os

from PIL import Image

WIDTH, HEIGHT = 800, 600
CARD_W, CARD_H = 80, 116
FACES = 'reference/Playing Cards/PNG-cards-1.3'
FELT = [21, 96, 54]
SLOT = [16, 78, 43]
SLOT_EDGE = [96, 160, 116]
next_id = 1


def new_id():
    global next_id
    next_id += 1
    return next_id - 1


def rect(l, t, r, b):
    return {'left': l, 'top': t, 'right': r, 'bottom': b}


def button(name, r, script='', style='RoundRect', **extra):
    p = {'id': new_id(), 'kind': 'Button', 'name': name, 'rect': rect(*r), 'script': script.strip('\n'),
         'style': style}
    p.update(extra)
    return p


def field(name, r, text='', size=12, style='Transparent', lock=True, align='Left', bold=False, **extra):
    p = {'id': new_id(), 'kind': 'Field', 'name': name, 'rect': rect(*r), 'script': '', 'style': style,
         'contents': text, 'text_size': size, 'text_style': 'bold' if bold else 'plain', 'text_align': align,
         'lock_text': lock, 'dont_wrap': False}
    p.update(extra)
    return p


def label(name, r, text, size=12, **extra):
    return field(name, r, text, size, **extra)


def picture():
    return {'width': WIDTH, 'height': HEIGHT, 'data': ''}


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


def icon_picture(path):
    im = Image.open(path).convert('RGBA').resize((CARD_W, CARD_H), Image.LANCZOS)
    out = io.BytesIO()
    im.save(out, 'PNG', optimize=True)
    return {'width': CARD_W, 'height': CARD_H, 'data': '', 'png': base64.b64encode(out.getvalue()).decode()}


FREECELL_SCRIPT = r'''
-- FreeCell, with the same numbered deals as Windows FreeCell: game 1 here
-- is game 1 there.
--
-- Each playing card is a button named for its card (AS, TH, 2C: rank,
-- then suit), whose icon is the card's face. While you play, the game is
-- in globals; it's also kept in hidden fields, so a game in progress is
-- saved with the stack:
--   gCols    8 lines, a column each, its cards bottom to top by commas
--   gCells   4 lines, the free cells
--   gHomes   4 lines, the top card on each foundation
--   gHistory a line per move, the state before it, for Undo

on openCard
  global gCols
  loadGame
  if gCols is empty then newGame random(32000)
  else layout
  pass openCard
end openCard

-- ----- Dealing -----

-- Deal game n as Windows FreeCell does: its random numbers, its order of
-- the deck (aces first, clubs diamonds hearts spades), and its shuffle.
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
  put n into seed
  repeat with remaining = 52 down to 1
    put (214013 * seed + 2531011) mod 2147483648 into seed
    put (seed div 65536) mod remaining + 1 into k
    put line k of deck into c
    put line remaining of deck into line k of deck
    delete line remaining of deck
    put (52 - remaining) mod 8 + 1 into col
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

function rankOf c
  if c is empty then return 0
  return offset(char 1 of c, "A23456789TJQK")
end rankOf

function isRed c
  return char 2 of c is in "DH"
end isRed

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
  put (x - 16) div 96 + 1 into n
  if x < 16 or n > 8 then return empty
  if y < 172 then
    if n <= 4 then return "cell," & n
    return "home," & (n - 4)
  end if
  return "col," & n
end dropPlace

-- ----- The mouse -----

-- Press a card and drag it, with any run on it, to where it goes; or just
-- click it to send it to the best place.
on mouseDown
  global gCols
  put the short name of the target into c
  if not isCardName(c) then pass mouseDown
  put whereIs(c) into src
  if src is empty or item 1 of src is "home" then exit mouseDown
  if item 1 of src is "col" then
    put line (item 2 of src) of gCols into pile
    put item 3 of src into i
    if not isRun(pile, i) then
      showStatus "Only a run (each card one lower, alternating colors) moves together."
      exit mouseDown
    end if
    put the number of items of pile into n
    put item i to n of pile into moving
  else
    put c into moving
  end if
  -- Pick the cards up: in front, keeping where each sits from the mouse.
  put the number of items of moving into howMany
  put empty into starts
  repeat with k = 1 to howMany
    put item k of moving into m
    set the partNumber of card button m to 200
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

-- ----- Showing the game -----

function slotLeft n
  return 24 + (n - 1) * 96
end slotLeft

on placeCard c, x, y
  set the rect of card button c to x & "," & y & "," & (x + 80) & "," & (y + 116)
  show card button c
  set the partNumber of card button c to 200
end placeCard

-- Put every card where the game says it is, in front of the ones it
-- covers. A long column squeezes up to fit.
on layout
  global gCols, gCells, gHomes
  lock screen
  repeat with n = 1 to 8
    put line n of gCols into pile
    put the number of items of pile into howMany
    put 26 into gap
    if howMany > 1 then put min(26, (590 - 182 - 116) div (howMany - 1)) into gap
    repeat with i = 1 to howMany
      placeCard item i of pile, slotLeft(n), 182 + (i - 1) * gap
    end repeat
  end repeat
  repeat with n = 1 to 4
    if line n of gCells is not empty then placeCard line n of gCells, slotLeft(n), 46
    put line n of gHomes into pileTop
    if pileTop is not empty then
      put char 2 of pileTop into suit
      put rankOf(pileTop) - 1 into below
      repeat with r = 1 to below
        hide card button (char r of "A23456789TJQK" & suit)
      end repeat
      placeCard pileTop, slotLeft(n + 4), 46
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
    put "Game" && gGame & "     Moves:" && gMoves & "     Cards left:" && remaining into msg
  end if
  put msg into field "Status"
end showStatus

on checkWin
  global gHomes, gGame, gMoves
  repeat with h = 1 to 4
    if rankOf(line h of gHomes) < 13 then exit checkWin
  end repeat
  -- A win counts once, even after Undo and winning again.
  if field "Finished" is not "won" then
    put "won" into field "Finished"
    add 1 to field "Won"
    add 1 to field "Streak"
    if field "Streak" > field "Best" then put field "Streak" into field "Best"
    showRecord
  end if
  if gMoves = 1 then put "move" into plural else put "moves" into plural
  showStatus "You won game" && gGame && "in" && gMoves && plural & "!"
  play "harpsichord" tempo 240 "c4e e g c5q"
  celebrate
end checkWin

-- Before leaving a game for another: one with moves made and not won
-- counts as lost, and ends the streak.
on abandonGame
  global gMoves
  if gMoves > 0 and field "Finished" is empty then
    add 1 to field "Lost"
    put 0 into field "Streak"
    showRecord
  end if
end abandonGame

function recordText
  put field "Won" + 0 into won
  put field "Lost" + 0 into lost
  put "Won" && won & ", lost" && lost into r
  if won + lost > 0 then put " (" & round(won * 100 / (won + lost)) & "%)" after r
  put ". Streak" && field "Streak" + 0 & ", best" && field "Best" + 0 & "." after r
  return r
end recordText

on showRecord
  put recordText() into field "Record" of card "Games"
end showRecord

-- The cards fly from the foundations into a ring around the news, a card
-- at a time. A click skips to the end.
on celebrate
  put "You won!" & return & recordText() into field "Banner"
  show field "Banner"
  show button "Play Again"
  put false into skip
  put 0 into k
  repeat with r = 1 to 13
    repeat with s = 1 to 4
      put char r of "A23456789TJQK" & char s of "CDHS" into c
      put k * 2 * pi / 52 - pi / 2 into angle
      put 400 + round(250 * cos(angle)) into x
      put 334 + round(200 * sin(angle)) into y
      show card button c
      set the partNumber of card button c to 200
      if the mouseClick then put true into skip
      if skip is not true then
        put the loc of card button c into startLoc
        repeat with f = 1 to 3
          put item 1 of startLoc + round((x - item 1 of startLoc) * f / 3) into sx
          put item 2 of startLoc + round((y - item 2 of startLoc) * f / 3) into sy
          set the loc of card button c to sx & "," & sy
          wait 1 tick
        end repeat
      end if
      set the loc of card button c to x & "," & y
      add 1 to k
    end repeat
  end repeat
  set the partNumber of field "Banner" to 200
  set the partNumber of button "Play Again" to 200
end celebrate

-- Edit ▸ Undo undoes a move here.
on doMenu what
  if what is "Undo" then undoMove
  else pass doMenu
end doMenu

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

-- The whole game on one line: columns, cells and foundations, separated
-- by |, each a list separated by /.
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
'''

ASK_GAME = r'''
on mouseUp
  global gGame
  ask "Play which game? (1 to 1000000)" with gGame
  if it is empty then exit mouseUp
  if it is not a number or it < 1 or it > 1000000 or it is not trunc(it) then
    answer "Games are numbered from 1 to 1000000."
    exit mouseUp
  end if
  put it into n
  abandonGame
  newGame n
end mouseUp
'''

RULES = (
    'Get all 52 cards onto the four foundations at the top right, each suit from ace up to king.\n\n'
    'Columns build down in alternating colors: a red 6 goes on a black 7. Any card can go in an empty '
    'column, and the four free cells at the top left hold one card each.\n\n'
    'Drag a card to move it, with the run on it: as many cards as the free cells and empty columns would '
    'let you move one at a time. Or click a card, and it goes to the best place it can. Cards that are no '
    'longer needed in the columns go up to the foundations on their own.\n\n'
    'Undo (or Edit ▸ Undo) takes back a move at a time. Game numbers match Windows FreeCell\'s, so you '
    'can play the same deal as a friend. Of its first 32,000 deals, only game 11982 can\'t be won.')


def build():
    global next_id
    next_id = 1
    table = {'id': new_id(), 'name': 'Table', 'script': '', 'picture': picture(), 'show_pict': True,
             'parts': [], 'colors': [{'kind': 'rect', 'rect': rect(0, 0, WIDTH, HEIGHT), 'color': FELT,
                                      'bevel': 0}]}
    plain = {'id': new_id(), 'name': 'Plain', 'script': '', 'picture': picture(), 'show_pict': True,
             'parts': []}
    cards = []

    def card(name, bg, script=''):
        c = {'id': new_id(), 'name': name, 'script': script.strip('\n'), 'background': bg['id'],
             'picture': picture(), 'show_pict': True, 'parts': []}
        cards.append(c)
        return c['parts']

    # ----- Games (the contents) -----
    p = card('Games', plain)
    p.append(label('Title', (40, 30, 760, 90), 'Solitaire', 44, align='Center', bold=True))
    p.append(label('Subtitle', (40, 92, 760, 116), 'Card games for one, made with RandomCard', 15,
                   align='Center', text_style='italic'))
    p.append(button('FreeCell', (40, 150, 220, 190), 'on mouseUp\n  visual effect dissolve fast\n  go to card "FreeCell"\nend mouseUp',
                    text_size=16, text_style='bold', back_color=FELT, fore_color=[255, 255, 255], border_color=FELT))
    p.append(label('FreeCell Rules', (240, 150, 760, 430), RULES, 13))
    p.append(label('Record', (40, 196, 220, 260), 'No games yet.', 12, align='Center'))
    p.append(button('About', (40, 540, 160, 570), 'on mouseUp\n  go to card "About"\nend mouseUp'))

    # ----- FreeCell -----
    p = card('FreeCell', table, FREECELL_SCRIPT)
    white = [255, 255, 255]
    bar = [
        ('New Game', 24, 112, 'on mouseUp\n  abandonGame\n  newGame random(32000)\nend mouseUp'),
        ('Game #…', 118, 196, ASK_GAME),
        ('Restart', 202, 270, 'on mouseUp\n  global gGame\n  abandonGame\n  newGame gGame\nend mouseUp'),
        ('Undo', 276, 334, 'on mouseUp\n  undoMove\nend mouseUp'),
        ('Games', 340, 404, 'on mouseUp\n  visual effect dissolve fast\n  go to card "Games"\nend mouseUp'),
    ]
    for name, l, r, script in bar:
        p.append(button(name, (l, 10, r, 34), script, text_size=12))
    p.append(field('Status', (414, 12, 776, 32), '', 13, align='Right', bold=True, fore_color=white,
                   dont_save=True))
    # Where cards go, drawn behind them.
    slot = dict(show_name=False, auto_hilite=False, back_color=SLOT, border_color=SLOT_EDGE)
    for n in range(4):
        x = 24 + n * 96
        p.append(button(f'Cell {n + 1}', (x, 46, x + CARD_W, 46 + CARD_H), '', 'RoundRect', **slot))
        x = 24 + (n + 4) * 96
        p.append(button(f'Foundation {n + 1}', (x, 46, x + CARD_W, 46 + CARD_H), '', 'RoundRect', **slot))
    for n in range(8):
        x = 24 + n * 96
        p.append(button(f'Column {n + 1}', (x, 182, x + CARD_W, 182 + CARD_H), '', 'RoundRect', **slot))
    # The game, kept with the stack.
    for name in ('State', 'Game', 'Moves', 'History', 'Finished'):
        p.append(field(name, (0, 0, 10, 10), '', 9, visible=False))
    for name in ('Won', 'Lost', 'Streak', 'Best'):
        p.append(field(name, (0, 0, 10, 10), '0', 9, visible=False))
    # The cards: a button each, its icon the card's face.
    icons = {}
    for i, code in enumerate(r + s for r in RANKS for s in SUITS):
        icon_id = 40001 + i
        icons[str(icon_id)] = {'name': code, 'picture': icon_picture(face_file(code))}
        p.append(button(code, (24, 182, 24 + CARD_W, 182 + CARD_H), '', 'Transparent', icon=icon_id,
                        show_name=False, auto_hilite=False))
    p.append(field('Banner', (270, 284, 530, 364), '', 18, style='Rectangle', align='Center', bold=True,
                   back_color=[255, 250, 230], visible=False, dont_save=True))
    p.append(button('Play Again', (340, 372, 460, 398), 'on mouseUp\n  newGame random(32000)\nend mouseUp',
                    visible=False))

    # ----- About -----
    p = card('About', plain)
    p.append(label('Title', (40, 30, 760, 70), 'About Solitaire', 28, bold=True))
    p.append(label('Credits', (40, 80, 760, 200),
                   'The cards are Byron Knoll\'s Vector Playing Cards (version 1.3), which he released into '
                   'the public domain.\n\n'
                   'FreeCell\'s numbered deals follow the ones Jim Horne wrote for Windows FreeCell, so game 1 '
                   'here is the same deal as game 1 there.', 13))
    p.append(label('Shows Label', (40, 214, 760, 236), 'What Solitaire shows about RandomCard', 15, bold=True))
    p.append(field('Shows', (40, 240, 760, 470),
                   'Each card is a button whose icon is the card\'s face: icons of any size, in color.\n'
                   'Dragging is HyperTalk: repeat while the mouse is down, moving the cards to the mouseLoc, '
                   'and setting their partNumber to bring them to the front.\n'
                   'The game\'s rules are the card\'s own script, in plain HyperTalk: choose Objects ▸ Card '
                   'Script on the FreeCell card to read them. Its shuffle is Windows FreeCell\'s, with the '
                   'same arithmetic.\n'
                   'A game in progress is kept in hidden fields, so it\'s saved with the stack, and Undo goes '
                   'back to the start of the deal.', 13, style='Scrolling'))
    p.append(button('Games', (40, 540, 160, 570), 'on mouseUp\n  go to card "Games"\nend mouseUp'))

    stack = {'format': 1, 'name': 'Solitaire', 'script': '', 'width': WIDTH, 'height': HEIGHT,
             'next_id': next_id, 'backgrounds': [table, plain], 'cards': cards, 'icons': icons}
    return stack


if __name__ == '__main__':
    stack = build()
    with open('examples/Solitaire.rcstack', 'w') as f:
        json.dump(stack, f, indent=2, ensure_ascii=False)
    print('wrote examples/Solitaire.rcstack:', len(stack['cards']), 'cards,', len(stack['icons']), 'icons')
