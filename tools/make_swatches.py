"""Write examples/Swatches.rcstack: colors looked up, converted and explained,
with The Color API (www.thecolorapi.com).

usage: python3 -I tools/make_swatches.py   (from the repository's top folder)

The stack's scripts are here, readable. Its pictures (the color wheels)
aren't: the stack paints them itself the first time each card opens, so
after writing the stack, open it once in RandomCard, visit Color Wheel and
Harmony, and save; or run, from the top folder:

  RANDOMCARD_STACK=examples/Swatches.rcstack \\
  RANDOMCARD_SCRIPT=$'go to card "Color Wheel"\\ngo to card "Harmony"\\nsave this stack' \\
  cargo test --release drive_stack -- --ignored --nocapture
"""
import json

WIDTH, HEIGHT = 640, 480
next_id = 1


def new_id():
    global next_id
    next_id += 1
    return next_id - 1


def rect(l, t, r, b):
    return {'left': l, 'top': t, 'right': r, 'bottom': b}


def hexrgb(h):
    h = h.lstrip('#')
    return [int(h[i:i + 2], 16) for i in (0, 2, 4)]


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


STACK_SCRIPT = r'''
-- Swatches: colors looked up, converted and explained.
--
-- The color math is here as plain handlers, so you can read how each
-- conversion works. The Color API (www.thecolorapi.com, by Josh Beckman)
-- names colors and answers for every color space.

on openStack
  global gColor
  if gColor is empty then put "0,71,171" into gColor
  set the httpTimeout to 15
  pass openStack
end openStack

-- ----- The Color API -----

-- The address of a request: apiURL("/id?hex=0047AB").
function apiURL path
  return card field "Base URL" of card "About" & path
end apiURL

-- A line along the bottom of every card.
on showStatus msg
  put msg into bkgnd field "Status"
end showStatus

-- Ask The Color API to name the current color; gotName shows it.
on nameColor
  global gColor
  load url apiURL("/id?rgb=" & gColor) with message "gotName"
end nameColor

on gotName theURL, reply
  global gColor
  if the result is not empty then
    showStatus "The Color API didn't answer:" && the result
    exit gotName
  end if
  -- Only the answer for the color showing now: an earlier one can be late.
  if ("rgb=" & gColor) is not in theURL then exit gotName
  if there is not a card field "Name" then exit gotName
  put key "value" of key "name" of json reply into name
  if key "exact_match_name" of key "name" of json reply is "true" then
    put "The Color API calls this" && name & "." into card field "Name"
  else
    put key "closest_named_hex" of key "name" of json reply into near
    put "The nearest named color is" && name && "(" & near & ")." into card field "Name"
  end if
end gotName

-- ----- Reading what's typed -----

-- The Color API's query for a color typed any common way, or empty if it
-- isn't one: "hex=0047AB", "rgb=0,71,171", "hsl=215,100%,34%" or
-- "cmyk=100,58,0,33". (The API answers even nonsense, with NaN for its
-- numbers, so Swatches checks first.)
function colorQuery typed
  put trimmed(typed) into t
  if char 1 of t is "#" then delete char 1 of t
  if isHex(t) and (length(t) = 3 or length(t) = 6) then return "hex=" & t
  put offset("(", t) into open
  if open < 2 or last char of t is not ")" then return empty
  put trimmed(char 1 to (open - 1) of t) into kind
  put withoutSpaces(char (open + 1) to (length(t) - 1) of t) into args
  if kind is "rgb" then
    if the number of items of args is not 3 then return empty
    repeat with i = 1 to 3
      if not inRange(item i of args, 0, 255) then return empty
    end repeat
    return "rgb=" & args
  end if
  put withoutPercent(args) into n
  if kind is "hsl" then
    if the number of items of n is not 3 then return empty
    if not inRange(item 1 of n, 0, 360) then return empty
    if not inRange(item 2 of n, 0, 100) or not inRange(item 3 of n, 0, 100) then return empty
    return "hsl=" & urlEncode(item 1 of n & "," & item 2 of n & "%," & item 3 of n & "%")
  end if
  if kind is "cmyk" then
    if the number of items of n is not 4 then return empty
    repeat with i = 1 to 4
      if not inRange(item i of n, 0, 100) then return empty
    end repeat
    return "cmyk=" & n
  end if
  return empty
end colorQuery

-- Red, green and blue for a color typed as hex or rgb(), without asking:
-- "0,71,171", or empty.
function localRGB typed
  put colorQuery(typed) into q
  if char 1 to 4 of q is "hex=" then return hexToRGB(char 5 to length(q) of q)
  if char 1 to 4 of q is "rgb=" then return char 5 to length(q) of q
  return empty
end localRGB

function trimmed t
  repeat while char 1 of t is space or char 1 of t is tab
    delete char 1 of t
  end repeat
  repeat while last char of t is space or last char of t is tab
    delete last char of t
  end repeat
  return t
end trimmed

function withoutSpaces t
  repeat while space is in t
    delete char offset(space, t) of t
  end repeat
  return t
end withoutSpaces

function withoutPercent t
  repeat while "%" is in t
    delete char offset("%", t) of t
  end repeat
  return t
end withoutPercent

function inRange n, low, high
  if n is not a number then return false
  return n >= low and n <= high
end inRange

function isHex t
  if t is empty then return false
  repeat with i = 1 to length(t)
    if char i of t is not in "0123456789ABCDEFabcdef" then return false
  end repeat
  return true
end isHex

-- ----- Hex and RGB -----

-- Two hexadecimal digits for a number from 0 to 255: 171 is AB.
function hex2 n
  put "0123456789ABCDEF" into digits
  put max(0, min(255, round(n))) into n
  return char ((n div 16) + 1) of digits & char ((n mod 16) + 1) of digits
end hex2

-- "#0047AB" for red, green and blue ("0,71,171").
function rgbToHex rgb
  return "#" & hex2(item 1 of rgb) & hex2(item 2 of rgb) & hex2(item 3 of rgb)
end rgbToHex

-- "0,71,171" for "#0047AB" (or "#04A", short for "#0044AA").
function hexToRGB hex
  if char 1 of hex is "#" then delete char 1 of hex
  if length(hex) = 3 then
    put char 1 of hex & char 1 of hex & char 2 of hex & char 2 of hex & char 3 of hex & char 3 of hex into hex
  end if
  put "0123456789ABCDEF" into digits
  put empty into rgb
  repeat with i = 0 to 2
    put offset(char (2 * i + 1) of hex, digits) - 1 into high
    put offset(char (2 * i + 2) of hex, digits) - 1 into low
    put (high * 16 + low) & "," after rgb
  end repeat
  delete last char of rgb
  return rgb
end hexToRGB

-- ----- HSV: hue, saturation and value -----

-- Hue (0 to 359 degrees), saturation and value (0 to 100) for red, green
-- and blue (0 to 255). Value is the brightest of the three; saturation is
-- how far the dimmest falls below it; the hue says which is brightest,
-- and how far toward the next.
function rgbToHSV rgb
  put (item 1 of rgb) / 255 into r
  put (item 2 of rgb) / 255 into g
  put (item 3 of rgb) / 255 into b
  put max(r, g, b) into high
  put min(r, g, b) into low
  put high - low into spread
  if spread = 0 then
    put 0 into h
  else if high = r then
    put 60 * ((g - b) / spread) into h
  else if high = g then
    put 60 * ((b - r) / spread + 2) into h
  else
    put 60 * ((r - g) / spread + 4) into h
  end if
  if h < 0 then add 360 to h
  if high = 0 then put 0 into s
  else put spread / high into s
  return (round(h) mod 360) & "," & round(s * 100) & "," & round(high * 100)
end rgbToHSV

-- Red, green and blue (0 to 255) for a hue (degrees), saturation and
-- value (0 to 100): the wheel turned back into light.
function hsvToRGB h, s, v
  put h mod 360 into h
  if h < 0 then add 360 to h
  put s / 100 into s
  put v / 100 into v
  put h / 60 into sector
  put trunc(sector) into i
  put sector - i into f
  put v * (1 - s) into p
  put v * (1 - f * s) into q
  put v * (1 - (1 - f) * s) into t
  if i = 0 then put v & "," & t & "," & p into rgb
  else if i = 1 then put q & "," & v & "," & p into rgb
  else if i = 2 then put p & "," & v & "," & t into rgb
  else if i = 3 then put p & "," & q & "," & v into rgb
  else if i = 4 then put t & "," & p & "," & v into rgb
  else put v & "," & p & "," & q into rgb
  return round((item 1 of rgb) * 255) & "," & round((item 2 of rgb) * 255) & "," & round((item 3 of rgb) * 255)
end hsvToRGB

-- ----- Contrast, by WCAG 2 -----

-- How bright a color looks, from 0 (black) to 1 (white): WCAG 2's
-- relative luminance. Each channel is turned from sRGB's curve back into
-- light, then weighted, since the eye is most sensitive to green.
function luminance rgb
  return 0.2126 * linear(item 1 of rgb) + 0.7152 * linear(item 2 of rgb) + 0.0722 * linear(item 3 of rgb)
end luminance

function linear c
  put c / 255 into c
  if c <= 0.04045 then return c / 12.92
  return ((c + 0.055) / 1.055) ^ 2.4
end linear

-- WCAG 2's contrast ratio between two colors, from 1 (none) to 21 (black
-- on white).
function contrastRatio rgb1, rgb2
  put luminance(rgb1) into a
  put luminance(rgb2) into b
  return (max(a, b) + 0.05) / (min(a, b) + 0.05)
end contrastRatio

-- Black or white, whichever reads better on a color.
function textColorFor rgb
  if contrastRatio(rgb, "0,0,0") >= contrastRatio(rgb, "255,255,255") then return "0,0,0"
  return "255,255,255"
end textColorFor

-- Show a color in a swatch field: filled with it, labeled in black or
-- white, whichever reads better on it.
on paintSwatch which, rgb, label
  set the backColor of card field which to rgb
  set the foreColor of card field which to textColorFor(rgb)
  put label into card field which
end paintSwatch

-- Copy a field's text, to paste into code or a design tool.
on copyField which
  if card field which is empty then exit copyField
  select text of card field which
  doMenu "Copy Text"
  select empty
  showStatus "Copied" && card field which & "."
end copyField

-- ----- Color wheels -----

-- Paint an HSV color wheel at full value: the hue goes around (red at the
-- right, then counterclockwise through yellow, green, cyan, blue and
-- magenta), and the saturation out from white at the center to vivid at
-- the edge. Each stroke of the line tool is one hue and one saturation.
on paintWheel cx, cy, radius
  lock screen
  choose line tool
  set the lineSize to 2
  repeat with k = 0 to 719
    put k / 2 into h
    put h * pi / 180 into a
    put cos(a) into dx
    put sin(a) into dy
    repeat with ring = 1 to 20
      set the penColor to hsvToRGB(h, ring * 5, 100)
      put (ring - 1) * radius / 20 into r1
      put ring * radius / 20 into r2
      drag from round(cx + r1 * dx), round(cy - r1 * dy) to round(cx + r2 * dx), round(cy - r2 * dy)
    end repeat
  end repeat
  choose browse tool
  unlock screen
end paintWheel

-- The hue and saturation at a point on a wheel, as "h,s", or empty if the
-- point is off it.
function wheelAt pt, cx, cy, radius
  put (item 1 of pt) - cx into dx
  put cy - (item 2 of pt) into dy
  put sqrt(dx * dx + dy * dy) into r
  if r > radius + 2 then return empty
  return angleOf(dx, dy) & "," & min(100, round(r / radius * 100))
end wheelAt

-- The angle of a point from the center, in degrees counterclockwise from
-- the right.
function angleOf dx, dy
  if dx = 0 then
    if dy >= 0 then return 90
    return 270
  end if
  put atan(dy / dx) * 180 / pi into a
  if dx < 0 then add 180 to a
  if a < 0 then add 360 to a
  return round(a) mod 360
end angleOf

-- Where on a wheel a hue and saturation sit.
function wheelPoint h, s, cx, cy, radius
  put h * pi / 180 into a
  put s / 100 * radius into r
  return round(cx + r * cos(a)) & "," & round(cy - r * sin(a))
end wheelPoint
'''

BG_SCRIPT = ''

# ----- Convert -----

CONVERT_SCRIPT = r'''
-- Convert: a color typed any common way, named and given every way by
-- The Color API.

on openCard
  global gColor
  if card field "Color" is empty then put rgbToHex(gColor) into card field "Color"
  pass openCard
end openCard

on returnInField
  if the short name of the target is "Color" then lookUp
  else pass returnInField
end returnInField

on enterInField
  if the short name of the target is "Color" then lookUp
  else pass enterInField
end enterInField

-- Choosing one of the examples from the ▼.
on closeField
  if the short name of the target is "Color" then lookUp
  else pass closeField
end closeField

on lookUp
  put colorQuery(card field "Color") into q
  if q is empty then
    answer "That isn't a color Swatches can read. Try #0047AB, 0047AB, rgb(0, 71, 171), hsl(215, 100%, 34%) or cmyk(100, 58, 0, 33)."
    exit lookUp
  end if
  showStatus "Asking The Color API…"
  load url apiURL("/id?" & q) with message "gotColor"
end lookUp

on gotColor theURL, reply
  global gColor
  if the result is not empty then
    showStatus "The Color API didn't answer:" && the result
    exit gotColor
  end if
  put (key "r" of key "rgb" of json reply) & "," & (key "g" of key "rgb" of json reply) & "," & (key "b" of key "rgb" of json reply) into rgb
  put rgb into gColor
  put key "value" of key "hex" of json reply into hex
  put key "value" of key "name" of json reply into name
  put hex into card field "Hex Value"
  put key "value" of key "rgb" of json reply into card field "RGB Value"
  put key "value" of key "hsl" of json reply into card field "HSL Value"
  put key "value" of key "hsv" of json reply into card field "HSV Value"
  put key "value" of key "cmyk" of json reply into card field "CMYK Value"
  paintSwatch "Swatch", rgb, name & return & hex
  if key "exact_match_name" of key "name" of json reply is "true" then
    put name && "(an exact match)" into card field "Name Value"
    hide card field "Nearest"
  else
    put key "closest_named_hex" of key "name" of json reply into near
    put "nearest named:" && name into card field "Name Value"
    paintSwatch "Nearest", hexToRGB(near), name && near
    show card field "Nearest"
  end if
  remember hex && name
  showStatus "The Color API calls it" && name & "."
end gotColor

-- Keep a lookup at the top of Recent, once, and the last 20.
on remember entry
  put card field "Recent" into recent
  repeat with i = the number of lines of recent down to 1
    if line i of recent is entry then delete line i of recent
  end repeat
  if recent is empty then put entry into recent
  else put entry & return & recent into recent
  repeat while the number of lines of recent > 20
    delete last line of recent
  end repeat
  put recent into card field "Recent"
end remember
'''

# ----- Mixing Light -----

MIXING_SCRIPT = r'''
-- Mixing Light: red, green and blue sliders, and the color they make.

on openCard
  global gColor
  setMix gColor
  pass openCard
end openCard

-- A slider dragged: its number, and the color, follow.
on sliderMoved
  put the short name of the target into channel
  put card button channel into card field (channel && "Value")
  showMix
end sliderMoved

-- Let go: ask what the color is called.
on sliderChanged
  nameColor
end sliderChanged

on returnInField
  if word 2 of the short name of the target is "Value" then applyValue the short name of the target
  else pass returnInField
end returnInField

on closeField
  if word 2 of the short name of the target is "Value" then applyValue the short name of the target
  else pass closeField
end closeField

-- A number typed: the slider takes it (kept from 0 to 255), and so does
-- the color.
on applyValue f
  put word 1 of f into channel
  put card field f into card button channel
  put card button channel into card field f
  showMix
  nameColor
end applyValue

on setMix rgb
  put item 1 of rgb into card button "Red"
  put item 2 of rgb into card button "Green"
  put item 3 of rgb into card button "Blue"
  repeat with i = 1 to 3
    put item i of "Red,Green,Blue" into channel
    put card button channel into card field (channel && "Value")
  end repeat
  showMix
  nameColor
end setMix

on showMix
  global gColor
  put card button "Red" & "," & card button "Green" & "," & card button "Blue" into rgb
  put rgb into gColor
  paintSwatch "Swatch", rgb, rgbToHex(rgb) & return & "rgb(" & rgb & ")"
  put empty into card field "Name"
end showMix
'''

# ----- Color Wheel -----

WHEEL = (200, 265, 150)
WHEEL_SCRIPT = r'''
-- Color Wheel: hue around, saturation outward, value on the slider.

on openCard
  global gColor
  if not (there is a cd picture) then paintWheel 200, 265, 150
  showRGB gColor
  nameColor
  pass openCard
end openCard

on showRGB rgb
  global gH, gS, gV
  put rgbToHSV(rgb) into hsv
  put item 1 of hsv into gH
  put item 2 of hsv into gS
  put item 3 of hsv into gV
  put gV into card button "Value"
  showHSV
end showRGB

on showHSV
  global gColor, gH, gS, gV
  put hsvToRGB(gH, gS, gV) into rgb
  put rgb into gColor
  paintSwatch "Swatch", rgb, rgbToHex(rgb)
  put "hsv(" & gH & ", " & gS & "%, " & gV & "%)" & return & "rgb(" & item 1 of rgb & ", " & item 2 of rgb & ", " & item 3 of rgb & ")" & return & rgbToHex(rgb) into card field "Readout"
  set the loc of card button "Marker" to wheelPoint(gH, gS, 200, 265, 150)
  put empty into card field "Name"
end showHSV

-- On the wheel: pick the hue and saturation there, following the mouse
-- until it's let go.
on mouseDown
  put the short name of the target into t
  if t is not the short name of this card and t is not "Marker" then pass mouseDown
  repeat
    pickAt the mouseLoc
    if the mouse is up then exit repeat
    wait 1 tick
  end repeat
  nameColor
end mouseDown

on pickAt pt
  global gH, gS
  put wheelAt(pt, 200, 265, 150) into hs
  if hs is empty then exit pickAt
  put item 1 of hs into gH
  put item 2 of hs into gS
  showHSV
end pickAt

on sliderMoved
  global gV
  put card button "Value" into gV
  showHSV
end sliderMoved

on sliderChanged
  nameColor
end sliderChanged
'''

# ----- Harmony -----

HARMONY_SCRIPT = r'''
-- Harmony: color schemes as turns of the hue around the wheel.

on openCard
  if not (there is a cd picture) then paintWheel 170, 270, 120
  if card field "Scheme" is empty then put "Triadic" into card field "Scheme"
  showScheme
  nameSwatches
  pass openCard
end openCard

-- A scheme chosen from the ▼, or typed.
on closeField
  if the short name of the target is "Scheme" then
    showScheme
    nameSwatches
  else
    pass closeField
  end if
end closeField

on returnInField
  if the short name of the target is "Scheme" then
    showScheme
    nameSwatches
  else
    pass returnInField
  end if
end returnInField

-- The turns of the hue, in degrees, for each scheme.
function turns scheme
  if scheme is "Complementary" then return "0,180"
  if scheme is "Analogous" then return "330,0,30"
  if scheme is "Triadic" then return "0,120,240"
  if scheme is "Split-Complementary" then return "0,150,210"
  if scheme is "Square" then return "0,90,180,270"
  return "0"
end turns

function rule scheme
  if scheme is "Complementary" then return "Complementary: the hue opposite, 180° around. The strongest contrast; vivid side by side."
  if scheme is "Analogous" then return "Analogous: the neighbors, 30° to each side. Calm and harmonious, as in a landscape."
  if scheme is "Triadic" then return "Triadic: three hues, 120° apart. Vivid, but balanced."
  if scheme is "Split-Complementary" then return "Split-Complementary: a hue and the two neighbors of its opposite, 150° and 210° around. Contrast that's easier to live with."
  if scheme is "Square" then return "Square: four hues, 90° apart. Rich; best with one of them leading."
  return "Choose a scheme from the ▼."
end rule

on showScheme
  global gColor
  put rgbToHSV(gColor) into hsv
  put card field "Scheme" into scheme
  put turns(scheme) into t
  repeat with i = 1 to 4
    if i > the number of items of t then
      hide card field ("Swatch" && i)
      hide card button ("Marker" && i)
    else
      put ((item 1 of hsv) + item i of t) mod 360 into hue
      put hsvToRGB(hue, item 2 of hsv, item 3 of hsv) into rgb
      paintSwatch "Swatch" && i, rgb, rgbToHex(rgb)
      show card field ("Swatch" && i)
      set the loc of card button ("Marker" && i) to wheelPoint(hue, item 2 of hsv, 170, 270, 120)
      show card button ("Marker" && i)
    end if
  end repeat
  put rule(scheme) into card field "Rule"
end showScheme

-- Ask The Color API for each swatch's name.
on nameSwatches
  repeat with i = 1 to 4
    if the visible of card field ("Swatch" && i) then
      put line 1 of card field ("Swatch" && i) into hex
      load url apiURL("/id?hex=" & char 2 to 7 of hex) with message "gotSwatchName"
    end if
  end repeat
end nameSwatches

on gotSwatchName theURL, reply
  if the result is not empty then
    showStatus "The Color API didn't answer:" && the result
    exit gotSwatchName
  end if
  put "#" & char (offset("hex=", theURL) + 4) to (offset("hex=", theURL) + 9) of theURL into hex
  repeat with i = 1 to 4
    if line 1 of card field ("Swatch" && i) is hex then
      put key "value" of key "name" of json reply into line 2 of card field ("Swatch" && i)
    end if
  end repeat
end gotSwatchName

-- On the wheel: a new starting hue and saturation, keeping the value.
on mouseDown
  global gColor
  put the short name of the target into t
  if t is not the short name of this card and word 1 of t is not "Marker" then pass mouseDown
  repeat
    put wheelAt(the mouseLoc, 170, 270, 120) into hs
    if hs is not empty then
      put hsvToRGB(item 1 of hs, item 2 of hs, item 3 of rgbToHSV(gColor)) into gColor
      showScheme
    end if
    if the mouse is up then exit repeat
    wait 1 tick
  end repeat
  nameSwatches
end mouseDown

-- The Color API's own scheme for the same color, to compare.
on askAPIScheme
  global gColor
  put card field "Scheme" into scheme
  if scheme is "Complementary" then put "complement" into mode
  else if scheme is "Analogous" then put "analogic" into mode
  else if scheme is "Triadic" then put "triad" into mode
  else if scheme is "Square" then put "quad" into mode
  else put "analogic-complement" into mode
  showStatus "Asking The Color API for its" && mode && "scheme…"
  load url apiURL("/scheme?hex=" & char 2 to 7 of rgbToHex(gColor) & "&mode=" & mode & "&count=5") with message "gotAPIScheme"
end askAPIScheme

on gotAPIScheme theURL, reply
  if the result is not empty then
    showStatus "The Color API didn't answer:" && the result
    exit gotAPIScheme
  end if
  put key "colors" of json reply into colors
  repeat with i = 1 to 5
    if i > the number of elements of json colors then
      hide card field ("API" && i)
    else
      put key "value" of key "hex" of element i of json colors into hex
      paintSwatch "API" && i, hexToRGB(hex), hex
      show card field ("API" && i)
    end if
  end repeat
  showStatus "The Color API's" && key "mode" of json reply && "scheme: it varies lightness too, not only the hue."
end gotAPIScheme
'''

# ----- Contrast -----

CONTRAST_SCRIPT = r'''
-- Contrast: will text be readable? WCAG 2's contrast ratio.

on openCard
  global gColor
  if card field "Background" is empty then put rgbToHex(gColor) into card field "Background"
  if card field "Text Color" is empty then put rgbToHex(textColorFor(gColor)) into card field "Text Color"
  checkContrast
  pass openCard
end openCard

on returnInField
  checkContrast
end returnInField

on closeField
  checkContrast
end closeField

on checkContrast
  put localRGB(card field "Text Color") into fore
  put localRGB(card field "Background") into back
  if fore is empty or back is empty then
    put "?" into card field "Ratio"
    put "Type each color as hex (#336699) or rgb(51, 102, 153)." into card field "Grades"
    exit checkContrast
  end if
  set the foreColor of card field "Sample" to fore
  set the backColor of card field "Sample" to back
  put contrastRatio(fore, back) into ratio
  -- Never rounded up: 4.499 fails a 4.5 minimum.
  put trunc(ratio * 100) / 100 into shown
  put shown & ":1" into card field "Ratio"
  put grade(ratio, 4.5, "AA, normal text") & return & grade(ratio, 3, "AA, large text") & return & grade(ratio, 7, "AAA, normal text") & return & grade(ratio, 4.5, "AAA, large text") into card field "Grades"
end checkContrast

function grade ratio, needed, what
  if ratio >= needed then return "✓ Passes" && what && "(needs" && needed & ":1)"
  return "✗ Fails" && what && "(needs" && needed & ":1)"
end grade

-- Black or white text, whichever reads better on the background.
on bestText
  put localRGB(card field "Background") into back
  if back is empty then exit bestText
  put rgbToHex(textColorFor(back)) into card field "Text Color"
  checkContrast
end bestText

on swapColors
  put card field "Text Color" into t
  put card field "Background" into card field "Text Color"
  put t into card field "Background"
  checkContrast
end swapColors

-- The Color API's own suggestion for text on the background.
on askAPI
  put localRGB(card field "Background") into back
  if back is empty then exit askAPI
  load url apiURL("/id?rgb=" & back) with message "gotSuggestion"
end askAPI

on gotSuggestion theURL, reply
  if the result is not empty then
    showStatus "The Color API didn't answer:" && the result
    exit gotSuggestion
  end if
  put key "value" of key "contrast" of json reply into suggestion
  showStatus "The Color API suggests" && suggestion && "text on" && key "value" of key "name" of key "name" of json reply & "."
end gotSuggestion
'''


def build():
    global next_id
    next_id = 1
    main = {'id': new_id(), 'name': 'Main', 'script': BG_SCRIPT, 'picture': picture(), 'show_pict': True,
            'parts': []}
    nav = [
        button('Contents', (20, 446, 110, 470), 'on mouseUp\n  visual effect dissolve fast\n  go to card "Contents"\nend mouseUp'),
        button('◀', (120, 446, 160, 470), 'on mouseUp\n  visual effect scroll right fast\n  go to previous card\nend mouseUp'),
        button('▶', (166, 446, 206, 470), 'on mouseUp\n  visual effect scroll left fast\n  go to next card\nend mouseUp'),
        field('Status', (220, 448, 620, 468), '', 11, align='Right', shared_text=True, dont_save=True),
    ]
    main['parts'] = nav
    cards = []

    def card(name, script=''):
        c = {'id': new_id(), 'name': name, 'script': script.strip('\n'), 'background': main['id'],
             'picture': picture(), 'show_pict': True, 'parts': []}
        cards.append(c)
        return c['parts']

    def title(parts, text, about=None, about_height=46):
        parts.append(label('Title', (20, 14, 620, 46), text, 24, bold=True))
        if about:
            parts.append(label('About', (20, 50, 620, 50 + about_height), about, 12))

    # ----- Contents -----
    p = card('Contents')
    p.append(label('Title', (20, 26, 620, 76), 'Swatches', 40, align='Center', bold=True))
    p.append(label('Subtitle', (20, 78, 620, 100), 'Look colors up, convert them, and see how they work', 15,
                   align='Center', text_style='italic'))
    p.append(label('About', (60, 106, 580, 158),
                   'A color tool for designers and programmers, a short course in how color works, and a tour of '
                   'what RandomCard can do. Names and conversions come from The Color API (www.thecolorapi.com), '
                   'over the web.', 12, align='Center'))
    entries = [
        ('Convert', '#0047AB', 'Look up any color by hex, rgb, hsl or cmyk: its name, and every other way to write it.'),
        ('Mixing Light', '#E53935', 'RGB: how a screen makes every color from red, green and blue light.'),
        ('Color Wheel', '#2E7D32', 'HSV: hue, saturation and value, the way people describe color.'),
        ('Harmony', '#8E24AA', 'Color schemes as geometry: complementary, triadic and more.'),
        ('Contrast', '#FDD835', 'Will your text be readable? The WCAG contrast ratio, worked out.'),
        ('About', '#546E7A', 'The Color API, and what this stack shows about RandomCard.'),
    ]
    for i, (name, color, text) in enumerate(entries):
        y = 166 + i * 45
        rgb = hexrgb(color)
        lum = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
        fg = [0, 0, 0] if lum > 150 else [255, 255, 255]
        p.append(button(name, (60, y, 230, y + 34),
                        f'on mouseUp\n  visual effect dissolve fast\n  go to card "{name}"\nend mouseUp',
                        back_color=rgb, fore_color=fg, border_color=rgb, text_size=13, text_style='bold'))
        p.append(label(f'{name} Note', (244, y + 2, 600, y + 34), text, 12))

    # ----- Convert -----
    p = card('Convert', CONVERT_SCRIPT)
    title(p, 'Convert', 'Type a color the way you have it, then press Return: #0047AB, 0047AB, rgb(0, 71, 171), '
                        'hsl(215, 100%, 34%) or cmyk(100, 58, 0, 33). The Color API names it and writes it every '
                        'other way, ready to paste.', 50)
    p.append(field('Color', (20, 106, 250, 132), '', 14, style='Rectangle', lock=False,
                   choices='#0047AB\nrgb(255, 99, 71)\nhsl(120, 60%, 70%)\ncmyk(0, 50, 100, 0)\n#FF69B4\n#40E0D0'))
    p.append(button('Look Up', (258, 106, 340, 132), 'on mouseUp\n  lookUp\nend mouseUp'))
    p.append(button('Random', (348, 106, 430, 132),
                    'on mouseUp\n  put rgbToHex((random(256) - 1) & "," & (random(256) - 1) & "," & (random(256) - 1)) into card field "Color"\n  lookUp\nend mouseUp'))
    rows = ['Hex', 'RGB', 'HSL', 'HSV', 'CMYK']
    for i, name in enumerate(rows):
        y = 144 + i * 30
        p.append(label(f'{name} Label', (20, y + 3, 70, y + 23), name, 12, bold=True))
        p.append(field(f'{name} Value', (74, y, 290, y + 24), '', 13, style='Rectangle', dont_save=True))
        p.append(button(f'Copy {name}', (296, y, 380, y + 24), f'on mouseUp\n  copyField "{name} Value"\nend mouseUp',
                        text_size=11))
    y = 144 + 5 * 30
    p.append(label('Name Label', (20, y + 3, 70, y + 23), 'Name', 12, bold=True))
    p.append(field('Name Value', (74, y, 380, y + 24), '', 13, style='Rectangle', dont_save=True))
    p.append(field('Swatch', (440, 106, 620, 270), '', 18, style='Rectangle', align='Center', bold=True,
                   dont_save=True))
    nearest = field('Nearest', (440, 276, 620, 318), '', 12, style='Rectangle', align='Center', dont_save=True,
                    visible=False)
    nearest['script'] = '-- The nearest named color: click it to look it up.\non mouseUp\n  put last word of me into card field "Color"\n  lookUp\nend mouseUp'
    p.append(nearest)
    p.append(label('Recent Label', (20, 328, 380, 346), 'Recent (click one to look it up again)', 12, bold=True))
    recent = field('Recent', (20, 348, 380, 436), '', 12, style='Scrolling', auto_select=True, dont_wrap=True)
    recent['script'] = 'on mouseUp\n  put word 1 of the selectedText of me into card field "Color"\n  lookUp\nend mouseUp'
    p.append(recent)
    for i, (name, dest) in enumerate([('Mix It', 'Mixing Light'), ('On the Wheel', 'Color Wheel'),
                                      ('Harmonize', 'Harmony'), ('Check Contrast', 'Contrast')]):
        y = 330 + i * 27
        p.append(button(name, (440, y, 620, y + 23), f'on mouseUp\n  go to card "{dest}"\nend mouseUp', text_size=11))

    # ----- Mixing Light -----
    p = card('Mixing Light', MIXING_SCRIPT)
    title(p, 'Mixing Light', 'A screen makes every color from red, green and blue light, each from 0 (off) to 255 '
                             '(full). Drag a slider or type a number. Let go, and The Color API names what you made.',
          34)
    for i, (name, color) in enumerate([('Red', [229, 57, 53]), ('Green', [67, 160, 71]), ('Blue', [30, 136, 229])]):
        y = 100 + i * 40
        p.append(button(name, (20, y, 390, y + 28), '', 'Slider', min=0, max=255, contents='0', title_width=60,
                        text_align='Left', text_size=13, text_style='bold', back_color=color))
        p.append(field(f'{name} Value', (400, y + 2, 460, y + 26), '0', 13, style='Rectangle', lock=False,
                       align='Right'))
    p.append(field('Swatch', (480, 96, 620, 220), '', 14, style='Rectangle', align='Center', bold=True,
                   dont_save=True))
    p.append(field('Name', (20, 226, 620, 246), '', 12, dont_save=True, text_style='italic'))
    lessons = [('Red + Green', '255,255,0'), ('Green + Blue', '0,255,255'), ('Red + Blue', '255,0,255'),
               ('All Three', '255,255,255'), ('None', '0,0,0'), ('Half of Each', '128,128,128')]
    for i, (name, rgb) in enumerate(lessons):
        x = 20 + i * 100
        p.append(button(name, (x, 256, x + 94, 282), f'on mouseUp\n  setMix "{rgb}"\nend mouseUp', text_size=11))
    p.append(label('Why', (20, 294, 620, 436),
                   'Why do red and green make yellow? Your eye has three kinds of color receptors, most sensitive '
                   'to long (reddish), medium (greenish) and short (bluish) light. Red and green light together '
                   'stir the first two just as yellow light does, so you see yellow: three lights are enough to '
                   'fool the eye, which is why screens use them.\n\n'
                   'Light adds up: all three at full is white, and none is black. Equal amounts make a gray. Paint '
                   'and ink work the other way, taking light away, which is why printers use cyan, magenta, yellow '
                   'and black (CMYK) instead.', 12))

    # ----- Color Wheel -----
    p = card('Color Wheel', WHEEL_SCRIPT)
    title(p, 'The Color Wheel', 'Hue, saturation and value: how people describe color. Around the wheel the hue '
                                'changes; from the center out, the saturation, from white to vivid. Value is '
                                'brightness. Click or drag on the wheel.', 50)
    p.append(button('Value', (380, 112, 620, 136), '', 'Slider', min=0, max=100, contents='100', title_width=50,
                    text_align='Left', text_size=12, text_style='bold'))
    p.append(field('Swatch', (380, 146, 620, 236), '', 16, style='Rectangle', align='Center', bold=True,
                   dont_save=True))
    p.append(field('Readout', (380, 242, 620, 296), '', 13, dont_save=True))
    p.append(field('Name', (380, 298, 620, 332), '', 12, dont_save=True, text_style='italic'))
    p.append(label('Why', (380, 334, 620, 404),
                   'Darker or lighter is one number in HSV, the value; in RGB, all three change. The wheel shows '
                   'value at 100%.', 12))
    p.append(button('Use in Convert', (380, 408, 498, 434),
                    'on mouseUp\n  global gColor\n  put rgbToHex(gColor) into card field "Color" of card "Convert"\n  go to card "Convert"\n  send "lookUp" to card "Convert"\nend mouseUp',
                    text_size=11))
    p.append(button('Harmonize', (504, 408, 620, 434), 'on mouseUp\n  go to card "Harmony"\nend mouseUp', text_size=11))
    marker = button('Marker', (194, 259, 206, 271), '', 'Oval', show_name=False, back_color=[255, 255, 255],
                    auto_hilite=False)
    p.append(marker)

    # ----- Harmony -----
    p = card('Harmony', HARMONY_SCRIPT)
    title(p, 'Harmony', 'Color schemes are geometry on the wheel: turn the hue and keep the rest. Choose a scheme, '
                        'and click the wheel for a new starting color. The Color API names each one.', 34)
    p.append(label('Scheme Label', (340, 94, 410, 116), 'Scheme', 12, bold=True))
    p.append(field('Scheme', (410, 92, 620, 118), '', 13, style='Rectangle', lock=False,
                   choices='Complementary\nAnalogous\nTriadic\nSplit-Complementary\nSquare'))
    for i in range(1, 5):
        y = 126 + (i - 1) * 56
        p.append(field(f'Swatch {i}', (340, y, 620, y + 50), '', 13, style='Rectangle', align='Center',
                       dont_save=True))
        p.append(button(f'Marker {i}', (164, 264, 176, 276), '', 'Oval', show_name=False,
                        back_color=[255, 255, 255], auto_hilite=False))
    p.append(field('Rule', (340, 352, 620, 400), '', 12, dont_save=True))
    p.append(button("The API's Own", (20, 406, 150, 432), 'on mouseUp\n  askAPIScheme\nend mouseUp', text_size=11))
    for i in range(1, 6):
        x = 160 + (i - 1) * 92
        p.append(field(f'API {i}', (x, 406, x + 86, 432), '', 10, style='Rectangle', align='Center',
                       dont_save=True, visible=False))

    # ----- Contrast -----
    p = card('Contrast', CONTRAST_SCRIPT)
    title(p, 'Contrast', 'Text has to stand out from what\'s behind it. WCAG 2, the W3C\'s accessibility '
                         'guidelines, measure contrast as a ratio, from 1:1 (none) to 21:1 (black on white).', 34)
    p.append(label('Text Label', (20, 96, 110, 116), 'Text', 12, bold=True))
    p.append(field('Text Color', (110, 92, 300, 118), '', 13, style='Rectangle', lock=False,
                   choices='#000000\n#FFFFFF\n#777777\n#0047AB\n#FDD835'))
    p.append(label('Background Label', (20, 128, 110, 148), 'Background', 12, bold=True))
    p.append(field('Background', (110, 124, 300, 150), '', 13, style='Rectangle', lock=False,
                   choices='#FFFFFF\n#000000\n#F5F5DC\n#0047AB\n#FDD835'))
    for i, (name, script) in enumerate([('Check', 'checkContrast'), ('Swap', 'swapColors'),
                                        ('Best Text', 'bestText'), ('Ask the API', 'askAPI')]):
        x = 314 + (i % 2) * 154
        y = 92 + (i // 2) * 32
        p.append(button(name, (x, y, x + 146, y + 26), f'on mouseUp\n  {script}\nend mouseUp'))
    p.append(field('Sample', (20, 162, 620, 262),
                   'Readable at a glance?\nSmall print matters most: it needs more contrast than a heading does.',
                   18, style='Rectangle', align='Center'))
    p.append(field('Ratio', (20, 276, 200, 316), '', 28, bold=True, dont_save=True))
    p.append(field('Grades', (210, 272, 620, 346), '', 12, dont_save=True))
    p.append(label('Why', (20, 352, 620, 436),
                   'The ratio compares the relative luminance of the two colors: how bright each looks, with the '
                   'eye\'s greater sensitivity to green counted in. AA is the usual goal and AAA the stricter one. '
                   'Large text is 18 point, or 14 point bold. The ratio is never rounded up: 4.49 fails a 4.5 '
                   'minimum.', 12))

    # ----- About -----
    p = card('About')
    title(p, 'About Swatches')
    p.append(label('Credits', (20, 52, 620, 120),
                   'The names and conversions come from The Color API (www.thecolorapi.com), a free service by '
                   'Josh Beckman, which needs no key. Swatches asks it only when you look something up. The color '
                   'math (hex, RGB and HSV, the wheels, WCAG contrast) is in this stack\'s own script: choose '
                   'Objects ▸ Stack Script to read it.', 12))
    p.append(label('Base URL Label', (20, 128, 110, 148), 'Base URL', 12, bold=True))
    p.append(field('Base URL', (110, 124, 450, 150), 'https://www.thecolorapi.com', 13, style='Rectangle',
                   lock=False))
    p.append(button('Test Connection', (460, 124, 620, 150),
                    'on mouseUp\n  put url apiURL("/id?hex=0047AB") into reply\n  if the result is not empty then\n    answer "No answer from The Color API:" && the result\n  else\n    answer "The Color API answered: #0047AB is" && key "value" of key "name" of json reply & "."\n  end if\nend mouseUp'))
    p.append(label('Shows Label', (20, 164, 620, 184), 'What Swatches shows about RandomCard', 13, bold=True))
    p.append(field('Shows', (20, 186, 620, 436),
                   'Convert: a web service called from HyperTalk (load url … with message, so the card never '
                   'waits), JSON read with key … of json, a combo box (the ▼ of examples), Don\'t Save fields for '
                   'results, and Copy Text to the system clipboard.\n'
                   'Mixing Light: sliders, each with its own track color, sending sliderMoved and sliderChanged.\n'
                   'Color Wheel: a picture painted by the stack\'s own script, stroke by stroke in color with the '
                   'line tool, a slider for value, and a marker that follows the mouse.\n'
                   'Harmony: several requests at once, each answer matched to its swatch, and JSON arrays '
                   '(element i of json) from the API\'s schemes.\n'
                   'Contrast: colored text and fills set from a script (foreColor and backColor), and WCAG 2\'s '
                   'formulas in plain HyperTalk.\n'
                   'Everywhere: buttons and fields in color, and functions in the stack script that every card '
                   'shares.', 12, style='Scrolling'))

    stack = {'format': 1, 'name': 'Swatches', 'script': STACK_SCRIPT.strip('\n'), 'width': WIDTH, 'height': HEIGHT,
             'next_id': next_id, 'backgrounds': [main], 'cards': cards}
    return stack


if __name__ == '__main__':
    stack = build()
    with open('examples/Swatches.rcstack', 'w') as f:
        json.dump(stack, f, indent=2, ensure_ascii=False)
    print('wrote examples/Swatches.rcstack:', len(stack['cards']), 'cards')
