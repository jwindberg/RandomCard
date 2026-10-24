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
        p.set(x, y, Pixel::Black);
    }
}

fn vline(p: &mut Picture, x: i32, y0: i32, y1: i32) {
    for y in y0..=y1 {
        p.set(x, y, Pixel::Black);
    }
}

fn line(p: &mut Picture, x0: i32, y0: i32, x1: i32, y1: i32) {
    let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for i in 0..=steps {
        let x = x0 + (x1 - x0) * i / steps;
        let y = y0 + (y1 - y0) * i / steps;
        p.set(x, y, Pixel::Black);
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
                p.set(cx + x, cy + y, Pixel::Black);
            }
        }
    }
}

fn house(p: &mut Picture) {
    // A little house with a sun, drawn the way you might with the paint tools.
    for y in 191..260 {
        for x in 91..209 {
            p.set(x, y, Pixel::White);
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
  put "Contents,Basics,Dice,Drawing,Animation,-,Address Book" into menu "Tour"
end openStack

on doMenu what
  if what is "Address Book" then
    visual effect zoom open
    go to first card of background "Address"
  else if "," & what & "," is in ",Contents,Basics,Dice,Drawing,Animation," then
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
        Rect::new(56, 92, 456, 140),
        "Click a button to explore. Every button here runs a HyperTalk script: pick the Button tool on the left and double-click one to see it. Ctrl+M opens the message box.",
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
        ("Address Book", "on mouseUp\n  visual effect zoom open\n  go to first card of background \"Address\"\nend mouseUp"),
    ];
    for (i, (name, script)) in buttons.iter().enumerate() {
        let (col, row) = ((i % 2) as i32, (i / 2) as i32);
        let x = 96 + col * 170;
        let y = 160 + row * 42;
        let p = b.button(name, Rect::new(x, y, x + 150, y + 28), script);
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
    lesson.contents = "Scripts are made of handlers that respond to messages. When you click a button, RandomCard sends it \"mouseUp\":\n\non mouseUp\n  put \"Hello\" into field 1\nend mouseUp\n\nIf the button doesn't handle a message it travels on: button → card → background → stack. That is the message path.\n\nEverything is text. Chunk expressions pick text apart:\n\n  put word 2 of line 3 of field \"Notes\" into x\n  put \"!\" after the last char of x\n\nAsk and answer talk to the user:\n\n  ask \"Your name?\"\n  answer \"Hi\" && it\n\nTry the buttons on the right, then open their scripts with the Button tool. Or type into the message box (Ctrl+M), e.g.\n\n  put the number of cards\n  go to card \"Dice\"\n  answer the long date\n\nClick any word in this text: the field's script shows the clickText and clickLine. The list under the buttons is an autoSelect field, and its script goes to the card you pick.".into();
    // Code in Monaco, key ideas in bold.
    let code = TextStyle { font: Some("Monaco".into()), size: Some(9), style: None };
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
    ];
    for (i, (name, script)) in basics.iter().enumerate() {
        let y = 48 + i as i32 * 40;
        let p = b.button(name, Rect::new(360, y, 492, y + 28), script);
        b.card_part(c, p);
    }

    // ----- Dice -----
    let c = b.card(main, "Dice");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "Roll the dice", 20);
    b.card_part(c, t);
    for (i, name) in ["Die 1", "Die 2"].iter().enumerate() {
        let x = 120 + i as i32 * 150;
        let mut d = Part::new_field(b.id(), name, Rect::new(x, 50, x + 110, 150));
        d.style = PartStyle::Shadow;
        d.lock_text = true;
        d.text_size = 64;
        d.text_align = TextAlign::Center;
        d.contents = (i + 3).to_string();
        b.card_part(c, d);
    }
    let mut result = b.label("Result", Rect::new(60, 160, 452, 184), "Click Roll.", 14);
    result.text_align = TextAlign::Center;
    b.card_part(c, result);
    let roll = b.button(
        "Roll",
        Rect::new(196, 196, 316, 226),
        r#"on mouseUp
  repeat 8 times
    put random(6) into field "Die 1"
    put random(6) into field "Die 2"
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
    b.card_part(c, effects);

    // ----- Drawing -----
    let c = b.card(main, "Drawing");
    house(&mut b.stack.cards[c].picture);
    let hint = b.label(
        "Hint",
        Rect::new(20, 8, 492, 60),
        "Choose a paint tool on the left (Pencil, Brush, Rect, Oval, Bucket...) and draw on this card. Ctrl+Z undoes the last stroke. Choose Browse to click buttons again.",
        12,
    );
    b.card_part(c, hint);
    let clear = b.button(
        "Erase Picture",
        Rect::new(372, 270, 492, 296),
        "on mouseUp\n  answer \"Erase the picture on this card?\" with \"Cancel\" or \"Erase\"\n  if it is \"Erase\" then doMenu \"Clear Picture\"\nend mouseUp",
    );
    b.card_part(c, clear);
    let pencil = b.button("Pencil", Rect::new(372, 236, 492, 262), "on mouseUp\n  choose pencil tool\nend mouseUp");
    b.card_part(c, pencil);

    // ----- Animation -----
    let c = b.card(main, "Animation");
    let t = b.label("Heading", Rect::new(20, 10, 492, 36), "Moving things with scripts", 20);
    b.card_part(c, t);
    let mut ball = b.button("Ball", Rect::new(240, 180, 264, 204), "on mouseUp\n  beep\nend mouseUp");
    ball.style = PartStyle::Oval;
    ball.show_name = false;
    ball.hilite = true;
    ball.auto_hilite = false;
    b.card_part(c, ball);
    let bounce = b.button(
        "Bounce",
        Rect::new(20, 48, 140, 76),
        r#"on mouseUp
  put 5 into dx
  put 4 into dy
  if the hilite of button "Fast" then put 0 into delay
  else put 2 into delay
  repeat 200 times
    get the loc of button "Ball"
    put item 1 of it + dx into x
    put item 2 of it + dy into y
    if x < 32 or x > 480 then multiply dx by -1
    if y < 96 or y > 290 then multiply dy by -1
    set the loc of button "Ball" to x, y
    wait delay ticks
    if the mouseClick then exit repeat
  end repeat
end mouseUp"#,
    );
    b.card_part(c, bounce);
    let mut show = b.button("Show the ball", Rect::new(160, 50, 290, 72), "on mouseUp\n  set the visible of button \"Ball\" to the hilite of me\nend mouseUp");
    show.style = PartStyle::CheckBox;
    show.hilite = true;
    show.text_align = TextAlign::Left;
    b.card_part(c, show);
    for (i, name) in ["Slow", "Fast"].iter().enumerate() {
        let x = 300 + i as i32 * 80;
        let mut r = b.button(name, Rect::new(x, 50, x + 72, 72), "");
        r.style = PartStyle::Radio;
        r.family = 1;
        r.hilite = i == 0;
        r.text_align = TextAlign::Left;
        b.card_part(c, r);
    }
    let note = b.label("Note", Rect::new(20, 80, 492, 96), "Click anywhere during the bounce to stop it.", 11);
    b.card_part(c, note);

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
