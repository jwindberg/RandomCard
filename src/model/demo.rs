//! The stack RandomCard opens when started without a file: a short tour that
//! doubles as a set of working HyperTalk examples.

use super::*;
use styles::TextStyle;

struct Builder {
    stack: Stack,
}

impl Builder {
    fn id(&mut self) -> Id {
        self.stack.alloc_id()
    }

    fn background(&mut self, name: &str, script: &str) -> Id {
        let id = self.id();
        let (w, h) = (self.stack.width, self.stack.height);
        self.stack.backgrounds.push(Background {
            id,
            name: name.into(),
            script: script.into(),
            picture: Picture::new(w, h),
            show_pict: true,
            ..Background::default()
        });
        id
    }

    fn card(&mut self, bg: Id, name: &str) -> usize {
        let id = self.id();
        let (w, h) = (self.stack.width, self.stack.height);
        self.stack.cards.push(Card {
            id,
            name: name.into(),
            background: bg,
            picture: Picture::new(w, h),
            show_pict: true,
            ..Card::default()
        });
        self.stack.cards.len() - 1
    }

    fn bg_part(&mut self, bg: Id, part: Part) {
        self.stack.background_mut(bg).unwrap().parts.push(part);
    }

    fn card_part(&mut self, card: usize, part: Part) {
        self.stack.cards[card].parts.push(part);
    }

    fn button(&mut self, name: &str, rect: Rect, script: &str) -> Part {
        let mut p = Part::new_button(self.id(), name, rect);
        p.script = script.trim_start().into();
        p
    }

    fn label(&mut self, name: &str, rect: Rect, text: &str, size: u32) -> Part {
        let mut p = Part::new_field(self.id(), name, rect);
        p.style = PartStyle::Transparent;
        p.lock_text = true;
        // Labels on a background show the same text on every card.
        p.shared_text = true;
        p.text_size = size;
        p.contents = text.into();
        p
    }
}

/// Give the first `needle` in a field's text a style.
fn style_text(field: &mut Part, needle: &str, style: &TextStyle) {
    let Some(at) = field.contents.find(needle) else { return };
    let start = field.contents[..at].chars().count();
    let (end, len) = (start + needle.chars().count(), field.contents.chars().count());
    styles::apply(&mut field.styles, start, end, len, |s| *s = style.clone());
}

fn hline(p: &mut Picture, x0: i32, x1: i32, y: i32) {
    for x in x0..=x1 {
        p.set(x, y, Pixel::BLACK);
    }
}

fn vline(p: &mut Picture, x: i32, y0: i32, y1: i32) {
    for y in y0..=y1 {
        p.set(x, y, Pixel::BLACK);
    }
}

fn line(p: &mut Picture, x0: i32, y0: i32, x1: i32, y1: i32) {
    let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for i in 0..=steps {
        let x = x0 + (x1 - x0) * i / steps;
        let y = y0 + (y1 - y0) * i / steps;
        p.set(x, y, Pixel::BLACK);
    }
}

fn frame(p: &mut Picture, l: i32, t: i32, r: i32, b: i32) {
    hline(p, l, r, t);
    hline(p, l, r, b);
    vline(p, l, t, b);
    vline(p, r, t, b);
}

fn circle(p: &mut Picture, cx: i32, cy: i32, r: i32, filled: bool) {
    for y in -r..=r {
        for x in -r..=r {
            let d = x * x + y * y;
            if d <= r * r && (filled || d >= (r - 1) * (r - 1)) {
                p.set(cx + x, cy + y, Pixel::BLACK);
            }
        }
    }
}

fn house(p: &mut Picture) {
    // A little house with a sun, drawn the way you might with the paint tools.
    for y in 191..260 {
        for x in 91..209 {
            p.set(x, y, Pixel::WHITE);
        }
    }
    frame(p, 90, 190, 210, 260);
    line(p, 80, 190, 150, 140);
    line(p, 150, 140, 220, 190);
    hline(p, 80, 220, 190);
    frame(p, 135, 220, 165, 260);
    frame(p, 105, 205, 125, 225);
    frame(p, 175, 205, 195, 225);
    circle(p, 300, 150, 18, true);
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::PI / 4.0;
        let (c, s) = (a.cos(), a.sin());
        line(
            p,
            300 + (c * 24.0) as i32,
            150 + (s * 24.0) as i32,
            300 + (c * 34.0) as i32,
            150 + (s * 34.0) as i32,
        );
    }
    hline(p, 20, 490, 261);
    for x in (30..480).step_by(37) {
        line(p, x, 270, x + 8, 262);
    }
}

const STACK_SCRIPT: &str = r#"-- A menu of the stack's own. Its items arrive as doMenu messages.
on openStack
  if there is no menu "Tour" then create menu "Tour"
  put "Contents,Basics,Dice,Drawing,Animation,Sound,Keys,Fields,Print & Find,Icons,Debugging,Palettes,The Web,Japanese,-,Address Book" into menu "Tour"
end openStack

on doMenu what
  if what is "Address Book" then
    visual effect zoom open
    go to first card of background "Address"
  else if "," & what & "," is in ",Contents,Basics,Dice,Drawing,Animation,Sound,Keys,Fields,Print & Find,Icons,Debugging,Palettes,The Web,Japanese," then
    visual effect dissolve
    go to card what
  else
    pass doMenu
  end if
end doMenu

on openCard
  if there is a background field "Page" then
    put "Card" && the number of this card && "of" && the number of cards into background field "Page"
  end if
end openCard

function plural n, noun
  if n = 1 then return n && noun
  return n && noun & "s"
end plural
"#;

const ADDRESS_BG_SCRIPT: &str = r#"-- Visitors use the address book; they don't redesign it.
on openBackground
  global savedLevel
  put the userLevel into savedLevel
  set the userLevel to 2
end openBackground

on closeBackground
  global savedLevel
  if savedLevel is a number then set the userLevel to savedLevel
end closeBackground

on openCard
  put the number of cards - the number of cards of this background into others
  put the number of this card - others && "of" && the number of cards of this background into field "Count"
end openCard
"#;

/// The Tour's palette: four buttons in a grid, drawn here instead of read
/// from a 'PLTE' resource.
fn tour_palette() -> super::Palette {
    let (cell_w, cell_h) = (72, 24);
    let (w, h) = (cell_w * 2 + 1, cell_h * 2 + 1);
    let mut pic = Picture::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            pic.set(x, y, Pixel::WHITE);
        }
    }
    frame(&mut pic, 0, 0, w - 1, h - 1);
    hline(&mut pic, 0, w - 1, cell_h);
    vline(&mut pic, cell_w, 0, h - 1);
    let cells = [("Prev", "go to previous card"), ("Next", "go to next card"), ("Contents", "go to card \"Contents\""), ("Beep", "beep")];
    let mut buttons = Vec::new();
    for (i, (label, message)) in cells.iter().enumerate() {
        let (l, t) = ((i as i32 % 2) * cell_w, (i as i32 / 2) * cell_h);
        crate::paint::draw_text(&mut pic, l + 10, t + 5, label, &crate::paint::PaintText { size: 12, font: "Geneva", style: "bold", color: Pixel::BLACK });
        let rect = Rect::new(l + 1, t + 1, l + cell_w, t + cell_h);
        buttons.push(super::PaletteButton { rect, message: message.to_string() });
    }
    super::Palette { name: "Tour Palette".into(), picture: pic, buttons }
}

pub fn build() -> Stack {
    let mut s = Stack::new("Tour");
    s.backgrounds.clear();
    s.cards.clear();
    s.script = STACK_SCRIPT.into();
    let mut b = Builder { stack: s };

    // ----- The main background: navigation along the bottom -----
    let main = b.background("Main", "");
    {
        let pic = &mut b.stack.background_mut(main).unwrap().picture;
        hline(pic, 0, 511, 306);
        hline(pic, 0, 511, 308);
    }
    let prev = b.button("◀", Rect::new(8, 314, 52, 336), "on mouseUp\n  visual effect scroll right fast\n  go to previous card\nend mouseUp");
    let next = b.button("▶", Rect::new(58, 314, 102, 336), "on mouseUp\n  visual effect scroll left fast\n  go to next card\nend mouseUp");
    // The Tour's own contents; Go ▸ Home is the Home stack.
    let contents = b.button(
        "Contents",
        Rect::new(108, 314, 184, 336),
        "on mouseUp\n  visual effect iris close\n  go to card \"Contents\"\nend mouseUp",
    );
    for mut p in [prev, next, contents] {
        p.style = PartStyle::RoundRect;
        b.bg_part(main, p);
    }
    let mut page = b.label("Page", Rect::new(330, 316, 504, 334), "", 11);
    page.text_align = TextAlign::Right;
    b.bg_part(main, page);

    // ----- Contents -----
    let c = b.card(main, "Contents");
    let mut title = b.label("Title", Rect::new(40, 18, 472, 62), "The RandomCard Tour", 30);
    title.text_align = TextAlign::Center;
    title.text_style = "bold".into();
    b.card_part(c, title);
    let mut sub = b.label("Subtitle", Rect::new(40, 64, 472, 86), "HyperCard, recreated in Rust", 15);
    sub.text_align = TextAlign::Center;
    sub.text_style = "italic".into();
    b.card_part(c, sub);
    let mut intro = b.label(
        "Intro",
        Rect::new(56, 96, 456, 132),
        "Click a button to explore. To read a button's script, choose the Button tool and double-click the button.",
        12,
    );
    intro.text_align = TextAlign::Center;
    b.card_part(c, intro);
    let buttons = [
        ("Start with the Basics", "on mouseUp\n  visual effect dissolve\n  go to card \"Basics\"\nend mouseUp"),
        ("Talk to Me", "on mouseUp\n  ask \"What is your name?\" with \"Bill Atkinson\"\n  if it is empty then exit mouseUp\n  answer \"Hello,\" && it & \"! Welcome to RandomCard.\" with \"Thanks\"\nend mouseUp"),
        ("Dice & Effects", "on mouseUp\n  visual effect checkerboard\n  go to card \"Dice\"\nend mouseUp"),
        ("Draw Something", "on mouseUp\n  visual effect wipe left\n  go to card \"Drawing\"\nend mouseUp"),
        ("Animation", "on mouseUp\n  visual effect venetian blinds\n  go to card \"Animation\"\nend mouseUp"),
        ("Sound", "on mouseUp\n  visual effect barn door open\n  go to card \"Sound\"\nend mouseUp"),
        ("Keys", "on mouseUp\n  visual effect wipe up\n  go to card \"Keys\"\nend mouseUp"),
        ("Fields & Text", "on mouseUp\n  visual effect dissolve\n  go to card \"Fields\"\nend mouseUp"),
        ("Print & Find", "on mouseUp\n  visual effect scroll left\n  go to card \"Print & Find\"\nend mouseUp"),
        ("Icons", "on mouseUp\n  visual effect zoom open\n  go to card \"Icons\"\nend mouseUp"),
        ("Debugging", "on mouseUp\n  visual effect iris open\n  go to card \"Debugging\"\nend mouseUp"),
        ("Palettes", "on mouseUp\n  visual effect wipe right\n  go to card \"Palettes\"\nend mouseUp"),
        ("Address Book", "on mouseUp\n  visual effect zoom open\n  go to first card of background \"Address\"\nend mouseUp"),
        ("The Web", "on mouseUp\n  visual effect dissolve\n  go to card \"The Web\"\nend mouseUp"),
        ("Japanese", "on mouseUp\n  visual effect wipe left\n  go to card \"Japanese\"\nend mouseUp"),
    ];
    for (i, (name, script)) in buttons.iter().enumerate() {
        let (col, row) = ((i % 3) as i32, (i / 3) as i32);
        let x = 36 + col * 150;
        let y = 142 + row * 32;
        let p = b.button(name, Rect::new(x, y, x + 140, y + 26), script);
        b.card_part(c, p);
    }

    // ----- HyperTalk basics -----
    let c = b.card(main, "Basics");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "HyperTalk in a nutshell", 20);
    b.card_part(c, {
        let mut t = t;
        t.text_style = "bold".into();
        t
    });
    let mut lesson = Part::new_field(b.id(), "Lesson", Rect::new(20, 40, 340, 296));
    lesson.style = PartStyle::Scrolling;
    lesson.lock_text = true;
    lesson.text_size = 12;
    lesson.contents = "Scripts are made of handlers that respond to messages. When you click a button, RandomCard sends it \"mouseUp\":\n\non mouseUp\n  put \"Hello\" into field 1\nend mouseUp\n\nIf the button doesn't handle a message it travels on: button → card → background → stack. That is the message path.\n\nEverything is text. Chunk expressions pick text apart:\n\n  put word 2 of line 3 of field \"Notes\" into x\n  put \"!\" after the last char of x\n\nAsk and answer talk to the user:\n\n  ask \"Your name?\"\n  answer \"Hi\" && it\n\nTry the buttons on the right, then open their scripts with the Button tool. Or type into the message box (SHORTCUT_M), e.g.\n\n  put the number of cards\n  go to card \"Dice\"\n  answer the long date\n\nClick any word in this text: the field's script shows the clickText and clickLine. The list under the buttons is an autoSelect field, and its script goes to the card you pick.".replace("SHORTCUT_M", &shortcut("M"));
    // Code in Monaco, key ideas in bold.
    let code = TextStyle { font: Some("Monaco".into()), size: Some(9), style: None, color: None };
    let bold = TextStyle { style: Some("bold".into()), ..Default::default() };
    for (needle, style) in [
        ("handlers", &bold),
        ("messages", &bold),
        ("on mouseUp\n  put \"Hello\" into field 1\nend mouseUp", &code),
        ("message path", &bold),
        ("Everything is text.", &bold),
        ("  put word 2 of line 3 of field \"Notes\" into x\n  put \"!\" after the last char of x", &code),
        ("  ask \"Your name?\"\n  answer \"Hi\" && it", &code),
        ("  put the number of cards\n  go to card \"Dice\"\n  answer the long date", &code),
    ] {
        style_text(&mut lesson, needle, style);
    }
    lesson.script = "on mouseUp\n  if the clickText is not empty then\n    put \"You clicked\" && quote & the clickText & quote && \"in\" && the clickLine\n  end if\nend mouseUp".into();
    b.card_part(c, lesson);
    let mut places = Part::new_field(b.id(), "Places", Rect::new(360, 246, 492, 296));
    places.lock_text = true;
    places.auto_select = true;
    places.text_size = 12;
    places.contents = "Dice\nDrawing\nAnimation".into();
    places.script = "on mouseUp\n  if the selectedText is empty then exit mouseUp\n  visual effect dissolve\n  go to card (the selectedText)\nend mouseUp".into();
    b.card_part(c, places);
    let basics = [
        ("Say Hello", "on mouseUp\n  ask \"What is your name?\" with \"Dan Winkler\"\n  if the result is \"Cancel\" then exit mouseUp\n  answer \"Hello,\" && it & \"!\"\nend mouseUp"),
        ("Count Words", "on mouseUp\n  put the number of words in field \"Lesson\" into n\n  answer \"The lesson has\" && plural(n, \"word\") && \"in\" && plural(the number of lines in field \"Lesson\", \"line\") & \".\"\nend mouseUp"),
        ("Chunks", "on mouseUp\n  put \"apple,banana,cherry\" into fruits\n  put item 2 of fruits into fruit\n  answer \"The second fruit is\" && fruit & \", which starts with\" && quote & char 1 of fruit & quote & \".\"\nend mouseUp"),
        ("Do Some Math", "on mouseUp\n  put 0 into total\n  repeat with i = 1 to 100\n    add i to total\n  end repeat\n  answer \"1 + 2 + ... + 100 =\" && total & return & \"The square root of 2 is\" && sqrt(2)\nend mouseUp"),
        ("What Time Is It?", "on mouseUp\n  answer \"It is\" && the time && \"on\" && the long date & \".\"\nend mouseUp"),
        ("Where Am I?", "on mouseUp\n  put the pathSeparator into sep\n  put the long name of this stack into p\n  delete char 1 to 7 of p\n  delete last char of p\n  if sep is not in p then\n    answer \"This stack isn't saved yet. Paths here use\" && quote & sep & quote & \", the pathSeparator.\"\n    exit mouseUp\n  end if\n  set the itemDelimiter to sep\n  put the number of items of p into n\n  answer \"This stack is\" && quote & item n of p & quote && \"in\" & return & item 1 to n - 1 of p & sep & return & return & \"Paths here use\" && quote & sep & quote & \", the pathSeparator.\"\nend mouseUp"),
    ];
    for (i, (name, script)) in basics.iter().enumerate() {
        let y = 48 + i as i32 * 33;
        let p = b.button(name, Rect::new(360, y, 492, y + 26), script);
        b.card_part(c, p);
    }

    // ----- Dice -----
    let c = b.card(main, "Dice");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "Roll the dice", 20);
    b.card_part(c, t);
    // AddColor colors the dice, as HyperCard 2.2's AddColor did.
    let colors = [[255, 205, 205], [205, 222, 255]];
    for (i, name) in ["Die 1", "Die 2"].iter().enumerate() {
        let x = 120 + i as i32 * 150;
        let mut d = Part::new_field(b.id(), name, Rect::new(x, 50, x + 110, 150));
        d.style = PartStyle::Shadow;
        d.lock_text = true;
        d.text_size = 64;
        d.text_align = TextAlign::Center;
        d.contents = (i + 3).to_string();
        let target = color::ColorTarget::Field { id: d.id, color: colors[i], bevel: 0 };
        b.stack.cards[c].colors.push(color::ColorItem { target, enabled: true });
        b.card_part(c, d);
    }
    let mut result = b.label("Result", Rect::new(60, 160, 452, 184), "Click Roll.", 14);
    result.text_align = TextAlign::Center;
    b.card_part(c, result);
    let mut sides = b.button(
        "Sides",
        Rect::new(20, 202, 170, 222),
        "on mouseUp\n  put \"Rolling\" && the selectedText of me & \"-sided dice.\" into field \"Result\"\nend mouseUp",
    );
    sides.style = PartStyle::Popup;
    sides.title_width = 48;
    sides.text_align = TextAlign::Left;
    sides.contents = "4\n6\n8\n12\n20".into();
    sides.selected_line = 2;
    b.card_part(c, sides);
    let roll = b.button(
        "Roll",
        Rect::new(196, 196, 316, 226),
        r#"on mouseUp
  put the selectedText of button "Sides" into sides
  repeat 8 times
    put random(sides) into field "Die 1"
    put random(sides) into field "Die 2"
    wait 3 ticks
  end repeat
  put field "Die 1" + field "Die 2" into total
  put "You rolled" && total & "." into field "Result"
  if total is 7 or total is 11 then
    beep
    put " Lucky!" after field "Result"
  end if
end mouseUp"#,
    );
    b.card_part(c, {
        let mut r = roll;
        r.style = PartStyle::Default;
        r
    });
    let effects = b.button(
        "Show Every Visual Effect",
        Rect::new(156, 250, 356, 278),
        r#"on mouseUp
  put "dissolve,wipe left,wipe up,scroll left,barn door open,barn door close,iris open,iris close,venetian blinds,checkerboard,zoom open" into effects
  repeat with i = 1 to the number of items in effects
    put "visual effect" && item i of effects into msg
    do "visual effect" && item i of effects
    go to card "Contents"
    do "visual effect" && item i of effects
    go to card "Dice"
  end repeat
  put "That's every effect." into msg
end mouseUp"#,
    );
    b.card_part(c, Part { style: PartStyle::Standard, ..effects });

    // ----- Drawing -----
    let c = b.card(main, "Drawing");
    house(&mut b.stack.cards[c].picture);
    let hint = b.label(
        "Hint",
        Rect::new(20, 8, 492, 28),
        &if cfg!(target_os = "macos") {
            // macOS keeps ⌘Tab for switching apps.
            "Draw with the paint tools on the left. The hand gets you back.".to_string()
        } else {
            format!("Draw with the paint tools on the left. The hand (or {}) gets you back.", shortcut("Tab"))
        },
        12,
    );
    b.card_part(c, hint);
    // Painting in color from a script: the pattern's set bits are in the
    // brush color, lines and borders in the pen color.
    let balloons = b.button(
        "Paint in Color",
        Rect::new(372, 40, 492, 62),
        r#"
on mouseUp
  put the penColor into oldPen
  put the brushColor into oldBrush
  put the filled into oldFilled
  put the pattern into oldPattern
  put "red,orange,lime,purple" into colors
  choose oval tool
  set the filled to true
  set the lineSize to 1
  set the penColor to "navy"
  repeat with i = 1 to 4
    put 368 + 30 * (i - 1) into x
    put 80 + 14 * (i mod 2) into y
    set the brushColor to item i of colors
    -- The last balloon shows a pattern in color.
    if i = 4 then set the pattern to 3 else set the pattern to 1
    drag from x & "," & y to (x + 26) & "," & (y + 32)
  end repeat
  choose line tool
  set the penColor to "gray"
  repeat with i = 1 to 4
    put 368 + 30 * (i - 1) into x
    put 80 + 14 * (i mod 2) into y
    drag from (x + 13) & "," & (y + 33) to "425,215"
  end repeat
  set the pattern to oldPattern
  set the filled to oldFilled
  set the penColor to oldPen
  set the brushColor to oldBrush
  choose browse tool
end mouseUp"#,
    );
    b.card_part(c, Part { style: PartStyle::Standard, ..balloons });
    // Shift while you drag: each drag below is a little off, and Shift
    // straightens it, the way it does with the mouse.
    let straight = b.button(
        "Draw with Shift",
        Rect::new(20, 40, 140, 62),
        r#"
on mouseUp
  put the filled into oldFilled
  put the lineSize into oldSize
  put the pattern into oldPattern
  choose oval tool
  set the filled to false
  set the lineSize to 2
  -- A drag for an oval: Shift makes it a circle.
  drag from "30,72" to "70,90" with shiftKey
  choose line tool
  set the lineSize to 1
  -- Spokes a few degrees off: Shift turns them to multiples of 15°.
  repeat with i = 1 to 12
    put (i * 30 + 6) * pi / 180 into angle
    put 50 + round(18 * cos(angle)) into x
    put 92 + round(18 * sin(angle)) into y
    drag from "50,92" to x & "," & y with shiftKey
  end repeat
  -- A sloping brush stroke: Shift keeps it straight across.
  choose brush tool
  set the pattern to 1
  drag from "90,84" to "240,96" with shiftKey
  set the pattern to oldPattern
  set the lineSize to oldSize
  set the filled to oldFilled
  choose browse tool
  put "Hold Shift as you drag for circles, squares, lines at 15° and straight strokes." into msg
end mouseUp"#,
    );
    b.card_part(c, Part { style: PartStyle::Standard, ..straight });
    // Import Paint and Export Paint: the card saved as a PNG, then brought
    // back onto a new card.
    let files = b.button(
        "Picture Files",
        Rect::new(196, 40, 316, 62),
        r#"
on mouseUp
  ask file "Save a picture of this card as:" with "Drawing card.png"
  if it is empty then exit mouseUp
  put it into fileName
  choose select tool
  export paint to file fileName
  choose browse tool
  if the result is not empty then
    answer the result
    exit mouseUp
  end if
  answer "Saved this card as a PNG. Import Paint reads PNG, JPEG, GIF and BMP, and old MacPaint and PICT files. You can also paste an image copied in another program, or drop a picture file onto the card." ¬
      with "Done" or "Import It"
  if it is "Done" then exit mouseUp
  doMenu "New Card"
  choose select tool
  import paint from file fileName
  answer "The picture came back in with Import Paint, selected." with "Delete This Card" or "Keep It"
  choose browse tool
  if it is "Delete This Card" then
    doMenu "Delete Card"
    go to card "Drawing"
  end if
end mouseUp"#,
    );
    b.card_part(c, Part { style: PartStyle::Standard, ..files });
    // AddColor, HyperCard 2's Color Tools: color rectangles behind the
    // black-and-white picture, which color only its white.
    let addcolor = b.button(
        "Color the House",
        Rect::new(196, 70, 316, 92),
        r#"
on mouseUp
  addColor getObjectType,cd,1
  if the result is empty then
    addColor addRect,cd,"0,96,512,261","42000,54000,65535",0,-1 -- sky
    addColor addRect,cd,"90,191,210,261","65535,60000,42000",3,-1 -- walls
    addColor addRect,cd,"0,262,512,304","30000,52000,30000",0,-1 -- grass
    addColor colorCard,dissolve,30
    put "AddColor colors the white of the picture; the black lines stay black."
  else
    repeat 3 times
      addColor removeObject,cd,1
    end repeat
    addColor colorCard,dissolve,30
    put "Back to black and white."
  end if
end mouseUp"#,
    );
    b.card_part(c, Part { style: PartStyle::Standard, ..addcolor });

    // ----- Animation -----
    // A small state machine in the card's script: whether the ball is
    // moving, and its velocity. Each idle message moves it one step, so the
    // buttons keep working while it moves.
    let c = b.card(main, "Animation");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "Moving things with scripts", 20);
    b.card_part(c, t);
    let start = b.button(
        "Start",
        Rect::new(20, 48, 100, 72),
        "on mouseUp\n  global ballMoving\n  put not (ballMoving is true) into ballMoving\n  showState\nend mouseUp",
    );
    b.card_part(c, start);
    let speeds = [
        ("Slower", "on mouseUp\n  global ballSpeed\n  put max(1, ballSpeed - 1) into ballSpeed\n  showState\nend mouseUp"),
        ("Faster", "on mouseUp\n  global ballSpeed\n  put min(12, ballSpeed + 1) into ballSpeed\n  showState\nend mouseUp"),
    ];
    for (i, (name, script)) in speeds.iter().enumerate() {
        let x = 112 + i as i32 * 76;
        let p = b.button(name, Rect::new(x, 48, x + 70, 72), script);
        b.card_part(c, p);
    }
    let speed = b.label("Speed", Rect::new(264, 52, 344, 70), "Speed: 4", 12);
    b.card_part(c, speed);
    let mut show = b.button("Show the ball", Rect::new(356, 50, 492, 72), "on mouseUp\n  set the visible of button \"Ball\" to the hilite of me\nend mouseUp");
    show.style = PartStyle::CheckBox;
    show.hilite = true;
    show.text_align = TextAlign::Left;
    b.card_part(c, show);
    let mut ball = b.button("Ball", Rect::new(240, 180, 264, 204), "on mouseUp\n  beep\nend mouseUp");
    ball.style = PartStyle::Oval;
    ball.show_name = false;
    ball.hilite = true;
    ball.auto_hilite = false;
    b.card_part(c, ball);
    // ZoomRect, the Finder's zoom, from a 1989 XCMD now built in.
    // Beside Contents, in the navigation row and its style.
    let mut zoom = b.button("Zoom", Rect::new(190, 314, 256, 336), "on mouseUp\n  zoomRect the rect of me, \"40,96,472,290\"\n  zoomRect \"40,96,472,290\", the rect of me\nend mouseUp");
    zoom.style = PartStyle::RoundRect;
    b.card_part(c, zoom);
    b.stack.cards[c].script = r#"-- The ball's state: whether it's moving, and its velocity (a speed, and
-- a direction across and down). Buttons change the state; idle moves it.
on openCard
  global ballMoving, ballSpeed, dirX, dirY
  put false into ballMoving
  if ballSpeed is empty then put 4 into ballSpeed
  if dirX is empty then put 1 into dirX
  if dirY is empty then put 1 into dirY
  showState
  pass openCard
end openCard

on closeCard
  global ballMoving
  put false into ballMoving
  pass closeCard
end closeCard

on idle
  global ballMoving, ballSpeed, dirX, dirY
  if ballMoving is true then
    get the loc of button "Ball"
    put item 1 of it + dirX * ballSpeed into x
    put item 2 of it + dirY * ballSpeed into y
    if x < 32 or x > 480 then multiply dirX by -1
    if y < 96 or y > 290 then multiply dirY by -1
    set the loc of button "Ball" to max(32, min(480, x)), max(96, min(290, y))
  end if
  pass idle
end idle

on showState
  global ballMoving, ballSpeed
  if ballMoving then set the name of card button 1 to "Stop"
  else set the name of card button 1 to "Start"
  put "Speed:" && ballSpeed into field "Speed"
end showState
"#
    .into();

    // ----- Sound -----
    let c = b.card(main, "Sound");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "Sound", 20);
    b.card_part(c, t);
    let about = b.label(
        "About",
        Rect::new(20, 42, 492, 76),
        "Change the notes below and click Play Tune.",
        12,
    );
    b.card_part(c, about);
    let sounds = [
        ("Boing", "on mouseUp\n  play \"boing\"\nend mouseUp"),
        // Notes can go without quotes, as many old stacks wrote them.
        ("Flute", "on mouseUp\n  play \"flute\" e4q g a b a g e4h. r f#4e g a#4q b4h\nend mouseUp"),
        ("Dial", "on mouseUp\n  dial \"555-0187\"\nend mouseUp"),
        ("Stop", "on mouseUp\n  play stop\nend mouseUp"),
    ];
    for (i, (name, script)) in sounds.iter().enumerate() {
        let x = 20 + i as i32 * 94;
        let p = b.button(name, Rect::new(x, 84, x + 86, 112), script);
        b.card_part(c, p);
    }
    // The tune and its button go together, apart from the one-shot sounds.
    let play = b.button(
        "Play Tune",
        Rect::new(400, 122, 492, 150),
        "on mouseUp\n  play \"harpsichord\" tempo 200 field \"Tune\"\nend mouseUp",
    );
    b.card_part(c, play);
    let mut tune = Part::new_field(b.id(), "Tune", Rect::new(20, 124, 390, 148));
    tune.contents = "c4q c g g a a g h f f e e d d c h".into();
    b.card_part(c, tune);
    let guide = b.label(
        "Notes Guide",
        Rect::new(20, 160, 492, 296),
        "Each note is a name, then a sharp or flat, an octave and a length, as in c4q or f#5e.\n\
         Name: c d e f g a b, r for a rest, or a MIDI number (60 is middle C).\n\
         Sharp or flat: # or b, as in f# or bb.\n\
         Octave: 0 to 9; middle C is c4.\n\
         Length: w whole, h half, q quarter, e eighth, s 16th, t 32nd, x 64th.\n\
         After the length, . makes a dotted note and 3 a triplet, as in q. or e3.\n\
         A note without an octave or length keeps the last one given.",
        12,
    );
    b.card_part(c, guide);

    // ----- Keys -----
    let c = b.card(main, "Keys");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "Keyboard messages", 20);
    b.card_part(c, t);
    let about = b.label(
        "About",
        Rect::new(20, 42, 492, 76),
        "Press keys: this card's script shows each key message. It doesn't pass them on, so the arrow keys don't change cards here.",
        12,
    );
    b.card_part(c, about);
    let mut last = b.label("Last Key", Rect::new(20, 100, 492, 140), "Press a key.", 24);
    last.text_align = TextAlign::Center;
    b.card_part(c, last);
    b.stack.cards[c].script = r#"on keyDown which
  put "keyDown" && quote & which & quote into field "Last Key"
  if CapsLock() then put " (Caps Lock is on)" after field "Last Key"
end keyDown

on arrowKey which
  put "arrowKey" && which into field "Last Key"
end arrowKey

on functionKey which
  put "functionKey" && which into field "Last Key"
end functionKey

on returnKey
  put "returnKey" into field "Last Key"
end returnKey

on tabKey
  put "tabKey" into field "Last Key"
end tabKey
"#
    .into();

    // ----- Fields and text -----
    let c = b.card(main, "Fields");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "Fields and text", 20);
    b.card_part(c, t);
    let hint = b.label("Hint", Rect::new(20, 40, 492, 56), "Select text in any field and use the Font and Style menus.", 12);
    b.card_part(c, hint);
    let fields = [
        (
            "Lined",
            Rect::new(20, 62, 250, 174),
            "showLines draws a line under each line of text.\nType here to add more.",
        ),
        (
            "Wide",
            Rect::new(262, 62, 492, 174),
            "wideMargins gives a field more room around its text.",
        ),
        (
            "No Wrap",
            Rect::new(20, 184, 250, 296),
            "dontWrap keeps each line on one line, however long it gets.\nShort lines are fine.",
        ),
        (
            "Scrolling",
            Rect::new(262, 184, 492, 296),
            "A scrolling field has a scroll bar, for more text than fits.\n\nHyperCard came out in August 1987, free with every Mac.\n\nBill Atkinson wrote it; Dan Winkler wrote HyperTalk.\n\nStacks were shared on floppy disks and bulletin boards.\n\nMyst began as a HyperCard stack.\n\nThe last version, HyperCard 2.4, came out in 1998.",
        ),
    ];
    for (name, rect, text) in fields {
        let mut f = Part::new_field(b.id(), name, rect);
        f.contents = text.into();
        match name {
            "Lined" => f.show_lines = true,
            "Wide" => f.wide_margins = true,
            "No Wrap" => f.dont_wrap = true,
            _ => f.style = PartStyle::Scrolling,
        }
        b.card_part(c, f);
    }
    // LineCount and Prompt, from Frédéric Rinaldi's externals, built in.
    let mut count = b.button(
        "Count Lines",
        Rect::new(190, 314, 290, 336),
        "on mouseUp\n  Prompt 1, \"Counting the lines in each field…\"\n  put empty into report\n  repeat with i = 1 to 4\n    put item i of \"Lined,Wide,No Wrap,Scrolling\" into n\n    put n & \":\" && LineCount(\"cd fld\" && quote & n & quote) && \"lines on screen,\" && the number of lines of cd fld n && \"in the text\" & return after report\n  end repeat\n  wait 30 ticks\n  Prompt 3\n  answer report\nend mouseUp",
    );
    count.style = PartStyle::RoundRect;
    b.card_part(c, count);
    // Switch, from the same collection: items to lines and back.
    let mut switch = b.button(
        "Switch",
        Rect::new(216, 12, 300, 34),
        "on mouseUp\n  put Switch(\"red,green,blue\") into asLines\n  answer \"Switch makes items lines:\" & return & asLines & return & return & \"and lines items again:\" && Switch(asLines)\nend mouseUp",
    );
    switch.style = PartStyle::RoundRect;
    b.card_part(c, switch);
    // ListSelect: choose any of the fields, sorted, to flash them.
    let mut flash = b.button(
        "Flash Fields",
        Rect::new(300, 314, 400, 336),
        "on mouseUp\n  global ListSelectExit\n  put ListSelect(\"*DIS\", \"Lined,Wide,No Wrap,Scrolling\", \"Choose fields to flash (Shift or Command for more):\", \"s=1,4\") into chosen\n  if chosen is empty then exit mouseUp\n  repeat 3 times\n    repeat with i = 1 to the number of items of chosen\n      hide cd fld (item i of chosen)\n    end repeat\n    wait 8 ticks\n    repeat with i = 1 to the number of items of chosen\n      show cd fld (item i of chosen)\n    end repeat\n    wait 8 ticks\n  end repeat\n  put \"Flashed\" && chosen && \"(you left with\" && ListSelectExit & \")\"\nend mouseUp",
    );
    flash.style = PartStyle::RoundRect;
    b.card_part(c, flash);
    // Copy Text and Paste Text keep fonts, sizes and styles.
    let mut styled = b.button(
        "Copy Text",
        Rect::new(402, 12, 492, 34),
        r#"on mouseUp
  set the textStyle of word 1 of cd fld "Wide" to "bold,italic"
  set the textFont of word 1 of cd fld "Wide" to "Times"
  select word 1 of cd fld "Wide"
  doMenu "Copy Text"
  put return after cd fld "Lined"
  select after text of cd fld "Lined"
  doMenu "Paste Text"
  select empty
  put "Copied the first word of Wide into Lined, with its font and style."
end mouseUp"#,
    );
    styled.style = PartStyle::RoundRect;
    b.card_part(c, styled);
    // Part colors: text, fill and frame.
    let mut colors = b.button(
        "Colors",
        Rect::new(306, 12, 396, 34),
        r#"on mouseUp
  if the backColor of me is empty then
    set the backColor of me to "navy"
    set the foreColor of me to "white"
    set the borderColor of me to "navy"
    set the backColor of cd fld "Lined" to "255,250,205"
    set the borderColor of cd fld "Lined" to "orange"
    -- One word in its own color, as Style ▸ Text Color… gives it.
    set the foreColor of word 1 of cd fld "Lined" to "red"
    set the foreColor of cd fld "Wide" to "purple"
    set the backColor of cd fld "No Wrap" to "230,240,255"
    set the foreColor of cd fld "No Wrap" to "navy"
    set the borderColor of cd fld "No Wrap" to "blue"
    set the borderColor of cd fld "Scrolling" to "green"
    put "Buttons and fields have a text, fill and frame color (the foreColor, backColor and borderColor), and words can have their own."
  else
    repeat with i = 1 to 4
      put item i of "Lined,Wide,No Wrap,Scrolling" into f
      set the foreColor of cd fld f to empty
      set the backColor of cd fld f to empty
      set the borderColor of cd fld f to empty
    end repeat
    set the foreColor of me to empty
    set the backColor of me to empty
    set the borderColor of me to empty
    put "Back to black and white."
  end if
end mouseUp"#,
    );
    colors.style = PartStyle::RoundRect;
    b.card_part(c, colors);

    // ----- Printing and finding -----
    let c = b.card(main, "Print & Find");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "Printing and finding", 20);
    b.card_part(c, t);
    let about = b.label(
        "About",
        Rect::new(20, 42, 492, 76),
        "Printing opens a PDF in your PDF viewer, and PostScript opens in its viewer. Find and Recent are also in the Go menu.",
        12,
    );
    b.card_part(c, about);
    let actions = [
        ("Print Card", "on mouseUp\n  print card\nend mouseUp"),
        ("Print Stack", "on mouseUp\n  print all cards\nend mouseUp"),
        ("Print Some Text", "on mouseUp\n  print \"Hello from RandomCard!\" & return & return & \"Printed on\" && the long date & \".\"\nend mouseUp"),
        ("Find Bill", "on mouseUp\n  visual effect dissolve\n  find \"Bill\"\nend mouseUp"),
        ("Find…", "on mouseUp\n  doMenu \"Find…\"\nend mouseUp"),
        ("Recent", "on mouseUp\n  doMenu \"Recent\"\nend mouseUp"),
        // `open`: a document in the program the system picks.
        ("Open a Document", "on mouseUp\n  answer file \"Open which document?\"\n  if it is empty then exit mouseUp\n  open it\n  if the result is not empty then answer \"Couldn't open\" && it\nend mouseUp"),
        // SendPS, from Frédéric Rinaldi's externals: PostScript to the viewer.
        ("Print PostScript", "on mouseUp\n  put \"%!PS\" & return & \"/Times-Bold findfont 36 scalefont setfont\" & return into ps\n  put \"72 700 moveto (Hello from RandomCard!) show\" & return after ps\n  put \"newpath 300 450 120 0 360 arc 0.5 setgray fill\" & return & \"showpage\" after ps\n  SendPS ps\n  if the result is not empty then answer the result\nend mouseUp"),
    ];
    for (i, (name, script)) in actions.iter().enumerate() {
        let (col, row) = ((i % 3) as i32, (i / 3) as i32);
        let x = 36 + col * 150;
        let y = 88 + row * 40;
        let p = b.button(name, Rect::new(x, y, x + 140, y + 28), script);
        b.card_part(c, p);
    }

    // ----- HyperCard's icons -----
    let c = b.card(main, "Icons");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "Buttons and icons", 20);
    b.card_part(c, t);
    let about = b.label(
        "About",
        Rect::new(20, 42, 492, 76),
        "HyperCard's own icons are built in. Give a button one in its Info window, or by name from a script.",
        12,
    );
    b.card_part(c, about);
    // Scripts change these buttons' icons by name.
    let icon_buttons = [
        ("Roll Me", "Dice,1", "on mouseUp\n  repeat 6 times\n    set the icon of me to \"Dice,\" & random(6)\n    wait 4 ticks\n  end repeat\nend mouseUp"),
        ("Juggle", "Juggler 2", "on mouseUp\n  repeat with i = 1 to 18\n    set the icon of me to \"Juggler\" && ((i mod 3) + 2)\n    wait 5 ticks\n  end repeat\nend mouseUp"),
    ];
    for (i, (name, icon, script)) in icon_buttons.iter().enumerate() {
        let x = 60 + i as i32 * 130;
        let mut p = b.button(name, Rect::new(x, 96, x + 90, 160), script);
        p.style = PartStyle::Transparent;
        if let Some((id, art)) = crate::import::hypercard_icon(icon) {
            p.icon = id;
            b.stack.icons.insert(id, art);
        }
        b.card_part(c, p);
    }
    // A hilited transparent button reverses what's under it.
    let mut under = Part::new_field(b.id(), "Under", Rect::new(330, 96, 480, 160));
    under.style = PartStyle::Rectangle;
    under.lock_text = true;
    under.text_size = 12;
    under.text_align = TextAlign::Center;
    under.contents = "Press and hold here: the button over this text reverses it.".into();
    b.card_part(c, under);
    let mut over = b.button("Reverse", Rect::new(330, 96, 480, 160), "");
    over.style = PartStyle::Transparent;
    over.show_name = false;
    over.auto_hilite = true;
    b.card_part(c, over);

    // ----- The debugger -----
    let c = b.card(main, "Debugging");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "The debugger", 20);
    b.card_part(c, t);
    let about = b.label(
        "About",
        Rect::new(20, 42, 492, 76),
        "Step Through Me stops at a checkpoint. Then F10 is Step, F11 Step Into, F5 Go, and Shift+F5 Abort.",
        12,
    );
    b.card_part(c, about);
    let step = b.button(
        "Step Through Me",
        Rect::new(36, 88, 176, 116),
        r#"on mouseUp
  debug checkpoint
  put 0 into total
  repeat with i = 1 to 5
    add square(i) to total
  end repeat
  put "1 + 4 + 9 + 16 + 25 =" && total into field "Answer"
end mouseUp

function square n
  return n * n
end square"#,
    );
    b.card_part(c, step);
    let watchers = [
        ("Message Watcher", "on mouseUp\n  show message watcher\nend mouseUp"),
        ("Variable Watcher", "on mouseUp\n  show variable watcher\nend mouseUp"),
    ];
    for (i, (name, script)) in watchers.iter().enumerate() {
        let x = 186 + i as i32 * 150;
        let p = b.button(name, Rect::new(x, 88, x + 140, 116), script);
        b.card_part(c, p);
    }
    // GlobalList, MacType, ClipInfo and WindName, from Frédéric Rinaldi's
    // externals, built in.
    let more = [
        ("List Globals", "on mouseUp\n  global tourVisits\n  add 1 to tourVisits\n  answer \"The globals now:\" && GlobalList(true) & return & return & \"This button has been clicked\" && tourVisits && \"times.\"\nend mouseUp"),
        ("This Computer", "on mouseUp\n  put MacType() into m\n  put ClipInfo() into clip\n  if clip is empty then put \"nothing\" into clip\n  answer \"System:\" && item 1 of m && item 2 of m & return & \"Processor:\" && item 3 of m & return & \"Copied:\" && clip\nend mouseUp"),
        ("Rename Window", "on mouseUp\n  ask \"New window title:\" with \"My RandomCard\"\n  if it is not empty then WindName it\nend mouseUp"),
    ];
    for (i, (name, script)) in more.iter().enumerate() {
        let x = 36 + i as i32 * 150;
        let p = b.button(name, Rect::new(x, 170, x + 140, 198), script);
        b.card_part(c, p);
    }
    // Mousoid, Infoid, Promptoid and Notification, from the same collection.
    let windoids = [
        ("Mouse Window", "on mouseUp\n  if \"Mousoid\" is in the windows then close window \"Mousoid\"\n  else Mousoid\nend mouseUp"),
        ("Info Window", "on mouseUp\n  if \"Infoid\" is in the windows then close window \"Infoid\"\n  else Infoid\nend mouseUp"),
        ("Call Me Back", "on mouseUp\n  Promptoid \"Switch to another window. I'll call you back in 5 seconds.\"\n  repeat with i = 4 down to 1\n    wait 1 second\n    set text of window \"Promptoid\" to \"Calling you back in\" && i && \"…\"\n  end repeat\n  wait 1 second\n  close window \"Promptoid\"\n  Notification\n  answer \"Welcome back!\"\nend mouseUp"),
    ];
    for (i, (name, script)) in windoids.iter().enumerate() {
        let x = 36 + i as i32 * 150;
        let p = b.button(name, Rect::new(x, 208, x + 140, 236), script);
        b.card_part(c, p);
    }
    // The script editor's Find and Replace and Go to Line.
    let find = b.button(
        "Find in a Script",
        Rect::new(36, 246, 176, 274),
        &format!(
            "on mouseUp\n  edit script of button \"Step Through Me\"\n  put \"In the script editor: {} finds, {} finds again, {} replaces, {} goes to a line.\" into msg\nend mouseUp",
            shortcut("F"),
            shortcut("G"),
            shortcut("R"),
            shortcut("J")
        ),
    );
    b.card_part(c, find);
    // Compact Stack, and the size and freeSize of a stack.
    let compact = b.button(
        "Compact Stack",
        Rect::new(186, 246, 326, 274),
        r#"on mouseUp
  put the size of this stack into before
  put the freeSize of this stack into free
  doMenu "Compact Stack"
  put "Compact Stack drops text and styles left by deleted background fields, and backgrounds no card uses." into what
  if free is 0 then answer "This stack is" && before && "bytes, with nothing to free." && what
  else answer what && "It freed" && free && "of this stack's" && before && "bytes."
end mouseUp"#,
    );
    b.card_part(c, compact);
    // Don't Save: a field that works as usual, but opens empty.
    let dont_save = b.button(
        "Don't Save",
        Rect::new(336, 246, 476, 274),
        r#"on mouseUp
  -- Results, replies and scratch work needn't be kept: a field marked
  -- Don't Save (in Field Info, or by script) opens empty next time.
  set the dontSave of card field "Answer" to not the dontSave of card field "Answer"
  if the dontSave of card field "Answer" then
    put "Don't Save: this line won't be saved with the stack." into card field "Answer"
  else
    put "This line is saved with the stack again." into card field "Answer"
  end if
end mouseUp"#,
    );
    b.card_part(c, dont_save);
    let mut answer = b.label("Answer", Rect::new(20, 132, 492, 156), "", 14);
    answer.text_align = TextAlign::Center;
    b.card_part(c, answer);

    // ----- Palettes -----
    let c = b.card(main, "Palettes");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "Palettes", 20);
    b.card_part(c, t);
    let about = b.label(
        "About",
        Rect::new(20, 42, 492, 90),
        "A palette floats over the cards, and each of its buttons sends a message. HyperCard stacks keep theirs as resources, and RandomCard brings them along. Show this one, then go to another card: it stays.",
        12,
    );
    b.card_part(c, about);
    b.stack.palettes.insert(1, tour_palette());
    let palette_buttons = [
        ("Show Palette", "on mouseUp\n  palette \"Tour Palette\", \"330,110\"\nend mouseUp"),
        ("Close Palette", "on mouseUp\n  if there is a window \"Tour Palette\" then close window \"Tour Palette\"\nend mouseUp"),
    ];
    for (i, (name, script)) in palette_buttons.iter().enumerate() {
        let y = 110 + i as i32 * 38;
        let p = b.button(name, Rect::new(36, y, 176, y + 28), script);
        b.card_part(c, p);
    }
    let mut said = b.label("Said", Rect::new(36, 200, 476, 222), "", 12);
    said.text_style = "italic".into();
    b.card_part(c, said);
    b.stack.cards[c].script = "on openPalette name, id\n  put \"openPalette\" && name && id into field \"Said\"\nend openPalette\n\non closePalette name\n  put \"closePalette\" && name into field \"Said\"\nend closePalette".into();

    // ----- The web: JSON on the card itself, and requests to an address
    // the user types. Nothing here calls a service of its own accord: the
    // Tour works anywhere, offline. Full examples are stacks in examples/.
    let c = b.card(main, "The Web");
    b.stack.cards[c].script = r#"-- A reply to Send's request: the URL, and what came back. The result
-- is empty when it worked; the responseStatus is the status either way.
on showReply theURL, reply
  if the responseStatus is empty then put the result into card field "Status"
  else put the responseStatus && "in" && the responseTime && "ms" into card field "Status"
  put reply into card field "Reply"
end showReply"#
        .into();
    let t = b.label("Heading", Rect::new(20, 8, 492, 32), "The web: REST and JSON", 20);
    b.card_part(c, t);
    let mut json = Part::new_field(b.id(), "JSON", Rect::new(20, 40, 250, 150));
    json.style = PartStyle::Scrolling;
    json.text_size = 10;
    json.contents = r#"{"name":"Ada Lovelace","born":1815,"languages":["English","French","Italian"],"notes":{"first":"program"}}"#.into();
    b.card_part(c, json);
    let read = b.button(
        "Read JSON",
        Rect::new(20, 156, 130, 178),
        r#"on mouseUp
  -- key … of json reads an object's key, element … of json an array's
  -- element (from 1), and they nest.
  put card field "JSON" into j
  put "Name:" && key "name" of json j into line 1 of r
  put "Languages:" && the number of elements of key "languages" of json j into line 2 of r
  put "The second:" && element 2 of key "languages" of json j into line 3 of r
  put "First note:" && key "first" of key "notes" of json j into line 4 of r
  put r into card field "Result"
end mouseUp"#,
    );
    b.card_part(c, Part { style: PartStyle::RoundRect, ..read });
    let build = b.button(
        "Build JSON",
        Rect::new(140, 156, 250, 178),
        r#"on mouseUp
  -- set key … of json builds JSON in a variable or field: numbers stay
  -- numbers, other text becomes strings.
  put "{}" into j
  set key "name" of json j to "Grace Hopper"
  set key "born" of json j to 1906
  set element 1 of key "languages" of json j to "COBOL"
  set element 2 of key "languages" of json j to "FLOW-MATIC"
  set key "first" of key "notes" of json j to "compiler"
  put j into card field "JSON"
  put "Built with set key … of json. Read JSON reads it back." into card field "Result"
end mouseUp"#,
    );
    b.card_part(c, Part { style: PartStyle::RoundRect, ..build });
    let mut result = Part::new_field(b.id(), "Result", Rect::new(20, 184, 250, 290));
    result.lock_text = true;
    result.text_size = 10;
    b.card_part(c, result);
    // Requests: a method, an address and a body of the user's own. The
    // method is a combo field: its ▼ offers the usual ones, and any other
    // can be typed.
    let l = b.label("Method Label", Rect::new(262, 42, 310, 58), "Method:", 10);
    b.card_part(c, l);
    let mut method = Part::new_field(b.id(), "Method", Rect::new(312, 40, 404, 60));
    method.text_size = 10;
    method.dont_wrap = true;
    method.contents = "GET".into();
    method.choices = "GET\nPOST\nPUT\nPATCH\nQUERY\nDELETE".into();
    b.card_part(c, method);
    let send = b.button(
        "Send",
        Rect::new(412, 38, 492, 62),
        r#"on mouseUp
  -- Any method: request m [with body] to url u. The reply goes into it,
  -- and the result is empty when it worked, else the status.
  if card field "URL" is empty then
    answer "Type the address of a web service: one of your own, or one you use."
    exit mouseUp
  end if
  put card field "Method" into m
  if m is empty then put "GET" into m
  -- with message: the script goes on, and showReply (in the card's
  -- script) gets the reply when it comes.
  put "Waiting…" into card field "Status"
  put empty into card field "Reply"
  if card field "Body" is empty then
    request m to url card field "URL" with message "showReply"
  else
    request m with card field "Body" to url card field "URL" with message "showReply"
  end if
end mouseUp"#,
    );
    b.card_part(c, Part { style: PartStyle::RoundRect, ..send });
    let l = b.label("URL Label", Rect::new(262, 68, 300, 84), "URL:", 10);
    b.card_part(c, l);
    let mut url = Part::new_field(b.id(), "URL", Rect::new(302, 66, 492, 84));
    url.text_size = 10;
    url.dont_wrap = true;
    b.card_part(c, url);
    let l = b.label("Body Label", Rect::new(262, 88, 492, 102), "Body (POST, PUT, PATCH, QUERY):", 9);
    b.card_part(c, l);
    let mut body = Part::new_field(b.id(), "Body", Rect::new(262, 102, 492, 150));
    body.text_size = 10;
    b.card_part(c, body);
    let mut status = Part::new_field(b.id(), "Status", Rect::new(262, 156, 492, 174));
    status.lock_text = true;
    status.text_size = 10;
    b.card_part(c, status);
    let mut reply = Part::new_field(b.id(), "Reply", Rect::new(262, 180, 492, 290));
    reply.style = PartStyle::Scrolling;
    reply.lock_text = true;
    reply.text_size = 9;
    b.card_part(c, reply);

    // ----- Japanese: stacks from HyperCard J come in as Unicode, drawn in
    // the system's Japanese font. Nothing on the card is Japanese until a
    // button puts it there, so the font loads only then.
    let c = b.card(main, "Japanese");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "Japanese text", 20);
    b.card_part(c, t);
    let about = b.label(
        "About",
        Rect::new(20, 42, 492, 112),
        "Apple sold HyperCard J in Japan, and its stacks keep their text in Shift-JIS. RandomCard imports them as Unicode and draws Japanese in your system's Japanese font, loaded the first time Japanese appears. Click a button, or type in the field with a Japanese input method.",
        12,
    );
    b.card_part(c, about);
    let mut field = Part::new_field(b.id(), "Japanese", Rect::new(36, 122, 476, 182));
    field.text_size = 18;
    b.card_part(c, field);
    let japanese_buttons = [
        ("Say Hello", "on mouseUp\n  put \"こんにちは。RandomCard へようこそ！\" into field \"Japanese\"\nend mouseUp"),
        (
            "Characters",
            "on mouseUp\n  put \"花札スタック\" into field \"Japanese\"\n  answer \"花札スタック (the Hanafuda stack) is\" && the number of chars of field \"Japanese\" && \"characters.\" & return & \"charToNum(\" & quote & \"花\" & quote & \") is\" && charToNum(\"花\") & \", its Unicode code point, and numToChar(26413) is\" && numToChar(26413) & \".\"\nend mouseUp",
        ),
        ("Clear", "on mouseUp\n  put empty into field \"Japanese\"\nend mouseUp"),
    ];
    for (i, (name, script)) in japanese_buttons.iter().enumerate() {
        let x = 36 + i as i32 * 150;
        let p = b.button(name, Rect::new(x, 196, x + 140, 224), script);
        b.card_part(c, p);
    }

    // ----- Address book: a second background with per-card field text -----
    let addr = b.background("Address", ADDRESS_BG_SCRIPT);
    {
        let pic = &mut b.stack.background_mut(addr).unwrap().picture;
        frame(pic, 6, 6, 505, 335);
        frame(pic, 8, 8, 503, 333);
        hline(pic, 8, 503, 290);
    }
    for (name, y) in [("Name:", 46), ("Phone:", 78), ("Notes:", 110)] {
        let mut l = b.label(&format!("{name} label"), Rect::new(24, y, 110, y + 22), name, 13);
        l.text_align = TextAlign::Right;
        l.text_style = "bold".into();
        b.bg_part(addr, l);
    }
    let t = b.label("Heading", Rect::new(24, 14, 300, 38), "Address Book", 18);
    b.bg_part(addr, t);
    let mut count = b.label("Count", Rect::new(330, 16, 490, 34), "", 11);
    count.text_align = TextAlign::Right;
    b.bg_part(addr, count);
    let name_field = Part::new_field(b.id(), "Name", Rect::new(120, 44, 490, 68));
    let name_id = name_field.id;
    b.bg_part(addr, name_field);
    let phone_field = Part::new_field(b.id(), "Phone", Rect::new(120, 76, 490, 100));
    let phone_id = phone_field.id;
    b.bg_part(addr, phone_field);
    let mut notes = Part::new_field(b.id(), "Notes", Rect::new(120, 108, 490, 280));
    notes.style = PartStyle::Scrolling;
    let notes_id = notes.id;
    b.bg_part(addr, notes);
    let addr_buttons = [
        ("◀", "on mouseUp\n  visual effect scroll right fast\n  go to previous card of this background\nend mouseUp"),
        ("▶", "on mouseUp\n  visual effect scroll left fast\n  go to next card of this background\nend mouseUp"),
        ("Find…", "on mouseUp\n  ask \"Find what?\"\n  if it is empty then exit mouseUp\n  find it in field \"Name\"\n  if the result is not empty then find it\n  if the result is not empty then answer \"Couldn't find\" && quote & it & quote & \".\"\nend mouseUp"),
        ("Sort", "on mouseUp\n  sort cards of this background by field \"Name\"\n  go to first card of this background\nend mouseUp"),
        ("New", "on mouseUp\n  doMenu \"New Card\"\n  select text of field \"Name\"\nend mouseUp"),
        ("Delete", "on mouseUp\n  if the number of cards of this background is 1 then\n    answer \"This is the last address.\"\n    exit mouseUp\n  end if\n  answer \"Delete\" && field \"Name\" & \"?\" with \"Cancel\" or \"Delete\"\n  if it is \"Delete\" then doMenu \"Delete Card\"\nend mouseUp"),
        ("Contents", "on mouseUp\n  visual effect zoom close\n  go to card \"Contents\"\nend mouseUp"),
    ];
    for (i, (name, script)) in addr_buttons.iter().enumerate() {
        let x = 20 + i as i32 * 68;
        let mut p = b.button(name, Rect::new(x, 300, x + 62, 324), script);
        p.style = PartStyle::RoundRect;
        b.bg_part(addr, p);
    }
    let people = [
        ("Grace Hopper", "555-0199", "Invented the first compiler.\nFound an actual bug (a moth) in the Harvard Mark II."),
        ("Ada Lovelace", "555-0100", "Wrote the first published algorithm, for Babbage's Analytical Engine."),
        ("Bill Atkinson", "555-0187", "Created HyperCard in 1987, along with MacPaint and QuickDraw."),
    ];
    for (name, phone, note) in people {
        let c = b.card(addr, name);
        let card = &mut b.stack.cards[c];
        card.bg_text.insert(name_id, name.into());
        card.bg_text.insert(phone_id, phone.into());
        card.bg_text.insert(notes_id, note.into());
    }
    b.stack
}

#[cfg(test)]
mod tests {
    use crate::hypertalk::parser::parse_script;

    #[test]
    fn demo_scripts_parse() {
        let s = super::build();
        let mut scripts = vec![("stack".to_string(), s.script.clone())];
        for b in &s.backgrounds {
            scripts.push((format!("bg {}", b.name), b.script.clone()));
            scripts.extend(b.parts.iter().map(|p| (p.name.clone(), p.script.clone())));
        }
        for c in &s.cards {
            scripts.push((format!("card {}", c.name), c.script.clone()));
            scripts.extend(c.parts.iter().map(|p| (p.name.clone(), p.script.clone())));
        }
        for (name, src) in scripts {
            if let Err(e) = parse_script(&src) {
                panic!("{name}: {e}\n{src}");
            }
        }
    }
}
