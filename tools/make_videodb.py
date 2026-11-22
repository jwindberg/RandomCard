"""Write examples/VideoDB.rcstack: the front end of videodb (a movie library
with a Kotlin service and a GraphQL API), as a stack.

usage: python3 -I tools/make_videodb.py   (from the repository's top folder)

The stack talks to videodb's front end server, which passes /graphql and
/omdb on to the service; its address is on the Settings card. A card for
each of the web front end's pages: Movies (a page at a time, searched or by
first letter), Movie (one movie's details, and the ones beside it), Edit
(every field, saved with the updateMovie mutation, and an OMDb lookup).
Posters (the page as a grid, and each movie's) are images whose source is
the poster's address, and Play streams the movie from the server.
"""
import json

WIDTH, HEIGHT = 640, 480
next_id = 1

# The movie's fields the Edit card shows a line each, in two columns.
LEFT = ['title', 'imdbID', 'type', 'filePath', 'year', 'rated', 'released', 'runtime', 'genre', 'director',
        'writer', 'actors']
RIGHT = ['poster', 'language', 'country', 'awards', 'metaScore', 'imdbRating', 'imdbVotes', 'dvd', 'boxOffice',
         'production', 'website', 'totalSeasons']
LABELS = {'imdbID': 'IMDb ID', 'filePath': 'File', 'metaScore': 'MetaScore', 'imdbRating': 'IMDb rating',
          'imdbVotes': 'IMDb votes', 'dvd': 'DVD', 'boxOffice': 'Box office', 'totalSeasons': 'Seasons'}
# Every field of a movie but its ratings, as the service names them.
ALL_FIELDS = LEFT + RIGHT + ['plot', 'response']


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


def image(name, r, script='', **extra):
    p = {'id': new_id(), 'kind': 'Image', 'name': name, 'rect': rect(*r), 'script': script.strip('\n'),
         'style': 'Transparent', 'contents': '', 'dont_save': True}
    p.update(extra)
    return p


def label(name, r, text, size=12, **extra):
    return field(name, r, text, size, **extra)


def picture():
    return {'width': WIDTH, 'height': HEIGHT, 'data': ''}


STACK_SCRIPT = r'''
-- VideoDB: the front end of videodb, a movie library with a GraphQL API.
--
-- Every page of the web front end is a card here, and every request is
-- plain HyperTalk: a GraphQL query is JSON posted to /graphql, and its
-- answer is read with key … of json.

on openStack
  set the httpTimeout to 20
  pass openStack
end openStack

-- The server: videodb's front end, which passes requests on to the service.
-- Its address is each user's own: the stack's "VideoDB" connection, set up
-- in File > Connections… (or on the Settings card), and never in the stack.
-- Requests name the connection; a movie's address is made from it.
function server
  return setting "Address" of connection "VideoDB"
end server

-- A line along the bottom of every card.
on showStatus msg
  put msg into bkgnd field "Status"
end showStatus

-- ----- GraphQL -----

-- Ask the service: a query (or mutation) and its variables, a JSON object.
-- The answer's data, or "Error: …".
function graphQL query, vars
  if vars is empty then put "{}" into vars
  put "{}" into body
  set key "query" of json body to jsonString(query)
  set key "variables" of json body to vars
  set the httpHeaders to "Content-Type: application/json"
  post body to url "/graphql" with connection "VideoDB"
  if the result is not empty then return "Error: the server didn't answer (" & the result & ")."
  put it into reply
  if key "errors" of json reply is not empty then
    return "Error:" && key "message" of element 1 of key "errors" of json reply
  end if
  return key "data" of json reply
end graphQL

-- Text as a JSON string, in quotes, so that a title like "1984" stays
-- text rather than becoming a number.
function jsonString t
  put empty into s
  -- By character code: comparing a space with return or tab as text
  -- needn't say they differ.
  repeat with i = 1 to the number of chars of t
    put char i of t into c
    put charToNum(c) into code
    if code is 34 then put "\" & quote after s
    else if code is 92 then put "\\" after s
    else if code is 10 or code is 13 then put "\n" after s
    else if code is 9 then put "\t" after s
    else put c after s
  end repeat
  return quote & s & quote
end jsonString

-- Text without spaces at either end.
function trimmed t
  repeat while t is not empty and charToNum(char 1 of t) is 32
    delete char 1 of t
  end repeat
  repeat while t is not empty and charToNum(last char of t) is 32
    delete last char of t
  end repeat
  return t
end trimmed

-- ----- Movies: a page at a time -----

-- The page showing: its number (from 0), the first letter chosen, and
-- what's searched for.
on showPage
  global gPage, gLetter, gQuery, gPages
  if gPage is empty then put 0 into gPage
  showStatus "Loading…"
  put "{}" into vars
  set key "page" of json vars to gPage
  set key "size" of json vars to 24
  if gLetter is empty then set key "titleStartsWith" of json vars to "null"
  else set key "titleStartsWith" of json vars to jsonString(gLetter)
  set key "query" of json vars to jsonString(gQuery)
  get graphQL("query Movies($page: Int!, $size: Int!, $titleStartsWith: String, $query: String) { movies(page: $page, size: $size, titleStartsWith: $titleStartsWith, query: $query) { content { id title year poster } totalPages totalElements number } }", vars)
  if word 1 of it is "Error:" then
    showStatus it
    exit showPage
  end if
  put key "movies" of json it into thePage
  put key "content" of json thePage into movies
  put empty into titles
  put empty into ids
  put empty into posters
  repeat with i = 1 to the number of elements of json movies
    put element i of json movies into m
    put key "title" of json m into t
    if key "year" of json m is not empty then put " (" & key "year" of json m & ")" after t
    put t & return after titles
    put key "id" of json m & return after ids
    put key "poster" of json m & return after posters
  end repeat
  showPosters titles, ids, posters
  delete last char of titles
  delete last char of ids
  put titles into card field "List" of card "Movies"
  put ids into card field "IDs" of card "Movies"
  put key "totalPages" of json thePage into gPages
  put key "totalElements" of json thePage into total
  if total is 1 then put "1 movie" into total
  else put total && "movies" into total
  put "Page" && gPage + 1 && "of" && max(1, gPages) & "," && total into card field "Page" of card "Movies"
  put card field "Page" of card "Movies" into card field "Page" of card "Posters"
  showStatus empty
end showPage

-- The Posters card: the same page as a grid, each poster an image over
-- its title (which shows until the poster comes, or if there's none).
on showPosters titles, ids, posters
  repeat with i = 1 to 24
    put line i of titles into card field ("T" & i) of card "Posters"
    put line i of posters into card image ("P" & i) of card "Posters"
  end repeat
  put ids into card field "IDs" of card "Posters"
  put card field "Page" of card "Movies" into card field "Page" of card "Posters"
end showPosters

on openPoster n
  get line n of card field "IDs" of card "Posters"
  if it is not empty then showMovie it
end openPoster

on turnPage delta
  global gPage, gPages
  if gPage + delta < 0 or gPage + delta >= gPages then
    beep
    exit turnPage
  end if
  add delta to gPage
  showPage
end turnPage

-- Titles starting with a letter, or all of them.
on showLetter letter
  global gPage, gLetter, gQuery
  if letter is "All" then put empty into gLetter
  else put letter into gLetter
  put empty into gQuery
  put empty into card field "Search" of card "Movies"
  put 0 into gPage
  showPage
end showLetter

on searchFor what
  global gPage, gQuery
  put trimmed(what) into gQuery
  put 0 into gPage
  showPage
end searchFor

-- As the web page does: search half a second after the typing stops, so
-- emptying the box shows every movie again.
on searchWhenTypingStops
  global gQuery, gTyped
  if gTyped is empty then exit searchWhenTypingStops
  if the ticks - gTyped < 30 then exit searchWhenTypingStops
  put empty into gTyped
  if trimmed(card field "Search") is not gQuery then searchFor card field "Search"
end searchWhenTypingStops

-- ----- Movie: one movie's details -----

on showMovie id
  global gMovie
  if id is empty then exit showMovie
  showStatus "Loading…"
  put "{}" into vars
  set key "id" of json vars to jsonString(id)
  get graphQL("query Movie($id: ID!) { movie(id: $id) { id title poster year rated released runtime genre director writer actors plot language country awards ratings { source value } metaScore imdbRating imdbVotes imdbID type dvd boxOffice production website response totalSeasons filePath } neighbors(id: $id) { previous { id title } next { id title } } }", vars)
  if word 1 of it is "Error:" then
    showStatus it
    exit showMovie
  end if
  put it into data
  put key "movie" of json data into gMovie
  if gMovie is empty then
    showStatus "There's no movie" && id & "."
    exit showMovie
  end if
  if the short name of this card is not "Movie" then go to card "Movie"
  put id into card field "ID"
  put movieField("title") into card field "Title"
  put empty into meta
  repeat with i = 1 to 4
    put movieField(item i of "year,rated,runtime,genre") into v
    if v is not empty then
      if meta is not empty then put "  •  " after meta
      put v after meta
    end if
  end repeat
  put meta into card field "Meta"
  put movieField("plot") into card field "Plot"
  put movieField("poster") into card image "Poster"
  showInfo
  put key "previous" of key "neighbors" of json data into prev
  put key "next" of key "neighbors" of json data into nxt
  put key "id" of json prev into card field "Previous ID"
  put key "id" of json nxt into card field "Next ID"
  put key "title" of json prev into card field "Previous Title"
  put key "title" of json nxt into card field "Next Title"
  showStatus empty
end showMovie

-- A field of the movie showing.
function movieField fieldName
  global gMovie
  return key fieldName of json gMovie
end movieField

-- The details, a line each, with their labels in bold.
on showInfo
  global gMovie
  put "Director: " & movieField("director") & return & ¬
      "Writer: " & movieField("writer") & return & ¬
      "Actors: " & movieField("actors") & return & return & ¬
      "Production: " & movieField("production") & return & ¬
      "Country: " & movieField("country") & return & ¬
      "Language: " & movieField("language") & return & ¬
      "Awards: " & movieField("awards") & return & return & ¬
      "IMDb: " & movieField("imdbRating") & " / 10 (" & movieField("imdbVotes") & " votes)" & return & ¬
      "MetaScore: " & movieField("metaScore") into info
  put key "ratings" of json gMovie into ratings
  repeat with i = 1 to the number of elements of json ratings
    put return & key "source" of element i of json ratings & ": " & key "value" of element i of json ratings after info
  end repeat
  put return & return & ¬
      "Released: " & movieField("released") & return & ¬
      "Box office: " & movieField("boxOffice") & return & ¬
      "DVD: " & movieField("dvd") & return & ¬
      "Type: " & movieField("type") after info
  if movieField("totalSeasons") is not empty then put " (" & movieField("totalSeasons") & " seasons)" after info
  put return & "Website: " & movieField("website") & return & return & ¬
      "File: " & movieField("filePath") & return & ¬
      "IMDb ID: " & movieField("imdbID") after info
  put info into card field "Info"
  repeat with i = 1 to the number of lines of info
    get offset(":", line i of info)
    if it > 0 then set the textStyle of char 1 to it of line i of card field "Info" to bold
  end repeat
end showInfo

-- ----- Edit: every field, saved -----

-- The fields the Edit card has a line for, and the plot.
function movieFields
  return "''' + ','.join(ALL_FIELDS) + r'''"
end movieFields

on editMovie
  global gMovie
  if gMovie is empty then exit editMovie
  go to card "Edit"
  put movieFields() into names
  repeat with i = 1 to the number of items of names
    put movieField(item i of names) into card field (item i of names)
  end repeat
  put key "ratings" of json gMovie into ratings
  put empty into lines
  repeat with i = 1 to the number of elements of json ratings
    put key "source" of element i of json ratings & ": " & key "value" of element i of json ratings & return after lines
  end repeat
  delete last char of lines
  put lines into card field "ratings"
end editMovie

-- The form as the service's MovieInput: every field as text, and the
-- ratings ("Source: value" a line) as a list.
function movieInput
  put "{}" into input
  put movieFields() into names
  repeat with i = 1 to the number of items of names
    set key (item i of names) of json input to jsonString(card field (item i of names) of card "Edit")
  end repeat
  set key "ratings" of json input to "[]"
  put card field "ratings" of card "Edit" into lines
  put 0 into n
  repeat with i = 1 to the number of lines of lines
    put line i of lines into r
    if trimmed(r) is empty then next repeat
    add 1 to n
    put offset(":", r) into colonAt
    if colonAt is 0 then
      put trimmed(r) into source
      put empty into value
    else
      put trimmed(char 1 to colonAt - 1 of r) into source
      put trimmed(char colonAt + 1 to length(r) of r) into value
    end if
    set key "source" of element n of key "ratings" of json input to jsonString(source)
    set key "value" of element n of key "ratings" of json input to jsonString(value)
  end repeat
  return input
end movieInput

on saveMovie
  global gMovie
  put movieField("id") into id
  if card field "title" of card "Edit" is empty then
    answer "A movie needs a title."
    exit saveMovie
  end if
  showStatus "Saving…"
  put "{}" into vars
  set key "id" of json vars to jsonString(id)
  set key "input" of json vars to movieInput()
  get graphQL("mutation UpdateMovie($id: ID!, $input: MovieInput!) { updateMovie(id: $id, input: $input) { id } }", vars)
  if word 1 of it is "Error:" then
    showStatus empty
    answer "The movie wasn't saved." & return & it
    exit saveMovie
  end if
  showMovie id
end saveMovie

-- OMDb's details for the title or IMDb ID, copied into the form if wanted.
on omdbLookup
  put "/omdb?title=" & urlEncode(card field "title") & "&imdbId=" & urlEncode(card field "imdbID") into u
  showStatus "Asking OMDb…"
  put url u with connection "VideoDB" into reply
  showStatus empty
  if the result is not empty then
    answer "OMDb didn't answer:" && the result
    exit omdbLookup
  end if
  if key "title" of json reply is empty then
    answer "OMDb found nothing for that title or IMDb ID."
    exit omdbLookup
  end if
  answer "OMDb found" && key "title" of json reply && "(" & key "year" of json reply & ")," && ¬
      "directed by" && key "director" of json reply & "." & return & return & key "plot" of json reply & return & return & ¬
      "Copy its details into the form?" with "Cancel" or "Copy"
  if it is not "Copy" then exit omdbLookup
  put movieFields() into names
  repeat with i = 1 to the number of items of names
    put item i of names into fieldName
    if fieldName is "metaScore" then get key "metascore" of json reply
    else get key fieldName of json reply
    if it is not empty and fieldName is not "filePath" then put it into card field fieldName
  end repeat
  put key "ratings" of json reply into ratings
  if the number of elements of json ratings > 0 then
    put empty into lines
    repeat with i = 1 to the number of elements of json ratings
      put element i of json ratings into r
      get key "Source" of json r
      if it is empty then get key "source" of json r
      put it & ": " into line i of lines
      get key "Value" of json r
      if it is empty then get key "value" of json r
      put it after line i of lines
    end repeat
    put lines into card field "ratings"
  end if
  showStatus "Copied from OMDb: save to keep it."
end omdbLookup
'''

BG_SCRIPT = ''


def build():
    global next_id
    next_id = 1
    main = {'id': new_id(), 'name': 'Main', 'script': BG_SCRIPT, 'picture': picture(), 'show_pict': True,
            'parts': [
                button('Movies', (20, 446, 110, 470), 'on mouseUp\n  go to card "Movies"\nend mouseUp'),
                button('Settings', (120, 446, 210, 470), 'on mouseUp\n  go to card "Settings"\nend mouseUp'),
                field('Status', (220, 448, 620, 468), '', 11, align='Right', shared_text=True, dont_save=True),
            ]}
    cards = []

    def card(name, script=''):
        c = {'id': new_id(), 'name': name, 'script': script.strip('\n'), 'background': main['id'],
             'picture': picture(), 'show_pict': True, 'parts': []}
        cards.append(c)
        return c['parts']

    def title(parts, text):
        parts.append(label('Heading', (20, 14, 400, 46), text, 24, bold=True))

    # ----- Movies -----
    p = card('Movies', '''
on openCard
  if card field "List" is empty then showPage
  pass openCard
end openCard

on idle
  searchWhenTypingStops
  pass idle
end idle
''')
    title(p, 'VideoDB')
    search = field('Search', (400, 18, 540, 42), '', 13, style='Rectangle', lock=False, dont_wrap=True)
    search['script'] = ('on keyDown\n  global gTyped\n  put the ticks into gTyped\n  pass keyDown\nend keyDown\n\n'
                        'on returnInField\n  global gTyped\n  put empty into gTyped\n  searchFor me\nend returnInField\n\n'
                        'on enterInField\n  returnInField\nend enterInField')
    p.append(search)
    p.append(button('Search', (548, 18, 620, 42), 'on mouseUp\n  searchFor card field "Search"\nend mouseUp'))
    # A button for each first letter, and All.
    x = 20
    for name in ['All'] + list('ABCDEFGHIJKLMNOPQRSTUVWXYZ'):
        w = 40 if name == 'All' else 21
        p.append(button(name, (x, 50, x + w, 74), 'on mouseUp\n  showLetter the short name of me\nend mouseUp',
                        style='Transparent', text_size=13, text_style='bold', auto_hilite=True))
        x += w + (1 if name == 'All' else 0)
    movie_list = field('List', (20, 80, 620, 404), '', 13, style='Scrolling', dont_save=True)
    movie_list['script'] = ('on mouseUp\n  put word 2 of the clickLine into n\n  if n is empty then exit mouseUp\n'
                            '  showMovie line n of card field "IDs"\nend mouseUp')
    p.append(movie_list)
    p.append(field('IDs', (20, 80, 100, 100), '', 10, visible=False, dont_save=True))
    p.append(button('◀ Previous', (20, 412, 130, 436), 'on mouseUp\n  turnPage -1\nend mouseUp'))
    p.append(field('Page', (140, 414, 400, 434), '', 12, align='Center', dont_save=True))
    p.append(button('Posters', (410, 412, 500, 436), 'on mouseUp\n  go to card "Posters"\nend mouseUp'))
    p.append(button('Next ▶', (510, 412, 620, 436), 'on mouseUp\n  turnPage 1\nend mouseUp'))

    # ----- Posters: the same page as a grid -----
    p = card('Posters', '''
on openCard
  if card field "IDs" is empty then showPage
  pass openCard
end openCard
''')
    title(p, 'VideoDB')
    p.append(button('List', (548, 18, 620, 42), 'on mouseUp\n  go to card "Movies"\nend mouseUp'))
    for i in range(24):
        col, row = i % 8, i // 8
        x, y = 20 + col * 75, 52 + row * 118
        n = i + 1
        p.append(field(f'T{n}', (x, y, x + 70, y + 106), '', 9, style='Rectangle', align='Center', dont_save=True))
        p.append(image(f'P{n}', (x, y, x + 70, y + 106), f'on mouseUp\n  openPoster {n}\nend mouseUp', fit='Fill'))
    p.append(field('IDs', (20, 80, 100, 100), '', 10, visible=False, dont_save=True))
    p.append(button('◀ Previous', (20, 412, 130, 436), 'on mouseUp\n  turnPage -1\nend mouseUp'))
    p.append(field('Page', (140, 414, 500, 434), '', 12, align='Center', dont_save=True))
    p.append(button('Next ▶', (510, 412, 620, 436), 'on mouseUp\n  turnPage 1\nend mouseUp'))

    # ----- Movie -----
    p = card('Movie', '''
-- The arrow keys go to the movies beside this one.
on arrowKey which
  if which is "left" then send mouseUp to card button "◀"
  else if which is "right" then send mouseUp to card button "▶"
  else pass arrowKey
end arrowKey
''')
    p.append(field('Title', (20, 14, 400, 46), '', 22, bold=True, dont_save=True))
    p.append(button('Edit', (408, 18, 468, 42), 'on mouseUp\n  editMovie\nend mouseUp'))
    p.append(button('Play', (476, 18, 536, 42),
                    'on mouseUp\n  -- The movie plays from the server, in a window of its own.\n'
                    '  movie server() & "/videos/" & card field "ID"\n'
                    '  if the result is not empty then answer "The movie didn\'t open:" && the result\nend mouseUp'))
    p.append(button('List', (544, 18, 620, 42), 'on mouseUp\n  go to card "Movies"\nend mouseUp'))
    p.append(field('Meta', (20, 48, 620, 68), '', 12, text_style='italic', dont_save=True))
    # The poster: an image over a field that says there's none, which shows
    # until the picture comes, or when it can't.
    p.append(field('No Poster', (20, 76, 150, 266), '\n\n\nNo poster', 11, style='Rectangle', align='Center'))
    p.append(image('Poster', (20, 76, 150, 266), fit='Fit'))
    p.append(field('Plot', (160, 76, 620, 156), '', 12, style='Scrolling', dont_save=True))
    p.append(field('Info', (160, 164, 620, 404), '', 12, style='Scrolling', dont_save=True))
    p.append(button('◀', (20, 412, 60, 436), 'on mouseUp\n  showMovie card field "Previous ID"\nend mouseUp'))
    p.append(field('Previous Title', (66, 414, 330, 434), '', 11, dont_save=True))
    p.append(field('Next Title', (334, 414, 574, 434), '', 11, align='Right', dont_save=True))
    p.append(button('▶', (580, 412, 620, 436), 'on mouseUp\n  showMovie card field "Next ID"\nend mouseUp'))
    for name in ['ID', 'Previous ID', 'Next ID']:
        p.append(field(name, (20, 280, 150, 300), '', 10, visible=False, dont_save=True))

    # ----- Edit -----
    p = card('Edit')
    title(p, 'Edit')
    p.append(button('OMDb Lookup', (300, 18, 410, 42), 'on mouseUp\n  omdbLookup\nend mouseUp'))
    p.append(button('Cancel', (418, 18, 510, 42), 'on mouseUp\n  go to card "Movie"\nend mouseUp'))
    p.append(button('Save', (518, 18, 620, 42), 'on mouseUp\n  saveMovie\nend mouseUp'))
    for col, names in enumerate([LEFT, RIGHT]):
        x = 20 + col * 306
        for row, name in enumerate(names):
            y = 54 + row * 26
            p.append(label(name + ' label', (x, y + 3, x + 86, y + 21), LABELS.get(name, name.capitalize()), 11,
                           align='Right', bold=True))
            p.append(field(name, (x + 92, y, x + 294, y + 22), '', 12, style='Rectangle', lock=False, dont_wrap=True,
                           dont_save=True))
    p.append(label('plot label', (20, 370, 106, 388), 'Plot', 11, align='Right', bold=True))
    p.append(field('plot', (112, 366, 370, 438), '', 12, style='Scrolling', lock=False, dont_save=True))
    p.append(label('ratings label', (376, 370, 430, 388), 'Ratings', 11, align='Right', bold=True))
    p.append(field('ratings', (436, 366, 620, 438), '', 11, style='Scrolling', lock=False, dont_wrap=True,
                   dont_save=True))
    p.append(field('response', (20, 300, 100, 320), '', 10, visible=False, dont_save=True))

    # ----- Settings -----
    p = card('Settings', '''
-- The server's address is the user's, kept on their computer: the box shows
-- it, and changing it sets it.
on openCard
  put setting "Address" of connection "VideoDB" into card field "Server"
  pass openCard
end openCard
''')
    title(p, 'Settings')
    p.append(label('Server label', (20, 60, 110, 80), 'Server', 13, bold=True))
    server = field('Server', (110, 56, 450, 82), '', 13, style='Rectangle', lock=False, dont_wrap=True,
                   dont_save=True)
    server['script'] = ('on closeField\n  set setting "Address" of connection "VideoDB" to me\n'
                        '  put setting "Address" of connection "VideoDB" into me\nend closeField')
    p.append(server)
    p.append(button('Test', (460, 56, 620, 82),
                    'on mouseUp\n  get graphQL("query { movies(page: 0, size: 1) { totalElements } }")\n'
                    '  if word 1 of it is "Error:" then answer it\n'
                    '  else answer "videodb answered: it has" && key "totalElements" of key "movies" of json it && "movies."\n'
                    'end mouseUp'))
    p.append(button('Connections…', (460, 90, 620, 116), 'on mouseUp\n  doMenu "Connections…"\nend mouseUp'))
    p.append(label('About', (20, 130, 620, 430),
                   'VideoDB is the front end of videodb, a movie library: a Kotlin service with a GraphQL API, and a '
                   'React front end on the web. This stack does what the web pages do, from HyperTalk: a GraphQL '
                   'request is JSON posted to /graphql (the graphQL function in the stack script), and the answer is '
                   'read with key … of json.\n\n'
                   'Movies: a page of 24 at a time, searched, or by first letter. Click a movie for its details.\n'
                   'Movie: its details, and the movies before and after it (the arrow keys go there too).\n'
                   'Edit: every field, saved with the updateMovie mutation; OMDb Lookup fills the form from OMDb.\n\n'
                   'Posters: the page as a grid of posters, each an image whose source is the poster\'s address; it loads in the background. Play plays the movie from the server, in a movie window.\n\n'
                   'The server is videodb\'s web front end, which passes /graphql, /omdb and /videos on to the '
                   'service. Its address is yours, not the stack\'s: the stack\'s VideoDB connection, kept on '
                   'this computer: File > Connections… sets it up.', 12))

    # The stack's identity stays the same each time it's written, so what
    # users set up for its connection lasts.
    connection = {'name': 'VideoDB',
                  'description': "videodb's web front end, which passes /graphql, /omdb and /videos on to the service.",
                  'settings': [{'name': 'Address', 'hint': 'The front end server, such as http://videos.local:9090',
                                'default': 'http://trashcan.local:9090', 'secret': False},
                               # A server with HTTP Basic needs these; an open one doesn't.
                               {'name': 'User name', 'hint': 'If the server asks you to sign in',
                                'optional': True},
                               {'name': 'Password', 'hint': 'Kept in the system\'s secure store', 'secret': True,
                                'optional': True}],
                  'credentials': [{'place': 'Basic', 'value': '{User name}:{Password}'}]}
    stack = {'format': 1, 'uid': 'a8c0f2b6d4e94f1e9b3c5d7e6f8a1b2c', 'name': 'VideoDB',
             'script': STACK_SCRIPT.strip('\n'), 'width': WIDTH, 'height': HEIGHT, 'next_id': next_id,
             'backgrounds': [main], 'connections': [connection], 'cards': cards}
    return stack


if __name__ == '__main__':
    stack = build()
    with open('examples/VideoDB.rcstack', 'w') as f:
        json.dump(stack, f, indent=2, ensure_ascii=False)
    print('wrote examples/VideoDB.rcstack:', len(stack['cards']), 'cards')
