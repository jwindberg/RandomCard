//! End-to-end interpreter tests: scripts run against a stack with no UI.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use super::interp::Interp;
use super::world::{DialogReply, Event, ObjRef, UiRequest, World, lock};
use crate::model::{Card, Id, Layer, Part, Picture, Rect, Stack};

/// A stack with three cards on one background, a background field "Name",
/// and a card button "Go" on the first card.
fn stack() -> Stack {
    let mut s = Stack::new("Test");
    let bg = s.backgrounds[0].id;
    s.backgrounds[0].parts.push(Part::new_field(10, "Name", Rect::new(10, 10, 200, 40)));
    s.cards[0].name = "One".into();
    s.cards[0].parts.push(Part::new_button(11, "Go", Rect::new(10, 60, 100, 90)));
    for (id, name) in [(20, "Two"), (21, "Three")] {
        s.cards.push(Card {
            id,
            name: name.into(),
            background: bg,
            picture: Picture::new(s.width, s.height),
            ..Card::default()
        });
    }
    for (i, n) in ["Apple", "banana", "Cherry pie"].iter().enumerate() {
        s.cards[i].bg_text.insert(10, n.to_string());
    }
    s
}

fn interp_with(s: Stack) -> Interp {
    let world = Arc::new(Mutex::new(World::new(s)));
    Interp::new(world, None, Arc::new(AtomicBool::new(false)))
}

/// Run message-box text and return the message box contents, or the error.
fn run(i: &mut Interp, src: &str) -> String {
    i.handle_event(Event::MsgBox(src.into()));
    let mut w = lock(&i.world);
    if let Some(UiRequest::ScriptError { msg, line, .. }) =
        w.requests.drain(..).find(|r| matches!(r, UiRequest::ScriptError { .. }))
    {
        return format!("ERROR: {msg} (line {line:?})");
    }
    w.msg.clone()
}

fn eval(src: &str) -> String {
    let mut i = interp_with(stack());
    run(&mut i, &format!("put {src}"))
}

fn set_script(i: &Interp, obj: ObjRef, script: &str) {
    let mut w = lock(&i.world);
    match obj {
        ObjRef::Stack => w.stack.script = script.into(),
        ObjRef::Card(id) => w.stack.card_mut(id).unwrap().script = script.into(),
        ObjRef::Background(id) => w.stack.background_mut(id).unwrap().script = script.into(),
        ObjRef::Part { layer, id, card } => w.stack.part_mut(card, layer, id).unwrap().script = script.into(),
        _ => {}
    }
}

#[test]
fn expressions() {
    assert_eq!(eval("1 + 2 * 3"), "7");
    assert_eq!(eval("(1 + 2) * 3"), "9");
    assert_eq!(eval("2 ^ 10"), "1024");
    assert_eq!(eval("7 div 2 & \",\" & 7 mod 2"), "3,1");
    assert_eq!(eval("10 / 4"), "2.5");
    assert_eq!(eval("1 / 3"), "0.333333");
    assert_eq!(eval("\"a\" && \"b\""), "a b");
    assert_eq!(eval("\"abc\" contains \"B\""), "true");
    assert_eq!(eval("\"x\" is in \"xyz\""), "true");
    assert_eq!(eval("\"x\" is not in \"abc\""), "true");
    assert_eq!(eval("\"10\" = 10.0"), "true");
    assert_eq!(eval("\"Hello\" is \"hello\""), "true");
    assert_eq!(eval("2 < 10"), "true");
    assert_eq!(eval("\"2\" < \"10\""), "true");
    assert_eq!(eval("\"abc\" < \"abd\""), "true");
    assert_eq!(eval("not true or false"), "false");
    assert_eq!(eval("5 is a number"), "true");
    assert_eq!(eval("\"5x\" is not a number"), "true");
    assert_eq!(eval("\"10,20\" is within \"0,0,100,100\""), "true");
    assert_eq!(eval("-3 + 1"), "-2");
    assert_eq!(eval("undefinedVar"), "undefinedVar");
    assert_eq!(eval("quote & \"hi\" & quote"), "\"hi\"");
}

#[test]
fn functions() {
    assert_eq!(eval("abs(-4)"), "4");
    assert_eq!(eval("the abs of -4"), "4");
    assert_eq!(eval("round(2.5) & round(3.5)"), "24");
    assert_eq!(eval("trunc(2.7)"), "2");
    assert_eq!(eval("max(3, 9, 2)"), "9");
    assert_eq!(eval("the max of \"3,9,2\""), "9");
    assert_eq!(eval("average(1,2,3,4)"), "2.5");
    assert_eq!(eval("length(\"hello\")"), "5");
    assert_eq!(eval("the length of \"hello\""), "5");
    assert_eq!(eval("offset(\"lo\", \"hello\")"), "4");
    assert_eq!(eval("charToNum(\"A\")"), "65");
    assert_eq!(eval("numToChar(66)"), "B");
    assert_eq!(eval("value(\"2 + 3\")"), "5");
    assert_eq!(eval("sqrt(16)"), "4");
    let r: i64 = eval("random(6)").parse().unwrap();
    assert!((1..=6).contains(&r));
    assert!(eval("the long date").contains(','));
    assert!(eval("the seconds").parse::<i64>().unwrap() > 3_000_000_000);
}

#[test]
fn chunks() {
    assert_eq!(eval("word 2 of \"one two three\""), "two");
    assert_eq!(eval("char 2 to 4 of \"abcdef\""), "bcd");
    assert_eq!(eval("item 2 of \"a,b,c\""), "b");
    assert_eq!(eval("the last item of \"a,b,c\""), "c");
    assert_eq!(eval("the first char of the last word of \"hello big world\""), "w");
    assert_eq!(eval("line 2 of (\"a\" & return & \"b\")"), "b");
    assert_eq!(eval("the number of words in \"a b  c\""), "3");
    assert_eq!(eval("the number of items of \"a,b,c\""), "3");
    assert_eq!(eval("the number of chars in \"hello\""), "5");
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "put \"a,b,c\" into x\nput \"X\" into item 2 of x\nput x"), "a,X,c");
    assert_eq!(run(&mut i, "put \"hello world\" into x\nput \"W\" into char 1 of word 2 of x\nput x"), "hello World");
    assert_eq!(run(&mut i, "put empty into x\nput \"c\" into line 3 of x\nput the number of lines in x"), "3");
    assert_eq!(run(&mut i, "put \"one two three\" into x\ndelete word 2 of x\nput x"), "one three");
    assert_eq!(run(&mut i, "put \"x\" into y\nput \"<\" before y\nput \">\" after y\nput y"), "<x>");
    assert_eq!(run(&mut i, "set the itemDelimiter to \"|\"\nput item 2 of \"a|b|c\""), "b");
    assert_eq!(run(&mut i, "put \"3\" & return & \"1\" & return & \"2\" into x\nsort lines of x numeric\nput x"), "1\n2\n3");
    assert_eq!(run(&mut i, "put \"b,a,c\" into x\nsort items of x descending\nput x"), "c,b,a");
}

#[test]
fn control_flow() {
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "put 0 into t\nrepeat with n = 1 to 10\nadd n to t\nend repeat\nput t"), "55");
    assert_eq!(run(&mut i, "put 0 into t\nrepeat 5 times\nadd 2 to t\nend repeat\nput t"), "10");
    assert_eq!(run(&mut i, "put 1 into t\nrepeat while t < 100\nmultiply t by 2\nend repeat\nput t"), "128");
    assert_eq!(run(&mut i, "put 10 into t\nrepeat until t = 0\nsubtract 1 from t\nend repeat\nput t"), "0");
    assert_eq!(
        run(&mut i, "put empty into s\nrepeat with n = 3 down to 1\nput n after s\nend repeat\nput s"),
        "321"
    );
    assert_eq!(
        run(
            &mut i,
            "put 0 into t\nrepeat with n = 1 to 10\nif n mod 2 = 0 then next repeat\nif n > 7 then exit repeat\nadd n to t\nend repeat\nput t"
        ),
        "16"
    );
    assert_eq!(run(&mut i, "if 1 < 2 then put \"yes\" else put \"no\""), "yes");
    assert_eq!(run(&mut i, "if 1 > 2 then\nput \"a\"\nelse if 2 > 1 then\nput \"b\"\nelse\nput \"c\"\nend if"), "b");
}

#[test]
fn handlers_and_message_path() {
    let mut i = interp_with(stack());
    set_script(
        &i,
        ObjRef::Stack,
        "function double n\n  return n * 2\nend double\n\non greet who\n  put \"Hello,\" && who into msg\nend greet\n\non mouseUp\n  put \"stack got\" && the short name of the target into msg\nend mouseUp",
    );
    assert_eq!(run(&mut i, "put double(21)"), "42");
    assert_eq!(run(&mut i, "greet \"World\""), "Hello, World");
    let btn = ObjRef::Part { layer: Layer::Card, id: 11, card: 3 };
    set_script(&i, btn, "on mouseUp\n  put \"button\" into msg\n  pass mouseUp\nend mouseUp");
    i.handle_event(Event::Send { target: btn, message: "mouseUp".into(), args: vec![] });
    assert_eq!(lock(&i.world).msg, "stack got Go");
    // A card handler intercepts before the stack.
    set_script(&i, ObjRef::Card(3), "on greet who\n  put \"card:\" && who into msg\nend greet");
    assert_eq!(run(&mut i, "greet \"x\""), "card: x");
    assert!(run(&mut i, "frobnicate").starts_with("ERROR: Can't understand"));
    // Globals are shared between handlers.
    set_script(&i, ObjRef::Card(3), "on setG\n  global g\n  put 5 into g\nend setG\non getG\n  global g\n  put g into msg\nend getG");
    assert_eq!(run(&mut i, "setG\ngetG"), "5");
    // Params
    set_script(&i, ObjRef::Card(3), "on p a, b\n  put the paramCount & \":\" & param(2) into msg\nend p");
    assert_eq!(run(&mut i, "p 1, 2"), "2:2");
    // send, do
    assert_eq!(run(&mut i, "send \"greet Bob\" to this stack"), "Hello, Bob");
    assert_eq!(run(&mut i, "do \"put 6*7\""), "42");
}

#[test]
fn objects_and_properties() {
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "put the number of cards"), "3");
    assert_eq!(run(&mut i, "put field \"Name\""), "Apple");
    assert_eq!(run(&mut i, "put bg field 1 of card 2"), "banana");
    assert_eq!(run(&mut i, "put the name of button 1"), "card button \"Go\"");
    assert_eq!(run(&mut i, "put the short name of card 3"), "Three");
    assert_eq!(run(&mut i, "put the number of card \"Two\""), "2");
    assert_eq!(run(&mut i, "put the id of this card"), "3");
    assert_eq!(run(&mut i, "put the rect of btn \"Go\""), "10,60,100,90");
    assert_eq!(run(&mut i, "set the loc of btn 1 to 100,100\nput the rect of btn 1"), "55,85,145,115");
    assert_eq!(run(&mut i, "set the name of btn 1 to \"Stop\"\nput the short name of btn 1"), "Stop");
    assert_eq!(run(&mut i, "put there is a card \"Two\""), "true");
    assert_eq!(run(&mut i, "put there is no button \"Nope\""), "true");
    assert_eq!(run(&mut i, "put \"Kiwi\" into field \"Name\" of card 2\nput field 1 of card 2"), "Kiwi");
    assert_eq!(run(&mut i, "put the number of bg fields"), "1");
    assert_eq!(run(&mut i, "put the number of card buttons"), "1");
    assert_eq!(run(&mut i, "set the hilite of btn 1 to true\nput the hilite of btn 1"), "true");
    assert_eq!(run(&mut i, "hide btn 1\nput the visible of btn 1"), "false");
    assert!(run(&mut i, "put field \"Missing\"").starts_with("ERROR: No such"));
    assert_eq!(run(&mut i, "put \"card 2\" into c\nput the short name of c"), "Two");
}

#[test]
fn navigation_messages() {
    let mut i = interp_with(stack());
    set_script(
        &i,
        ObjRef::Stack,
        "on openCard\n  global log\n  put \"open\" && the short name of this card & \";\" after log\nend openCard\non closeCard\n  global log\n  put \"close;\" after log\nend closeCard",
    );
    assert_eq!(
        run(&mut i, "global log\nput empty into log\ngo next card\ngo to card \"Three\"\ngo back\nput log"),
        "close;open Two;close;open Three;close;open Two;"
    );
    assert_eq!(run(&mut i, "go to card 1\nput the short name of this card"), "One");
    assert_eq!(run(&mut i, "go to last card\nput the number of this card"), "3");
    assert_eq!(run(&mut i, "go next\nput the number of this card"), "1");
    assert_eq!(run(&mut i, "go prev\nput the number of this card"), "3");
    assert_eq!(run(&mut i, "go to card \"Nowhere\"\nput the result"), "No such card.");
    assert_eq!(run(&mut i, "lock messages\ngo to card 1\nput the number of this card"), "1");
}

#[test]
fn find_and_mark() {
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "find \"cher\"\nput the short name of this card & \":\" & the foundText"), "Three:Cher");
    assert_eq!(run(&mut i, "find \"zzz\"\nput the result"), "not found");
    assert_eq!(run(&mut i, "find whole \"pie\"\nput the foundText"), "pie");
    assert_eq!(run(&mut i, "mark cards where field \"Name\" contains \"an\"\nput the number of marked cards"), "1");
    assert_eq!(run(&mut i, "unmark all cards\nput the number of marked cards"), "0");
    assert_eq!(
        run(&mut i, "sort cards descending by field \"Name\"\nput field 1 of card 1 & \",\" & field 1 of card 3"),
        "Cherry pie,Apple"
    );
}

#[test]
fn sorting_past_a_card_without_the_field() {
    // Apple's "Sort Cards by Name" button: an index card on its own
    // background has no field "Name", and sorts as if it were empty.
    let mut s = stack();
    let mut index_bg = s.backgrounds[0].clone();
    index_bg.id = 30;
    index_bg.parts.clear();
    s.backgrounds.push(index_bg);
    s.cards.insert(0, Card { id: 31, name: "Index".into(), background: 30, picture: Picture::new(s.width, s.height), ..Card::default() });
    let mut i = interp_with(s);
    assert_eq!(run(&mut i, "sort by field \"Name\"\nput the short name of card 1 && the short name of card 4"), "Index Three");
    assert_eq!(run(&mut i, "sort descending by field \"Name\"\nput the short name of card 1 && the short name of card 4"), "Three Index");
}

#[test]
fn dialogs_and_menus() {
    let mut i = interp_with(stack());
    i.canned_answers.push(DialogReply::Button("Yes".into()));
    assert_eq!(run(&mut i, "answer \"OK?\" with \"No\" or \"Yes\"\nput it"), "Yes");
    i.canned_answers.push(DialogReply::Text(Some("Ada".into())));
    assert_eq!(run(&mut i, "ask \"Name?\"\nput it"), "Ada");
    i.canned_answers.push(DialogReply::Text(None));
    assert_eq!(run(&mut i, "ask \"Name?\"\nput the result"), "Cancel");
    assert_eq!(run(&mut i, "doMenu \"New Card\"\nput the number of cards & \",\" & the number of this card"), "4,2");
    assert_eq!(run(&mut i, "doMenu \"Delete Card\"\nput the number of cards"), "3");
    set_script(&i, ObjRef::Card(3), "on doMenu item\n  if item is \"Next\" then put \"blocked\" into msg\n  else pass doMenu\nend doMenu");
    assert_eq!(run(&mut i, "go to card 1\ndoMenu \"Next\"\nput msg && the number of this card"), "blocked 1");
    assert_eq!(run(&mut i, "doMenu \"Last\"\nput the number of this card"), "3");
    assert_eq!(run(&mut i, "convert \"10/23/26\" to dateItems\nput item 1 to 3 of it"), "2026,10,23");
    assert_eq!(run(&mut i, "put \"1/2/2000\" into d\nconvert d to long date\nput d"), "Sunday, January 2, 2000");
}

#[test]
fn script_errors_report_lines() {
    let mut i = interp_with(stack());
    set_script(&i, ObjRef::Card(3), "on boom\n  put 1 into x\n  add \"a\" to x\nend boom");
    let r = run(&mut i, "boom");
    assert!(r.contains("Expected a number") && r.contains("Some(3)"), "{r}");
    set_script(&i, ObjRef::Card(3), "on broken\n  if then\nend broken");
    assert!(run(&mut i, "broken").contains("Script error"));
}

fn click(i: &mut Interp, card_name: &str, button: &str) -> Option<String> {
    let target = {
        let w = lock(&i.world);
        let card = w.stack.cards.iter().find(|c| c.name == card_name).expect(card_name);
        let bg = w.stack.background(card.background).unwrap();
        let (layer, id) = card
            .parts
            .iter()
            .find(|p| p.name == button)
            .map(|p| (Layer::Card, p.id))
            .or_else(|| bg.parts.iter().find(|p| p.name == button).map(|p| (Layer::Background, p.id)))
            .expect(button);
        ObjRef::Part { layer, id, card: card.id }
    };
    i.handle_event(Event::Send { target, message: "mouseUp".into(), args: vec![] });
    let mut w = lock(&i.world);
    w.requests.drain(..).find_map(|r| match r {
        UiRequest::ScriptError { msg, line, .. } => Some(format!("{msg} (line {line:?})")),
        _ => None,
    })
}

#[test]
fn startup_comes_before_open_stack_once() {
    let mut i = interp_with(stack());
    set_script(&i, ObjRef::Stack, "on startUp\n global log\n put \"startUp \" after log\nend startUp\non openStack\n global log\n put \"openStack\" after log\nend openStack");
    i.handle_event(Event::Startup);
    assert_eq!(run(&mut i, "global log\nput log"), "startUp openStack");
    i.handle_event(Event::Startup);
    assert_eq!(run(&mut i, "global log\nput log"), "startUp openStackopenStack");
}

#[test]
fn demo_stack_runs() {
    let mut i = interp_with(Stack::demo());
    i.handle_event(Event::Startup);
    assert_eq!(run(&mut i, "put field \"Page\""), "Card 1 of 22");
    let current = |i: &Interp| {
        let w = lock(&i.world);
        w.stack.card(w.card_id).unwrap().name.clone()
    };
    // The stack's own Tour menu.
    assert_eq!(run(&mut i, "put the number of menuItems of menu \"Tour\""), "21");
    i.handle_event(Event::DoMenu("Dice".into()));
    assert_eq!(current(&i), "Dice");
    i.handle_event(Event::DoMenu("First".into()));
    assert_eq!(click(&mut i, "Contents", "Start with the Basics"), None);
    assert_eq!(current(&i), "Basics");
    for b in ["Count Words", "Chunks", "Do Some Math", "What Time Is It?", "Where Am I?"] {
        i.canned_answers.push(DialogReply::Button("OK".into()));
        assert_eq!(click(&mut i, "Basics", b), None, "{b}");
    }
    run(&mut i, "go to card \"Fields\"");
    for n in ["Lined", "Wide", "No Wrap", "Scrolling"] {
        let lines = run(&mut i, &format!("put LineCount(\"cd fld\" && quote & \"{n}\" & quote)"));
        assert!(lines.parse::<usize>().is_ok_and(|l| l >= 1), "{n}: {lines}");
    }
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "Fields", "Count Lines"), None);
    assert!(!lock(&i.world).script_windows.iter().any(|t| t.name == "Prompt"));
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "Fields", "Switch"), None);
    i.canned_answers.push(DialogReply::List(vec![0, 3], "Validated".into()));
    assert_eq!(click(&mut i, "Fields", "Flash Fields"), None);
    assert!(lock(&i.world).msg.contains("Flashed Lined,Wide"), "{}", lock(&i.world).msg);
    // Copy Text and Paste Text bring the font and style along.
    assert_eq!(click(&mut i, "Fields", "Copy Text"), None);
    {
        let w = lock(&i.world);
        let card = w.stack.cards.iter().find(|c| c.name == "Fields").unwrap();
        let lined = card.parts.iter().find(|p| p.name == "Lined").unwrap();
        assert!(lined.contents.ends_with("\nwideMargins"), "{:?}", lined.contents);
        let at = lined.contents.chars().count() - 3;
        let style = crate::model::styles::style_at(&lined.styles, at);
        assert_eq!((style.font.as_deref(), style.style.as_deref()), (Some("Times"), Some("bold,italic")));
        let before = crate::model::styles::style_at(&lined.styles, 3);
        assert_eq!(before.font, None, "the text already there keeps its look");
        assert!(w.text_clip.as_ref().is_some_and(|c| c.text == "wideMargins"));
    }
    // Colors on, then off again.
    assert_eq!(click(&mut i, "Fields", "Colors"), None);
    {
        let w = lock(&i.world);
        let card = w.stack.cards.iter().find(|c| c.name == "Fields").unwrap();
        let part = |n: &str| card.parts.iter().find(|p| p.name == n).unwrap();
        assert_eq!(part("Wide").fore_color, Some([128, 0, 128]));
        assert_eq!(part("Colors").back_color, Some([0, 0, 128]));
        assert_eq!(part("Lined").styles[0].style.color, Some([255, 0, 0]));
    }
    assert_eq!(click(&mut i, "Fields", "Colors"), None);
    {
        let w = lock(&i.world);
        let card = w.stack.cards.iter().find(|c| c.name == "Fields").unwrap();
        assert!(card.parts.iter().all(|p| p.fore_color.is_none() && p.back_color.is_none() && p.border_color.is_none()));
        assert!(card.parts.iter().all(|p| p.styles.iter().all(|r| r.style.color.is_none())));
    }
    // Painting in color from a script, with the colors put back after.
    assert_eq!(click(&mut i, "Drawing", "Paint in Color"), None);
    {
        let w = lock(&i.world);
        let pic = &w.stack.card(w.card_id).unwrap().picture;
        assert!(pic.has_color());
        assert_eq!(pic.get(381, 110), crate::model::Pixel::rgba(255, 0, 0, 255), "the red balloon's middle");
        assert_eq!(pic.get(368, 110), crate::model::Pixel::rgba(0, 0, 128, 255), "its navy border");
        assert_eq!((w.line_color, w.fill_color, w.tool), (crate::model::Pixel::BLACK, crate::model::Pixel::BLACK, crate::runtime::world::Tool::Browse));
    }
    assert_eq!(run(&mut i, "set the brushColor to \"#FF8000\"\nput the brushColor"), "255,128,0");
    run(&mut i, "set the brushColor to black");
    // Shift straightens each drag: a circle, spokes at multiples of 15°,
    // and a brush stroke straight across.
    assert_eq!(click(&mut i, "Drawing", "Draw with Shift"), None);
    {
        let w = lock(&i.world);
        let pic = &w.stack.card(w.card_id).unwrap().picture;
        let black = |x, y| pic.get(x, y) == crate::model::Pixel::BLACK;
        assert!(black(50, 111), "the circle's bottom, past the oval's drag");
        assert!(black(50, 106), "the spoke turned straight down");
        assert!(black(230, 84) && !black(230, 96), "the brush stroke straight across");
        assert_eq!(w.tool, crate::runtime::world::Tool::Browse);
    }
    // With no window, its file dialog cancels straight away.
    assert_eq!(click(&mut i, "Drawing", "Picture Files"), None);
    // FatBits: the pencil, magnified.
    assert_eq!(click(&mut i, "Drawing", "FatBits"), None);
    assert_eq!(lock(&i.world).tool, crate::runtime::world::Tool::Pencil);
    run(&mut i, "choose browse tool");
    // The brush cursor's button picks a big brush.
    assert_eq!(click(&mut i, "Drawing", "Brush Cursor"), None);
    assert_eq!(lock(&i.world).brush, 5);
    run(&mut i, "choose browse tool");
    // The spray can, small and light, then big and heavy.
    let (size, density) = {
        let w = lock(&i.world);
        (w.spray_size, w.spray_density)
    };
    // Select and Revert: a shape painted after Keep, then selected.
    lock(&i.world).requests.clear();
    assert_eq!(click(&mut i, "Drawing", "Select and Revert"), None);
    {
        let w = lock(&i.world);
        let shape = w.new_element().expect("the shape is the new element");
        assert!(shape.contains(50, 150), "{:?}", shape.rect);
        assert!(w.kept_picture().is_some_and(|k| k.get(50, 150) != w.stack.card(w.card_id).unwrap().picture.get(50, 150)));
    }
    run(&mut i, "choose browse tool");
    // Reshape: a rectangle around the house, then Rotate's handles.
    lock(&i.world).requests.clear();
    i.handle_event(Event::MsgBox("send mouseUp to card button \"Reshape\" of card \"Drawing\"".into()));
    let asked = std::mem::take(&mut lock(&i.world).requests);
    assert!(
        matches!(asked.as_slice(), [UiRequest::SelectPaint((70, 130, 230, 272)), UiRequest::Reshape(crate::paint::Reshape::Rotate)]),
        "{asked:?}"
    );
    run(&mut i, "choose browse tool");
    assert_eq!(click(&mut i, "Drawing", "Spray Sizes"), None);
    {
        let w = lock(&i.world);
        assert_eq!((w.spray_size, w.spray_density), (size, density));
        let pic = &w.stack.card(w.card_id).unwrap().picture;
        let dots = |x0: i32, x1: i32| (x0..=x1).flat_map(|x| (280..=320).map(move |y| (x, y))).filter(|(x, y)| pic.get(*x, *y) == crate::model::Pixel::BLACK).count();
        let (light, heavy) = (dots(260, 330), dots(425, 480));
        assert!(heavy > light * 2, "light {light}, heavy {heavy}");
    }
    run(&mut i, "choose browse tool");
    // Draw Multiple: rings from one point, and boxes lined up on the grid.
    assert_eq!(click(&mut i, "Drawing", "Draw Multiple"), None);
    {
        let w = lock(&i.world);
        let pic = &w.stack.card(w.card_id).unwrap().picture;
        let rings = (430..=470).filter(|x| pic.get(*x, 140) == crate::model::Pixel::BLACK).count();
        assert!(rings >= 5, "{rings} rings");
        assert!(!w.multiple && !w.centered && !w.grid);
    }
    run(&mut i, "choose browse tool");
    // The Web: JSON read and built on the card, offline; Send asks for a URL.
    run(&mut i, "go to card \"The Web\"");
    assert_eq!(click(&mut i, "The Web", "Read JSON"), None);
    assert_eq!(run(&mut i, "put card field \"Result\""), "Name: Ada Lovelace\nLanguages: 3\nThe second: French\nFirst note: program");
    assert_eq!(click(&mut i, "The Web", "Build JSON"), None);
    assert_eq!(click(&mut i, "The Web", "Read JSON"), None);
    assert_eq!(run(&mut i, "put line 1 of card field \"Result\""), "Name: Grace Hopper");
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "The Web", "Send"), None);
    assert_eq!(run(&mut i, "put card field \"Reply\""), "");
    // Its method is a combo field: choices to pick from, or type another.
    assert_eq!(run(&mut i, "put line 5 of the choices of card field \"Method\""), "QUERY");
    run(&mut i, "set the choices of card field \"Method\" to \"GET\" & return & \"HEAD\"");
    assert_eq!(run(&mut i, "put the number of lines of the choices of card field \"Method\""), "2");
    // Channels: two at once, the soundVolume, the Tour's own song.
    run(&mut i, "go to card \"Channels\"");
    for name in ["Play Both", "Stop Bass", "Stop All", "Play Song", "Stop Song"] {
        assert_eq!(click(&mut i, "Channels", name), None, "{name}");
    }
    assert_eq!(run(&mut i, "put the soundChannel"), "1");
    // The Volume slider: dragged to 3, it sets the soundVolume.
    run(&mut i, "put 3 into button \"Volume\"\nsend \"sliderMoved\" to button \"Volume\"");
    assert_eq!(run(&mut i, "put the soundVolume && the style of button \"Volume\" && the max of button \"Volume\""), "3 slider 7");
    assert_eq!(run(&mut i, "put field \"Volume\""), "the soundVolume: 3 of 7");
    // Movies: the Tour's own movie, opened, played and closed, when the
    // system can play movies.
    run(&mut i, "go to card \"Movies\"");
    assert_eq!(click(&mut i, "Movies", "Open Movie"), None);
    if run(&mut i, "put the number of movies") == "1" {
        assert_eq!(run(&mut i, "put the name of window \"Card Trick\""), "Card Trick.mp4");
        assert_eq!(run(&mut i, "put the rect of window \"Card Trick\""), "20,112,244,296");
        for name in ["Pause", "Play", "Rewind", "Backward", "Faster", "Loop", "Controller", "Where Is It?"] {
            assert_eq!(click(&mut i, "Movies", name), None, "{name}");
        }
        assert_eq!(run(&mut i, "put the loop of movie 1 && the rate of movie 1"), "true 2");
        assert!(run(&mut i, "put field \"Movie Messages\"").starts_with("currTime "));
        run(&mut i, "go to card \"Keys\"");
        assert_eq!(run(&mut i, "put the number of movies"), "0");
    }
    // More Text: the list's selection, textWidth, the selectedButton.
    run(&mut i, "go to card \"More Text\"");
    assert_eq!(run(&mut i, "put the textFont of field \"Pixel\""), "Tour Pixel");
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "More Text", "Which Color?"), None);
    run(&mut i, "select line 5 of field \"Colors\"");
    assert_eq!(run(&mut i, "put the selectedText of field \"Colors\" && the selectedLine of field \"Colors\""), "Blue line 5 of card field 4");
    for name in ["Which Color?", "Measure", "Which Size?"] {
        i.canned_answers.push(DialogReply::Button("OK".into()));
        assert_eq!(click(&mut i, "More Text", name), None, "{name}");
    }
    assert_eq!(run(&mut i, "put the selectedButton of card family 1"), "card button 5");
    // Keys: the card shows the keysDown when idle.
    run(&mut i, "go to card \"Keys\"");
    run(&mut i, "send \"idle\" to this card");
    assert_eq!(run(&mut i, "put field \"Keys Down\""), "Hold some keys down.");
    // Japanese: a character each, and charToNum gives Unicode.
    assert_eq!(click(&mut i, "Contents", "Japanese"), None);
    assert_eq!(current(&i), "Japanese");
    assert_eq!(click(&mut i, "Japanese", "Say Hello"), None);
    assert_eq!(run(&mut i, "put card field \"Japanese\""), "こんにちは。RandomCard へようこそ！");
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "Japanese", "Characters"), None);
    assert_eq!(run(&mut i, "put the number of chars of card field \"Japanese\""), "6");
    assert_eq!(run(&mut i, "put charToNum(\"花\") && numToChar(26413)"), "33457 札");
    assert_eq!(click(&mut i, "Japanese", "Clear"), None);
    // Don't Save on the Debugging card's answer line, then off.
    run(&mut i, "go to card \"Debugging\"");
    assert_eq!(click(&mut i, "Debugging", "Don't Save"), None);
    assert_eq!(run(&mut i, "put the dontSave of card field \"Answer\""), "true");
    assert_eq!(click(&mut i, "Debugging", "Don't Save"), None);
    assert_eq!(run(&mut i, "put the dontSave of card field \"Answer\""), "false");
    // AddColor on, then off, on the card it's clicked on.
    run(&mut i, "go to card \"Drawing\"");
    assert_eq!(click(&mut i, "Drawing", "Color the House"), None);
    let drawing_colors = |i: &Interp| lock(&i.world).stack.cards.iter().find(|c| c.name == "Drawing").unwrap().colors.len();
    assert_eq!(drawing_colors(&i), 4);
    assert_eq!(run(&mut i, "addColor getPictName,cd,4\nput the result"), "Rainbow");
    assert_eq!(click(&mut i, "Drawing", "Color the House"), None);
    assert_eq!(drawing_colors(&i), 0);
    let before = i.printed.len();
    assert_eq!(click(&mut i, "Print & Find", "Print PostScript"), None);
    assert_eq!(click(&mut i, "Print & Find", "Open a Document"), None);
    assert!(i.printed.len() == before + 1 && i.printed.last().unwrap().extension().is_some_and(|e| e == "ps"));
    run(&mut i, "go to card \"Debugging\"");
    for b in ["List Globals", "This Computer"] {
        i.canned_answers.push(DialogReply::Button("OK".into()));
        assert_eq!(click(&mut i, "Debugging", b), None, "{b}");
    }
    assert_eq!(run(&mut i, "global tourVisits\nput tourVisits"), "1");
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "Debugging", "Compact Stack"), None);
    // Find in a Script opens the editor on another button's script.
    assert_eq!(click(&mut i, "Debugging", "Find in a Script"), None);
    assert!(lock(&i.world).msg.contains("replaces"), "{}", lock(&i.world).msg);
    for b in ["Mouse Window", "Info Window"] {
        assert_eq!(click(&mut i, "Debugging", b), None, "{b}");
    }
    assert_eq!(run(&mut i, "put \"Mousoid\" is in the windows and \"Infoid\" is in the windows"), "true");
    for b in ["Mouse Window", "Info Window"] {
        assert_eq!(click(&mut i, "Debugging", b), None, "{b}");
    }
    assert_eq!(run(&mut i, "put \"Mousoid\" is in the windows"), "false");
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "Debugging", "Call Me Back"), None);
    i.canned_answers.push(DialogReply::Text(Some("Tour Window".into())));
    assert_eq!(click(&mut i, "Debugging", "Rename Window"), None);
    assert_eq!(lock(&i.world).window_title.as_ref().map(|t| t.1.as_str()), Some("Tour Window"));
    run(&mut i, "go to card \"Basics\"");
    // Saved, the stack knows its folder.
    lock(&i.world).stack.path = Some(std::env::temp_dir().join("Tour.rcstack"));
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "Basics", "Where Am I?"), None);
    lock(&i.world).stack.path = None;
    i.canned_answers.push(DialogReply::Text(Some("Dan".into())));
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "Basics", "Say Hello"), None);
    // The palette floats over every card, and says when it opens and closes.
    assert_eq!(click(&mut i, "Contents", "Palettes"), None);
    assert_eq!(click(&mut i, "Palettes", "Show Palette"), None);
    assert_eq!(run(&mut i, "put field \"Said\" of card \"Palettes\""), "openPalette Tour Palette 1");
    i.handle_event(Event::PaletteButton("Tour Palette".into(), 1));
    assert_eq!(current(&i), "Card Window");
    i.handle_event(Event::PaletteButton("Tour Palette".into(), 2));
    assert_eq!(current(&i), "Contents");
    run(&mut i, "go to card \"Palettes\"");
    assert_eq!(click(&mut i, "Palettes", "Close Palette"), None);
    assert_eq!(run(&mut i, "put field \"Said\" of card \"Palettes\""), "closePalette Tour Palette");
    // Home opens in a window of its own, beside the Tour.
    let home_dir = std::env::temp_dir().join(format!("randomcard-tourhome-{}", std::process::id()));
    std::fs::create_dir_all(&home_dir).unwrap();
    Stack::home().save(&home_dir.join("Home.rcstack")).unwrap();
    lock(&i.world).home_path = Some(home_dir.join("Home.rcstack"));
    assert_eq!(click(&mut i, "Palettes", "Home in a Window"), None);
    assert_eq!(run(&mut i, "put field \"Said\" of card \"Palettes\" of stack \"Tour\""), "suspendStack, for Home.rcstack: the destination");
    assert_eq!(run(&mut i, "put the short name of this stack && the number of lines of the stacks"), "Home 2");
    run(&mut i, "close window \"Home\"");
    assert_eq!(current(&i), "Palettes");
    // Home's cards, read from here.
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "Palettes", "Ask Home"), None);
    assert_eq!(current(&i), "Palettes");
    // HyperCard's Navigator, drawn by RandomCard: its Next goes on, and
    // the button closes it again.
    assert_eq!(click(&mut i, "Palettes", "Navigator"), None);
    assert!(run(&mut i, "put field \"Said\"").starts_with("HyperCard's Navigator"));
    i.handle_event(Event::PaletteButton("Navigator".into(), 6));
    assert_eq!(current(&i), "Card Window");
    run(&mut i, "go to card \"Palettes\"");
    assert_eq!(click(&mut i, "Palettes", "Navigator"), None);
    assert_eq!(run(&mut i, "put there is a window \"Navigator\""), "false");
    // The palette moved and put back, then shown with `show pattern window`.
    lock(&i.world).palette_state.rect = crate::model::Rect::new(10, 60, 146, 256);
    assert_eq!(click(&mut i, "Palettes", "Move the Tools"), None);
    assert_eq!(run(&mut i, "put the loc of tool window"), "10,60");
    assert!(run(&mut i, "put field \"Said\"").contains("went from 10,60"));
    assert_eq!(click(&mut i, "Palettes", "Edit a Pattern"), None);
    assert_eq!(lock(&i.world).tool, crate::runtime::world::Tool::Brush);
    // The Regular Polygon's sides go round and come back.
    let sides = lock(&i.world).poly_sides;
    assert_eq!(click(&mut i, "Palettes", "Tool Icons"), None);
    assert_eq!(lock(&i.world).poly_sides, sides);
    assert!(run(&mut i, "put field \"Said\"").contains("icon shows its setting"));
    assert_eq!(click(&mut i, "Palettes", "Peek"), None);
    assert!(run(&mut i, "put field \"Said\"").contains("hidden button"));
    // GeneralPalette, built in: its center button goes to Contents, and
    // closing it sends paletteDismiss.
    assert_eq!(click(&mut i, "Palettes", "Apple's Palette"), None);
    assert_eq!(run(&mut i, "global HaveGeneralPalette\nput HaveGeneralPalette && there is a window \"GeneralPalette\""), "true true");
    i.handle_event(Event::PaletteButton("GeneralPalette".into(), 1));
    assert_eq!(current(&i), "Contents");
    run(&mut i, "go to card \"Palettes\"");
    assert_eq!(click(&mut i, "Palettes", "Apple's Palette"), None);
    assert_eq!(run(&mut i, "global HaveGeneralPalette\nput HaveGeneralPalette && there is a window \"GeneralPalette\""), "false false");
    assert_eq!(run(&mut i, "put field \"Said\""), "paletteDismiss: GeneralPalette closed.");
    // The card window: smaller and scrolled, zoomed, and bigger cards, all
    // put back on leaving.
    lock(&i.world).screen_rect = crate::model::Rect::new(0, 0, 1024, 768);
    assert_eq!(click(&mut i, "Contents", "Card Window"), None);
    assert_eq!(click(&mut i, "Card Window", "Window Size"), None);
    assert_eq!(run(&mut i, "put the width of card window && the height of card window"), "360 240");
    assert_eq!(run(&mut i, "put field \"Sent\""), "sizeWindow: 360 by 240");
    assert_eq!(click(&mut i, "Card Window", "Scroll Around"), None);
    assert!(run(&mut i, "put field \"Said\"").contains("150,100"));
    assert_eq!(run(&mut i, "put the scroll of card window"), "0,0");
    assert_eq!(click(&mut i, "Card Window", "Zoom"), None);
    assert_eq!(run(&mut i, "put the zoomed of card window && the width of card window"), "true 512");
    assert_eq!(click(&mut i, "Card Window", "Zoom"), None);
    assert_eq!(run(&mut i, "put the zoomed of card window && the width of card window"), "false 360");
    assert_eq!(click(&mut i, "Card Window", "Card Size"), None);
    assert_eq!(run(&mut i, "put the width of card \"Contents\" && the height of this card"), "640 400");
    run(&mut i, "go to card \"Contents\"");
    assert_eq!(run(&mut i, "put the width of this card && the height of this card && the width of card window"), "512 342 512");
    run(&mut i, "go to card \"Palettes\"");
    assert_eq!(click(&mut i, "Palettes", "Secret"), None);
    assert_eq!(run(&mut i, "put field \"Said\""), "You found the hidden button.");
    assert_eq!(click(&mut i, "Palettes", "Paint Menu"), None);
    assert_eq!(lock(&i.world).tool, crate::runtime::world::Tool::Select);
    run(&mut i, "choose browse tool");
    assert_eq!(click(&mut i, "Palettes", "Hide the Tools"), None);
    assert_eq!(run(&mut i, "put the visible of pattern window"), "false");
    assert_eq!(click(&mut i, "Palettes", "Show the Tools"), None);
    assert_eq!(run(&mut i, "put the visible of tool window"), "true");
    lock(&i.world).home_path = None;
    std::fs::remove_dir_all(&home_dir).ok();
    assert_eq!(click(&mut i, "Contents", "Dice & Effects"), None);
    assert_eq!(click(&mut i, "Dice", "Roll"), None);
    assert!(run(&mut i, "put field \"Result\" of card \"Dice\"").starts_with("You rolled"));
    // The Sides popup chooses the dice.
    assert_eq!(click(&mut i, "Dice", "Sides"), None);
    assert_eq!(run(&mut i, "put field \"Result\" of card \"Dice\""), "Rolling 6-sided dice.");
    run(&mut i, "select line 1 of button \"Sides\" of card \"Dice\"");
    assert_eq!(click(&mut i, "Dice", "Roll"), None);
    for die in ["Die 1", "Die 2"] {
        let n: u32 = run(&mut i, &format!("put field \"{die}\" of card \"Dice\"")).parse().unwrap();
        assert!((1..=4).contains(&n), "{die} rolled {n} on a 4-sided die");
    }
    // The ball moves on idle only while it's moving, by its speed.
    run(&mut i, "go to card \"Animation\"");
    let idle = |i: &mut Interp| run(i, "send \"idle\" to this card\nput the loc of button \"Ball\"");
    let still = idle(&mut i);
    assert_eq!(idle(&mut i), still, "the ball starts out still");
    assert_eq!(run(&mut i, "put field \"Speed\""), "Speed: 4");
    assert_eq!(click(&mut i, "Animation", "Start"), None);
    assert_eq!(run(&mut i, "put the name of card button 1"), "card button \"Stop\"");
    assert_eq!(idle(&mut i), "256,196");
    assert_eq!(click(&mut i, "Animation", "Faster"), None);
    assert_eq!(run(&mut i, "put field \"Speed\""), "Speed: 5");
    assert_eq!(idle(&mut i), "261,201");
    // Hiding the ball leaves it moving.
    assert_eq!(click(&mut i, "Animation", "Show the ball"), None);
    assert_eq!(idle(&mut i), "266,206");
    assert_eq!(click(&mut i, "Animation", "Stop"), None);
    assert_eq!(idle(&mut i), "266,206");
    assert_eq!(run(&mut i, "put the name of card button 1"), "card button \"Start\"");
    // ZoomRect animates, then clears itself.
    assert_eq!(click(&mut i, "Animation", "Zoom"), None);
    assert!(lock(&i.world).zoom.is_none());
    assert!(run(&mut i, "zoomRect \"1,2,3\"").contains("two rectangles"));
    // Every Contents button reaches its card.
    for (button, card) in [
        ("Draw Something", "Drawing"),
        ("Sound", "Sound"),
        ("Keys", "Keys"),
        ("Fields & Text", "Fields"),
        ("Print & Find", "Print & Find"),
        ("Icons", "Icons"),
        ("Debugging", "Debugging"),
    ] {
        assert_eq!(click(&mut i, "Contents", button), None, "{button}");
        assert_eq!(current(&i), card);
    }
    // Scripts name fields on the card that's showing, so go to each card first.
    run(&mut i, "go to card \"Sound\"");
    for b in ["Boing", "Play Tune", "Flute", "Dial", "Stop"] {
        assert_eq!(click(&mut i, "Sound", b), None, "{b}");
    }
    // The Keys card reports keys it's sent.
    run(&mut i, "go to card \"Keys\"");
    run(&mut i, "send \"arrowKey left\" to card \"Keys\"");
    assert_eq!(run(&mut i, "put field \"Last Key\" of card \"Keys\""), "arrowKey left");
    run(&mut i, "send \"functionKey 5\" to card \"Keys\"");
    assert_eq!(run(&mut i, "put field \"Last Key\" of card \"Keys\""), "functionKey 5");
    // The icon buttons change their icons by name, to HyperCard's own.
    run(&mut i, "go to card \"Icons\"");
    assert_eq!(click(&mut i, "Icons", "Roll Me"), None);
    let die = run(&mut i, "put the icon of card button \"Roll Me\"").parse::<i32>().unwrap();
    assert!((2101..=2106).contains(&die), "rolled icon {die}");
    assert_eq!(click(&mut i, "Icons", "Juggle"), None);
    assert_eq!(run(&mut i, "put the icon of card button \"Juggle\""), "21574");
    // Deal shows playing cards: icons of any size, in color.
    let card_now = |i: &mut Interp| {
        let id = run(i, "put the icon of card button \"Deal\"").parse::<i32>().unwrap();
        let w = i.w();
        let icon = &w.stack.icons[&id];
        assert!(icon.picture.has_color());
        (icon.name.clone(), icon.picture.width, icon.picture.height)
    };
    assert_eq!(card_now(&mut i), ("King of Hearts".to_string(), 72, 105));
    assert_eq!(click(&mut i, "Icons", "Deal"), None);
    assert_eq!(card_now(&mut i).0, "Queen of Clubs");
    assert_eq!(click(&mut i, "Icons", "Deal"), None);
    assert_eq!(click(&mut i, "Icons", "Deal"), None);
    assert_eq!(card_now(&mut i).0, "Ace of Spades");
    assert_eq!(click(&mut i, "Icons", "Icon Editor"), None);
    // The debugger card's script runs to its answer (tests don't stop at checkpoints).
    run(&mut i, "go to card \"Debugging\"");
    assert_eq!(click(&mut i, "Debugging", "Step Through Me"), None);
    assert_eq!(run(&mut i, "put field \"Answer\" of card \"Debugging\""), "1 + 4 + 9 + 16 + 25 = 55");
    assert_eq!(click(&mut i, "Debugging", "Message Watcher"), None);
    run(&mut i, "go to card \"Print & Find\"");
    assert_eq!(click(&mut i, "Print & Find", "Find Bill"), None);
    assert_eq!(current(&i), "Bill Atkinson");
    // Address book on its own background.
    assert_eq!(click(&mut i, "Contents", "Address Book"), None);
    assert_eq!(current(&i), "Grace Hopper");
    // The address book is for using, not editing: it runs at the Typing level.
    assert_eq!(run(&mut i, "put the userLevel"), "2");
    assert_eq!(run(&mut i, "put field \"Count\""), "1 of 3");
    assert_eq!(click(&mut i, "Grace Hopper", "Sort"), None);
    assert_eq!(current(&i), "Ada Lovelace");
    assert_eq!(click(&mut i, "Ada Lovelace", "▶"), None);
    assert_eq!(current(&i), "Bill Atkinson");
    assert_eq!(click(&mut i, "Bill Atkinson", "▶"), None);
    assert_eq!(current(&i), "Grace Hopper");
    i.canned_answers.push(DialogReply::Text(Some("compiler".into())));
    assert_eq!(click(&mut i, "Grace Hopper", "Find…"), None);
    assert_eq!(run(&mut i, "put the foundText"), "compiler");
    assert_eq!(click(&mut i, "Grace Hopper", "New"), None);
    assert_eq!(run(&mut i, "put the number of cards of this background"), "4");
    i.canned_answers.push(DialogReply::Button("Delete".into()));
    assert_eq!(click(&mut i, "", "Delete"), None);
    assert_eq!(run(&mut i, "put the number of cards"), "22");
    assert_eq!(click(&mut i, "Grace Hopper", "Contents"), None);
    assert_eq!(current(&i), "Contents");
    assert_eq!(run(&mut i, "put the userLevel"), "5");
}

#[test]
fn icons_by_id_or_name() {
    let mut s = stack();
    s.icons.insert(128, crate::model::Icon { name: "Star".into(), picture: Picture::new(32, 32) });
    let mut i = interp_with(s);
    assert_eq!(run(&mut i, "set the icon of button 1 to \"star\"\nput the icon of button 1"), "128");
    assert_eq!(run(&mut i, "set the icon of button 1 to 0\nput the icon of button 1"), "0");
    assert!(run(&mut i, "set the icon of button 1 to \"Nope\"").starts_with("ERROR: No such icon"));
    // HyperCard's own icons, by name or id, join the stack's.
    assert_eq!(run(&mut i, "set the icon of button 1 to \"black return arrow\"\nput the icon of button 1"), "7417");
    assert_eq!(lock(&i.world).stack.icons.get(&7417).map(|i| i.name.as_str()), Some("Black Return Arrow"));
    assert_eq!(run(&mut i, "set the icon of button 1 to 1012\nput the icon of button 1"), "1012");
    assert!(lock(&i.world).stack.icons.contains_key(&1012));
    // A number is accepted even with no icon behind it.
    assert_eq!(run(&mut i, "set the icon of button 1 to 31999\nput the icon of button 1"), "31999");
}

#[test]
fn hypercard_icons_are_built_in() {
    let icons = crate::import::hypercard_icons();
    assert_eq!(icons.len(), 216);
    let arrow = &icons[&7417];
    assert_eq!(arrow.name, "Black Return Arrow");
    assert_eq!((arrow.picture.width, arrow.picture.height), (32, 32));
    assert!(arrow.picture.iter().any(|x| x == crate::model::Pixel::BLACK));
    assert_eq!(icons[&11645].name, "Bill's Rolo");
}

#[test]
fn playing_sounds() {
    let mut s = stack();
    let ding = crate::model::Sound { name: "Ding".into(), rate: 11025.0, base_note: 60, samples: vec![0; 11025], ..Default::default() };
    s.sounds.insert(9, ding);
    let mut i = interp_with(s);
    assert_eq!(run(&mut i, "put the sound"), "done");
    assert_eq!(run(&mut i, "play \"ding\"\nput the sound"), "Ding");
    assert_eq!(run(&mut i, "play stop\nput the sound"), "done");
    // HyperCard's voices fall back to RandomCard's own.
    assert_eq!(run(&mut i, "play \"harpsichord\" tempo 200 \"c4e d e f g a b c5h\"\nput the sound"), "harpsichord");
    run(&mut i, "play stop");
    assert_eq!(run(&mut i, "play \"nothing\"\nput the result"), "Can't find sound \"nothing\"");
    assert_eq!(run(&mut i, "dial \"555-1234\"\nput the sound"), "dial");
}

#[test]
fn built_in_commands_are_messages() {
    let mut i = interp_with(stack());
    // A handler takes the command; it doesn't run.
    set_script(&i, ObjRef::Stack, "on go dest\n  put \"stopped:\" && dest into msg\nend go");
    assert_eq!(run(&mut i, "go to next card\nput msg && the number of this card"), "stopped: to next card 1");
    // Passing it lets the command run.
    set_script(&i, ObjRef::Stack, "on go\n  global log\n  put \"seen\" into log\n  pass go\nend go");
    assert_eq!(run(&mut i, "global log\ngo next\nput log && the number of this card"), "seen 2");
    // A command inside its own handler runs directly instead of recursing.
    set_script(&i, ObjRef::Stack, "on go\n  go to card \"Three\"\nend go");
    assert_eq!(run(&mut i, "go to card 1\nput the short name of this card"), "Three");
    // choose gets the tool's name and number.
    set_script(&i, ObjRef::Stack, "on choose toolName, toolNumber\n  put toolName & \",\" & toolNumber into msg\nend choose");
    assert_eq!(run(&mut i, "choose pencil tool\nput msg && the tool"), "pencil,6 browse tool");
    // Any command can be intercepted, including put.
    set_script(&i, ObjRef::Stack, "on beep\n  put \"quiet\" into msg\nend beep");
    assert_eq!(run(&mut i, "beep\nput msg"), "quiet");
    assert_eq!(lock(&i.world).beeps, 0);
    // Setting a script from HyperTalk takes effect at once.
    set_script(&i, ObjRef::Stack, "");
    assert_eq!(
        run(&mut i, "set the script of this stack to \"on beep\" & return & \"put 7 into msg\" & return & \"end beep\"\nbeep\nput msg"),
        "7"
    );
}

#[test]
fn keyboard_messages() {
    use super::world::KeyAction;
    let mut i = interp_with(stack());
    let field = ObjRef::Part { layer: Layer::Background, id: 10, card: 3 };
    let key = |i: &mut Interp, message: &str, args: &[&str], default: KeyAction| {
        i.handle_event(Event::Key {
            target: field,
            message: message.into(),
            args: args.iter().map(|a| a.to_string()).collect(),
            default,
            at: std::time::Instant::now(),
        });
        std::mem::take(&mut lock(&i.world).requests)
    };
    // Unhandled: the UI is asked to type the key.
    let r = key(&mut i, "keyDown", &["a"], KeyAction::Insert("a".into()));
    assert!(matches!(&r[..], [UiRequest::Key(o, KeyAction::Insert(t))] if *o == field && t == "a"));
    // A handler that doesn't pass swallows keys; one that passes lets them through.
    set_script(&i, ObjRef::Stack, "on keyDown k\n  if k is a number then pass keyDown\nend keyDown");
    assert!(key(&mut i, "keyDown", &["x"], KeyAction::Insert("x".into())).is_empty());
    assert_eq!(key(&mut i, "keyDown", &["7"], KeyAction::Insert("7".into())).len(), 1);
    // returnInField reaches the field's own script first.
    set_script(&i, field, "on returnInField\n  put \"done\" into msg\nend returnInField");
    assert!(key(&mut i, "returnInField", &[], KeyAction::Insert("\n".into())).is_empty());
    assert_eq!(lock(&i.world).msg, "done");
    // An unhandled command key does its menu command on the script thread.
    let before = lock(&i.world).stack.cards.len();
    assert!(key(&mut i, "commandKeyDown", &["n"], KeyAction::Menu("New Card".into())).is_empty());
    assert_eq!(lock(&i.world).stack.cards.len(), before + 1);
}

#[test]
fn script_painting() {
    use crate::model::Pixel;
    let mut i = interp_with(stack());
    let pixel = |i: &Interp, x: i32, y: i32| lock(&i.world).stack.cards[0].picture.get(x, y);
    // drag with the line tool paints a line.
    run(&mut i, "choose line tool\ndrag from 10,10 to 50,10");
    assert_eq!(pixel(&i, 30, 10), Pixel::BLACK);
    // Filled rectangles use the current pattern inside.
    run(&mut i, "choose rectangle tool\nset the filled to true\nset the pattern to 2\ndrag from 100,100 to 140,140");
    assert_eq!((pixel(&i, 100, 120), pixel(&i, 120, 120)), (Pixel::BLACK, Pixel::WHITE));
    // click at with the bucket fills with the pattern (black).
    run(&mut i, "set the pattern to 1\nchoose bucket tool\nclick at 120,120");
    assert_eq!(pixel(&i, 120, 120), Pixel::BLACK);
    // The text tool: click to place, then type paints text.
    run(&mut i, "choose text tool\nset the textSize to 24\nclick at 200,200\ntype \"Hi\"");
    let w = lock(&i.world);
    let pic = &w.stack.cards[0].picture;
    let inked = (200..240).flat_map(|x| (200..230).map(move |y| (x, y))).filter(|(x, y)| pic.get(*x, *y) == Pixel::BLACK).count();
    assert!(inked > 20, "{inked}");
    drop(w);
    assert_eq!(run(&mut i, "put the pattern & \",\" & the textSize & \",\" & the tool"), "1,24,text tool");
    assert_eq!(run(&mut i, "set the polySides to 6\nset the brush to 9\nput the polySides && the brush"), "6 9");
}

#[test]
fn keep_and_new_element() {
    let mut i = interp_with(stack());
    // What a drag paints is the new element; Select works on it.
    run(&mut i, "choose line tool\ndrag from 10,10 to 50,10");
    let r = lock(&i.world).new_element().unwrap();
    assert_eq!(r.rect, (10, 10, 51, 11));
    // A second drag is a new element of its own.
    run(&mut i, "drag from 10,30 to 20,30");
    let r = lock(&i.world).new_element().unwrap();
    assert_eq!(r.rect.1, 30);
    // Revert would go back to before the first drag: an empty picture.
    let kept = lock(&i.world).kept_picture().cloned().unwrap();
    assert_eq!(kept.get(30, 10), crate::model::Pixel::CLEAR);
    // Keep forgets it, and so does going to another card.
    run(&mut i, "doMenu \"Keep\"");
    assert!(lock(&i.world).kept_picture().is_none());
    run(&mut i, "drag from 10,50 to 20,50");
    assert!(lock(&i.world).kept_picture().is_some());
    run(&mut i, "doMenu \"New Card\"");
    let w = lock(&i.world);
    assert!(w.kept_picture().is_none() && w.new_element().is_none());
    drop(w);
    // With a paint tool, doMenu "Select", "Pickup" and "Revert" go to the
    // Paint menu; with the browse tool, "select" still chooses the tool.
    lock(&i.world).requests.clear();
    i.handle_event(Event::MsgBox("doMenu \"Select\"\ndoMenu \"Pickup\"\ndoMenu \"Revert\"".into()));
    let asked = lock(&i.world).requests.clone();
    assert!(matches!(asked.as_slice(), [UiRequest::SelectPainted, UiRequest::Pickup, UiRequest::Revert]), "{asked:?}");
    run(&mut i, "choose browse tool\ndoMenu \"Select\"");
    assert_eq!(lock(&i.world).tool, super::world::Tool::Select);
}

#[test]
fn paint_options() {
    use crate::model::Pixel;
    let mut i = interp_with(stack());
    let pixel = |i: &Interp, x: i32, y: i32| lock(&i.world).stack.cards[0].picture.get(x, y);
    let clear = |i: &mut Interp| run(i, "choose line tool\ndoMenu \"Clear Picture\"");
    // The Options menu turns its settings on and off, and the properties say so.
    assert_eq!(run(&mut i, "put the grid && the centered && the multiple && the powerKeys"), "false false false false");
    run(&mut i, "doMenu \"Grid\"\ndoMenu \"Draw Centered\"\ndoMenu \"Draw Multiple\"\ndoMenu \"Draw Filled\"");
    assert_eq!(run(&mut i, "put the grid && the centered && the multiple && the filled"), "true true true true");
    run(&mut i, "set the grid to false\nset the centered to false\nset the multiple to false\nset the filled to false");
    // Grid: a line drawn from 13,13 to 61,13 starts and ends on the 8-pixel grid.
    run(&mut i, "set the grid to true\nchoose line tool\ndrag from 13,13 to 61,13");
    assert_eq!((pixel(&i, 16, 16), pixel(&i, 64, 16), pixel(&i, 66, 16)), (Pixel::BLACK, Pixel::BLACK, Pixel::CLEAR));
    run(&mut i, "set the grid to false");
    clear(&mut i);
    // Draw Centered: the rectangle grows both ways from where the drag starts.
    run(&mut i, "set the centered to true\nchoose rectangle tool\ndrag from 100,100 to 120,110");
    assert_eq!((pixel(&i, 80, 100), pixel(&i, 120, 100), pixel(&i, 100, 90)), (Pixel::BLACK, Pixel::BLACK, Pixel::BLACK));
    run(&mut i, "set the centered to false");
    clear(&mut i);
    // Draw Multiple leaves the shape behind along the drag, multiSpace apart.
    run(&mut i, "set the multiple to true\nset the multiSpace to 5\nchoose rectangle tool\ndrag from 10,10 to 40,40");
    let edges = (10..=40).filter(|x| pixel(&i, *x, 25) == Pixel::BLACK).count();
    assert!(edges >= 6, "{edges}");
    run(&mut i, "set the multiple to false");
    // Line Size… asks the UI for its dialog, and Power Keys is kept in Settings.
    lock(&i.world).requests.clear();
    i.handle_event(Event::MsgBox("doMenu \"Edit Pattern…\"".into()));
    assert!(lock(&i.world).requests.drain(..).any(|r| matches!(r, UiRequest::EditPattern)));
    i.handle_event(Event::MsgBox("doMenu \"Line Size…\"\ndoMenu \"Power Keys\"".into()));
    let reqs = std::mem::take(&mut lock(&i.world).requests);
    assert!(
        matches!(&reqs[..], [UiRequest::PaintDialog(super::world::PaintDialog::LineSize), UiRequest::SavePowerKeys(true)]),
        "{reqs:?}"
    );
    assert_eq!(run(&mut i, "put the powerKeys"), "true");
}

#[test]
fn undo_card_and_part_edits() {
    use super::world::EditOp;
    let mut i = interp_with(stack());
    let undo = |i: &Interp| lock(&i.world).undo();
    let cards = |i: &Interp| lock(&i.world).stack.cards.len();
    // New Card, then undo removes it and redo brings it back.
    run(&mut i, "doMenu \"New Card\"");
    assert_eq!(cards(&i), 4);
    assert!(undo(&i));
    assert_eq!(cards(&i), 3);
    assert_eq!(run(&mut i, "put the number of this card"), "1");
    assert!(lock(&i.world).redo());
    assert_eq!(cards(&i), 4);
    // Delete Card, then undo restores it with its contents.
    run(&mut i, "go to card \"Two\"\ndoMenu \"Delete Card\"");
    assert_eq!(run(&mut i, "put there is a card \"Two\""), "false");
    assert!(undo(&i));
    assert_eq!(run(&mut i, "put field \"Name\" of card \"Two\""), "banana");
    // Deleting a part from a script can be undone too.
    run(&mut i, "go to card 1\ndelete button \"Go\"");
    assert_eq!(run(&mut i, "put there is a button \"Go\""), "false");
    assert!(undo(&i));
    assert_eq!(run(&mut i, "put there is a button \"Go\""), "true");
    // Clear Picture is undoable.
    run(&mut i, "choose line tool\ndrag from 0,0 to 20,0\ndoMenu \"Clear Picture\"");
    assert!(lock(&i.world).stack.cards[0].picture.is_blank());
    assert!(undo(&i));
    assert!(!lock(&i.world).stack.cards[0].picture.is_blank());
    // Cut/Copy/Paste/Clear go to the UI, which knows the selection.
    lock(&i.world).requests.clear();
    i.handle_event(Event::MsgBox("doMenu \"Copy Button\"\ndoMenu \"paste\"".into()));
    let reqs = std::mem::take(&mut lock(&i.world).requests);
    assert!(matches!(&reqs[..], [UiRequest::Edit(EditOp::Copy), UiRequest::Edit(EditOp::Paste)]), "{reqs:?}");
}

#[test]
fn user_levels_and_protection() {
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "put the userLevel"), "5");
    assert_eq!(run(&mut i, "set the userLevel to 2\nput the userLevel"), "2");
    // A stack's protection caps the level, until its password is given.
    lock(&i.world).stack.protection.user_level_limit = 3;
    assert_eq!(run(&mut i, "set the userLevel to 5\nput the userLevel"), "3");
    lock(&i.world).unlocked = true;
    assert_eq!(run(&mut i, "set the userLevel to 5\nput the userLevel"), "5");
    // Protection properties.
    assert_eq!(run(&mut i, "set the cantPeek of this stack to true\nput the cantPeek of this stack"), "true");
    assert_eq!(run(&mut i, "put the cantAbort of this stack"), "false");
    // A Can't Modify stack can't be saved over, and is browse-only for the user.
    let dir = std::env::temp_dir().join(format!("randomcard-cm-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    lock(&i.world).stack.path = Some(dir.join("locked.rcstack"));
    run(&mut i, "set the cantModify of this stack to true");
    assert!(run(&mut i, "save this stack").contains("can't be modified"));
    assert_eq!(lock(&i.world).editing_level(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn private_access_stacks_ask_for_their_password() {
    let dir = std::env::temp_dir().join(format!("randomcard-pa-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("secret.rcstack");
    let mut secret = Stack::new("Secret");
    secret.protection.private_access = true;
    secret.protection.password_hash = Some(crate::model::password::stored_hash("Open Sesame"));
    secret.save(&path).unwrap();

    let mut i = interp_with(stack());
    // Canceling leaves the current stack open.
    i.canned_answers.push(DialogReply::Text(None));
    i.handle_event(Event::OpenFile(path.clone()));
    assert_eq!(lock(&i.world).stack.name, "Test");
    // A wrong password asks again; the right one (any case) opens it.
    i.canned_answers.push(DialogReply::Text(Some("nope".into())));
    i.canned_answers.push(DialogReply::Text(Some("open sesame".into())));
    i.handle_event(Event::OpenFile(path));
    assert_eq!(lock(&i.world).stack.name, "Secret");
    assert!(i.canned_answers.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn popup_buttons() {
    let mut i = interp_with(stack());
    run(&mut i, "set the style of button \"Go\" to popup");
    assert_eq!(run(&mut i, "put the style of button 1"), "popup");
    run(&mut i, "put \"Small\" & return & \"Medium\" & return & \"Large\" into button \"Go\"");
    assert_eq!(run(&mut i, "put the selectedText of button 1"), "");
    run(&mut i, "select line 2 of button \"Go\"");
    assert_eq!(run(&mut i, "put the selectedText of button 1"), "Medium");
    assert_eq!(run(&mut i, "put the selectedLine of button 1"), "line 2 of card button 1");
    assert_eq!(run(&mut i, "put the selectedChunk of button 1"), "char 7 to 12 of card button 1");
    run(&mut i, "select last line of button 1");
    assert_eq!(run(&mut i, "put the selectedText of button 1"), "Large");
    assert!(run(&mut i, "select line 5 of button 1").starts_with("ERROR"));
    // Choosing an item leaves any text selection alone.
    assert_eq!(run(&mut i, "put the selectedText"), "");
    run(&mut i, "set the titleWidth of button 1 to 40");
    assert_eq!(run(&mut i, "put titleWidth of button 1"), "40");
    run(&mut i, "set the style of button 1 to standard");
    assert_eq!(run(&mut i, "put the style of button 1"), "standard");
}

#[test]
fn selection_and_select() {
    let mut i = interp_with(stack());
    run(&mut i, "put \"one two three\" & return & \"four five\" into field \"Name\"");
    assert_eq!(run(&mut i, "put the selectedText"), "");
    run(&mut i, "select word 2 of line 1 of field \"Name\"");
    assert_eq!(lock(&i.world).focus, Some((Layer::Background, 10)));
    assert_eq!(run(&mut i, "put the selectedText"), "two");
    assert_eq!(run(&mut i, "put the selection"), "two");
    assert_eq!(run(&mut i, "put the selectedChunk"), "char 5 to 7 of bkgnd field 1");
    assert_eq!(run(&mut i, "put the selectedLine"), "line 1 of bkgnd field 1");
    assert_eq!(run(&mut i, "put the selectedField"), "bkgnd field 1");
    run(&mut i, "put \"2\" into the selection");
    assert_eq!(run(&mut i, "put line 1 of field \"Name\""), "one 2 three");
    // The insertion point follows what was put.
    assert_eq!(run(&mut i, "put the selectedChunk"), "char 6 to 5 of bkgnd field 1");
    run(&mut i, "put \"!\" into the selection");
    assert_eq!(run(&mut i, "put line 1 of field \"Name\""), "one 2! three");
    run(&mut i, "select text of field \"Name\"");
    assert_eq!(run(&mut i, "put the selectedLine"), "line 1 to 2 of bkgnd field 1");
    run(&mut i, "select after line 2 of field \"Name\"");
    assert_eq!(run(&mut i, "put the selectedChunk"), "char 23 to 22 of bkgnd field 1");
    run(&mut i, "select before word 1 of field \"Name\"");
    assert_eq!(run(&mut i, "put the selectedChunk"), "char 1 to 0 of bkgnd field 1");
    run(&mut i, "select empty");
    assert_eq!(run(&mut i, "put the selectedChunk"), "");
    assert_eq!(lock(&i.world).focus, None);
    // The selection belongs to its card.
    run(&mut i, "select word 1 of field \"Name\"");
    run(&mut i, "go next card");
    assert_eq!(run(&mut i, "put the selectedText"), "");
    // Selecting a part picks its tool.
    run(&mut i, "go card 1\nselect button \"Go\"");
    assert_eq!(run(&mut i, "put the tool"), "button tool");
    assert!(run(&mut i, "select text of button \"Go\"").starts_with("ERROR"));
}

#[test]
fn locked_field_selection_and_clicks() {
    let mut i = interp_with(stack());
    run(&mut i, "set the lockText of field 1 to true\nput \"alpha\" & return & \"beta gamma\" into field 1");
    run(&mut i, "select line 2 of field 1");
    // A locked field shows its selection without opening for typing.
    assert_eq!(lock(&i.world).focus, None);
    assert_eq!(run(&mut i, "put the selectedText"), "beta gamma");
    // The UI records clicks in locked fields.
    {
        let mut w = lock(&i.world);
        let card = w.card_id;
        w.click = Some(super::world::FieldRange { card, layer: Layer::Background, field: 10, start: 11, end: 16 });
    }
    assert_eq!(run(&mut i, "put the clickText"), "gamma");
    assert_eq!(run(&mut i, "put the clickChunk"), "char 12 to 16 of bkgnd field 1");
    assert_eq!(run(&mut i, "put the clickLine"), "line 2 of bkgnd field 1");
    run(&mut i, "set the multipleLines of field 1 to true");
    assert_eq!(run(&mut i, "put the multipleLines of field 1"), "true");
    assert_eq!(super::world::word_at("one two", 5), (4, 7));
    assert_eq!(super::world::word_at("one two", 3), (3, 3));
    assert_eq!(super::world::line_at("a\nbc\nd", 3), (2, 4));
}

#[test]
fn styled_text() {
    let mut i = interp_with(stack());
    let f = "field \"Name\"";
    // The line height follows the size until it's set, as in HyperCard.
    run(&mut i, &format!("set the textSize of {f} to 12"));
    assert_eq!(run(&mut i, &format!("put the textHeight of {f}")), "16");
    run(&mut i, &format!("set the textHeight of {f} to 20"));
    run(&mut i, &format!("set the fixedLineHeight of {f} to true"));
    assert_eq!(run(&mut i, &format!("put the textHeight of {f} && the fixedLineHeight of {f}")), "20 true");
    run(&mut i, &format!("set the textSize of {f} to 9"));
    assert_eq!(run(&mut i, &format!("put the textHeight of {f}")), "12");
    run(&mut i, &format!("put \"plain bold italic\" into {f}"));
    run(&mut i, &format!("set the textStyle of word 2 of {f} to bold"));
    assert_eq!(run(&mut i, &format!("put the textStyle of word 2 of {f}")), "bold");
    assert_eq!(run(&mut i, &format!("put the textStyle of word 1 of {f}")), "plain");
    assert_eq!(run(&mut i, &format!("put the textStyle of line 1 of {f}")), "mixed");
    run(&mut i, &format!("set the textFont of char 1 to 5 of {f} to \"Times\""));
    assert_eq!(run(&mut i, &format!("put the textFont of word 1 of {f}")), "Times");
    assert_eq!(run(&mut i, &format!("put the textFont of word 3 of {f}")), run(&mut i, &format!("put the textFont of {f}")));
    run(&mut i, &format!("set the textSize of word 3 of {f} to 18"));
    assert_eq!(run(&mut i, &format!("put the textSize of word 3 of {f}")), "18");
    // Styles stay with their text as text around them changes.
    run(&mut i, &format!("put \"Very \" before {f}"));
    assert_eq!(run(&mut i, &format!("put word 3 of {f} && the textStyle of word 3 of {f}")), "bold bold");
    assert_eq!(run(&mut i, &format!("put the textFont of word 2 of {f}")), "Times");
    // The Style menu toggles styles of the selection.
    run(&mut i, &format!("select word 4 of {f}\ndoMenu \"Italic\""));
    assert_eq!(run(&mut i, &format!("put the textStyle of word 4 of {f}")), "italic");
    assert_eq!(run(&mut i, "put the textStyle of the selection"), "italic");
    run(&mut i, "doMenu \"Italic\"");
    assert_eq!(run(&mut i, &format!("put the textStyle of word 4 of {f}")), "plain");
    run(&mut i, &format!("select word 3 of {f}\ndoMenu \"Bold\"\ndoMenu \"Underline\""));
    assert_eq!(run(&mut i, &format!("put the textStyle of word 3 of {f}")), "underline");
    run(&mut i, "doMenu \"24 Point\"\ndoMenu \"Courier\"");
    assert_eq!(run(&mut i, &format!("put the textSize of word 3 of {f} && the textFont of word 3 of {f}")), "24 Courier");
    run(&mut i, "set the textStyle of the selection to \"Bold, Italic\"");
    assert_eq!(run(&mut i, "put the textStyle of the selection"), "bold,italic");
    // Setting the field's own style restyles all its text.
    run(&mut i, &format!("set the textStyle of {f} to italic"));
    assert_eq!(run(&mut i, &format!("put the textStyle of line 1 of {f}")), "italic");
    assert_eq!(run(&mut i, &format!("put the textFont of word 2 of {f}")), "Times");
    // Background field text is per card, and so are its styles.
    run(&mut i, &format!("set the textStyle of {f} to plain\nset the textStyle of word 2 of {f} to bold\ngo next card"));
    assert_eq!(run(&mut i, &format!("put the textStyle of {f}")), "plain");
    assert_eq!(run(&mut i, &format!("put the textStyle of line 1 of {f}")), "plain");
}

#[test]
fn scripted_menus() {
    let mut i = interp_with(stack());
    assert!(run(&mut i, "put the menus").starts_with("Apple\nFile\nEdit\nGo"));
    run(&mut i, "create menu \"Game\"");
    assert!(run(&mut i, "put there is a menu \"Game\"") == "true");
    assert!(run(&mut i, "put there is no menu \"Toys\"") == "true");
    assert!(run(&mut i, "create menu \"Game\"").starts_with("ERROR"));
    run(&mut i, "put \"New Game,-,High Scores\" into menu \"Game\" with menuMessages \"newGame,,showScores\"");
    assert_eq!(run(&mut i, "put menu \"Game\""), "New Game\n-\nHigh Scores");
    assert_eq!(run(&mut i, "put the number of menuItems of menu \"Game\""), "3");
    assert_eq!(run(&mut i, "put last menuItem of menu \"Game\""), "High Scores");
    assert_eq!(run(&mut i, "put the menuMessage of menuItem 1 of menu \"Game\""), "newGame");
    run(&mut i, "put \"Pause\" after menuItem \"New Game\" of menu \"Game\"");
    assert_eq!(run(&mut i, "put menuItem 2 of menu \"Game\""), "Pause");
    // Choosing an item sends its menuMessage instead of doMenu.
    let card = ObjRef::Card(lock(&i.world).card_id);
    set_script(&i, card, "on newGame\n  put \"started\" into field \"Name\"\nend newGame\non doMenu what\n  put what into field \"Name\"\nend doMenu");
    i.handle_event(Event::DoMenu("New Game".into()));
    assert_eq!(run(&mut i, "put field \"Name\""), "started");
    i.handle_event(Event::DoMenu("Pause".into()));
    assert_eq!(run(&mut i, "put field \"Name\""), "Pause");
    set_script(&i, card, "on newGame\n  put \"again\" into field \"Name\"\nend newGame");
    // Script doMenu runs an item's menuMessage too; items of its own do nothing.
    assert_eq!(run(&mut i, "doMenu \"New Game\"\nput field \"Name\""), "again");
    assert_eq!(run(&mut i, "doMenu \"Pause\"\nput the result"), "");
    // Properties.
    run(&mut i, "disable menuItem \"Delete Card\" of menu \"Edit\"\nset the checkMark of menuItem 2 of menu \"Game\" to true");
    assert_eq!(run(&mut i, "put the enabled of menuItem \"Delete Card\" of menu \"Edit\""), "false");
    assert_eq!(run(&mut i, "put the checkMark of menuItem \"Pause\" of menu \"Game\""), "true");
    run(&mut i, "set the commandChar of menuItem 1 of menu \"Game\" to \"G\"\ndisable menu \"Game\"");
    assert_eq!(run(&mut i, "put the commandChar of menuItem 1 of menu \"Game\" && the enabled of menu \"Game\""), "G false");
    run(&mut i, "set the name of menuItem 2 of menu \"Game\" to \"Resume\"");
    assert_eq!(run(&mut i, "put menuItem 2 of menu \"Game\""), "Resume");
    run(&mut i, "delete menuItem \"Resume\" of menu \"Game\"");
    assert_eq!(run(&mut i, "put the number of menuItems of menu \"Game\""), "3");
    let n = run(&mut i, "put the number of menus");
    run(&mut i, "delete menu \"Game\"");
    assert_eq!(run(&mut i, "put the number of menus + 1"), n);
    assert!(run(&mut i, "put menu \"Game\"").starts_with("ERROR"));
    // Reset brings back the standard menus.
    run(&mut i, "delete menu \"Go\"\nreset menuBar");
    assert_eq!(run(&mut i, "put there is a menu \"Go\" && the enabled of menuItem \"Delete Card\" of menu \"Edit\""), "true true");
}

#[test]
fn home_has_hypercards_own_cards() {
    let dir = std::env::temp_dir().join(format!("randomcard-homecards-{}", std::process::id()));
    let far = dir.join("far away");
    std::fs::create_dir_all(&far).unwrap();
    // A Home from before the cards: they're added when it loads.
    let home_path = dir.join("Home.rcstack");
    let mut old = Stack::home();
    old.cards.truncate(1);
    old.save(&home_path).unwrap();
    Stack::new("Faraway").save(&far.join("Faraway.rcstack")).unwrap();
    let mut main = stack();
    main.path = Some(dir.join("Main.rcstack"));
    // Stack Dump's own getHomeInfo, from HyperCard 1's Home script.
    main.script = "on getHomeInfo\n  global stacks,applications,documents,userName\n  set lockScreen to true\n  set lockMessages to true\n  push this card\n  go to card \"User Preferences\" of stack \"Home\"\n  put card field \"User Name\" into userName\n  set userLevel to card field \"User Level\"\n  set powerKeys to the hilite of button \"Power Keys\"\n  set textArrows to the hilite of button \"Text Arrows\"\n  set blindTyping to the hilite of button \"Blind Typing\"\n  put field \"paths\" of card \"stacks\" into stacks\n  put field \"paths\" of card \"applications\" into applications\n  put field \"paths\" of card \"documents\" into documents\n  pop card\n  set lockScreen to false\n  set lockMessages to false\nend getHomeInfo".into();
    main.save(&dir.join("Main.rcstack")).unwrap();
    let mut i = interp_with(main);
    {
        let mut w = lock(&i.world);
        w.home_path = Some(home_path.clone());
        w.user_level = 3;
        w.load_home();
    }
    assert!(Stack::load(&home_path).unwrap().cards.iter().any(|c| c.name == "User Preferences"));
    // The level shows on the card, as HyperCard kept it there.
    let home_part = |i: &Interp, name: &str| -> (String, bool) {
        let mut w = lock(&i.world);
        let h = w.home_mut().unwrap();
        let c = h.cards.iter().find(|c| c.name == "User Preferences").unwrap();
        let p = c.parts.iter().find(|p| p.name == name).unwrap();
        (p.contents.clone(), p.hilite)
    };
    assert_eq!(home_part(&i, "User Level").0, "3");
    run(&mut i, "set userLevel to 4");
    assert_eq!(home_part(&i, "User Level").0, "4");
    assert!(home_part(&i, "Authoring").1);
    assert!(!home_part(&i, "Painting").1);
    // Set from a stack, the level isn't kept in Settings.
    assert!(!lock(&i.world).requests.iter().any(|r| matches!(r, UiRequest::SaveUserLevel(_))));
    let current_stack = |i: &Interp| lock(&i.world).stack.name.clone();
    let main_name = current_stack(&i);
    assert_eq!(run(&mut i, "getHomeInfo\nput the userLevel"), "4");
    assert_eq!(current_stack(&i), main_name);
    // The Stacks card's folders are searched for a stack.
    let folder = super::paths::shown(&far);
    run(&mut i, &format!("go to card \"Stacks\" of stack \"Home\"\nput \"{folder}\" into field \"Paths\"\ngo to stack \"Faraway\""));
    assert_eq!(current_stack(&i), "Faraway");
    // On Home's card, the level is the user's choice, kept in Settings.
    run(&mut i, "go to card \"User Preferences\" of stack \"Home\"");
    lock(&i.world).requests.clear();
    i.handle_event(Event::MsgBox("send \"mouseUp\" to button \"Typing\"".into()));
    assert_eq!(lock(&i.world).user_level, 2);
    assert!(lock(&i.world).requests.iter().any(|r| matches!(r, UiRequest::SaveUserLevel(2))));
}

#[test]
fn stack_windows() {
    let dir = std::env::temp_dir().join(format!("randomcard-windows-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let home_path = dir.join("Home.rcstack");
    Stack::home().save(&home_path).unwrap();
    // Each stack logs the window messages it gets.
    let logger = |name: &str| {
        ["openStack", "closeStack", "suspendStack", "resumeStack"]
            .map(|m| format!("on {m}\n  global log\n  put \"{name} {m},\" after log\nend {m}"))
            .join("\n")
    };
    let mut lib = stack();
    lib.name = "Lib".into();
    lib.script = logger("Lib");
    let lib_path = dir.join("Lib.rcstack");
    lib.path = Some(lib_path.clone());
    lib.save(&lib_path).unwrap();
    let mut main = stack();
    main.name = "Main".into();
    main.script = logger("Main");
    let main_path = dir.join("Main.rcstack");
    main.path = Some(main_path.clone());
    main.save(&main_path).unwrap();

    let mut i = interp_with(main);
    {
        let mut w = lock(&i.world);
        w.home_path = Some(home_path.clone());
        w.load_home();
    }
    let log = |i: &mut Interp| run(i, "global log\nput log\nput empty into log");
    let here = |i: &mut Interp| run(i, "put the short name of this stack && the short name of this card");

    // A stack in a new window comes in front; the one behind is suspended.
    run(&mut i, "go to stack \"Lib\" in a new window");
    assert_eq!(here(&mut i), "Lib One");
    assert_eq!(log(&mut i), "Main suspendStack,Lib openStack,");
    assert_eq!(run(&mut i, "put the number of lines of the stacks"), "2");
    assert!(run(&mut i, "put line 1 of the stacks").ends_with("Lib.rcstack"));
    assert!(run(&mut i, "put line 2 of the stacks").ends_with("Main.rcstack"));
    assert!(run(&mut i, "put the windows").starts_with("Lib\nMain\n"));
    // Each window keeps its own card.
    run(&mut i, "go to card \"Two\"");
    run(&mut i, "go to stack \"Main\"");
    assert_eq!(here(&mut i), "Main One");
    assert_eq!(log(&mut i), "Lib suspendStack,Main resumeStack,");
    assert_eq!(run(&mut i, "put the number of lines of the stacks"), "2");
    // A stack behind is still there to ask about, and Home still answers.
    assert_eq!(run(&mut i, "put the short name of stack \"Lib\""), "Lib");
    assert_eq!(run(&mut i, "put homeVersion()"), "Home 1.0");
    // Its cards and parts can be read and changed from here, without going
    // there, and this stack stays current.
    assert_eq!(run(&mut i, "put the short name of card 2 of stack \"Lib\""), "Two");
    assert_eq!(run(&mut i, "put the number of cards of stack \"Lib\""), "3");
    assert_eq!(run(&mut i, "put the number of buttons of card 1 of stack \"Lib\""), "1");
    run(&mut i, "put \"from Main\" into field \"Name\" of card \"Two\" of stack \"Lib\"");
    assert_eq!(run(&mut i, "put field \"Name\" of card \"Two\" of stack \"Lib\""), "from Main");
    assert_eq!(run(&mut i, "put word 2 of field \"Name\" of card \"Two\" of stack \"Lib\""), "Main");
    assert_eq!(run(&mut i, "put there is a card \"Three\" of stack \"Lib\""), "true");
    run(&mut i, "set the name of card 3 of stack \"Lib\" to \"Third\"");
    assert_eq!(run(&mut i, "put the short name of card 3 of stack \"Lib\""), "Third");
    assert_eq!(here(&mut i), "Main One");
    assert_eq!(run(&mut i, "put field \"Name\""), "Apple");
    // Read there, written here.
    run(&mut i, "put field \"Name\" of card \"Two\" of stack \"Lib\" into field \"Name\"");
    assert_eq!(run(&mut i, "put field \"Name\""), "from Main");
    assert!(run(&mut i, "put the short name of card 1 of stack \"Nowhere\"").contains("No such stack"));
    // Opening it again in a new window brings its window forward.
    run(&mut i, "go to stack \"Lib\" in a new window");
    assert_eq!(here(&mut i), "Lib Two");
    assert_eq!(run(&mut i, "put the number of lines of the stacks"), "2");
    log(&mut i);

    // Closing a window saves its stack and brings the next one forward.
    run(&mut i, "put \"changed\" into field \"Name\"\nclose window \"Lib\"");
    assert_eq!(here(&mut i), "Main One");
    assert_eq!(log(&mut i), "Lib closeStack,Main resumeStack,");
    assert_eq!(run(&mut i, "put the number of lines of the stacks"), "1");
    let saved = Stack::load(&lib_path).unwrap();
    assert_eq!(saved.field_text(20, Layer::Background, 10), "changed");
    assert_eq!(saved.cards[2].name, "Third");
    // Closing a window behind leaves the current stack in front, unsuspended.
    run(&mut i, "go to stack \"Lib\" in a new window\ngo to stack \"Main\"");
    log(&mut i);
    run(&mut i, "close window \"Lib\"");
    assert_eq!(here(&mut i), "Main One");
    assert_eq!(log(&mut i), "Lib closeStack,");
    // The first window stays.
    assert_eq!(run(&mut i, "close window \"Main\"\nput the result"), "The first card window stays open.");
    assert_eq!(here(&mut i), "Main One");
    // A stack converted from HyperCard, named by its file, is the one open,
    // not another copy (OASIS Stack goes back to itself this way).
    {
        let mut w = lock(&i.world);
        w.stack.path = None;
        w.stack.imported_from = Some(main_path.clone());
    }
    run(&mut i, "go to stack \"Lib\" in a new window");
    run(&mut i, &format!("go to stack \"{}\" in a new window", main_path.display()));
    assert_eq!(here(&mut i), "Main One");
    assert_eq!(run(&mut i, "put the number of lines of the stacks"), "2");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn stacks_home_and_libraries() {
    let dir = std::env::temp_dir().join(format!("randomcard-stacks-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let home_path = dir.join("Home.rcstack");
    Stack::home().save(&home_path).unwrap();
    let lib_path = dir.join("Lib.rcstack");
    let mut lib = Stack::new("Lib");
    lib.script = "function libFn\n  return \"from lib\"\nend libFn\non libraryStack\n  global inUse\n  put \"yes\" into inUse\nend libraryStack\nfunction libCallsBack\n  return mainFn()\nend libCallsBack".into();
    lib.save(&lib_path).unwrap();
    let main_path = dir.join("Main.rcstack");
    let mut main = stack();
    main.name = "Main".into();
    main.script = "function mainFn\n  return \"from main\"\nend mainFn".into();
    main.path = Some(main_path.clone());
    main.save(&main_path).unwrap();

    let mut i = interp_with(main);
    {
        let mut w = lock(&i.world);
        w.home_path = Some(home_path.clone());
        w.load_home();
    }
    let this_stack = |i: &mut Interp| run(i, "put the short name of this stack");
    // Home's script is in every stack's message path.
    assert_eq!(run(&mut i, "put homeVersion()"), "Home 1.0");
    // A bare Home names the Home stack (Apple's stacks `send "startUp" to Home`).
    assert!(!run(&mut i, "send \"beep\" to Home").starts_with("ERROR"));
    assert_eq!(run(&mut i, "put the short name of this stack"), "Main");
    // A library's is too, once it's in use.
    assert!(run(&mut i, "put libFn()").starts_with("ERROR"));
    run(&mut i, "start using stack \"Lib\"");
    assert_eq!(run(&mut i, "put libFn()"), "from lib");
    // The library's handlers reach the current stack too (the dynamic path),
    // as Macintosh Reference's call to stackNameWithPath did.
    assert_eq!(run(&mut i, "put libCallsBack()"), "from main");
    assert_eq!(run(&mut i, "global inUse\nput inUse"), "yes");
    assert_eq!(run(&mut i, "global inUse\nput empty into inUse\nsend \"libraryStack\" to stack \"Lib\"\nput inUse"), "yes");
    assert!(run(&mut i, "put the stacksInUse").ends_with("Lib.rcstack"));
    assert_eq!(run(&mut i, "put the short name of stack \"Lib\""), "Lib");
    // Changing another stack's script saves it.
    run(&mut i, "set the script of stack \"Lib\" to \"function libFn\" & return & \"return 2\" & return & \"end libFn\"");
    assert_eq!(run(&mut i, "put libFn()"), "2");
    assert!(Stack::load(&lib_path).unwrap().script.contains("return 2"));
    run(&mut i, "stop using stack \"Lib\"");
    assert!(run(&mut i, "put libFn()").starts_with("ERROR"));
    // Cards of other stacks can be reached without going there.
    assert_eq!(run(&mut i, "put the number of cards of stack \"Lib\""), "1");
    assert!(!run(&mut i, "put the name of card 1 of stack \"Lib\"").starts_with("ERROR"));

    // Going to another stack saves the one left.
    run(&mut i, "put \"changed\" into field \"Name\"\ngo to stack \"Lib\"");
    assert_eq!(this_stack(&mut i), "Lib");
    let saved = Stack::load(&main_path).unwrap();
    assert_eq!(saved.field_text(saved.cards[0].id, Layer::Background, 10), "changed");
    run(&mut i, "go home");
    assert_eq!(this_stack(&mut i), "Home");
    // Home's own script isn't run twice from Home.
    assert_eq!(run(&mut i, "put homeVersion()"), "Home 1.0");
    run(&mut i, "go to card \"Two\" of stack \"Main\"");
    assert_eq!(run(&mut i, "put the short name of this stack && the short name of this card"), "Main Two");
    // A card pushed in another stack pops back to that stack, as DateBook's
    // Return button does after a visit to the Address stack.
    run(&mut i, "push card\ngo to stack \"Lib\"\npush card\ngo to stack \"Main\"");
    run(&mut i, "pop card");
    assert_eq!(this_stack(&mut i), "Lib");
    run(&mut i, "pop card");
    assert_eq!(run(&mut i, "put the short name of this stack && the short name of this card"), "Main Two");
    // The built-in Tour has no file; Home's button goes there.
    run(&mut i, "go home");
    assert_eq!(click(&mut i, "Home", "Take the Tour"), None);
    assert_eq!(this_stack(&mut i), "Tour");
    run(&mut i, "go home");
    assert_eq!(this_stack(&mut i), "Home");
    assert_eq!(run(&mut i, "go to stack \"Nowhere\"\nput the result"), "No such stack.");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn debugger_steps_and_checkpoints() {
    use super::world::DebugCmd;
    let mut i = interp_with(stack());
    let card_id = lock(&i.world).card_id;
    let (card, btn) = (ObjRef::Card(card_id), ObjRef::Part { layer: Layer::Card, id: 11, card: card_id });
    let script = "on mouseUp\n  global g\n  put 1 into x\n  debug checkpoint\n  put x into g\n  helper\n  put g + 1 into g\nend mouseUp\non helper\n  put 9 into y\nend helper";
    set_script(&i, btn, script);
    let click = |i: &mut Interp| i.handle_event(Event::Send { target: btn, message: "mouseUp".into(), args: vec![] });

    // `debug checkpoint` stops at the next line. Step runs to the next line of
    // the same handler, Step Into goes into `helper`, and Step from its last
    // line comes back out to the line after the call.
    i.canned_debug = vec![DebugCmd::Step, DebugCmd::StepInto, DebugCmd::Step, DebugCmd::Go];
    click(&mut i);
    assert_eq!(i.paused_lines, vec![5, 6, 10, 7]);
    assert!(lock(&i.world).debug.is_none(), "the debugger lets go when the handler ends");

    // A checkpoint stops there; Go runs on. The variable watcher can change
    // a local while stopped.
    i.paused_lines.clear();
    set_script(&i, btn, &script.replace("debug checkpoint", "get 0"));
    lock(&i.world).checkpoints.push((btn, 5));
    i.canned_debug = vec![DebugCmd::SetVar { name: "x".into(), value: "41".into() }, DebugCmd::Go];
    click(&mut i);
    assert_eq!(i.paused_lines, vec![5]);
    assert_eq!(run(&mut i, "global g\nput g"), "42");

    // The message watcher logs messages in the order sent, and whether a
    // handler took them.
    lock(&i.world).checkpoints.clear();
    lock(&i.world).watch_messages = true;
    click(&mut i);
    i.handle_event(Event::Send { target: card, message: "nobodyHome".into(), args: vec![] });
    let log: Vec<(String, usize, bool)> =
        lock(&i.world).message_log.iter().map(|m| (m.name.clone(), m.depth, m.handled)).collect();
    assert_eq!(
        log,
        vec![("mouseUp".into(), 0, true), ("helper".into(), 1, true), ("nobodyHome".into(), 0, false)]
    );
    assert_eq!(run(&mut i, "set the traceDelay to 5\nput the traceDelay"), "5");
    i.handle_event(Event::MsgBox("show message watcher\nhide variable watcher".into()));
    let reqs: Vec<UiRequest> = std::mem::take(&mut lock(&i.world).requests);
    assert!(reqs.iter().any(|r| matches!(r, UiRequest::MessageWatcher(true))));
    assert!(reqs.iter().any(|r| matches!(r, UiRequest::VariableWatcher(false))));
}

#[test]
fn printing() {
    let mut i = interp_with(stack());
    let pages = |i: &Interp| {
        let pdf = std::fs::read(i.printed.last().unwrap()).unwrap();
        let text = String::from_utf8_lossy(&pdf).to_string();
        let at = text.find("/Count ").unwrap() + 7;
        text[at..].split_whitespace().next().unwrap().parse::<usize>().unwrap()
    };
    run(&mut i, "print card");
    assert_eq!((i.printed.len(), pages(&i)), (1, 1));
    // Three cards, two to a page.
    run(&mut i, "print all cards");
    assert_eq!(pages(&i), 2);
    // Cards printed between open and close printing make one document.
    run(&mut i, "open printing with dialog\nprint card 1\nprint card \"Three\"\nprint next card\nclose printing");
    assert_eq!((i.printed.len(), pages(&i)), (3, 2));
    // Printing left open ends with the handler.
    run(&mut i, "open printing\nprint 2 cards");
    assert_eq!((i.printed.len(), pages(&i)), (4, 1));
    run(&mut i, "print \"Hello\" & return & \"world\"");
    assert!(i.printed.last().unwrap().to_string_lossy().ends_with("text.pdf"));
    run(&mut i, "print card from 0,0 to 100,50");
    assert_eq!(i.printed.len(), 6);
    run(&mut i, "mark card 2\nprint marked cards");
    assert_eq!((i.printed.len(), pages(&i)), (7, 1));
    // The File menu prints too.
    i.handle_event(Event::DoMenu("Print Stack…".into()));
    assert_eq!((i.printed.len(), pages(&i)), (8, 2));
}

#[test]
fn find_and_recent_menus() {
    let mut i = interp_with(stack());
    // Find… readies the message box with the cursor between the quotes.
    i.handle_event(Event::DoMenu("Find…".into()));
    {
        let mut w = lock(&i.world);
        assert_eq!(w.msg, "find \"\"");
        assert!(w.msg_visible);
        assert!(w.requests.drain(..).any(|r| matches!(r, UiRequest::FocusMessage(6))));
    }
    // Running it again finds the next match.
    run(&mut i, "find string \"e\"");
    let first = run(&mut i, "put the foundChunk");
    run(&mut i, "find string \"e\"");
    assert_ne!(run(&mut i, "put the foundChunk"), first);

    // Recent keeps each card once, most recent last.
    run(&mut i, "go to card 1\ngo to card 3\ngo to card 2\ngo to card 3");
    let names: Vec<String> = {
        let w = lock(&i.world);
        w.recent.iter().map(|id| w.stack.card(*id).unwrap().name.clone()).collect()
    };
    assert_eq!(names, vec!["One", "Two", "Three"]);
    i.handle_event(Event::DoMenu("Recent".into()));
    assert!(lock(&i.world).requests.iter().any(|r| matches!(r, UiRequest::Recent)));
}

#[test]
fn deep_recursion_fits_the_script_stack() {
    // The script thread's stack must hold the deepest recursion allowed,
    // with expressions nested at each level, even in a debug build.
    let t = std::thread::Builder::new()
        .stack_size(super::SCRIPT_STACK)
        .spawn(|| {
            let mut i = interp_with(stack());
            set_script(
                &i,
                ObjRef::Stack,
                "on dive n\n  put ((n + 1) * 2 - 2) / 2 into m\n  if m < 100000 then dive m\nend dive\nfunction f n\n  return f(n + 1)\nend f",
            );
            let a = run(&mut i, "dive 0");
            let b = run(&mut i, "put f(0)");
            (a, b)
        })
        .unwrap();
    let (a, b) = t.join().expect("the script thread overflowed its stack");
    assert!(a.contains("Too much recursion"), "{a}");
    assert!(b.contains("Too much recursion"), "{b}");
}

#[test]
fn plurals_as_variable_names() {
    // Apple's Human Interface Notes stack loops with a variable named "fields".
    let mut i = interp_with(stack());
    run(&mut i, "repeat with fields = 1 to the number of bg fields\n  put \"x\" into bg field fields of this card\nend repeat");
    assert_eq!(run(&mut i, "put field \"Name\""), "x");
    assert_eq!(run(&mut i, "put 1 into buttons\nput the short name of cd btn buttons"), "Go");
    // The plurals still count parts.
    assert_eq!(run(&mut i, "put the number of cd buttons"), "1");
}

#[test]
fn a_broken_handler_fails_only_when_it_runs() {
    let mut i = interp_with(stack());
    let card = ObjRef::Card(lock(&i.world).card_id);
    set_script(&i, card, "on good\n  put \"fine\"\nend good\non bad\n  put field\nend bad\nfunction odd.name\n  return 1\nend odd.name\non later\n  put \"also fine\"\nend later");
    assert_eq!(run(&mut i, "good"), "fine");
    assert_eq!(run(&mut i, "later"), "also fine");
    let e = run(&mut i, "bad");
    assert!(e.starts_with("ERROR: Script error in card") && e.contains("line 5"), "{e}");
}

#[test]
fn repeating_messages_report_an_error_once() {
    let mut i = interp_with(stack());
    let card = ObjRef::Card(lock(&i.world).card_id);
    set_script(&i, card, "on mouseWithin\n  put field\nend mouseWithin\non mouseUp\n  put field\nend mouseUp");
    let errors = |i: &mut Interp, message: &str| {
        i.handle_event(Event::Send { target: card, message: message.into(), args: vec![] });
        let mut w = lock(&i.world);
        w.requests.drain(..).filter(|r| matches!(r, UiRequest::ScriptError { .. })).count()
    };
    // The card's handlers don't parse. The first mouseWithin reports it; the
    // ones after don't, so the error doesn't reopen while it's being fixed.
    assert_eq!(errors(&mut i, "mouseWithin"), 1);
    assert_eq!(errors(&mut i, "mouseWithin"), 0);
    assert_eq!(errors(&mut i, "idle"), 0);
    // A click is the user's doing, so it's reported again.
    assert_eq!(errors(&mut i, "mouseUp"), 1);
    assert_eq!(errors(&mut i, "mouseWithin"), 1);
}

/// Exercises a real stack: opens it, then goes to every card and clicks
/// every visible button (background buttons once each), printing each
/// script error once. A click that
/// waits for typing that never comes is stopped after two seconds. Run it on any stack with
/// RANDOMCARD_EXERCISE=path/to/stack cargo test exercise_stack -- --ignored --nocapture
#[test]
#[ignore]
fn exercise_stack() {
    // Scripts get the stack the app gives them, not a test thread's.
    std::thread::Builder::new().stack_size(super::SCRIPT_STACK).spawn(exercise).unwrap().join().unwrap();
}

fn exercise() {
    let Ok(path) = std::env::var("RANDOMCARD_EXERCISE") else { return };
    let r = exercise_one(std::path::Path::new(&path), true);
    println!("{} clicks, {} distinct errors, {} in all, {} stopped", r.clicks, r.errors.len(), r.error_count, r.stopped);
}

#[derive(Default)]
struct Exercised {
    clicks: usize,
    /// Each distinct error, first seen first, with what was clicked.
    errors: Vec<String>,
    error_count: usize,
    stopped: usize,
    /// How many fix-step changes our copy got.
    fixes: usize,
}

/// Give a test interpreter RandomCard's Home stack, as the app has, so `go
/// home` leaves the stack instead of falling back to card 1. A fresh one
/// each time, one file to a thread: the click test runs stacks side by
/// side, and some write to Home (the Developer Info Assistant adds its path
/// to Home's list of stacks).
fn give_home(i: &Interp) {
    let thread = format!("{:?}", std::thread::current().id()).chars().filter(char::is_ascii_digit).collect::<String>();
    let home = std::env::temp_dir().join(format!("randomcard-harness-home-{}-{thread}.rcstack", std::process::id()));
    let mut h = Stack::home();
    h.path = Some(home.clone());
    h.save(&home).unwrap();
    lock(&i.world).home_path = Some(home);
}

/// Open a stack, go to every card and click every button that has a script.
fn exercise_one(path: &std::path::Path, print: bool) -> Exercised {
    let mut out = Exercised::default();
    // The discs' folders here are named with quotation marks ("The Silence
    // Of The ROMs"), which no Mac path had and which break scripts taking
    // `word 2 of the long name`: open the stack through a link without them.
    // The link stands for the outermost folder with quotes (the disc's), so
    // folders beside the stack's, where companions are, come along.
    let linked;
    let quoted = path.ancestors().filter(|a| a.file_name().is_some_and(|n| n.to_string_lossy().contains('"'))).last();
    let path = if let Some(dir) = quoted
        && let Ok(rest) = path.strip_prefix(dir)
    {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        dir.hash(&mut h);
        let link = std::env::temp_dir().join(format!("randomcard-discs-{}", std::process::id())).join(format!("{:x}", h.finish()));
        let _ = std::fs::create_dir_all(link.parent().unwrap());
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(std::fs::canonicalize(dir).unwrap_or(dir.to_path_buf()), &link);
        linked = link.join(rest);
        if linked.exists() && !rest.to_string_lossy().contains('"') { linked.as_path() } else { path }
    } else {
        path
    };
    // Our copy gets its listed fix steps (FIX_STEPS.tsv) as it's converted.
    let stack = match crate::import::load_any_fixed(path, None) {
        Ok((s, fixes)) => {
            out.fixes = fixes;
            s
        }
        Err(e) => {
            out.errors.push(format!("doesn't import: {e}"));
            return out;
        }
    };
    let name = stack.name.clone();
    let original = stack.clone();
    let mut i = interp_with(stack);
    // No one is watching: effects and waits don't pause.
    i.skip_pauses = true;
    give_home(&i);
    let abort = i.abort.clone();
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    // A click waiting on something that never comes (a loop until the user
    // types a number) is stopped after two seconds. Answers whether it was.
    // The watchdog wakes as soon as the click is done, rather than napping
    // through the rest of a poll.
    let with_watchdog = |abort: &Arc<AtomicBool>, f: &mut dyn FnMut()| {
        let (done, wait) = std::sync::mpsc::channel::<()>();
        let a = abort.clone();
        let watchdog = std::thread::spawn(move || {
            if wait.recv_timeout(std::time::Duration::from_secs(2)) == Err(std::sync::mpsc::RecvTimeoutError::Timeout) {
                a.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        });
        let start = std::time::Instant::now();
        f();
        let _ = done.send(());
        let _ = watchdog.join();
        start.elapsed() > std::time::Duration::from_secs(2)
    };
    let mut step = |i: &mut Interp, what: String, ev: Event| {
        let trace = std::env::var("RANDOMCARD_TRACE").is_ok();
        if trace {
            eprintln!("> {what}");
        }
        let start = std::time::Instant::now();
        let mut ev = Some(ev);
        let mut stopped = with_watchdog(&abort, &mut || i.handle_event(ev.take().unwrap()));
        if trace {
            eprintln!("  {:?}", start.elapsed());
        }
        // A click that leaves the stack: come back the way a user would, so
        // the stack gets openStack again (and resets its globals). Coming
        // back gets two seconds of its own.
        let left = {
            let mut w = lock(&i.world);
            let left = w.stack.name != name;
            if left {
                if print {
                    println!("  ({what} went to stack \"{}\"; going back)", w.stack.name);
                }
                // Stacks it opened in new windows close, as a user would
                // close them; then the stack starts again from the original.
                w.windows.clear();
                w.window_key = 0;
                w.replace_stack(original.clone());
                // The changed copy it left behind stays loaded (it can't be
                // saved to the read-only disc); a user going back would get
                // that copy, but the test starts from the original, so drop it.
                w.others.retain(|o| o.stack.name != name);
                w.load_home();
                if trace {
                    let c = w.stack.card(w.card_id).map(|c| c.parts.iter().map(|p| p.name.clone()).collect::<Vec<_>>());
                    eprintln!("  back on card {} {:?}", w.card_id, c);
                }
            }
            left.then_some(w.card_id)
        };
        if let Some(card) = left {
            stopped |= with_watchdog(&abort, &mut || {
                for m in ["openStack", "openBackground", "openCard"] {
                    i.handle_event(Event::Send { target: ObjRef::Card(card), message: m.into(), args: vec![] });
                }
            });
        }
        if stopped {
            if print {
                println!("STOPPED after 2s: {what}");
            }
            out.stopped += 1;
        }
        let mut w = lock(&i.world);
        for r in w.requests.drain(..) {
            if let UiRequest::ScriptError { msg, obj, line } = r {
                let key = format!("{msg} ({obj:?} line {line:?})");
                let n = seen.entry(key.clone()).or_default();
                if *n == 0 {
                    if print {
                        println!("ERROR on {what}: {key}");
                    }
                    out.errors.push(format!("{what}: {msg}"));
                }
                out.error_count += 1;
                *n += 1;
            }
        }
    };
    step(&mut i, "openStack".into(), Event::Startup);
    let mut clicks = 0;
    // Background buttons are the same on every card: click them once.
    let mut clicked_bg = std::collections::HashSet::<(Id, Id)>::new();
    let cards: Vec<(Id, Id)> = lock(&i.world).stack.cards.iter().map(|c| (c.id, c.background)).collect();
    for (n, (card, bg)) in cards.iter().enumerate() {
        step(&mut i, format!("going to card {}", n + 1), Event::MsgBox(format!("go to card id {card}")));
        let buttons: Vec<(Rect, String)> = {
            let w = lock(&i.world);
            let b = w.stack.background(*bg).map(|b| b.parts.clone()).unwrap_or_default();
            let c = w.stack.card(*card).map(|c| c.parts.clone()).unwrap_or_default();
            b.into_iter()
                .filter(|p| clicked_bg.insert((*bg, p.id)))
                .map(|p| (Layer::Background, p))
                .chain(c.into_iter().map(|p| (Layer::Card, p)))
                .filter(|(_, p)| p.kind == crate::model::PartKind::Button && p.visible && p.enabled && !p.script.trim().is_empty())
                .map(|(_, p)| (p.rect, p.name))
                .collect()
        };
        // Clicks land on whatever is on top, as a real click does.
        for (rect, bname) in buttons {
            let what = format!("card {} button \"{bname}\"", n + 1);
            step(&mut i, what.clone(), Event::MsgBox(format!("go to card id {card}")));
            let (x, y) = ((rect.left + rect.right) / 2, (rect.top + rect.bottom) / 2);
            step(&mut i, what, Event::MsgBox(format!("click at {x},{y}")));
            clicks += 1;
        }
    }
    drop(step);
    out.clicks = clicks;
    out
}

/// Level 4 of the index: click everything in each stack listed, one path per
/// line, in RANDOMCARD_LEVEL4 (relative to the list's folder). Writes the
/// results beside the list, a row at a time: LEVEL4.list gives LEVEL4.tsv.
#[test]
#[ignore]
fn click_every_button() {
    use std::sync::mpsc;
    let Ok(list) = std::env::var("RANDOMCARD_LEVEL4") else { return };
    let list = std::path::PathBuf::from(list);
    let root = list.parent().unwrap().to_path_buf();
    let paths: Vec<String> = std::fs::read_to_string(&list).unwrap().lines().filter(|l| !l.is_empty()).map(String::from).collect();
    let out_path = list.with_extension("tsv");
    std::fs::write(&out_path, "path\tresult\tclicks\tdistinct_errors\terrors\tstopped\tfirst_error\tfixes\n").unwrap();
    let queue = Arc::new(Mutex::new(paths.clone()));
    let (tx, rx) = mpsc::channel::<(String, Option<Exercised>)>();
    let total = paths.len();
    for _ in 0..8 {
        let (queue, tx, root) = (queue.clone(), tx.clone(), root.clone());
        std::thread::Builder::new()
            .stack_size(super::SCRIPT_STACK)
            .spawn(move || loop {
                let Some(p) = queue.lock().unwrap().pop() else { break };
                let full = root.join(&p);
                let r = std::panic::catch_unwind(|| exercise_one(&full, false)).ok();
                if tx.send((p, r)).is_err() {
                    break;
                }
            })
            .unwrap();
    }
    drop(tx);
    let mut done = std::collections::HashSet::new();
    let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
    // A stack that runs for ten minutes is marked hung and left behind.
    while let Ok((p, r)) = rx.recv_timeout(std::time::Duration::from_secs(600)) {
        {
            {
                let row = match r {
                    Some(r) => {
                        let result = if r.errors.first().is_some_and(|e| e.starts_with("doesn't import")) {
                            "doesn't import"
                        } else if r.errors.is_empty() && r.clicks == 0 {
                            "nothing to click"
                        } else if r.errors.is_empty() {
                            "every click clean"
                        } else {
                            "errors"
                        };
                        format!(
                            "{p}\t{result}\t{}\t{}\t{}\t{}\t{}\t{}",
                            r.clicks,
                            r.errors.len(),
                            r.error_count,
                            r.stopped,
                            clean(r.errors.first().map(String::as_str).unwrap_or("")),
                            r.fixes
                        )
                    }
                    None => format!("{p}\tcrashed\t0\t0\t0\t0\t\t0"),
                };
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new().append(true).open(&out_path).unwrap();
                writeln!(f, "{row}").unwrap();
                done.insert(p);
                println!("{}/{total}", done.len());
            }
        }
    }
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().append(true).open(&out_path).unwrap();
    for p in paths.iter().filter(|p| !done.contains(*p)) {
        writeln!(f, "{p}\thung\t0\t0\t0\t0\t\t0").unwrap();
    }
    drop(f);
    println!("wrote {}", out_path.display());
    // Hung workers would keep the test running.
    std::process::exit(0);
}


#[test]
fn the_path_separator_finds_a_stacks_folder() {
    let sep = super::paths::SEPARATOR;
    assert_eq!(eval("the pathSeparator"), sep);
    let dir = std::env::temp_dir().join(format!("randomcard-sep-{}", std::process::id()));
    let mut s = stack();
    s.path = Some(dir.join("Notes.rcstack"));
    let mut i = interp_with(s);
    // The Technical Notes stack's way to its folder, with the separator in
    // place of its ":".
    let script = "put the long name of this stack into p\ndelete char 1 to 7 of p\ndelete last char of p\n\
        repeat until last char of p = the pathSeparator\ndelete last char of p\nend repeat\nput p";
    assert_eq!(run(&mut i, script), super::paths::folder(&dir));
}

#[test]
fn rinaldi_externals() {
    let mut i = interp_with(stack());
    // A field 190 wide: one short line, then a long one that wraps.
    run(&mut i, "put \"Hi\" & return & \"a line much too long for this narrow field to hold\" into bg fld \"Name\"");
    let lines: usize = run(&mut i, "put LineCount(\"bg fld Name\")").parse().unwrap();
    assert!(lines >= 3, "{lines}");
    assert_eq!(run(&mut i, "put LineCount(\"bg fld 9\")"), "Error : Bad field param");
    assert_eq!(run(&mut i, "put LineCount(\"?\")"), "LineCount(\"<field descriptor>\")");
    // FullSort keeps its order for co-sorting.
    assert_eq!(run(&mut i, "put FullSort(\"c,a,b\")"), "a,b,c");
    assert_eq!(run(&mut i, "global FullSortList\nput FullSortList"), "2,3,1");
    assert_eq!(run(&mut i, "put FullSort(\"three,one,two\", \"d=l\")"), "one,two,three");
    assert_eq!(run(&mut i, "put DateSort(\"b 2/1/90\" & return & \"a 1/1/90\")"), "a 1/1/90\nb 2/1/90");
    // Prompt: a message window, changed, then taken away.
    let prompt = |i: &Interp| lock(&i.world).script_windows.iter().find(|t| t.name == "Prompt").map(|t| t.text.clone());
    run(&mut i, "Prompt 1, \"Working…\"");
    assert_eq!(prompt(&i).as_deref(), Some("Working…"));
    run(&mut i, "Prompt 2, \"Still working\"");
    assert_eq!(prompt(&i).as_deref(), Some("Still working"));
    run(&mut i, "Prompt 3");
    assert_eq!(prompt(&i), None);
    assert_eq!(run(&mut i, "Prompt 3\nput the result"), "Error : Pointer Not Declared");
    assert_eq!(run(&mut i, "Prompt 7\nput the result"), "Error : Bad Mode Parameter");
    // GetPassword: the number ask password gives, or empty if canceled.
    i.canned_answers.push(DialogReply::Text(Some("secret".into())));
    assert_eq!(run(&mut i, "put GetPassword()"), crate::model::password::first_hash("secret").to_string());
    i.canned_answers.push(DialogReply::Text(None));
    assert_eq!(run(&mut i, "put GetPassword(\"Code:\")"), "");
    assert_eq!(run(&mut i, "put item 1 of GetMode()"), "C");
    // MacType's six items; Switch turns items to lines and back.
    assert_eq!(run(&mut i, "put the number of items of MacType()"), "6");
    assert_eq!(run(&mut i, "put Switch(\"a,b,c\")"), "a\nb\nc");
    assert_eq!(run(&mut i, "put Switch(\"a\" & return & \"b\" & return)"), "a,b");
    assert_eq!(run(&mut i, "set itemDelimiter to \";\"\nput Switch(\"a;b\")"), "a\nb");
    assert_eq!(run(&mut i, "put CapsLock()"), "false");
    lock(&i.world).mouse.caps_lock = true;
    assert_eq!(run(&mut i, "put CapsLock()"), "true");
    assert_eq!(run(&mut i, "put ClipInfo()"), "");
    assert_eq!(run(&mut i, "put the number of lines of ATalkZones()"), "1");
    assert_eq!(run(&mut i, "global zz1, aa2\nput GlobalList(false)"), "aa2,fullsortlist,zz1");
    assert_eq!(run(&mut i, "put GlobalList(7)"), "Error : Parameter must be true or false");
    // WindName titles the window until another stack opens.
    run(&mut i, "WindName \"My Window\"");
    let serial = lock(&i.world).stack_serial;
    assert_eq!(lock(&i.world).window_title, Some((serial, "My Window".to_string())));
    // ListSelect: names or numbers (in the list given, even sorted), by the
    // separator asked for, and ListSelectExit.
    let exit = |i: &mut Interp| run(i, "global ListSelectExit\nput ListSelectExit");
    i.canned_answers.push(DialogReply::List(vec![0, 2], "Validated".into()));
    assert_eq!(run(&mut i, "put ListSelect(\"DIS\", \"a,b,c\", \"Pick:\")"), "a,c");
    assert_eq!(exit(&mut i), "OK");
    i.canned_answers.push(DialogReply::List(vec![0], "Validated".into()));
    assert_eq!(run(&mut i, "put ListSelect(\"*1#\", \"pear\" & return & \"apple\", empty, return, \"Take\")"), "2");
    assert_eq!(exit(&mut i), "Take");
    i.canned_answers.push(DialogReply::List(vec![0, 1], "Validated".into()));
    assert_eq!(run(&mut i, "put ListSelect(\"CON\", \"a,b,c\", return)"), "a\nb");
    i.canned_answers.push(DialogReply::List(vec![], "Cancelled".into()));
    assert_eq!(run(&mut i, "put ListSelect(\"ONE\", \"a,b\", \"Pick\", \"Go\", \"Stop\")"), "");
    assert_eq!(exit(&mut i), "Stop");
    assert_eq!(run(&mut i, "put ListSelect(\"X\", \"a,b\")"), "Error : Bad Mode Parameter");
    assert_eq!(exit(&mut i), "Error");
    assert_eq!(run(&mut i, "put ListSelect(\"ONE\")"), "Error : Missing Parameter(s)");
    assert_eq!(run(&mut i, "put ListSelect(1, \"a,b\", \"Monaco,big\")"), "Error : Bad Font size");
    // With no one to answer, it's cancelled.
    assert_eq!(run(&mut i, "put ListSelect(1, \"a,b\")"), "");
    assert_eq!(exit(&mut i), "Cancel");
    // Mousoid and Infoid: windoids the UI keeps up to date.
    run(&mut i, "Mousoid");
    run(&mut i, "Infoid");
    assert_eq!(run(&mut i, "put \"Infoid\" is in the windows"), "true");
    assert_eq!(run(&mut i, "put the properties of window \"Mousoid\""), "loc,rect,visible,properties");
    assert_eq!(run(&mut i, "send \"Update\" to window \"Infoid\"\nput the result"), "");
    run(&mut i, "close window \"Mousoid\"");
    assert_eq!(run(&mut i, "put \"Mousoid\" is in the windows"), "false");
    assert_eq!(run(&mut i, "Mousoid \"x\", 3\nput the result"), "Error : Bad Coordinate");
    // Promptoid: Prompt's window by HyperCard 2's means.
    run(&mut i, "Promptoid \"Working\"");
    assert_eq!(run(&mut i, "Promptoid \"Again\"\nput the result"), "Error : Promptoid already displayed");
    run(&mut i, "set text of window \"Promptoid\" to \"Test 2\"");
    assert_eq!(run(&mut i, "put the text of window \"Promptoid\""), "Test 2");
    run(&mut i, "close window \"Promptoid\"");
    assert_eq!(run(&mut i, "Promptoid \"A\", empty, 40, \"wide\"\nput the result"), "Error : Bad width");
    // SendPS saves PostScript to open in the viewer; a path opens that file.
    let before = i.printed.len();
    assert_eq!(run(&mut i, "SendPS \"%!PS\" & return & \"showpage\"\nput the result"), "");
    let ps = i.printed.last().unwrap().clone();
    assert_eq!(i.printed.len(), before + 1);
    assert!(ps.extension().is_some_and(|e| e == "ps"));
    assert!(std::fs::read_to_string(&ps).unwrap().starts_with("%!PS"));
    assert_eq!(run(&mut i, "SendPS \"/no/such/file.ps\"\nput the result"), "Error : File not found");
    assert_eq!(run(&mut i, "SendPS \"%!\", \"maybe\"\nput the result"), "Error : Second param must be \"true\" or \"false\"");
    assert_eq!(run(&mut i, "SendPS empty\nput the result"), "Error : Missing parameter");
    // Notification waits only when RandomCard is in the background.
    assert_eq!(run(&mut i, "Notification\nput the result"), "");
    // A SuperCard branch's `top of window` breaks only its own handler,
    // which HyperCard never runs (checked in HyperCard 2.4.1).
    set_script(&i, ObjRef::Stack, "on works\n  put 42\nend works\nfunction SCTop\n  return top of window\nend SCTop");
    assert_eq!(run(&mut i, "works"), "42");
}

#[test]
fn hypercard_leniencies_run() {
    let mut i = interp_with(stack());
    let dir = std::env::temp_dir();
    let d = crate::runtime::paths::shown(&dir);
    assert_eq!(run(&mut i, &format!("put there is a folder \"{d}\"")), "true");
    assert_eq!(run(&mut i, &format!("put there is not a folder \"{d}/nothing here\"")), "true");
    let doc = dir.join("RandomCard leniency.txt");
    std::fs::write(&doc, "hello").unwrap();
    run(&mut i, &format!("print \"{}\" with \"TeachText\"", crate::runtime::paths::shown(&doc)));
    assert_eq!(i.printed.last(), Some(&doc));
    assert_eq!(run(&mut i, "answer program \"Which?\"\nput the result"), "Cancel");
    assert!(!run(&mut i, "play \"boing\" c#4 g3h.\nput 1").starts_with("ERROR"));
}

#[test]
fn hypercard_1_documents_stack() {
    let dir = std::env::temp_dir().join(format!("randomcard-documents-{}", std::process::id()));
    let docs = dir.join("Letters");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("Note.txt"), "hi").unwrap();
    std::fs::write(docs.join("Draft.txt"), "draft").unwrap();
    Stack::new("Old").save(&docs.join("Old.rcstack")).unwrap();
    // Home lists the folder on its Documents card.
    let home_path = dir.join("Home.rcstack");
    let mut home = Stack::home();
    crate::model::home::add_preference_cards(&mut home);
    let card = home.cards.iter().position(|c| c.name == "Documents").unwrap();
    let bg = home.cards[card].background;
    let paths = home.background(bg).unwrap().parts.iter().find(|p| p.name == "Paths").unwrap().id;
    home.cards[card].bg_text.insert(paths, crate::runtime::paths::shown(&docs));
    home.save(&home_path).unwrap();
    // The Documents stack's background, with the fields GetDocs fills.
    let mut s = Stack::new("Documents");
    for (n, name) in ["Name", "Creator", "Created", "Modified", "Size", "Type", "Notes", "Where"].iter().enumerate() {
        s.backgrounds[0].parts.push(Part::new_field(100 + n as Id, name, Rect::new(10, 10 + 20 * n as i32, 300, 28 + 20 * n as i32)));
    }
    s.cards[0].name = "About".into();
    s.path = Some(dir.join("Documents.rcstack"));
    let mut i = interp_with(s);
    lock(&i.world).home_path = Some(home_path);
    run(&mut i, "getHomeInfo");
    assert_eq!(run(&mut i, "global documents\nput documents"), crate::runtime::paths::shown(&docs));
    // The Update button's own script.
    run(&mut i, "global documents\ngetdocs line 1 of documents");
    assert_eq!(run(&mut i, "put the number of cards"), "4");
    assert_eq!(run(&mut i, "go to card 2\nput the last item of field \"Name\"").rsplit(['/', '\\', ':']).next(), Some("Draft.txt"));
    assert_eq!(run(&mut i, "go to card 4\nput field \"Type\" && field \"Creator\""), "STAK WILD");
    assert_eq!(run(&mut i, "go to card 3\nput field \"Size\""), "2");
    // Only new files on the next update.
    run(&mut i, "global documents\ngetdocs line 1 of documents");
    assert_eq!(run(&mut i, "put the number of cards"), "4");
    // Open, with the program GetCreator names.
    assert_eq!(run(&mut i, "put GetCreator(\"WILD\")"), "HyperCard");
    run(&mut i, "put GetCreator(field \"Creator\") into appName\nopen field \"Name\" with appName");
    assert_eq!(i.printed.last(), Some(&docs.join("Note.txt")));
    assert_eq!(run(&mut i, "open \"No Such Program\"\nput the result"), "Cancel");
    // `show card window at` moves the window.
    run(&mut i, "show card window at 30,40");
    assert_eq!(lock(&i.world).window_origin, (30, 40));
    // Convert leaves what isn't a date alone, as HyperCard did.
    assert_eq!(run(&mut i, "put \"soon\" into x\nconvert x to long date\nput the result && x"), "Invalid date soon");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn externals_done_by_randomcard() {
    assert_eq!(eval("isnum(12.5)"), "true");
    assert_eq!(eval("isnum(\"12a\")"), "false");
    assert_eq!(eval("isnum(empty)"), "true");
    assert_eq!(eval("fontExists(\"Palatino\", 14)"), "true");
    assert_eq!(eval("fontExists(\"Zapf Chancery\")"), "false");
    assert!(!run(&mut interp_with(stack()), "getHomeInfo").starts_with("ERROR"));
    assert_eq!(eval("main(\"Ner jr ybfg va gur jrrqf?\")"), "Are we lost in the weeds?");
    assert_eq!(eval("stringwidth(\"Hi\")"), eval("stringwidth(\"H\") + stringwidth(\"i\")"));
    assert_eq!(eval("item 3 of sysEnv()"), std::env::consts::ARCH);
    assert_eq!(eval("item 1 of line 2 of sysEnv()"), usize::BITS.to_string());
    // With no UI, file dialogs are canceled.
    assert_eq!(eval("filename(\"STAK\", \"Choose\")"), "");
    // FileExists answers true or false, unless the stack tests it against 0.
    assert_eq!(eval("fileExists(\"/no/such/file\")"), "false");
    let mut i = interp_with(stack());
    set_script(&i, ObjRef::Stack, "on check\n  if fileExists(\"/\") = 0 then put \"there\"\nend check");
    assert_eq!(run(&mut i, "put fileExists(\"/no/such/file\")"), "-43");
    assert_eq!(run(&mut i, "check"), "there");
    // A handler with the same name wins.
    set_script(&i, ObjRef::Stack, "function isnum x\n  return \"mine\"\nend isnum");
    assert_eq!(run(&mut i, "put isnum(1)"), "mine");
    // Commands: selectLineXCMD selects a line; spinAbout does nothing.
    let mut i = interp_with(stack());
    run(&mut i, "put \"a\" & return & \"b\" into bg field \"Name\"");
    assert_eq!(run(&mut i, "spinAbout"), "");
    run(&mut i, "selectLineXCMD the name of bg field \"Name\", 2");
    assert_eq!(run(&mut i, "put the selectedText"), "b");
    run(&mut i, "TNPict \"pic\",1,0");
    assert_eq!(run(&mut i, "put the result"), "Error file not found");
}

#[test]
fn message_box_keeps_its_place_for_scripts() {
    let mut i = interp_with(stack());
    let out = run(&mut i, "get the loc of msg\nshow msg at -32767,-32767\nput the loc of msg into x\nset the loc of msg to it\nput x && the loc of msg");
    assert!(out.starts_with("-32767,-32767 "), "{out}");
    assert!(!out.ends_with("-32767,-32767"), "{out}");
}

#[test]
fn dynamic_path_reaches_the_current_card() {
    let mut i = interp_with(stack());
    let two = lock(&i.world).stack.cards[1].id;
    set_script(&i, ObjRef::Card(two), "on cardTwoOnly\n  put \"card two\"\nend cardTwoOnly\nfunction twoFn\n  return 2\nend twoFn");
    set_script(&i, ObjRef::Stack, "on tryIt\n  go card 2\n  cardTwoOnly\n  put twoFn() after msg\nend tryIt");
    assert_eq!(run(&mut i, "tryIt"), "card two2");
    // Without going there, card two's handlers aren't in the path.
    run(&mut i, "go card 1");
    assert!(run(&mut i, "cardTwoOnly").contains("Can't understand"));
}






#[test]
fn forms_from_apples_stacks() {
    let mut i = interp_with(stack());
    set_script(&i, ObjRef::Stack, "on showCategory a, b\n  put a & \"/\" & b\nend showCategory");
    // `do` with several words runs them as a statement, as HyperCard did.
    assert_eq!(run(&mut i, "put 3 into cardNum\ndo ShowCategory CardNum,2"), "3/2");
    assert_eq!(run(&mut i, "do ShowCategory \"1\""), "1/");
    // A part's number, with or without `the`.
    assert_eq!(run(&mut i, "put number of bg fld \"Name\""), "1");
    assert_eq!(run(&mut i, "put the number of bg fld \"Name\""), "1");
    assert_eq!(run(&mut i, "put the number of bg flds"), "1");
    // Commas between answer's buttons.
    i.canned_answers.push(DialogReply::Button("Yes".into()));
    assert_eq!(run(&mut i, "answer \"Delete?\" with \"No\", \"Yes\"\nput it"), "Yes");
    // Variables named like keywords.
    assert_eq!(run(&mut i, "put 5 into to\nput 2 into times\nput to * times"), "10");
    assert_eq!(run(&mut i, "put 2 into where\nput line where of (\"a\" & return & \"b\")"), "b");
    // Windows, the palettes and the stack's size.
    assert_eq!(run(&mut i, "put there is a window \"message box\""), "true");
    assert_eq!(run(&mut i, "put there is no window \"Describe Problem\""), "true");
    i.handle_event(Event::MsgBox("hide tool window".into()));
    let palette = |r: &UiRequest| matches!(r, UiRequest::ShowPalette(false));
    assert!(lock(&i.world).requests.drain(..).any(|r| palette(&r)));
    assert_eq!(run(&mut i, "put the visible of tool window"), "false");
    // `pattern window` is the same palette: moving it, its loc is where it went.
    i.handle_event(Event::MsgBox("show pattern window at 30,200".into()));
    let moved = |r: &UiRequest| matches!(r, UiRequest::MovePalette(30, 200));
    assert!(lock(&i.world).requests.drain(..).any(|r| moved(&r)));
    assert_eq!(run(&mut i, "put the loc of pattern window && the visible of pattern window"), "30,200 true");
    assert_eq!(run(&mut i, "put the name of window \"Tools\""), "Tools");
    assert_eq!(run(&mut i, "put freeSize of this stack"), "0");
    assert_eq!(run(&mut i, "put size of this stack > 0"), "true");
    // Popping more cards than were pushed gives the Home card.
    assert_eq!(run(&mut i, "pop card into backFrom\nput backFrom contains \"Home\""), "true");
    // An unquoted menu item is the rest of the line.
    let cards = lock(&i.world).stack.cards.len();
    run(&mut i, "domenu new card");
    assert_eq!(lock(&i.world).stack.cards.len(), cards + 1);
}


#[test]
fn hypercard_2_menu_items() {
    let mut i = interp_with(stack());
    // Apple's stacks edit HyperCard 2's items by name, including ones the
    // menus don't show.
    assert_eq!(run(&mut i, "delete menuItem \"print report...\" of menu \"file\""), "");
    assert_eq!(run(&mut i, "disable menuItem \"Compact Stack\" of menu \"File\""), "");
    assert_eq!(run(&mut i, "put the enabled of menuItem \"Compact Stack\" of menu \"File\""), "false");
    assert!(!run(&mut i, "doMenu \"Page Setup...\"").starts_with("ERROR"));
    // With one window, Close Stack goes Home (here, with no Home, the first card).
    run(&mut i, "go to card 3");
    run(&mut i, "doMenu \"Close Stack\"");
    assert_eq!(run(&mut i, "put the short name of this card"), "One");
    // Delete Stack… only deletes a stack's own RandomCard file.
    let e = run(&mut i, "doMenu \"Delete Stack...\"");
    assert!(e.contains("has no RandomCard file"), "{e}");
    // Print Field… prints the field's text in the field's own font.
    run(&mut i, "put \"Printed\" into bg field \"Name\"\nset the textFont of bg field \"Name\" to \"Courier\"");
    run(&mut i, "print bg field \"Name\"");
    let pdf = String::from_utf8_lossy(&std::fs::read(i.printed.last().unwrap()).unwrap()).to_string();
    assert!(pdf.contains("/BaseFont /Courier"), "the field printed in Courier");
}

#[test]
fn compact_stack_frees_space_and_saves() {
    let dir = std::env::temp_dir().join(format!("randomcard-compact-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Roomy.rcstack");
    let mut s = stack();
    // Text a deleted background field left on the first card.
    s.cards[0].bg_text.insert(4242, "nobody reads this any more".into());
    s.save(&path).unwrap();
    s.path = Some(path.clone());
    let mut i = interp_with(s);
    let free: usize = run(&mut i, "put the freeSize of this stack").parse().unwrap();
    assert!(free > 20, "{free}");
    let size: usize = run(&mut i, "put the size of this stack").parse().unwrap();
    run(&mut i, "doMenu \"Compact Stack\"");
    assert_eq!(run(&mut i, "put the freeSize of this stack"), "0");
    assert_eq!(run(&mut i, "put the size of this stack"), (size - free).to_string());
    // It's saved, as HyperCard's Compact Stack wrote the stack.
    let saved = Stack::load(&path).unwrap();
    assert!(!saved.cards[0].bg_text.contains_key(&4242));
    assert!(!lock(&i.world).dirty);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn delete_stack_removes_its_file_after_asking() {
    let dir = std::env::temp_dir().join(format!("randomcard-del-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Doomed.rcstack");
    let mut s = stack();
    s.save(&path).unwrap();
    s.path = Some(path.clone());
    let mut i = interp_with(s);
    // Cancel keeps it.
    i.canned_answers.push(DialogReply::Button("Cancel".into()));
    run(&mut i, "doMenu \"Delete Stack…\"");
    assert!(path.exists());
    i.canned_answers.push(DialogReply::Button("Delete".into()));
    run(&mut i, "doMenu \"Delete Stack…\"");
    assert!(!path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Builds an index of every stack under a folder (the Developer CDs that
/// tools/devcds.py unpacks): what each is, where it's found, and how far it
/// works in RandomCard. Identical stacks on several discs are tried once.
/// RANDOMCARD_INDEX=examples/apple/devcds cargo test --release index_stacks -- --ignored --nocapture
/// writes INDEX.tsv in that folder.
#[test]
#[ignore]
fn index_stacks() {
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};
    use std::io::Read;
    use std::path::{Path, PathBuf};
    let Ok(root) = std::env::var("RANDOMCARD_INDEX") else { return };
    let root = PathBuf::from(root);

    // Every stack, found by its data ("STAK" at byte 4), grouped by content.
    // ORIGINALS holds the untouched copies of stacks whose typos were fixed.
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() && p.file_name().is_some_and(|n| n == "ORIGINALS") {
                continue;
            }
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_none_or(|x| x != "rsrc") {
                let mut head = [0u8; 8];
                if std::fs::File::open(&p).and_then(|mut f| f.read_exact(&mut head)).is_ok() && &head[4..8] == b"STAK" {
                    out.push(p);
                }
            }
        }
    }
    let _ = std::fs::remove_file(root.join("INDEX.partial.tsv"));
    let mut found = Vec::new();
    walk(&root, &mut found);
    let mut groups: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for p in found {
        let Ok(data) = std::fs::read(&p) else { continue };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        data.hash(&mut h);
        groups.entry(h.finish()).or_default().push(p);
    }
    let mut work: Vec<Vec<PathBuf>> = groups.into_values().collect();
    work.sort_by(|a, b| a[0].cmp(&b[0]));
    println!("{} distinct stacks", work.len());

    fn disc(root: &Path, p: &Path) -> String {
        p.strip_prefix(root).ok().and_then(|r| r.components().next()).map(|c| c.as_os_str().to_string_lossy().to_string()).unwrap_or_default()
    }
    fn clean(s: &str) -> String {
        s.replace(['\t', '\n', '\r'], " ").trim().to_string()
    }
    let queue = Arc::new(Mutex::new(work.into_iter().enumerate().collect::<Vec<_>>()));
    let rows = Arc::new(Mutex::new(Vec::<(usize, String)>::new()));
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let (queue, rows, root) = (queue.clone(), rows.clone(), root.clone());
            std::thread::spawn(move || loop {
                let Some((n, paths)) = queue.lock().unwrap().pop() else { break };
                let path = paths[0].clone();
                // Each stack on its own thread: a crash marks it, and one that
                // never finishes (a loop outside any script) is left behind.
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::Builder::new()
                    .stack_size(super::SCRIPT_STACK)
                    .spawn(move || {
                        let facts = std::panic::catch_unwind(|| index_one(&path))
                            .unwrap_or_else(|_| Facts { works: "crashed".into(), ..Facts::default() });
                        let _ = tx.send(facts);
                    })
                    .unwrap();
                let result = rx
                    .recv_timeout(std::time::Duration::from_secs(60))
                    .unwrap_or_else(|_| Facts { works: "hung".into(), ..Facts::default() });
                let p0 = &paths[0];
                let mut discs: Vec<String> = paths.iter().map(|p| disc(&root, p)).collect();
                discs.dedup();
                let row = [
                    p0.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                    result.works,
                    result.cards.to_string(),
                    result.bad_scripts.to_string(),
                    result.open_errors.to_string(),
                    clean(&result.first_error),
                    clean(&result.about),
                    p0.parent().and_then(|d| d.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                    discs.len().to_string(),
                    discs.join("; "),
                    p0.strip_prefix(&root).unwrap_or(p0).display().to_string(),
                ]
                // Not trimmed: some Mac file names end in a space.
                .map(|s| s.replace(['\t', '\n', '\r'], " "))
                .join("\t");
                // Written as it goes, so a run that stops loses nothing.
                {
                    use std::io::Write;
                    let mut f = std::fs::OpenOptions::new().append(true).create(true).open(root.join("INDEX.partial.tsv")).unwrap();
                    let _ = writeln!(f, "{row}");
                }
                rows.lock().unwrap().push((n, row));
                let done = rows.lock().unwrap().len();
                if done % 200 == 0 {
                    println!("{done} done");
                }
            })
        })
        .collect();
    for w in workers {
        w.join().unwrap();
    }
    let mut rows = Arc::try_unwrap(rows).unwrap().into_inner().unwrap();
    rows.sort_by_key(|r| r.0);
    let header = "name\tworks\tcards\tbad_scripts\topen_errors\tfirst_error\tabout\tfolder\tdisc_count\tdiscs\tpath";
    let text = std::iter::once(header.to_string()).chain(rows.into_iter().map(|r| r.1)).collect::<Vec<_>>().join("\n");
    std::fs::write(root.join("INDEX.tsv"), text + "\n").unwrap();
    let _ = std::fs::remove_file(root.join("INDEX.partial.tsv"));
    println!("wrote {}", root.join("INDEX.tsv").display());
    // Threads of hung stacks are still spinning; end the process.
    std::process::exit(0);
}

#[derive(Default)]
struct Facts {
    works: String,
    cards: usize,
    bad_scripts: usize,
    open_errors: usize,
    first_error: String,
    about: String,
}

/// How far one stack works: imports, parses, opens.
fn index_one(path: &std::path::Path) -> Facts {
    let stack = match crate::import::load_any(path, None) {
        Ok(s) => s,
        Err(e) => return Facts { works: "doesn't import".into(), first_error: e, ..Facts::default() },
    };
    let mut f = Facts { cards: stack.cards.len(), ..Facts::default() };
    // What it is: the comment at the top of its stack script, or the first
    // line of text on its first card (Japanese counting double: 北海道 is a
    // name).
    let comment: Vec<String> = stack
        .script
        .lines()
        .map(str::trim)
        .take_while(|l| l.is_empty() || l.starts_with("--"))
        .map(|l| l.trim_start_matches('-').trim().to_string())
        .filter(|l| l.chars().any(char::is_alphanumeric))
        .take(2)
        .collect();
    f.about = if !comment.is_empty() {
        comment.join(" — ")
    } else {
        let c = &stack.cards[0];
        let texts = c.parts.iter().filter(|p| p.kind == crate::model::PartKind::Field).map(|p| stack.field_text(c.id, Layer::Card, p.id))
            .chain(stack.background(c.background).into_iter().flat_map(|b| b.parts.iter()).filter(|p| p.kind == crate::model::PartKind::Field).map(|p| stack.field_text(c.id, Layer::Background, p.id)));
        texts.flat_map(|t| t.lines().map(str::to_string).collect::<Vec<_>>()).find(|l| l.chars().filter(|c| c.is_alphanumeric()).map(|c| if c as u32 >= 0x3000 { 2 } else { 1 }).sum::<usize>() > 3).unwrap_or_default()
    };
    f.about = f.about.chars().take(140).collect();
    let mut scripts = vec![stack.script.clone()];
    for b in &stack.backgrounds {
        scripts.push(b.script.clone());
        scripts.extend(b.parts.iter().map(|p| p.script.clone()));
    }
    for c in &stack.cards {
        scripts.push(c.script.clone());
        scripts.extend(c.parts.iter().map(|p| p.script.clone()));
    }
    f.bad_scripts = scripts.iter().filter(|s| crate::hypertalk::parser::parse_script(s).is_err()).count();
    // Open it, stopping anything that runs longer than three seconds.
    let mut i = interp_with(stack);
    let abort = i.abort.clone();
    let done = Arc::new(AtomicBool::new(false));
    let d = done.clone();
    let watchdog = std::thread::spawn(move || {
        for _ in 0..60 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if d.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
        }
        abort.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    i.handle_event(Event::Startup);
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = watchdog.join();
    let errors: Vec<String> = lock(&i.world)
        .requests
        .drain(..)
        .filter_map(|r| match r {
            UiRequest::ScriptError { msg, .. } => Some(msg),
            _ => None,
        })
        .collect();
    f.open_errors = errors.len();
    f.first_error = errors.first().cloned().unwrap_or_default();
    f.works = match (f.bad_scripts, f.open_errors) {
        (0, 0) => "opens cleanly",
        (_, 0) => "opens; some scripts don't parse",
        _ => "errors on opening",
    }
    .into();
    f
}

#[test]
fn scripts_move_the_card_window() {
    let mut i = interp_with(stack());
    let (w, h) = { let w = lock(&i.world); (w.stack.width as i32, w.stack.height as i32) };
    // Apple's stacks center the window on the screen this way.
    run(&mut i, "get the screenRect\nset the loc of cd window to (item 3 of it-512)/2,(item 4 of it-342)/2");
    assert_eq!(run(&mut i, "put the loc of card window"), "704,369");
    assert_eq!(run(&mut i, "put the rect of this stack"), format!("704,369,{},{}", 704 + w, 369 + h));
    run(&mut i, "set the rect of cd window to 10,40,600,400");
    assert_eq!(run(&mut i, "put the topLeft of card window"), "10,40");
    assert_eq!(run(&mut i, "put the width of card window"), w.to_string());
    run(&mut i, "set the left of cd window to 0");
    assert_eq!(run(&mut i, "put the left of this stack && the top of this stack"), "0 40");
}

#[test]
fn day_and_month_names_from_datebook() {
    assert_eq!(eval("dayMonthName(1, 1)"), "Sunday");
    assert_eq!(eval("dayMonthName(2, 12)"), "December");
    assert_eq!(eval("dayMonthName(3, 7)"), "Sat");
    assert_eq!(eval("dayMonthName(4, 2)"), "Feb");
}

#[test]
fn clicking_with_the_button_tool_selects() {
    // DateBook removes its Extend button by clicking it with the button
    // tool; the click mustn't run the button's script.
    let mut i = interp_with(stack());
    let card = lock(&i.world).card_id;
    set_script(&i, ObjRef::Part { layer: Layer::Card, id: 11, card }, "on mouseUp\n  put \"ran\" into msg\nend mouseUp");
    run(&mut i, "put empty into msg");
    assert_eq!(run(&mut i, "choose button tool\nclick at 50,75\nput msg"), "");
    assert_eq!(run(&mut i, "choose browse tool\nclick at 50,75\nput msg"), "ran");
}

#[test]
fn pop_up_menus_for_scripts() {
    let mut i = interp_with(stack());
    // PopUpMenu answers the item's number; nothing chosen is 0.
    i.canned_answers.push(DialogReply::Text(Some("2".into())));
    assert_eq!(run(&mut i, "put PopUpMenu(\"Map\" & return & \"Index\", 1, 100, 50)"), "2");
    i.canned_answers.push(DialogReply::Text(None));
    assert_eq!(run(&mut i, "put PopUpMenu(\"Map,Index\", 0, 100, 50)"), "0");
    // PopMenu answers "true,item,menu", or empty.
    i.canned_answers.push(DialogReply::Text(Some("3".into())));
    assert_eq!(run(&mut i, "put PopMenu(10, 20, 0, \"Undo,(-,Cut,Copy\")"), "true,3,1");
    i.canned_answers.push(DialogReply::Text(None));
    assert_eq!(run(&mut i, "put PopMenu(10, 20, 0, \"Undo,Cut\") is empty"), "true");
    // With no one to choose (no UI), nothing is chosen.
    assert_eq!(run(&mut i, "put PopUpMenu(\"A,B\")"), "0");
    assert_eq!(run(&mut i, "put FilePath(\"TEXT\", \"Which file?\") is empty"), "true");
    assert_eq!(run(&mut i, "put NewFileName(\"Save as:\", \"Untitled\") is empty"), "true");
    assert!(!run(&mut i, "put MacType()").is_empty());
}

#[test]
fn script_windows_from_scripts() {
    // Apple's Bug Reporter opens its fields in TEXWindow windows and reads
    // them back as HyperCard 2 windows.
    let mut i = interp_with(stack());
    run(&mut i, "TEXWindow \"Notes\", \"Hello\", 10, 10, 300, 200, \"Geneva\", 12, 16, -1");
    assert_eq!(run(&mut i, "put there is a window \"Notes\""), "true");
    assert_eq!(run(&mut i, "put the text of window \"Notes\""), "Hello");
    assert_eq!(run(&mut i, "put line 1 of the windows"), "Notes");
    lock(&i.world).script_windows[0].text = "Edited".into();
    lock(&i.world).script_windows[0].dirty = true;
    let card = lock(&i.world).card_id;
    set_script(&i, ObjRef::Card(card), "on TEXWindowClosing name, changed\n  put the text of window name && changed into msg\nend TEXWindowClosing");
    i.handle_event(Event::CloseWindow("Notes".into()));
    assert_eq!(lock(&i.world).msg, "Edited true");
    assert_eq!(run(&mut i, "put there is a window \"Notes\""), "false");
    // C:F Registration Requests shows a field's text read-only.
    run(&mut i, "TEXWindow \"Notes\", \"Read me\"\nset the locktext of window \"Notes\" to true");
    assert_eq!(run(&mut i, "put the lockText of window \"Notes\""), "true");
    // Zooming it again brings it to the front.
    run(&mut i, "TEXWindow \"Other\", \"\"");
    run(&mut i, "send \"select\" to window \"Notes\"");
    assert_eq!(run(&mut i, "put line 1 of the windows"), "Notes");
    run(&mut i, "close window \"Other\"");
    run(&mut i, "close window \"Notes\"");
    run(&mut i, "TEXWindow \"Notes\", \"Again\"\nclose window \"Notes\"");
    assert_eq!(run(&mut i, "put there is a window \"Notes\""), "false");
}

#[test]
fn apple_menu() {
    // As the Developer Info Assistant renames HyperCard's About item.
    let mut i = interp_with(stack());
    let card = lock(&i.world).card_id;
    set_script(&i, ObjRef::Card(card), "on AboutThisStack\n  put \"about this stack\" into msg\nend AboutThisStack");
    assert_eq!(run(&mut i, "put there is a menu \"Apple\" && line 1 of the menus"), "true Apple");
    run(&mut i, "put \"About\" && the short name of this stack & \"...\" into menuItem 1 of menu \"Apple\"\nset the menuMsg of menuItem 1 of menu \"Apple\" to \"AboutThisStack\"");
    assert_eq!(run(&mut i, "put menuItem 1 of menu \"Apple\""), "About Test...");
    run(&mut i, "doMenu \"About Test...\"");
    assert_eq!(lock(&i.world).msg, "about this stack");
    run(&mut i, "reset menuBar");
    assert_eq!(run(&mut i, "put menuItem 1 of menu \"Apple\""), "About RandomCard…");
    // About RandomCard says which version.
    i.canned_answers.push(DialogReply::Button("OK".into()));
    run(&mut i, "doMenu \"About HyperCard…\"");
}

#[test]
fn texas_windows() {
    // As the Developer Info Assistant uses them: a discussion read a
    // message at a time, a message ready to send, and feedback.
    let mut i = interp_with(stack());
    let card = lock(&i.world).card_id;
    set_script(
        &i,
        ObjRef::Card(card),
        "on texasReplyMsg w\n  put \"reply \" & w & \";\" after msg\nend texasReplyMsg\n\
         on texasClose w, f\n  put \"close \" & the style of window w && f & \";\" after msg\nend texasClose\n\
         function texasSendMsg w, f\n  put \"send \" & f & \";\" after msg\n  close window w\n  return true\nend texasSendMsg",
    );
    lock(&i.world).stack.cards[0].bg_text.insert(10, super::texas::discussion(&["First", "Second"]));
    run(&mut i, "Texas \"Thread\", bg fld \"Name\", \"Disc\"");
    assert_eq!(run(&mut i, "put WindIsTexas(\"Thread\") && WindIsTexas(\"Nope\")"), "true false");
    assert_eq!(run(&mut i, "put FrontIfTexas()"), "Thread");
    assert_eq!(run(&mut i, "put the style of window \"Thread\""), "Disc");
    assert_eq!(run(&mut i, "put the messageList of window \"Thread\""), "First\nSecond");
    // DIA leaves out "the".
    assert_eq!(run(&mut i, "put messageName of window \"Thread\""), "First");
    assert_eq!(run(&mut i, "put the text of window \"Thread\""), "The text of First.");
    assert_eq!(run(&mut i, "put the enclosure of window \"Thread\""), "FALSE");
    assert_eq!(run(&mut i, "put the whole of window \"Thread\" is bg fld \"Name\""), "true");
    i.handle_event(Event::TexasShow { name: "Thread".into(), n: 1 });
    assert_eq!(run(&mut i, "put the messageName of window \"Thread\""), "Second");
    // A reply comes in: threaded on, and shown.
    let reply = super::texas::discussion(&["Third"]);
    lock(&i.world).stack.cards[1].bg_text.insert(10, reply);
    run(&mut i, "set the newMessage of window \"Thread\" to bg fld \"Name\" of card 2");
    assert_eq!(run(&mut i, "put the messageName of window \"Thread\" && the number of lines of the messageList of window \"Thread\""), "Third 3");
    run(&mut i, "put empty into msg");
    i.handle_event(Event::Texas { name: "Thread".into(), action: super::texas::TexasAction::Reply });
    i.handle_event(Event::Texas { name: "Thread".into(), action: super::texas::TexasAction::Close });
    assert_eq!(lock(&i.world).msg, "reply Thread;close Disc noWhere;");
    assert_eq!(run(&mut i, "put there is a window \"Thread\""), "false");

    // A message to send: its header, then its text.
    run(&mut i, "Texas \"Hello\", \"Source: DIA 4.1\" & return & \"Thread ID: 1\" & return & \"Thread Title: Hello\" & return & \"Message Title: Hello\" & return & return & \"Body\", \"Send\"");
    assert_eq!(run(&mut i, "put the style of window \"Hello\" && the text of window \"Hello\""), "New Body");
    assert_eq!(run(&mut i, "put line 4 of the whole of window \"Hello\""), "Message Title: Hello");
    run(&mut i, "set the enclosure of window \"Hello\" to \"TRUE && Notes.Pkg\"");
    assert_eq!(run(&mut i, "put word 3 to 99 of the enclosure of window \"Hello\""), "Notes.Pkg");
    run(&mut i, "set the enclosure of window \"Hello\" to \"FALSE\"");
    assert_eq!(run(&mut i, "put the enclosure of window \"Hello\""), "FALSE");
    // Send: the stack sends it and closes the window.
    run(&mut i, "put empty into msg");
    i.handle_event(Event::Texas { name: "Hello".into(), action: super::texas::TexasAction::Send });
    assert_eq!(lock(&i.world).msg, "send SendBtn;close New yesSend;");
    assert_eq!(run(&mut i, "put there is a window \"Hello\""), "false");

    // Feedback is written in the window; closing it unsent asks first.
    run(&mut i, "Texas \"Comments\", empty, \"feed\"\nset the text of window \"Comments\" to \"Nice\"");
    assert_eq!(run(&mut i, "put the lockText of window \"Comments\" && the text of window \"Comments\""), "false Nice");
    run(&mut i, "put empty into msg");
    i.canned_answers.push(DialogReply::Button("Cancel".into()));
    i.handle_event(Event::Texas { name: "Comments".into(), action: super::texas::TexasAction::Close });
    assert_eq!(run(&mut i, "put there is a window \"Comments\""), "true");
    run(&mut i, "put empty into msg");
    i.canned_answers.push(DialogReply::Button("Don't Send".into()));
    i.handle_event(Event::Texas { name: "Comments".into(), action: super::texas::TexasAction::Close });
    assert_eq!(lock(&i.world).msg, "close Feed noSend;");
    assert!(run(&mut i, "Texas \"X\", empty, \"Fancy\"").starts_with("ERROR: Texas has no window style"));
}

#[test]
fn palettes() {
    // As Apple's Info Island stacks use them: a navigation palette whose
    // buttons send messages to the card, opened at a point and closed by
    // the stack's scripts.
    use crate::model::{Palette, PaletteButton, Picture, Rect};
    let mut i = interp_with(stack());
    let buttons = vec![
        PaletteButton { rect: Rect::new(0, 0, 50, 20), message: "menuNavigation \"Stack Map\"".into() },
        PaletteButton { rect: Rect::new(50, 0, 100, 20), message: "showMarker".into() },
    ];
    let palette = Palette { name: "mainMenu".into(), picture: Picture::new(100, 20), buttons };
    lock(&i.world).stack.palettes.insert(25263, palette);
    let card = lock(&i.world).card_id;
    set_script(
        &i,
        ObjRef::Card(card),
        "on openPalette n, id\n  put n && id into msg\nend openPalette\n\
         on closePalette n\n  put \"closed\" && n into msg\nend closePalette\n\
         on menuNavigation where\n  put where into msg\nend menuNavigation",
    );
    run(&mut i, "palette \"MAINMENU\",\"327,184\"");
    assert_eq!(lock(&i.world).msg, "mainMenu 25263");
    assert_eq!(run(&mut i, "put there is a window \"mainMenu\""), "true");
    assert_eq!(run(&mut i, "put the loc of window \"mainMenu\""), "327,184");
    assert_eq!(run(&mut i, "put the buttonCount of window \"mainMenu\""), "2");
    assert_eq!(run(&mut i, "put line 2 of the commands of window \"mainMenu\""), "showMarker");
    assert_eq!(run(&mut i, "put the hilitedButton of window \"mainMenu\""), "-1");
    run(&mut i, "set the hilitedButton of window \"mainMenu\" to 2");
    assert_eq!(run(&mut i, "put the hilitedButton of window \"mainMenu\""), "2");
    i.handle_event(Event::PaletteButton("mainMenu".into(), 0));
    assert_eq!(lock(&i.world).msg, "Stack Map");
    run(&mut i, "close window \"mainMenu\"");
    assert_eq!(lock(&i.world).msg, "closed mainMenu");
    assert_eq!(run(&mut i, "put there is a window \"mainMenu\""), "false");
    // Without a point it goes back where it was.
    run(&mut i, "palette \"mainMenu\"");
    assert_eq!(run(&mut i, "put the loc of window \"mainMenu\""), "327,184");
    assert!(run(&mut i, "palette \"nothing\"").contains("Can't find palette"));
}

#[test]
fn the_windows_name_the_card_window_by_its_stack() {
    // The develop stacks build a Windows menu from the windows with
    // "develop" in their names, and check its fifth item: the stack's own.
    let mut s = stack();
    s.name = "develop Index".into();
    let mut i = interp_with(s);
    run(&mut i, "TEXWindow \"Notes\", \"Hi\"");
    assert_eq!(run(&mut i, "put line 1 to 2 of the windows"), "Notes\ndevelop Index");
    assert_eq!(run(&mut i, "put there is a window \"develop index\""), "true");
    assert_eq!(run(&mut i, "put there is a window \"card window\""), "true");
    let script = "create menu \"Windows\"\n\
        put the windows into all\n\
        repeat with i = the number of lines in all down to 1\n\
        if \"develop\" is not in line i of all then delete line i of all\n\
        end repeat\n\
        put \"Show Page Snapshots\" & return & \"-\" & return & \"Show All Issues\" & return & \"-\" & return & all into menu \"Windows\"\n\
        set checkmark of menuitem 5 of menu \"Windows\" to true\n\
        show window \"develop Index\"\n\
        show window \"Patterns\"\n\
        put menuitem 5 of menu \"Windows\"";
    assert_eq!(run(&mut i, script), "develop Index");
}

#[test]
fn mail_and_volumes() {
    let mut i = interp_with(stack());
    i.handle_event(Event::MsgBox(
        "MakeSendMe5 \"DEVSUPPORT\", \"\", \"Q&A 1\", \"Line one\" & return & \"Line two\"".into(),
    ));
    let urls: Vec<String> = lock(&i.world)
        .requests
        .drain(..)
        .filter_map(|r| {
            if let UiRequest::OpenUrl(u) = r {
                Some(u)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        urls,
        vec!["mailto:DEVSUPPORT?subject=Q%26A%201&body=Line%20one%0D%0ALine%20two".to_string()]
    );
    assert!(!run(&mut i, "put GetVol()").is_empty());
}

#[test]
fn progress_windows() {
    // As Apple's ProgressWindoid stack drives one: create it hidden, show
    // it, fill it, move it, read it back, close it.
    let mut i = interp_with(stack());
    assert_eq!(
        run(
            &mut i,
            "put ProgressWindoid(\"Copying\", \"20,50\", 300, \"Copying files:\", 0.25)"
        ),
        ""
    );
    assert_eq!(
        run(
            &mut i,
            "put there is a window \"Copying\" && the visible of window \"Copying\""
        ),
        "true false"
    );
    run(&mut i, "show window \"Copying\"");
    assert_eq!(run(&mut i, "put the visible of window \"Copying\""), "true");
    assert_eq!(
        run(&mut i, "put the fraction of window \"Copying\""),
        "0.25"
    );
    run(
        &mut i,
        "set the fraction of window \"Copying\" to 0.75\nset the text of window \"Copying\" to \"Almost\"",
    );
    assert_eq!(
        run(
            &mut i,
            "put the fraction of window \"Copying\" && the text of window \"Copying\""
        ),
        "0.75 Almost"
    );
    run(&mut i, "set the loc of window \"Copying\" to 100,120");
    assert_eq!(
        run(
            &mut i,
            "put the loc of window \"Copying\" && the width of window \"Copying\""
        ),
        "100,120 300"
    );
    assert_eq!(
        run(&mut i, "put the properties of window \"Copying\""),
        "width,text,fraction,textFont,textSize"
    );
    assert!(run(&mut i, "put ProgressWindoid(\"Copying\", \"0,0\")").starts_with('•'));
    run(&mut i, "hide window \"Copying\"");
    assert_eq!(
        run(&mut i, "put the visible of window \"Copying\""),
        "false"
    );
    run(&mut i, "close window \"Copying\"");
    assert!(run(&mut i, "put the fraction of window \"Copying\"").contains("No such window"));
}

#[test]
fn files_and_folders() {
    // Rinaldi's file externals, on real files in a scratch folder.
    let dir = std::env::temp_dir().join(format!("randomcard-files-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("Sub")).unwrap();
    std::fs::write(dir.join("Notes"), "hello").unwrap();
    let d = dir.display().to_string();
    let mut i = interp_with(stack());
    let r = |i: &mut Interp, s: String| run(i, &s);
    // Copy, and not over an existing file unless told to.
    assert_eq!(r(&mut i, format!("FileCopy \"{d}/Notes\", \"{d}/Copy\"\nput the result")), "");
    assert_eq!(std::fs::read_to_string(dir.join("Copy")).unwrap(), "hello");
    assert_eq!(r(&mut i, format!("FileCopy \"{d}/Notes\", \"{d}/Copy\"\nput the result")), "Error : Duplicate file name");
    assert_eq!(r(&mut i, format!("FileCopy \"{d}/Notes\", \"{d}/Copy\", replace\nput the result")), "");
    assert_eq!(r(&mut i, format!("FileCopy \"{d}/Nothing\", \"{d}/X\"\nput the result")), "Error : File not found");
    // List the folder: files, folders, both; with filters and a layout.
    assert_eq!(r(&mut i, format!("put GetDir(\"{d}\")")), "Copy\nNotes\nSub");
    assert_eq!(r(&mut i, format!("put GetDir(\"{d}\", \"F\")")), "Copy\nNotes");
    assert_eq!(r(&mut i, format!("put GetDir(\"{d}\", \"D\")")), "Sub");
    assert_eq!(r(&mut i, format!("put GetDir(\"{d}\", \"F\", \"n<No\", \"o=nd\")")), "Notes,5");
    assert!(r(&mut i, format!("put GetDir(\"{d}\", \"F\", \"q=1\")")).starts_with("Error : First char"));
    // FolderContents, from Apple's DTS externals, as the Developer Info
    // Assistant asks: problems go in a global instead of an alert.
    std::fs::write(dir.join("Copy.rsrc"), "fork").unwrap();
    assert_eq!(r(&mut i, format!("put FolderContents(\"{d}\", \"files\", \"noDialog:gErr\")")), "Copy\nNotes");
    assert_eq!(r(&mut i, format!("put FolderContents(\"{d}\", \"folders\")")), "Sub");
    assert_eq!(r(&mut i, format!("get FolderContents(\"{d}/None\", \"files\", \"noDialog:gErr\")\nput (it is empty) && gErr")), format!("true Couldn't read the folder \"{d}/None\"."));
    std::fs::remove_file(dir.join("Copy.rsrc")).unwrap();
    // Where file dialogs open: defaultFolder, or the Developer Info
    // Assistant's StdFile; a file means its folder.
    assert_eq!(r(&mut i, format!("put IsFolder(\"{d}/Sub\") && IsFolder(\"{d}/Notes\")")), "true false");
    assert_eq!(r(&mut i, format!("stdFile \"{d}/Notes\"\nput the result")), "");
    assert_eq!(i.dialog_folder.as_deref(), Some(dir.as_path()));
    assert_eq!(r(&mut i, format!("defaultFolder \"{d}/Sub\"\nput GetVolume()")), super::paths::folder(&dir.join("Sub")));
    assert!(r(&mut i, format!("defaultFolder \"{d}/None/X\"\nput the result")).starts_with("Error : No folder"));
    // Rename and move.
    assert_eq!(r(&mut i, format!("FullRename \"{d}/Copy\", \"Copy 2\"\nput the result")), "");
    assert_eq!(r(&mut i, format!("FullMove \"{d}/Copy 2\", \"{d}/Sub\"\nput the result")), "");
    assert_eq!(r(&mut i, format!("put GetDir(\"{d}\", \"FR\", \"o=n\")")), "Notes\nCopy 2");
    // Finder information: 23 items.
    let info = r(&mut i, format!("put GetFInfo(\"{d}/Notes\")"));
    let items: Vec<&str> = info.split(',').collect();
    assert_eq!((items.len(), items[0], items[4], items[5]), (23, "????", "5", "5"));
    // Delete.
    assert_eq!(r(&mut i, format!("EraseFile \"{d}/Sub/Copy 2\"\nput the result")), "");
    assert_eq!(r(&mut i, format!("EraseFile \"{d}/Sub/Copy 2\"\nput the result")), "Error : File not found");
    // Each answers "?" with its syntax and "!" with its notice.
    assert_eq!(run(&mut i, "put GetDir(\"?\")"), "GetDir(<folder>[, <mode>][, <filter 1>...[, <filter N>]])");
    assert_eq!(run(&mut i, "FileCopy \"!\"\nput word 2 of the result"), "2.0");
    assert!(!run(&mut i, "put GetBootVol()").is_empty());
    assert_eq!(run(&mut i, "put leafName(\"/home/ada/Notes\") & \",\" & leafName(\"Disk:Stacks:Home\") & \",\" & leafName(\"/home/ada/\")"), "Notes,Home,ada");
    assert_eq!(run(&mut i, "put selectDir(\"Which?\") & fileN(\"Which?\") & folderPath(\"Which?\")"), "");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn forms_from_apples_develop_stacks() {
    use crate::hypertalk::parser::parse_script;
    // The develop stacks' index: the Scroll palette, a card and stack named
    // with &&, and a handler with a period in its name.
    assert!(parse_script("on openStack\n  hide scroll window\n  show scroll window\nend openStack").is_ok());
    assert!(parse_script("function BuildQuery.old\n  return 1\nend BuildQuery.old").is_ok());
    // develop Index: a menu named by an expression in parentheses.
    let mut i = interp_with(stack());
    run(&mut i, "create menu \"Extras\"\nput \"Extras\" into m\ndelete menu (m)");
    assert_eq!(run(&mut i, "put there is a menu \"Extras\""), "false");
    assert!(!run(&mut i, "hide groups\nshow groups\nput XVersion(1)").starts_with("ERROR"));
    // Macintosh Reference: a chunk kept in a variable, selected again.
    let mut i = interp_with(stack());
    run(&mut i, "select char 2 to 4 of field 1\nput the selectedChunk into c\nselect empty\nselect c");
    assert_eq!(run(&mut i, "put the selection"), "ppl");
    // develop's resize button: Command-Shift-E, then a click on the button
    // itself, which goes to the window's size box, not back to the button.
    let card = lock(&i.world).card_id;
    set_script(&i, ObjRef::Part { layer: Layer::Card, id: 11, card }, "on mouseDown\n  type \"e\" with commandKey, shiftKey\n  click at 50,75\nend mouseDown");
    assert!(!run(&mut i, "click at 50,75").starts_with("ERROR"));
    // develop: `select ""` clears the selection.
    assert_eq!(run(&mut i, "select \"\"\nput the selection"), "");
    // `the card window`, and its scroll.
    let mut i = interp_with(stack());
    run(&mut i, "set the rect of the card window to 0,40,512,382");
    assert_eq!(run(&mut i, "put the top of the cd window & \":\" & the scroll of cd window"), "40:0,0");
    run(&mut i, "set the scroll of card window to 0,20");
    // A window the card's size can't scroll.
    assert_eq!(run(&mut i, "put the scroll of card window"), "0,0");
    // HIN Stack 2.0: a menu message written without quotes.
    let mut i = interp_with(stack());
    run(&mut i, "create menu \"Extras\"\nput \"Search\" after menu \"Extras\" with menuMessages go stack \"Search Knowledge Base\"");
    let msg = lock(&i.world).menus.iter().find(|m| m.name == "Extras").map(|m| m.items[0].message.clone());
    assert_eq!(msg.as_deref(), Some("go stack \"Search Knowledge Base\""));
    let mut s = stack();
    s.cards[1].name = "page 7".into();
    let mut i = interp_with(s);
    assert_eq!(run(&mut i, "put 7 into n\ngo cd \"page\" && n of stack \"Test\"\nput the short name of this card"), "page 7");
}

#[test]
fn an_empty_loop_can_be_stopped() {
    let mut s = stack();
    s.script = "on openStack\n  repeat until the mouse is down\n  end repeat\nend openStack".into();
    let mut i = interp_with(s);
    let abort = i.abort.clone();
    let stopper = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        abort.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    i.handle_event(Event::Startup);
    stopper.join().unwrap();
}

/// Print every script in a stack, with line numbers, for bug hunting:
/// RANDOMCARD_DUMP=path cargo test --release dump_scripts -- --ignored --nocapture
#[test]
#[ignore]
fn dump_scripts() {
    let Ok(path) = std::env::var("RANDOMCARD_DUMP") else { return };
    let s = crate::import::load_any(std::path::Path::new(&path), None).expect("can't open the stack");
    let show = |what: String, script: &str| {
        if script.trim().is_empty() {
            return;
        }
        println!("== {what}");
        // RANDOMCARD_DUMP_ERRORS: only the scripts that don't parse, with the
        // line each stops at.
        let errors_only = std::env::var("RANDOMCARD_DUMP_ERRORS").is_ok();
        match crate::hypertalk::parser::parse_script(script) {
            Err(e) => {
                println!("   PARSE ERROR: {e}");
                let line: Option<usize> = e.to_string().strip_prefix("line ").and_then(|r| r.split(':').next()).and_then(|n| n.parse().ok());
                if let Some(n) = line {
                    for (i, l) in script.lines().enumerate().skip(n.saturating_sub(3)).take(5) {
                        println!("   {}{:4} {l}", if i + 1 == n { ">" } else { " " }, i + 1);
                    }
                }
            }
            Ok(_) if errors_only => return,
            Ok(_) => {}
        }
        if errors_only {
            return;
        }
        for (n, l) in script.lines().enumerate() {
            println!("{:4} {l}", n + 1);
        }
    };
    show(format!("stack \"{}\"", s.name), &s.script);
    for b in &s.backgrounds {
        show(format!("bkgnd id {} \"{}\"", b.id, b.name), &b.script);
        for p in &b.parts {
            show(format!("bkgnd id {} part id {} \"{}\"", b.id, p.id, p.name), &p.script);
        }
    }
    for (n, c) in s.cards.iter().enumerate() {
        show(format!("card {} id {} \"{}\"", n + 1, c.id, c.name), &c.script);
        for p in &c.parts {
            show(format!("card {} part id {} \"{}\"", n + 1, p.id, p.name), &p.script);
        }
    }
}

#[test]
#[ignore]
fn vocabulary_probe() {
    let Ok(path) = std::env::var("RANDOMCARD_VOCAB") else { return };
    std::thread::Builder::new()
        .stack_size(super::SCRIPT_STACK)
        .spawn(move || {
            let text = std::fs::read_to_string(path).unwrap();
            for line in text.lines() {
                let mut f = line.split('\t');
                let (Some(term), Some(cat)) = (f.next(), f.next()) else { continue };
                let name: String = {
                    let t = term.split(['(', '[']).next().unwrap_or(term).trim();
                    t.to_string()
                };
                let tries: Vec<String> = match cat {
                    "Function" => vec![format!("the {name}"), format!("{name}()"), format!("{name}(1)"), format!("{name}(1,2)"), format!("{name}(\"a\")")],
                    "Property" => ["", " of this card", " of this stack", " of card button 1", " of card field 1", " of bkgnd field 1", " of this bkgnd", " of card window", " of msg", " of menu 1", " of menuItem 1 of menu 1", " of HyperCard"]
                        .iter()
                        .map(|o| format!("the {name}{o}"))
                        .collect(),
                    "Constant" => vec![name.clone()],
                    _ => continue,
                };
                let mut answers = Vec::new();
                let mut ok = None;
                for t in &tries {
                    let mut i = interp_with(stack());
                    let r = run(&mut i, &format!("put {t} into x\nput x"));
                    if !r.starts_with("ERROR") {
                        ok = Some(format!("{t} = {:?}", r.chars().take(40).collect::<String>()));
                        break;
                    }
                    answers.push(format!("{t}: {r}"));
                }
                match ok {
                    Some(o) => println!("OK\t{term}\t{cat}\t{o}"),
                    None => println!("FAIL\t{term}\t{cat}\t{}", answers.first().cloned().unwrap_or_default()),
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

/// Parse a concrete example of every command form in HyperCard's syntax
/// summary (form TAB statement, one per line, in RANDOMCARD_COMMANDS):
/// FAIL if it doesn't parse, IGNORED if it parses to nothing.
#[test]
#[ignore]
fn command_forms_probe() {
    let Ok(path) = std::env::var("RANDOMCARD_COMMANDS") else { return };
    for line in std::fs::read_to_string(path).unwrap().lines() {
        let Some((form, stmt)) = line.split_once('\t') else { continue };
        match crate::hypertalk::parser::parse_statements(stmt) {
            Err(e) => println!("FAIL\t{form}\t{stmt}\t{e}"),
            Ok(stmts) => {
                let ignored = stmts.iter().all(|s| matches!(&s.kind, crate::hypertalk::ast::StmtKind::Cmd { cmd: crate::hypertalk::ast::Cmd::NoOp, .. }));
                println!("{}\t{form}\t{stmt}", if ignored { "IGNORED" } else { "OK" });
            }
        }
    }
}

/// Open a stack and run message box lines in order (one per line of
/// RANDOMCARD_SCRIPT), printing each line's result or error: for checking a
/// stack in a sensible order, as a person would use it.
#[test]
#[ignore]
fn drive_stack() {
    let (Ok(path), Ok(script)) = (
        std::env::var("RANDOMCARD_STACK"),
        std::env::var("RANDOMCARD_SCRIPT"),
    ) else {
        return;
    };
    std::thread::Builder::new()
        .stack_size(super::SCRIPT_STACK)
        .spawn(move || {
            let stack = crate::import::load_any(std::path::Path::new(&path), None)
                .expect("can't open the stack");
            let mut i = interp_with(stack);
            give_home(&i);
            let start = std::time::Instant::now();
            // RANDOMCARD_NO_OPEN: skip startUp and openStack, to step
            // through them by hand.
            if std::env::var("RANDOMCARD_NO_OPEN").is_err() {
                i.handle_event(Event::Startup);
            }
            println!("opened in {:?}", start.elapsed());
            for r in lock(&i.world).requests.drain(..) {
                if let UiRequest::ScriptError { msg, obj, line } = r {
                    println!("ERROR at open: {msg} ({obj:?} line {line:?})");
                }
            }
            for line in script.lines().filter(|l| !l.trim().is_empty()) {
                use std::io::Write;
                print!("{line}  =>  ");
                let _ = std::io::stdout().flush();
                println!("{}", run(&mut i, line));
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

/// Run every steps file in RANDOMCARD_DRIVEN (tests/driven): each names its
/// stacks with `path:` lines (relative to RANDOMCARD_DEVCDS) and lists
/// message box lines to run in order. A stack passes if no line gives an
/// error. Writes DRIVEN.tsv in RANDOMCARD_DEVCDS, for tools/working_md.py.
#[test]
#[ignore]
fn driven_checks() {
    let (Ok(dir), Ok(root)) = (
        std::env::var("RANDOMCARD_DRIVEN"),
        std::env::var("RANDOMCARD_DEVCDS"),
    ) else {
        return;
    };
    std::thread::Builder::new()
        .stack_size(super::SCRIPT_STACK)
        .spawn(move || {
            let root = std::path::PathBuf::from(root);
            let mut files: Vec<_> = std::fs::read_dir(&dir)
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .collect();
            files.sort();
            let mut rows = vec!["path\tresult\tsteps\tfirst_error\tchecks".to_string()];
            for file in files {
                let text = std::fs::read_to_string(&file).unwrap();
                let lines = text
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty() && !l.starts_with('#'));
                let (paths, steps): (Vec<&str>, Vec<&str>) =
                    lines.partition(|l| l.starts_with("path:"));
                let checks = file.file_name().unwrap().to_string_lossy().to_string();
                for p in paths {
                    let rel = p["path:".len()..].trim();
                    let first_error = match crate::import::load_any(&root.join(rel), None) {
                        Err(e) => Some(format!("doesn't import: {e}")),
                        Ok(stack) => {
                            let mut i = interp_with(stack);
                            i.handle_event(Event::Startup);
                            lock(&i.world).requests.clear();
                            steps.iter().find_map(|s| {
                                let r = run(&mut i, s);
                                r.starts_with("ERROR").then(|| format!("{s}: {r}"))
                            })
                        }
                    };
                    let result = if first_error.is_some() {
                        "errors"
                    } else {
                        "driven clean"
                    };
                    println!("{result}\t{rel}");
                    rows.push(format!(
                        "{rel}\t{result}\t{}\t{}\t{checks}",
                        steps.len(),
                        first_error.unwrap_or_default().replace(['\t', '\n'], " ")
                    ));
                }
            }
            std::fs::write(root.join("DRIVEN.tsv"), rows.join("\n") + "\n").unwrap();
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn import_and_export_paint() {
    use crate::model::Pixel;
    let dir = std::env::temp_dir().join(format!("randomcard-paint-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // A 3×2 PNG: red, then black, the rest white.
    let mut rgba = vec![255u8; 3 * 2 * 4];
    rgba[..4].copy_from_slice(&[255, 0, 0, 255]);
    rgba[4..8].copy_from_slice(&[0, 0, 0, 255]);
    let png = dir.join("Frog.png");
    std::fs::write(&png, crate::paint::image_file::encode_png(3, 2, &rgba).unwrap()).unwrap();
    let mut i = interp_with(stack());
    let png_name = png.display().to_string();
    // Only with a paint tool, as in HyperCard; otherwise it's in the result.
    let r = run(&mut i, &format!("import paint from file \"{png_name}\"\nput the result"));
    assert!(r.contains("paint tool"), "{r}");
    // (Straight to the interpreter, so the requests are still there.)
    i.handle_event(Event::MsgBox(format!("choose brush tool\nimport paint from file \"{png_name}\"\nput the result")));
    {
        let w = lock(&i.world);
        let pic = &w.stack.card(w.card_id).unwrap().picture;
        assert_eq!(pic.get(0, 0), Pixel::rgba(255, 0, 0, 255));
        assert_eq!(pic.get(1, 0), Pixel::BLACK);
        assert_eq!(pic.get(2, 1), Pixel::WHITE);
        assert_eq!(pic.get(3, 0), Pixel::CLEAR);
        // It's left selected.
        assert_eq!(w.msg, "");
        assert!(w.requests.iter().any(|r| matches!(r, UiRequest::SelectPaint((0, 0, 3, 2)))));
        assert!(w.dirty);
    }
    // Undo takes it away.
    {
        let mut w = lock(&i.world);
        assert!(w.undo());
        assert_eq!(w.stack.card(w.card_id).unwrap().picture.get(0, 0), Pixel::CLEAR);
    }
    let r = run(&mut i, &format!("import paint from file \"{}\"\nput the result", dir.join("None.png").display()));
    assert!(r.starts_with("Can't read"), "{r}");
    std::fs::write(dir.join("Junk.txt"), "not a picture").unwrap();
    let r = run(&mut i, &format!("import paint from file \"{}\"\nput the result", dir.join("Junk.txt").display()));
    assert_eq!(r, "Not a picture RandomCard can read.");
    // Export Paint makes the file and asks the window for the picture.
    let out = dir.join("Card.png");
    i.handle_event(Event::MsgBox(format!("export paint to file \"{}\"\nput the result", out.display())));
    assert_eq!(lock(&i.world).msg, "");
    assert!(out.exists());
    assert!(lock(&i.world).requests.iter().any(|r| matches!(r, UiRequest::ExportPaint(p) if *p == out)));
    let r = run(&mut i, &format!("choose browse tool\nexport paint to file \"{}\"\nput the result", out.display()));
    assert!(r.contains("paint tool"), "{r}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn part_colors() {
    let mut i = interp_with(stack());
    // Black and white until given one.
    assert_eq!(run(&mut i, "put the foreColor of button \"Go\""), "");
    run(&mut i, "set the foreColor of button \"Go\" to \"navy\"");
    run(&mut i, "set the backColor of button \"Go\" to \"255,200,0\"");
    run(&mut i, "set the borderColor of field \"Name\" to \"#FF0000\"");
    assert_eq!(run(&mut i, "put the foreColor of button \"Go\""), "0,0,128");
    assert_eq!(run(&mut i, "put the backColor of button \"Go\""), "255,200,0");
    assert_eq!(run(&mut i, "put the borderColor of field \"Name\""), "255,0,0");
    let r = run(&mut i, "set the backColor of button \"Go\" to \"chartreuse\"");
    assert!(r.contains("Expected a color"), "{r}");
    // Empty goes back to black and white.
    run(&mut i, "set the foreColor of button \"Go\" to empty");
    assert_eq!(run(&mut i, "put the foreColor of button \"Go\""), "");
    // Saved with the stack; parts without colors save as before.
    let json = serde_json::to_string(&lock(&i.world).stack).unwrap();
    assert!(json.contains("\"back_color\":[255,200,0]"));
    assert!(!json.contains("fore_color"));
}

#[test]
fn text_colors() {
    let mut i = interp_with(stack());
    run(&mut i, "put \"one two three\" into field \"Name\"");
    run(&mut i, "set the foreColor of word 2 of field \"Name\" to \"red\"");
    assert_eq!(run(&mut i, "put the foreColor of word 2 of field \"Name\""), "255,0,0");
    assert_eq!(run(&mut i, "put the foreColor of word 1 of field \"Name\""), "");
    assert_eq!(run(&mut i, "put the foreColor of line 1 of field \"Name\""), "mixed");
    // Copy Text keeps it.
    {
        let w = lock(&i.world);
        let card = w.card_id;
        let r = super::world::FieldRange { card, layer: Layer::Background, field: 10, start: 4, end: 7 };
        let clip = w.copy_text(&r);
        assert_eq!(clip.runs[0].style.color, Some([255, 0, 0]));
    }
    // A color for the whole field replaces the words' own.
    run(&mut i, "set the foreColor of field \"Name\" to \"blue\"");
    assert_eq!(run(&mut i, "put the foreColor of line 1 of field \"Name\""), "0,0,255");
    let r = run(&mut i, "set the foreColor of word 1 of field \"Name\" to \"plaid\"");
    assert!(r.contains("Expected a color"), "{r}");
}

#[test]
fn addcolor_commands() {
    let mut i = interp_with(stack());
    let ac = |i: &mut Interp, call: &str| run(i, &format!("addColor {call}\nput the result"));
    // Button 11 ("Go", at 10,60,100,90) in front; a rectangle at the back.
    assert_eq!(ac(&mut i, "addButton,cd,11,\"65535,0,0\",3,-1"), "");
    assert_eq!(ac(&mut i, "addRect,cd,\"0,0,50,50\",\"0,0,65535\",2,0"), "");
    assert_eq!(ac(&mut i, "getButtonIndex,cd,11"), "2");
    assert_eq!(ac(&mut i, "getObjectClicked,cd,\"20,70\""), "2,1");
    assert_eq!(ac(&mut i, "getObjectClicked,cd,\"5,5\""), "1,3");
    assert_eq!(ac(&mut i, "getObjectClicked,cd,\"5,5\",buttonType"), "-1,-1");
    assert_eq!(ac(&mut i, "getObjectColor,cd,2"), "65535,0,0");
    assert_eq!(ac(&mut i, "getObjectBevel,cd,1"), "2");
    assert_eq!(ac(&mut i, "getObjectBounds,cd,2"), "10,60,100,90");
    ac(&mut i, "changeObjectColor,cd,2,\"0,65535,0\"");
    assert_eq!(ac(&mut i, "getObjectColor,cd,2"), "0,65535,0");
    // Moving to the back puts the button first.
    assert_eq!(ac(&mut i, "moveToBack,cd,2"), "1");
    assert_eq!(ac(&mut i, "getObjectType,cd,1"), "buttonType");
    // A turned-off item isn't clicked, but keeps its place.
    ac(&mut i, "disableObject,cd,1");
    assert_eq!(ac(&mut i, "getObjectClicked,cd,\"20,70\""), "-1,-1");
    assert_eq!(ac(&mut i, "getButtonIndex,cd,11"), "1");
    ac(&mut i, "removeObject,cd,1");
    assert_eq!(ac(&mut i, "getButtonIndex,cd,11"), "-1");
    assert_eq!(ac(&mut i, "getObjectColor,cd,9"), "");
    // Colors of deleted parts go with compact.
    ac(&mut i, "addButton,cd,999,\"0,0,0\",0,-1");
    ac(&mut i, "compact,cd");
    assert_eq!(lock(&i.world).stack.cards[0].colors.len(), 1);
    // Temporary colors last until colorCard.
    ac(&mut i, "colorRect,cd,\"0,0,10,10\",\"65535,65535,0\",0");
    assert_eq!(lock(&i.world).addcolor.temp.len(), 1);
    ac(&mut i, "colorCard");
    assert!(lock(&i.world).addcolor.temp.is_empty());
    // While disabled, calls are ignored.
    ac(&mut i, "disable");
    ac(&mut i, "addRect,cd,\"0,0,5,5\",\"0,0,0\",0,-1");
    ac(&mut i, "enable");
    assert_eq!(lock(&i.world).stack.cards[0].colors.len(), 1);
    ac(&mut i, "remove");
    assert!(lock(&i.world).addcolor.removed_in.is_some());
    ac(&mut i, "install, 16");
    assert_eq!(ac(&mut i, "depth"), "16");
    // The background's own list.
    ac(&mut i, "addField,bg,10,\"65535,65535,0\",0,-1");
    assert_eq!(ac(&mut i, "getFieldIndex,bg,10"), "1");
    assert_eq!(ac(&mut i, "getFieldIndex,cd,10"), "-1");
    assert!(ac(&mut i, "fly,cd").contains("no function"));
    assert!(ac(&mut i, "addRect,xx,\"0,0,5,5\",\"0,0,0\",0,-1").contains("cd or bg"));
}

#[test]
fn rest_from_hypertalk() {
    let base = crate::web::http::test_server::start(|_, path, headers, _| match path {
        "/jokes/random" => (
            200,
            "application/json",
            r#"{"id":"x1","value":"Chuck Norris can unit test an entire application with a single assert.","categories":["dev"],"owner":{"name":"Ada"}}"#.into(),
        ),
        "/whoami" => (200, "text/plain", headers.lines().find(|l| l.to_ascii_lowercase().starts_with("authorization")).unwrap_or("").trim().to_string()),
        _ => (404, "application/json", r#"{"status":404,"error":"Not Found"}"#.into()),
    });
    let mut i = interp_with(stack());
    run(&mut i, &format!("put \"{base}\" into base"));
    // GET, and read the reply's keys.
    assert_eq!(run(&mut i, "put url (base & \"/jokes/random\") into joke\nput the result"), "");
    assert_eq!(run(&mut i, "put key \"value\" of json joke"), "Chuck Norris can unit test an entire application with a single assert.");
    assert_eq!(run(&mut i, "put key \"name\" of key \"owner\" of json joke"), "Ada");
    assert_eq!(run(&mut i, "put key \"categories\" of json joke"), "[\"dev\"]");
    assert_eq!(run(&mut i, "put key \"nope\" of json joke"), "");
    assert_eq!(run(&mut i, "get url (base & \"/jokes/random\")\nput key \"id\" of json it"), "x1");
    // Straight from the reply.
    assert_eq!(run(&mut i, "put key \"id\" of json url (base & \"/jokes/random\")"), "x1");
    // A 404 is in the result, and its body is still there.
    assert_eq!(run(&mut i, "put url (base & \"/jokes/nope\") into reply\nput the result"), "404 Not Found");
    assert_eq!(run(&mut i, "put key \"error\" of json reply"), "Not Found");
    // Headers go with the request; the reply's are kept.
    run(&mut i, "set the httpHeaders to \"Authorization: Bearer abc\"");
    // (Header names aren't case-sensitive; they go out in lowercase.)
    assert_eq!(run(&mut i, "put url (base & \"/whoami\")").to_ascii_lowercase(), "authorization: bearer abc");
    assert!(run(&mut i, "put the responseHeaders").to_ascii_lowercase().contains("content-type: text/plain"));
    run(&mut i, "set the httpTimeout to 5");
    assert_eq!(run(&mut i, "put the httpTimeout"), "5");
    // No server: the result says why, and the value is empty.
    let r = run(&mut i, "put url \"http://127.0.0.1:1/\" into x\nput the result & \"|\" & x");
    assert!(r.ends_with('|') && r.len() > 1, "{r}");
    // Not JSON is an error.
    assert!(run(&mut i, "put key \"a\" of json \"hello\"").contains("Expected JSON"));
    // Arrays: elements from 1, nested in keys, counted, listed.
    run(&mut i, "put \"{\" & quote & \"total\" & quote & \":2,\" & quote & \"result\" & quote & \":[{\" & quote & \"value\" & quote & \":\" & quote & \"one\" & quote & \"},{\" & quote & \"value\" & quote & \":\" & quote & \"two\" & quote & \"}],\" & quote & \"tags\" & quote & \":[\" & quote & \"x\" & quote & \",\" & quote & \"y\" & quote & \"]}\" into found");
    assert_eq!(run(&mut i, "put key \"value\" of element 2 of key \"result\" of json found"), "two");
    assert_eq!(run(&mut i, "put element last of key \"tags\" of json found"), "y");
    assert_eq!(run(&mut i, "put the number of elements of key \"result\" of json found"), "2");
    assert_eq!(run(&mut i, "put the number of keys of json found"), "3");
    assert_eq!(run(&mut i, "put the keys of json found"), "total\nresult\ntags");
    assert_eq!(run(&mut i, "put the elements of key \"tags\" of json found"), "x\ny");
    assert_eq!(run(&mut i, "put 0 into n\nrepeat with j = 1 to the number of elements of key \"tags\" of json found\n  put element j of key \"tags\" of json found into item j of n\nend repeat\nput n"), "x,y");
    // Pretty JSON, for reading; text that isn't JSON shows as it is.
    assert_eq!(run(&mut i, "put the pretty of json \"[1,2]\""), "[\n  1,\n  2\n]");
    assert_eq!(run(&mut i, "put the pretty of json \"plain words\""), "plain words");
    // The status even when it worked, and how long it took.
    assert_eq!(run(&mut i, "get url (base & \"/jokes/random\")\nput the responseStatus"), "200 OK");
    assert!(run(&mut i, "put the responseTime").parse::<f64>().is_ok());
    // URL and form encoding.
    assert_eq!(run(&mut i, "put urlEncode(\"a b+c\") & \"|\" & formEncode(\"a b+c\")"), "a%20b%2Bc|a+b%2Bc");
    assert_eq!(run(&mut i, "put urlDecode(\"a+b\") & \"|\" & formDecode(\"a+b\")"), "a+b|a b");
    // Variables called url, json, key and element still work.
    assert_eq!(run(&mut i, "put 4 into element\nput element * 2"), "8");
    assert_eq!(run(&mut i, "put 1 into url\nput 2 into json\nput 3 into key\nput url + json + key"), "6");
}

#[test]
fn icndb_stack_calls_its_api() {
    fn joke(id: &str, cats: &str) -> String {
        format!(
            r#"{{"categories":[{cats}],"created_at":"2020-01-05 13:42:19","id":"{id}","url":"https://api.chucknorris.io/jokes/{id}","value":"Joke {id}."}}"#
        )
    }
    let base = crate::web::http::test_server::start(|_, path, _, _| match path {
        "/jokes/random" => (200, "application/json", joke("r0", "")),
        "/jokes/random?category=dev&category=food" => (200, "application/json", joke("r2", r#""dev","food""#)),
        "/jokes/categories" => (200, "application/json", r#"["dev","food","science"]"#.into()),
        "/jokes/abc123" => (200, "application/json", joke("abc123", r#""science""#)),
        "/jokes/search?query=computer" => (
            200,
            "application/json",
            format!(r#"{{"total":2,"result":[{},{}]}}"#, joke("s1", ""), joke("s2", r#""dev""#)),
        ),
        _ => (404, "application/json", r#"{"status":404,"error":"Not Found"}"#.into()),
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/ICNDB.rcstack");
    let mut i = interp_with(Stack::load(&path).unwrap());
    run(&mut i, &format!("put \"{base}\" into card field \"Base URL\" of card \"Connection\""));
    // Connection: Test Connection.
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "Connection", "Test Connection"), None);
    assert_eq!(run(&mut i, "put field \"Status\""), "OK");
    // Random Joke: the checklist comes from /jokes/categories on opening.
    run(&mut i, "go to card \"Random Joke\"");
    assert_eq!(run(&mut i, "put card field \"Choices\""), "dev\nfood\nscience");
    assert_eq!(click(&mut i, "Random Joke", "Submit"), None);
    assert_eq!(run(&mut i, "put card field \"ID\""), "r0");
    // Two checked: a joke from either.
    run(&mut i, "put \"✓ dev\" into line 1 of card field \"Choices\"\nput \"✓ food\" into line 2 of card field \"Choices\"");
    assert_eq!(click(&mut i, "Random Joke", "Submit"), None);
    assert_eq!(run(&mut i, "put field \"Request\""), format!("GET {base}/jokes/random?category=dev&category=food"));
    assert_eq!(run(&mut i, "put card field \"Joke\""), "Joke r2.");
    assert_eq!(run(&mut i, "put card field \"Categories\""), "dev,food");
    // Categories.
    run(&mut i, "go to card \"Categories\"");
    assert_eq!(click(&mut i, "Categories", "Submit"), None);
    assert_eq!(run(&mut i, "put card field \"List\""), "dev\nfood\nscience");
    assert_eq!(run(&mut i, "put card field \"Count\""), "3 categories");
    // Search: the result array, one joke a line.
    run(&mut i, "go to card \"Search\"\nput \"computer\" into card field \"Query\"");
    assert_eq!(click(&mut i, "Search", "Submit"), None);
    assert_eq!(run(&mut i, "put card field \"Results\""), "Joke s1.\nJoke s2.");
    assert_eq!(run(&mut i, "put card field \"Total\""), "2 jokes");
    // Joke by ID, and one that isn't there.
    run(&mut i, "go to card \"Joke by ID\"\nput \"abc123\" into card field \"Joke ID\"");
    assert_eq!(click(&mut i, "Joke by ID", "Submit"), None);
    assert_eq!(run(&mut i, "put card field \"Categories\""), "science");
    run(&mut i, "put \"nope\" into card field \"Joke ID\"");
    assert_eq!(click(&mut i, "Joke by ID", "Submit"), None);
    assert_eq!(run(&mut i, "put field \"Status\""), "404 Not Found");
    assert_eq!(run(&mut i, "put card field \"Joke\""), "");
}

#[test]
fn dont_save_fields_open_empty() {
    let dir = std::env::temp_dir().join(format!("randomcard-dontsave-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Results.rcstack");
    let mut s = stack();
    s.path = Some(path.clone());
    let mut i = interp_with(s);
    run(&mut i, "put \"typed\" into field \"Name\"\nset the dontSave of field \"Name\" to true");
    assert_eq!(run(&mut i, "put the dontSave of field \"Name\""), "true");
    run(&mut i, "doMenu \"Save Stack\"");
    // Still there while the stack is open, empty when it opens again.
    assert_eq!(run(&mut i, "put field \"Name\""), "typed");
    let back = Stack::load(&path).unwrap();
    assert_eq!(back.field_text(back.cards[0].id, Layer::Background, 10), "");
    assert!(back.part(back.cards[0].id, Layer::Background, 10).unwrap().dont_save);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_http_verb_from_hypertalk() {
    let base = crate::web::http::test_server::start(|method, path, _, body| match path {
        "/echo" => (200, "text/plain", format!("{method}:{body}")),
        _ => (404, "text/plain", "gone".into()),
    });
    let mut i = interp_with(stack());
    run(&mut i, &format!("put \"{base}/echo\" into u"));
    let verb = |i: &mut Interp, line: &str| run(i, &format!("{line}\nput it & \"|\" & the result"));
    assert_eq!(verb(&mut i, "post \"a=1\" to url u"), "POST:a=1|");
    assert_eq!(verb(&mut i, "put \"whole\" to url u"), "PUT:whole|");
    assert_eq!(verb(&mut i, "patch \"part\" to url u"), "PATCH:part|");
    assert_eq!(verb(&mut i, "query \"name=chuck\" to url u"), "QUERY:name=chuck|");
    assert_eq!(verb(&mut i, "delete url u"), "DELETE:|");
    assert_eq!(verb(&mut i, "request \"OPTIONS\" to url u"), "OPTIONS:|");
    // Methods are case-sensitive (RFC 9110): sent as written.
    assert_eq!(verb(&mut i, "request \"options\" to url u"), "options:|");
    assert_eq!(verb(&mut i, "put \"PATCH\" into m\nrequest m with \"x\" to url u"), "PATCH:x|");
    // The URL is a whole expression; a 404 is in the result.
    assert_eq!(verb(&mut i, &format!("post \"b\" to url \"{base}\" & \"/nope\"")), "gone|404 Not Found");
    // Plain put and delete are unchanged, and handlers named post or
    // query still get their messages.
    assert_eq!(run(&mut i, "put \"x\" into v\nput \"y\" after v\nput v"), "xy");
    let card = lock(&i.world).card_id;
    set_script(&i, ObjRef::Card(card), "on query q\n  put \"asked\" && q into msg\nend query\non post\n  put \"posted\" into msg\nend post");
    assert_eq!(run(&mut i, "query \"z\""), "asked z");
    assert_eq!(run(&mut i, "post"), "posted");
    assert!(run(&mut i, "request \"BAD METHOD\" to url u\nput the result").contains("isn't an HTTP method"));
}

#[test]
fn writing_json_from_hypertalk() {
    let mut i = interp_with(stack());
    run(&mut i, "put \"{}\" into body");
    run(&mut i, "set key \"name\" of json body to \"Ada\"");
    run(&mut i, "set key \"age\" of json body to 36");
    run(&mut i, "set element 1 of key \"tags\" of json body to \"math\"");
    run(&mut i, "set element 2 of key \"tags\" of json body to \"poetry\"");
    run(&mut i, "set key \"city\" of key \"home\" of json body to \"London\"");
    assert_eq!(run(&mut i, "put body"), r#"{"name":"Ada","age":36,"tags":["math","poetry"],"home":{"city":"London"}}"#);
    // Read back what was written; change in place.
    assert_eq!(run(&mut i, "set key \"age\" of json body to 37\nput key \"age\" of json body"), "37");
    // Into a field, too.
    run(&mut i, "put \"[]\" into field \"Name\"\nset element 1 of json field \"Name\" to \"first\"");
    assert_eq!(run(&mut i, "put field \"Name\""), r#"["first"]"#);
    // Past the end of an array (other than just past it) is an error.
    run(&mut i, "put \"[1]\" into nums");
    assert!(run(&mut i, "set element 5 of json nums to 1").contains("no element 5"));
    // An unset variable starts empty.
    assert_eq!(run(&mut i, "set key \"a\" of json fresh to 1\nput fresh"), r#"{"a":1}"#);
}

#[test]
fn postcard_stack_sends_requests() {
    // Echoes the request back as JSON, as httpbin does.
    let base = crate::web::http::test_server::start(|method, path, headers, body| {
        let ctype = headers
            .lines()
            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-type:").map(|v| v.trim().to_string()))
            .unwrap_or_default();
        let token = headers.to_ascii_lowercase().contains("x-token: 7");
        let echo = serde_json::json!({"method": method, "path": path, "type": ctype, "token": token, "body": body});
        let status = if method == "QUERY" { 405 } else { 200 };
        (status, "application/json", echo.to_string())
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/PostCard.rcstack");
    let mut i = interp_with(Stack::load(&path).unwrap());
    assert_eq!(run(&mut i, "put the number of cards"), "6");
    let send = |i: &mut Interp, card: &str, url_path: &str| {
        run(i, &format!("go to card \"{card}\"\nput \"{base}{url_path}\" into field \"URL\""));
        assert_eq!(click(i, card, "Send"), None);
    };
    // Several values: the name repeats, each part encoded.
    send(&mut i, "Several values", "/get");
    assert_eq!(run(&mut i, "put key \"path\" of json field \"Reply\""), "/get?color=blue&color=green&color=red");
    assert_eq!(run(&mut i, "put field \"Status\""), "200 OK");
    assert!(run(&mut i, "put field \"Time\"").ends_with(" ms"));
    // Pretty: laid out over several lines.
    assert!(run(&mut i, "put the number of lines of field \"Reply\"").parse::<usize>().unwrap() > 3);
    // Post JSON: the body and its type; headers from the card.
    send(&mut i, "Post JSON", "/post");
    run(&mut i, "put field \"Headers\" & return & \"X-Token: 7\" into field \"Headers\"");
    assert_eq!(click(&mut i, "Post JSON", "Send"), None);
    assert_eq!(run(&mut i, "put key \"method\" of json field \"Reply\""), "POST");
    assert_eq!(run(&mut i, "put key \"type\" of json field \"Reply\""), "application/json");
    assert_eq!(run(&mut i, "put key \"token\" of json field \"Reply\""), "true");
    assert_eq!(run(&mut i, "put key \"born\" of json key \"body\" of json field \"Reply\""), "1815");
    // A method typed in, and a status that isn't OK.
    send(&mut i, "Query", "/anything");
    assert_eq!(run(&mut i, "put field \"Status\""), "405 Method Not Allowed");
    assert_eq!(run(&mut i, "put field \"Sent\""), format!("QUERY {base}/anything"));
    // No URL: it asks for one.
    run(&mut i, "put empty into field \"URL\"");
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "Query", "Send"), None);
}

#[test]
fn requests_that_dont_wait() {
    let base = crate::web::http::test_server::start(|method, path, _, body| match path {
        "/slow" => {
            std::thread::sleep(std::time::Duration::from_millis(150));
            (200, "application/json", r#"{"value":"late joke"}"#.into())
        }
        "/echo" => (201, "text/plain", format!("{method}:{body}")),
        _ => (404, "text/plain", "gone".into()),
    });
    let mut i = interp_with(stack());
    let (tx, rx) = std::sync::mpsc::channel();
    i.event_tx = Some(tx);
    let card = lock(&i.world).card_id;
    set_script(
        &i,
        ObjRef::Card(card),
        "on gotIt theURL, reply\n  put key \"value\" of json reply && \"from\" && theURL && the responseStatus into field \"Name\"\nend gotIt\non posted theURL, reply\n  put reply && the result && the responseStatus into field \"Name\"\nend posted",
    );
    run(&mut i, &format!("put \"{base}\" into base"));
    // The script goes on at once; the reply comes later as a message.
    assert_eq!(run(&mut i, "load url (base & \"/slow\") with message \"gotIt\"\nput \"went on\""), "went on");
    assert_eq!(run(&mut i, "put field \"Name\""), "Apple");
    let reply = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("a reply");
    i.handle_event(reply);
    assert_eq!(run(&mut i, "put field \"Name\""), format!("late joke from {base}/slow 200 OK"));
    // Any verb can wait for its reply this way.
    run(&mut i, "post \"hi\" to url (base & \"/echo\") with message \"posted\"");
    let reply = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("a reply");
    i.handle_event(reply);
    assert_eq!(run(&mut i, "put field \"Name\""), "POST:hi  201 Created");
    // A handler named load still gets its message.
    set_script(&i, ObjRef::Card(card), "on load x\n  put \"loaded\" && x into msg\nend load");
    assert_eq!(run(&mut i, "load 3"), "loaded 3");
    assert!(run(&mut i, "load url \"x\"").contains("with message"));
}

#[test]
fn the_tours_web_card_sends_and_waits() {
    let base = crate::web::http::test_server::start(|method, _, _, body| (200, "application/json", format!(r#"{{"method":"{method}","body":"{body}"}}"#)));
    let mut i = interp_with(Stack::demo());
    let (tx, rx) = std::sync::mpsc::channel();
    i.event_tx = Some(tx);
    i.handle_event(Event::Startup);
    run(&mut i, &format!("go to card \"The Web\"\nput \"{base}/x\" into card field \"URL\"\nput \"PUT\" into card field \"Method\"\nput \"hello\" into card field \"Body\""));
    assert_eq!(click(&mut i, "The Web", "Send"), None);
    assert_eq!(run(&mut i, "put card field \"Status\""), "Waiting…");
    let reply = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("a reply");
    i.handle_event(reply);
    assert!(run(&mut i, "put card field \"Status\"").starts_with("200 OK in "));
    assert_eq!(run(&mut i, "put card field \"Reply\""), r#"{"method":"PUT","body":"hello"}"#);
}

#[test]
fn the_selected_button_of_a_family() {
    let mut s = stack();
    for (id, name) in [(31, "Easy"), (32, "Hard")] {
        let mut b = Part::new_button(id, name, Rect::new(10, 100, 80, 120));
        b.family = 2;
        s.cards[0].parts.push(b);
    }
    let mut bg = Part::new_button(41, "Loud", Rect::new(10, 130, 80, 150));
    bg.family = 1;
    s.backgrounds[0].parts.push(bg);
    let mut i = interp_with(s);
    assert_eq!(run(&mut i, "put the selectedButton of family 2"), "");
    run(&mut i, "set the hilite of button \"Hard\" to true");
    // Card button 3: "Go" is the first card button.
    assert_eq!(run(&mut i, "put the selectedButton of family 2"), "card button 3");
    assert_eq!(run(&mut i, "put the selectedButton of card family 2 of card 1"), "card button 3");
    run(&mut i, "set the hilite of bkgnd button \"Loud\" to true");
    assert_eq!(run(&mut i, "put the selectedButton of bkgnd family 1"), "bkgnd button 1");
    // The descriptor works as an object, as scripts use it.
    assert_eq!(run(&mut i, "do \"put the short name of\" && the selectedButton of family 2"), "Hard");
}

#[test]
fn text_width() {
    let mut i = interp_with(stack());
    let plain: u32 = run(&mut i, "put textWidth(\"Hello\", \"Geneva\", 12, \"plain\")").parse().unwrap();
    let bold: u32 = run(&mut i, "put textWidth(\"Hello\", \"Geneva\", 12, \"bold\")").parse().unwrap();
    let big: u32 = run(&mut i, "put textWidth(\"Hello\", \"Geneva\", 24, \"plain\")").parse().unwrap();
    assert!(plain > 20 && bold > plain && big > plain * 3 / 2, "{plain} {bold} {big}");
    // Geneva 12 plain unless given.
    assert_eq!(run(&mut i, "put textWidth(\"Hello\")"), plain.to_string());
    // A chunk of a field: in its own font, size and style.
    run(&mut i, "put \"Hello there\" into field \"Name\"\nset the textSize of field \"Name\" to 12\nset the textFont of field \"Name\" to \"Geneva\"");
    assert_eq!(run(&mut i, "put the textWidth of word 1 of field \"Name\""), plain.to_string());
    run(&mut i, "set the textStyle of word 1 of field \"Name\" to bold");
    assert_eq!(run(&mut i, "put the textWidth of word 1 of field \"Name\""), bold.to_string());
}

#[test]
fn keys_held_and_flushed() {
    use super::world::KeyAction;
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "put the keysDown"), "");
    lock(&i.world).keys_down = vec!["left".into(), "space".into()];
    assert_eq!(run(&mut i, "put the keysDown"), "left,space");
    assert_eq!(run(&mut i, "if \"left\" is in the keysDown then put \"moving\""), "moving");
    // Keys typed before `flush keys` are dropped; later ones arrive.
    let card = lock(&i.world).card_id;
    set_script(&i, ObjRef::Card(card), "on keyDown k\n  put k after msg\nend keyDown");
    run(&mut i, "put empty into msg");
    let typed_before = std::time::Instant::now();
    run(&mut i, "flush keys");
    let key = |at| Event::Key { target: ObjRef::Card(card), message: "keyDown".into(), args: vec!["x".into()], default: KeyAction::Nothing, at };
    i.handle_event(key(typed_before));
    assert_eq!(lock(&i.world).msg, "");
    i.handle_event(key(std::time::Instant::now()));
    assert_eq!(lock(&i.world).msg, "x");
    // A handler named flush still gets its message.
    set_script(&i, ObjRef::Card(card), "on flush\n  put \"flushed\" into msg\nend flush");
    assert_eq!(run(&mut i, "flush"), "flushed");
}

/// Import a stack and report its songs, rendering a second of each:
/// RANDOMCARD_SONGS=path cargo test --release stack_songs -- --ignored --nocapture
#[test]
#[ignore]
fn stack_songs() {
    let Ok(path) = std::env::var("RANDOMCARD_SONGS") else { return };
    let s = crate::import::load_any(std::path::Path::new(&path), None).expect("can't open the stack");
    for (id, song) in &s.songs {
        let src = crate::sound::song::SongSource::new(&song.data, 44_100).expect("a song");
        let second: Vec<f32> = src.take(88_200).collect();
        let loudest = second.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        println!("song {id} {:?}: {} bytes, loudest sample in its first second {loudest:.3}", song.name, song.data.len());
    }
}

#[test]
fn songs_save_and_play() {
    let mut s = stack();
    // Not a real module: what matters here is storing and finding it.
    s.songs.insert(1000, crate::model::Song { name: "title music".into(), data: vec![1, 2, 3, 250] });
    let json = s.to_json().unwrap();
    let back: Stack = serde_json::from_str(&json).unwrap();
    assert_eq!(back.songs, s.songs);
    assert!(back.find_song("Title Music").is_some());
    // play finds it as a song, not a sound; this one can't be played.
    let mut i = interp_with(back);
    let r = run(&mut i, "play \"title music\"\nput the result");
    assert!(r.contains("Not a song") || r.is_empty(), "{r}");
    assert!(!run(&mut i, "put the result").contains("Can't find sound"));
    // Stacks without songs save as before.
    assert!(!stack().to_json().unwrap().contains("\"songs\""));
}

/// Import a stack and show its bitmap fonts, with a few characters drawn
/// in text: RANDOMCARD_FONTS=path RANDOMCARD_CHARS=abc cargo test --release stack_fonts -- --ignored --nocapture
#[test]
#[ignore]
fn stack_fonts() {
    let Ok(path) = std::env::var("RANDOMCARD_FONTS") else { return };
    let chars = std::env::var("RANDOMCARD_CHARS").unwrap_or_else(|_| "AB".into());
    let s = crate::import::load_any(std::path::Path::new(&path), None).expect("can't open the stack");
    for f in &s.fonts {
        let Some(font) = f.parse() else {
            println!("{} {}: can't read", f.family, f.size);
            continue;
        };
        println!("{} {}: ascent {} descent {} leading {}, {} characters", font.family, font.size, font.ascent, font.descent, font.leading, font.glyphs.len());
        for c in chars.chars() {
            let Some(g) = font.glyphs.get(&c) else { continue };
            println!("  {c:?} advance {} left {} width {}", g.advance, g.left, g.width);
            for row in g.pixels.chunks(g.width.max(1) as usize) {
                println!("    {}", row.iter().map(|p| if *p { '#' } else { '.' }).collect::<String>());
            }
        }
    }
}

/// Import a HyperCard stack (or open any stack) and save it as a RandomCard
/// stack: RANDOMCARD_CONVERT=in RANDOMCARD_OUT=out.rcstack cargo test --release convert_stack -- --ignored
#[test]
#[ignore]
fn convert_stack() {
    let (Ok(input), Ok(output)) = (std::env::var("RANDOMCARD_CONVERT"), std::env::var("RANDOMCARD_OUT")) else { return };
    let s = crate::import::load_any(std::path::Path::new(&input), None).expect("can't open the stack");
    s.save(std::path::Path::new(&output)).expect("can't save");
    println!("{} cards, {} backgrounds, {} fonts, {} songs, {} sounds -> {output}", s.cards.len(), s.backgrounds.len(), s.fonts.len(), s.songs.len(), s.sounds.len());
}

#[test]
fn a_parameter_named_message() {
    let mut i = interp_with(stack());
    let card = lock(&i.world).card_id;
    set_script(&i, ObjRef::Card(card), "on relay message, target\n  put message into field \"Name\"\n  send message to target\nend relay\non hello\n  put \"hello came\" into msg\nend hello");
    run(&mut i, "put \"box text\" into msg");
    assert_eq!(run(&mut i, "relay \"hello\", \"this card\"\nput msg"), "hello came");
    assert_eq!(run(&mut i, "put field \"Name\""), "hello");
}

#[test]
fn picture_in_a_card() {
    // HyperTalk takes "in" for "of" (Adventures of Sean).
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "hide picture in card 2\nput the showPict of card 2"), "false");
    assert_eq!(run(&mut i, "show picture of card 2\nput the showPict of card 2"), "true");
}

#[test]
fn message_box_variables_are_globals() {
    // HyperCard: a variable used in the message box is a global.
    let mut i = interp_with(stack());
    set_script(&i, ObjRef::Stack, "on setIt\nglobal score\nput 12 into score\nend setIt\nfunction score\nglobal score\nreturn score\nend score");
    assert_eq!(run(&mut i, "setIt\nput score"), "12");
    assert_eq!(run(&mut i, "put 30 into score\nput score"), "30");
    assert_eq!(run(&mut i, "put score()"), "30");
    // One that was never set is still its own name.
    assert_eq!(run(&mut i, "put nothingHere"), "nothingHere");
}

#[test]
fn the_selected_text_of_a_field() {
    // HyperCard 2.2: a list field's chosen line, asked of the field.
    let mut i = interp_with(stack());
    run(&mut i, "put \"red\" & return & \"green\" & return & \"blue\" into field \"Name\"");
    run(&mut i, "select line 2 of field \"Name\"");
    assert_eq!(run(&mut i, "put the selectedText of field \"Name\""), "green");
    assert_eq!(run(&mut i, "put the selectedLine of field \"Name\""), "line 2 of bkgnd field 1");
    assert_eq!(run(&mut i, "put the selectedChunk of field \"Name\""), "char 5 to 9 of bkgnd field 1");
    // Nothing chosen in it: empty.
    run(&mut i, "select empty");
    assert_eq!(run(&mut i, "put the selectedText of field \"Name\""), "");
}


/// What HyperGames needed, each checked against HyperCard 2.4.1.
#[test]
fn hypergames_language() {
    let mut i = interp_with(stack());
    // "is no integer" is "is not an integer".
    assert_eq!(run(&mut i, "put (\"5\" is no integer) && (\"x\" is no integer) && (\"x\" is no number)"), "false true true");
    // random of 0 or less is 1.
    assert_eq!(run(&mut i, "put random(0) && random(-3)"), "1 1");
    // A variable before parentheses is the sound, the parentheses the notes.
    assert_eq!(run(&mut i, "put \"boing\" into theSound\nplay theSound (\"c4\" & \"e\")\nput \"played\""), "played");
    // Whether a card or background has any painting.
    assert_eq!(run(&mut i, "put (there is a cd picture) && (there is no bg picture)"), "false true");
    // The paint spacing for Draw Multiple: 1 until set.
    assert_eq!(run(&mut i, "put the multiSpace"), "1");
    assert_eq!(run(&mut i, "set the multiSpace to 16\nput the multiSpace"), "16");
}

#[test]
fn the_found_chunk_as_a_chunk() {
    // Story Machine: style the found text, then replace it.
    let mut i = interp_with(stack());
    let id = lock(&i.world).card_id;
    lock(&i.world).stack.card_mut(id).unwrap().parts.push(Part::new_field(30, "Answer", Rect::new(10, 100, 200, 200)));
    run(&mut i, "put \"Once --@hero@-- went\" into cd fld \"Answer\"");
    assert_eq!(run(&mut i, "find string \"--@hero@--\" in cd fld \"Answer\"\nput the foundChunk"), "char 6 to 15 of card field 1");
    run(&mut i, "set the textStyle of the foundChunk to \"bold\"");
    assert_eq!(run(&mut i, "put the textStyle of char 7 of cd fld \"Answer\""), "bold");
    assert_eq!(run(&mut i, "put \"Sam\" into the foundChunk\nput cd fld \"Answer\""), "Once Sam went");
}

#[test]
fn names_of_spaces_and_tabs() {
    // Tic-Tac-Toe names its squares tab & "X" & tab: the target finds them.
    let mut i = interp_with(stack());
    let id = lock(&i.world).card_id;
    for name in [" ", "\tX\t"] {
        lock(&i.world).stack.card_mut(id).unwrap().parts[0].name = name.into();
        set_script(&i, ObjRef::Part { layer: Layer::Card, id: 11, card: id }, "on mouseUp\nset the name of the target to (tab & \"O\" & tab)\nput the short name of me\nend mouseUp");
        assert_eq!(run(&mut i, "send mouseUp to card button id 11"), "\tO\t");
    }
}

#[test]
fn sound_channels() {
    // HyperCard 2.4: `play` uses the current channel, so a chord is three.
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "put the soundChannel"), "1");
    run(&mut i, "set soundChannel to 1\nplay \"harpsichord\" c\nset soundChannel to 2\nplay \"harpsichord\" e\nset soundChannel to 3\nplay \"harpsichord\" g");
    assert_eq!(run(&mut i, "put the soundChannel && the sound"), "3 harpsichord");
    // play stop: this channel only; stop sound: all of them.
    assert_eq!(run(&mut i, "play stop\nput the sound"), "done");
    assert_eq!(run(&mut i, "set soundChannel to 1\nput the sound"), "harpsichord");
    assert_eq!(run(&mut i, "stop sound\nput the sound"), "done");
}

#[test]
fn a_variable_named_box() {
    // Get Rich Quick keeps "global box"; HyperCard allows it, and "message
    // box" still means the message box.
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "put 5 into box\nput box + 1"), "6");
    assert_eq!(run(&mut i, "if box = 5 then put \"yes\""), "yes");
    assert_eq!(run(&mut i, "put \"hi\" into message box"), "hi");
    assert_eq!(run(&mut i, "put \"there\" into msg box\nput msg box"), "there");
}

#[test]
fn a_variable_in_a_chunk_position() {
    // Shattered Stone: `line scroll of cd fld "events"`, scroll a variable,
    // not the field's scroll property.
    let mut i = interp_with(stack());
    run(&mut i, "put \"a\" & return & \"b\" into field 1");
    assert_eq!(run(&mut i, "put 2 into scroll\nput \"x\" into line scroll of field 1\nput line 2 of field 1"), "x");
    assert_eq!(run(&mut i, "put line 1 to scroll of field 1"), "a\nx");
    // Properties without `the` still work elsewhere, and in parentheses.
    assert_eq!(run(&mut i, "put scroll of field 1"), "0");
    assert_eq!(run(&mut i, "put line (number of lines of field 1) of field 1"), "x");
    // Functions do work there (Printer Checklist).
    assert_eq!(run(&mut i, "put \"abcdef\" into g\nput \"cd\" into w\nget offset(w, g)\ndelete char it to it+number of chars in w of g\nput g"), "abf");
    assert_eq!(run(&mut i, "put char 2 to length of \"xyz\" of \"abcdef\""), "bc");
}

#[test]
fn a_bare_colon() {
    // HyperCard takes `set itemDelimiter to :` as the colon (checked in 2.4.1).
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "set itemDelimiter to :\nput item 2 of \"a:b:c\" && the itemDelimiter"), "b :");
}

#[test]
fn home_from_before_loses_import_button() {
    // A Home saved when it had Import a HyperCard Stack… third of five.
    let mut home = Stack::home();
    let card = &mut home.cards[0];
    let new_stack = card.parts.iter().position(|p| p.name == "New Stack…").unwrap();
    let r = card.parts[new_stack].rect;
    let mut import = card.parts[new_stack].clone();
    import.name = "Import a HyperCard Stack…".into();
    import.script = "on mouseUp\n  doMenu \"Import HyperCard Stack…\"\nend mouseUp".into();
    for p in card.parts.iter_mut().filter(|p| p.rect.top >= r.top && p.kind == crate::model::PartKind::Button) {
        p.rect = crate::model::Rect::new(p.rect.left, p.rect.top + 38, p.rect.right, p.rect.bottom + 38);
    }
    card.parts.push(import);
    assert!(crate::model::home::update(&mut home));
    let card = &home.cards[0];
    assert!(!card.parts.iter().any(|p| p.name.starts_with("Import")));
    assert_eq!(card.parts.iter().find(|p| p.name == "New Stack…").unwrap().rect, r);
    // Up to date now: nothing more to change.
    assert!(!crate::model::home::update(&mut home));
}

#[test]
fn setting_the_part_number() {
    // A script can bring a part to the front, or move it back.
    let mut i = interp_with(stack());
    run(&mut i, "doMenu \"New Button\"\nset the name of last button to \"Two\"\ndoMenu \"New Button\"\nset the name of last button to \"Three\"");
    let order = |i: &mut Interp| run(i, "put the short name of button 1 & \",\" & the short name of button 2 & \",\" & the short name of button 3");
    let first = run(&mut i, "put the short name of button 1");
    assert_eq!(order(&mut i), format!("{first},Two,Three"));
    run(&mut i, "set the partNumber of button 1 to 99");
    assert_eq!(order(&mut i), format!("Two,Three,{first}"));
    assert_eq!(run(&mut i, "put the partNumber of button \"Three\""), "2");
    run(&mut i, "set the partNumber of button \"Three\" to 1");
    assert_eq!(order(&mut i), format!("Three,Two,{first}"));
}

#[test]
fn sliders() {
    // A slider is a button style; its contents is its value, kept in its
    // range and on its steps, as HTML's range input does.
    let mut i = interp_with(stack());
    run(&mut i, "set the style of button 1 to slider");
    assert_eq!(run(&mut i, "put the style of button 1 && button 1"), "slider 0");
    assert_eq!(run(&mut i, "put the min of button 1 && the max of button 1 && the step of button 1"), "0 100 1");
    run(&mut i, "set the max of button 1 to 255");
    assert_eq!(run(&mut i, "put 300 into button 1\nput button 1"), "255");
    assert_eq!(run(&mut i, "put -5 into button 1\nput button 1"), "0");
    assert_eq!(run(&mut i, "put 12.4 into button 1\nput button 1"), "12");
    assert_eq!(run(&mut i, "put \"loud\" into button 1\nput button 1"), "0");
    run(&mut i, "set the step of button 1 to 0.5\nput 12.4 into button 1");
    assert_eq!(run(&mut i, "put button 1"), "12.5");
    // A narrower range moves the value into it.
    run(&mut i, "set the max of button 1 to 10");
    assert_eq!(run(&mut i, "put button 1"), "10");
    assert!(run(&mut i, "set the step of button 1 to 0").contains("more than 0"));
    // `the max of` anything but a button is still the function.
    assert_eq!(run(&mut i, "put the max of \"3,7,5\" && max(2, 9)"), "7 9");
    run(&mut i, "put \"4,8,1\" into field 1");
    assert_eq!(run(&mut i, "put the max of field 1"), "8");
    // Fields can't be sliders.
    assert!(run(&mut i, "set the style of field 1 to slider").contains("can't be sliders"));
}

#[test]
fn slider_geometry() {
    use crate::model::{Part, PartStyle, Rect};
    let mut p = Part::new_button(1, "Red", Rect::new(10, 10, 210, 30));
    p.style = PartStyle::Slider;
    p.max = 255.0;
    let (a, b) = p.slider_track();
    assert!(a > 10 && b < 210 && a < b);
    assert_eq!(p.slider_value_at(a), 0.0);
    assert_eq!(p.slider_value_at(b), 255.0);
    assert_eq!(p.slider_value_at(b + 50), 255.0);
    assert_eq!(p.slider_value_at((a + b) / 2), 128.0);
    // A title takes room at the left.
    p.title_width = 50;
    assert!(p.slider_track().0 >= 60);
    // Taller than wide: vertical, the min at the bottom.
    p.rect = Rect::new(10, 10, 30, 210);
    assert!(p.slider_vertical());
    let (bottom, top) = p.slider_track();
    assert!(bottom > top);
    assert_eq!(p.slider_value_at(bottom), 0.0);
    assert_eq!(p.slider_value_at(top), 255.0);
    // Steps that don't divide the range: the last whole step.
    p.max = 10.0;
    p.step = 3.0;
    assert_eq!(p.slider_snap(10.0), 9.0);
    assert_eq!(p.slider_snap(0.1 + 0.2), 0.0);
}

#[test]
fn the_solitaire_stack() {
    let stack = crate::import::load_any(std::path::Path::new("examples/Solitaire.rcstack"), None).unwrap();
    // Every script parses.
    let mut scripts = vec![("stack".to_string(), stack.script.clone())];
    for c in &stack.cards {
        scripts.push((c.name.clone(), c.script.clone()));
        scripts.extend(c.parts.iter().map(|p| (format!("{} {}", c.name, p.name), p.script.clone())));
    }
    for (name, script) in &scripts {
        if let Err(e) = crate::hypertalk::parser::parse_script(script) {
            panic!("{name}: {e:?}");
        }
    }
    // Every card has its face as a color icon.
    assert_eq!(stack.icons.len(), 53);
    assert!(stack.icons.values().all(|i| i.picture.has_color() && i.picture.width == 64));

    let mut i = interp_with(stack);
    run(&mut i, "go to card \"FreeCell\"\nnewGame 1");
    let state = |i: &mut Interp| run(i, "put packState()");
    // Game 1 is Windows FreeCell's game 1.
    assert_eq!(
        state(&mut i),
        "JD,KD,2S,4C,3S,6D,6S/2D,KC,KS,5C,TD,8S,9C/9H,9S,9D,TS,4S,8D,2H/JC,5S,QD,QH,TH,QS,6H/\
         5D,AD,JS,4H,8H,6C/7H,QC,AS,AC,2C,3D/7C,KH,AH,4D,JH,8C/5H,3H,3C,7S,7D,TC|///|///"
    );
    assert_eq!(run(&mut i, "put field \"Status\""), "Game 1   Moves: 0   Cards left: 52");
    assert_eq!(run(&mut i, "put maxMove(false) && maxMove(true)"), "5 5");
    // The 2 of hearts has nowhere better than a free cell.
    assert_eq!(run(&mut i, "put clickMove(\"2H\", \"col,3,7\")"), "");
    assert_eq!(run(&mut i, "put line 1 of gCells && whereIs(\"2H\") && gMoves"), "2H cell,1 1");
    assert_eq!(run(&mut i, "put maxMove(false)"), "4");
    // Not on a card of the wrong rank, or the same color.
    assert_eq!(run(&mut i, "put tryMove(\"6S\", \"col,1,7\", \"col,2\")"), "That card doesn't go there.");
    assert_eq!(run(&mut i, "put tryMove(\"8D\", \"col,3,6\", \"col,2\")"), "");
    assert_eq!(run(&mut i, "put tryMove(\"8C\", \"col,7,6\", \"col,2\")"), "That card doesn't go there.");
    // Only a run moves together.
    assert_eq!(run(&mut i, "put isRun(line 2 of gCols, 7) && isRun(line 2 of gCols, 6)"), "true false");
    // Undo goes back a move at a time.
    run(&mut i, "undoMove");
    assert_eq!(run(&mut i, "put whereIs(\"8D\") && gMoves"), "col,3,6 1");
    run(&mut i, "undoMove");
    assert!(state(&mut i).ends_with("|///|///"));
    assert_eq!(run(&mut i, "undoMove\nput field \"Status\""), "Nothing to undo.");

    // A run moves as one, as far as the free cells and empty columns allow.
    run(&mut i, "unpackState \"9H,KS/8S,7H,6C/QD//////|///|///\"");
    assert_eq!(run(&mut i, "put maxMove(false)"), "160");
    assert_eq!(run(&mut i, "put tryMove(\"8S,7H,6C\", \"col,2,1\", \"col,3\")"), "That card doesn't go there.");
    run(&mut i, "unpackState \"9H/8S,7H,6C/QD//////|AS/AS/AS/AS|///\"");
    assert_eq!(run(&mut i, "put maxMove(false)"), "32");
    run(&mut i, "unpackState \"9H/8S,7H,6C/QD/KC/KC/KC/KC/KC|AS/AS/AS/|///\"");
    assert_eq!(run(&mut i, "put tryMove(\"8S,7H,6C\", \"col,2,1\", \"col,1\")"), "There's room to move 2 cards at once.");
    run(&mut i, "unpackState \"9H/8S,7H,6C/QD/KC/KC/KC/KC/KC|AS/AS//|///\"");
    assert_eq!(run(&mut i, "put tryMove(\"8S,7H,6C\", \"col,2,1\", \"col,1\")"), "");
    assert_eq!(run(&mut i, "put line 1 of gCols & \"/\" & line 2 of gCols"), "9H,8S,7H,6C/");

    // Kings home: the rest follow on their own, and the game is won.
    run(&mut i, "unpackState \"KC/KD/KH/KS//////|///|QC/QD/QH/QS\"\nput 7 into gGame\nput 90 into gMoves");
    assert_eq!(run(&mut i, "put clickMove(\"KC\", \"col,1,1\")"), "");
    assert_eq!(run(&mut i, "put gHomes"), "KC\nKD\nKH\nKS");
    run(&mut i, "checkWin");
    assert_eq!(run(&mut i, "put field \"Status\""), "You won game 7 in 91 moves!");
    // The game is kept in its fields, to be there next time.
    assert_eq!(run(&mut i, "put line 1 of field \"State\""), "///////|///|KC/KD/KH/KS");
    // The win is counted once, and the cards end in a ring around the news.
    assert_eq!(run(&mut i, "put field \"FreeCell Record\" of card \"Games\""), "Won 1, lost 0 (100%). Streak 1, best 1.");
    run(&mut i, "checkWin");
    assert_eq!(run(&mut i, "put field \"Won\""), "1");
    assert_eq!(run(&mut i, "put the visible of field \"Banner\" && the loc of card button \"AC\""), "true 336,107");
    // Leaving a game with moves made, not won, is a loss; a fresh deal isn't.
    run(&mut i, "global gMoves\nabandonGame gMoves\nnewGame 2");
    assert_eq!(run(&mut i, "put the visible of field \"Banner\""), "false");
    run(&mut i, "global gMoves\nabandonGame gMoves\nnewGame 3\nget clickMove(last item of line 1 of gCols, \"col,1,\" & the number of items of line 1 of gCols)");
    assert_eq!(run(&mut i, "put gMoves"), "1");
    run(&mut i, "global gMoves\nabandonGame gMoves\nnewGame 4");
    assert_eq!(run(&mut i, "put field \"FreeCell Record\" of card \"Games\""), "Won 1, lost 1 (50%). Streak 0, best 1.");
    // Edit ▸ Undo undoes a move.
    run(&mut i, "get clickMove(last item of line 1 of gCols, \"col,1,\" & the number of items of line 1 of gCols)");
    run(&mut i, "doMenu \"Undo\"");
    assert_eq!(run(&mut i, "put gMoves"), "0");

    // ----- Klondike -----
    run(&mut i, "go to card \"Klondike\"\nnewGame 1");
    // One to seven cards a column, only the top one face up; 24 in the stock.
    for n in 1..=7 {
        let col = run(&mut i, &format!("put line {n} of gKCols"));
        let items: Vec<&str> = col.split(',').collect();
        assert_eq!(items.len(), n);
        assert!(items[..n - 1].iter().all(|c| c.starts_with('-')) && !items[n - 1].starts_with('-'), "{col}");
    }
    assert_eq!(run(&mut i, "put the number of items of gKStock && gKWaste"), "24 ");
    // The stock deals one, or three with Draw Three; then turns the waste over.
    run(&mut i, "dealStock");
    assert_eq!(run(&mut i, "put the number of items of gKStock && the number of items of gKWaste"), "23 1");
    run(&mut i, "set the hilite of button \"Draw Three\" to true\ndealStock");
    assert_eq!(run(&mut i, "put the number of items of gKStock && the number of items of gKWaste"), "20 4");
    run(&mut i, "set the hilite of button \"Draw Three\" to false");
    run(&mut i, "unpackState \"KH/QS/-5D,JD////|2C,3C|4C|///\"\ndealStock\ndealStock");
    assert_eq!(run(&mut i, "put gKStock & \"|\" & gKWaste"), "|4C,3C,2C");
    run(&mut i, "dealStock");
    assert_eq!(run(&mut i, "put gKStock & \"|\" & gKWaste"), "2C,3C,4C|");
    // Down in alternating colors, kings only to an empty column, and the
    // card left on top turns over.
    assert_eq!(run(&mut i, "put tryMove(\"QS\", \"col,2,1\", \"col,1\")"), "");
    assert_eq!(run(&mut i, "put tryMove(\"JD\", \"col,3,2\", \"col,2\")"), "Only a king can go in an empty column.");
    assert_eq!(run(&mut i, "put tryMove(\"KH,QS\", \"col,1,1\", \"col,3\")"), "That card doesn't go there.");
    assert_eq!(run(&mut i, "put tryMove(\"JD\", \"col,3,2\", \"col,1\")"), "");
    assert_eq!(run(&mut i, "put line 1 of gKCols & \"|\" & line 3 of gKCols"), "KH,QS,JD|5D");
    // The waste's top card goes to a foundation with a click.
    run(&mut i, "unpackState \"KH//////|2C|AH|///\"");
    assert_eq!(run(&mut i, "put whereIs(\"AH\") && pickUp(\"AH\", \"waste\")"), "waste AH");
    assert_eq!(run(&mut i, "put clickMove(\"AH\", \"waste\") & line 1 of gKHomes"), "AH");
    // Kings home, and the game is won.
    run(&mut i, "unpackState \"KC/KD/KH/KS////|||QC/QD/QH/QS\"");
    assert_eq!(run(&mut i, "put clickMove(\"KC\", \"col,1,1\")"), "");
    run(&mut i, "checkWin");
    assert_eq!(run(&mut i, "put field \"Klondike Record\" of card \"Games\""), "Won 1, lost 0 (100%). Streak 1, best 1.");

    // ----- Spider -----
    run(&mut i, "go to card \"Spider\"\nnewGame 1");
    let tall: Vec<String> = (1..=10).map(|n| run(&mut i, &format!("put the number of items of line {n} of gSCols"))).collect();
    assert_eq!(tall, ["6", "6", "6", "6", "5", "5", "5", "5", "5", "5"]);
    assert_eq!(run(&mut i, "put the number of items of gSStock && the number of lines of gSFaces"), "50 104");
    // One suit: every card a spade.
    assert_eq!(run(&mut i, "put \"H\" is in gSFaces or \"D\" is in gSFaces or \"C\" is in gSFaces"), "false");
    run(&mut i, "dealStock");
    assert_eq!(run(&mut i, "put the number of items of gSStock && the number of items of line 10 of gSCols"), "40 6");
    // Known faces: s1 to s13 are the king down to the ace of spades, s14
    // the five of spades and s15 the four of hearts.
    run(&mut i, "repeat with k = 1 to 13\nput char (14 - k) of \"A23456789TJQK\" & \"S\" into line k of gSFaces\nend repeat\nput \"5S\" into line 14 of gSFaces\nput \"4H\" into line 15 of gSFaces");
    run(&mut i, "unpackState \"s1,s2,s3,s4,s5,s6,s7,s8,s9,s10,s11,s12/s13/s14,s15///////|s20|\"");
    // Only a run of one suit moves together.
    assert_eq!(run(&mut i, "put pickUp(\"s14\", \"col,3,1\")"), "");
    assert_eq!(run(&mut i, "put pickUp(\"s11\", \"col,1,11\")"), "s11,s12");
    // The ace finishes king to ace: the run comes off.
    assert_eq!(run(&mut i, "put tryMove(\"s13\", \"col,2,1\", \"col,1\")"), "");
    assert_eq!(run(&mut i, "put gSDone & \"|\" & line 1 of gSCols"), "s1|");
    // The stock won't deal onto an empty column.
    run(&mut i, "dealStock");
    assert_eq!(run(&mut i, "put field \"Status\""), "Every column needs a card before the stock deals.");
    // Eight runs off, and the game is won.
    run(&mut i, "unpackState \"//////////||s1,s14,s27,s40,s53,s66,s79,s92\"\ncheckWin");
    assert_eq!(run(&mut i, "put field \"Spider Record\" of card \"Games\""), "Won 1, lost 0 (100%). Streak 1, best 1.");
}

#[test]
fn the_swatches_stack() {
    // A stand-in for The Color API: colors by hex or rgb, Cobalt the one
    // exact name, everything else nearest to Test Gray.
    let base = crate::web::http::test_server::start(|_, path, _, _| {
        let query = path.split_once('?').map(|(_, q)| q.to_string()).unwrap_or_default();
        let param = |name: &str| query.split('&').find_map(|kv| kv.strip_prefix(&format!("{name}="))).map(String::from);
        if path.starts_with("/scheme") {
            let colors: Vec<String> = (1..=5).map(|i| format!(r##"{{"hex":{{"value":"#{i}{i}{i}{i}{i}{i}"}}}}"##)).collect();
            let mode = param("mode").unwrap_or_default();
            return (200, "application/json", format!(r#"{{"mode":"{mode}","count":"5","colors":[{}]}}"#, colors.join(",")));
        }
        let (r, g, b) = if let Some(h) = param("hex") {
            let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0);
            (v(0), v(2), v(4))
        } else if let Some(rgb) = param("rgb") {
            let n: Vec<u8> = rgb.split(',').map(|x| x.parse().unwrap_or(0)).collect();
            (n[0], n[1], n[2])
        } else {
            (0, 71, 171)
        };
        let hex = format!("#{r:02X}{g:02X}{b:02X}");
        let exact = hex == "#0047AB";
        let name = if exact { "Cobalt" } else { "Test Gray" };
        let near = if exact { "#0047AB" } else { "#808080" };
        let body = format!(
            r##"{{"hex":{{"value":"{hex}"}},"rgb":{{"r":{r},"g":{g},"b":{b},"value":"rgb({r}, {g}, {b})"}},"hsl":{{"value":"hsl(1, 2%, 3%)"}},"hsv":{{"value":"hsv(4, 5%, 6%)"}},"cmyk":{{"value":"cmyk(7, 8, 9, 10)"}},"name":{{"value":"{name}","closest_named_hex":"{near}","exact_match_name":{exact}}},"contrast":{{"value":"#ffffff"}}}}"##
        );
        (200, "application/json", body)
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/Swatches.rcstack");
    let mut i = interp_with(Stack::load(&path).unwrap());
    let (tx, rx) = std::sync::mpsc::channel();
    i.event_tx = Some(tx);
    run(&mut i, &format!("put \"{base}\" into card field \"Base URL\" of card \"About\""));
    i.handle_event(Event::Startup);
    // Replies to requests that don't wait, as they arrive.
    let replies = |i: &mut Interp, n: usize| {
        for _ in 0..n {
            let reply = rx.recv_timeout(std::time::Duration::from_secs(5)).expect("a reply");
            i.handle_event(reply);
        }
    };
    let no_errors = |i: &mut Interp| {
        let errors: Vec<String> = lock(&i.world)
            .requests
            .drain(..)
            .filter_map(|r| match r {
                UiRequest::ScriptError { msg, line, .. } => Some(format!("{msg} (line {line:?})")),
                _ => None,
            })
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
    };

    // The color math in the stack script.
    assert_eq!(run(&mut i, "put rgbToHex(\"0,71,171\") && hexToRGB(\"#0047ab\") && hexToRGB(\"#04A\")"), "#0047AB 0,71,171 0,68,170");
    assert_eq!(run(&mut i, "put rgbToHSV(\"0,71,171\") && hsvToRGB(215, 100, 67) && hsvToRGB(-60, 100, 100)"), "215,100,67 0,71,171 255,0,255");
    assert_eq!(run(&mut i, "put round(contrastRatio(\"0,0,0\", \"255,255,255\")) && textColorFor(\"253,216,53\")"), "21 0,0,0");
    assert_eq!(run(&mut i, "put colorQuery(\" rgb(0, 71, 171) \")"), "rgb=0,71,171");
    assert_eq!(run(&mut i, "put colorQuery(\"#04A\") && colorQuery(\"cmyk(100, 58, 0, 33)\")"), "hex=04A cmyk=100,58,0,33");
    assert_eq!(run(&mut i, "put colorQuery(\"hsl(215, 100%, 34%)\")"), "hsl=215%2C100%25%2C34%25");
    assert_eq!(run(&mut i, "put colorQuery(\"rgb(300, 0, 0)\") & colorQuery(\"zzz\") & colorQuery(\"#12345\")"), "");
    assert_eq!(run(&mut i, "put wheelAt(\"350,265\", 200, 265, 150) && wheelAt(\"200,115\", 200, 265, 150) && wheelAt(\"0,0\", 200, 265, 150)"), "0,100 90,100 ");

    // Convert: an exact name, then a nearest one, and Recent.
    run(&mut i, "go to card \"Convert\"\nput \"#0047AB\" into card field \"Color\"\nlookUp");
    replies(&mut i, 1);
    assert_eq!(run(&mut i, "put card field \"Hex Value\" && card field \"Name Value\""), "#0047AB Cobalt (an exact match)");
    assert_eq!(run(&mut i, "put the backColor of card field \"Swatch\" && the foreColor of card field \"Swatch\""), "0,71,171 255,255,255");
    assert_eq!(run(&mut i, "put the visible of card field \"Nearest\" && line 1 of card field \"Recent\""), "false #0047AB Cobalt");
    run(&mut i, "put \"rgb(128, 129, 130)\" into card field \"Color\"\nlookUp");
    replies(&mut i, 1);
    assert_eq!(run(&mut i, "put card field \"Name Value\" && card field \"Nearest\""), "nearest named: Test Gray Test Gray #808080");
    assert_eq!(run(&mut i, "put the visible of card field \"Nearest\" && the number of lines of card field \"Recent\""), "true 2");
    assert_eq!(click(&mut i, "Convert", "Copy Hex"), None);
    assert_eq!(run(&mut i, "put bkgnd field \"Status\""), "Copied #808182.");
    assert_eq!(click(&mut i, "Convert", "Random"), None);
    replies(&mut i, 1);
    assert_eq!(run(&mut i, "put the number of lines of card field \"Recent\""), "3");
    i.canned_answers.push(DialogReply::Button("OK".into()));
    run(&mut i, "put \"not a color\" into card field \"Color\"\nlookUp");
    no_errors(&mut i);

    // Mixing Light: the sliders follow the color, and a lesson mixes one.
    run(&mut i, "go to card \"Mixing Light\"");
    replies(&mut i, 1);
    assert_eq!(click(&mut i, "Mixing Light", "Red + Green"), None);
    replies(&mut i, 1);
    assert_eq!(run(&mut i, "put line 1 of card field \"Swatch\" && card button \"Green\" && card field \"Green Value\""), "#FFFF00 255 255");
    assert_eq!(run(&mut i, "put card field \"Name\""), "The nearest named color is Test Gray (#808080).");
    run(&mut i, "put 300 into card field \"Blue Value\"\napplyValue \"Blue Value\"");
    replies(&mut i, 1);
    assert_eq!(run(&mut i, "put card field \"Blue Value\" && line 1 of card field \"Swatch\""), "255 #FFFFFF");
    no_errors(&mut i);

    // Color Wheel: painted by the stack, then picked from.
    run(&mut i, "go to card \"Color Wheel\"");
    replies(&mut i, 1);
    assert_eq!(run(&mut i, "put there is a cd picture"), "true");
    run(&mut i, "put 100 into card button \"Value\"\nsend \"sliderMoved\" to card button \"Value\"\npickAt \"350,265\"");
    assert_eq!(run(&mut i, "put line 1 of card field \"Readout\" && line 3 of card field \"Readout\""), "hsv(0, 100%, 100%) #FF0000");
    assert_eq!(run(&mut i, "put the loc of card button \"Marker\""), "350,265");
    run(&mut i, "put 50 into card button \"Value\"\nsend \"sliderMoved\" to card button \"Value\"");
    assert_eq!(run(&mut i, "put line 3 of card field \"Readout\""), "#800000");
    no_errors(&mut i);

    // Harmony: three swatches for a triad, named by the API; four for a square.
    run(&mut i, "go to card \"Harmony\"");
    replies(&mut i, 3);
    assert_eq!(run(&mut i, "put line 1 of card field \"Swatch 1\" && line 1 of card field \"Swatch 2\" && line 2 of card field \"Swatch 3\""), "#800000 #008000 Test Gray");
    assert_eq!(run(&mut i, "put the visible of card field \"Swatch 4\""), "false");
    run(&mut i, "put \"Square\" into card field \"Scheme\"\nsend \"closeField\" to card field \"Scheme\"");
    replies(&mut i, 4);
    assert_eq!(run(&mut i, "put the visible of card field \"Swatch 4\" && (card field \"Rule\" contains \"90°\")"), "true true");
    assert_eq!(click(&mut i, "Harmony", "The API's Own"), None);
    replies(&mut i, 1);
    assert_eq!(run(&mut i, "put line 1 of card field \"API 1\" && the visible of card field \"API 5\""), "#111111 true");
    no_errors(&mut i);

    // Contrast: WCAG 2's ratio, never rounded up.
    run(&mut i, "go to card \"Contrast\"\nput \"#777777\" into card field \"Text Color\"\nput \"#FFFFFF\" into card field \"Background\"\ncheckContrast");
    assert_eq!(run(&mut i, "put card field \"Ratio\" && line 1 of card field \"Grades\""), "4.47:1 ✗ Fails AA, normal text (needs 4.5:1)");
    assert_eq!(run(&mut i, "put line 2 of card field \"Grades\""), "✓ Passes AA, large text (needs 3:1)");
    assert_eq!(click(&mut i, "Contrast", "Best Text"), None);
    assert_eq!(run(&mut i, "put card field \"Text Color\" && card field \"Ratio\""), "#000000 21:1");
    assert_eq!(click(&mut i, "Contrast", "Swap"), None);
    assert_eq!(run(&mut i, "put the backColor of card field \"Sample\""), "0,0,0");
    assert_eq!(click(&mut i, "Contrast", "Ask the API"), None);
    replies(&mut i, 1);
    assert!(run(&mut i, "put bkgnd field \"Status\"").starts_with("The Color API suggests #ffffff"));
    no_errors(&mut i);

    // The wheel painting itself, on a card with no picture yet.
    run(&mut i, "go to card \"About\"");
    assert_eq!(run(&mut i, "put there is a cd picture"), "false");
    run(&mut i, "paintWheel 300, 300, 40");
    assert_eq!(run(&mut i, "put there is a cd picture && the tool"), "true browse tool");
    no_errors(&mut i);
    // About: Test Connection, which waits for its answer.
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "About", "Test Connection"), None);
}

/// Every example in HYPERTALK.md, the language reference, parses.
#[test]
fn hypertalk_reference_examples_parse() {
    // Windows checks text files out with CRLF line endings.
    let doc = include_str!("../../HYPERTALK.md").replace("\r\n", "\n");
    let mut blocks = 0;
    for (n, block) in doc.split("```hypertalk\n").skip(1).enumerate() {
        let src = block.split("```").next().unwrap_or_default();
        if let Err(e) = crate::hypertalk::parser::parse_script(src) {
            panic!("HYPERTALK.md example {}: {e}\n{src}", n + 1);
        }
        blocks += 1;
    }
    assert!(blocks >= 10, "only {blocks} examples found");
}

/// Parse every script of one or more stacks (separated by |) and list the
/// ones that don't parse: RANDOMCARD_STACK="a.rcstack|b.rcstack" cargo test
/// --release parse_stack_scripts -- --ignored --nocapture
#[test]
#[ignore]
fn parse_stack_scripts() {
    for path in std::env::var("RANDOMCARD_STACK").unwrap().split('|') {
        let s = crate::import::load_any(std::path::Path::new(path), None).unwrap();
        let mut all: Vec<(String, String)> = vec![("stack".into(), s.script.clone())];
        for b in &s.backgrounds {
            all.push((format!("bg {}", b.id), b.script.clone()));
            for p in &b.parts {
                all.push((format!("bg {} part {}", b.id, p.id), p.script.clone()));
            }
        }
        for c in &s.cards {
            all.push((format!("card {}", c.id), c.script.clone()));
            for p in &c.parts {
                all.push((format!("card {} part {}", c.id, p.id), p.script.clone()));
            }
        }
        let bad: Vec<String> = all.iter().filter_map(|(d, sc)| crate::hypertalk::parser::parse_script(sc).err().map(|e| format!("  {d}: {e:?}"))).collect();
        println!("{path}: {} scripts, {} don't parse", all.len(), bad.len());
        for b in bad {
            println!("{b}");
        }
    }
}


#[test]
fn go_to_a_card_id_left_out() {
    // Shattered Stone leaves exits to nowhere as `go to cd id --`: HyperCard
    // compiles it, and says "No such card." only when it runs.
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "if 1 = 2 then\ngo to cd id\nend if\nput 44"), "44");
    assert!(run(&mut i, "go to cd id -- nowhere yet\nput 55").contains("No such card."));
    // A card that isn't there: the result says so, as before.
    assert_eq!(run(&mut i, "go to card id 99999\nput the result"), "No such card.");
}

/// HyperCard 2.4's movie syntax, and a movie played through it when the
/// system can play movies (GStreamer on Linux).
#[test]
fn movies() {
    let mut i = interp_with(stack());
    let (tx, rx) = std::sync::mpsc::channel();
    i.event_tx = Some(tx);
    // No movies yet: references to one are errors, counting isn't.
    assert_eq!(run(&mut i, "put the number of movies"), "0");
    assert!(run(&mut i, "put the rate of movie 1").contains("No such movie 1"));
    assert!(run(&mut i, "play movie \"Intro\"").contains("No such movie Intro"));
    assert!(run(&mut i, "put the visible of controller of last movie").contains("No such movie"));
    // A stack's own pause handler still gets `pause`.
    run(&mut i, "put 1 into pause");
    assert_eq!(run(&mut i, "put pause"), "1");
    let file = concat!(env!("CARGO_MANIFEST_DIR"), "/src/model/tour_movie/Card Trick.mp4");
    run(&mut i, &format!("Movie \"{file}\", \"rect\", \"10,20\", \"visible\""));
    run(&mut i, "put the result");
    let opened = lock(&i.world).msg.clone();
    if !opened.is_empty() {
        // No movie player here: the Movie command says so in the result.
        eprintln!("movies skipped: {opened}");
        assert_eq!(run(&mut i, "put the number of movies"), "0");
        return;
    }
    assert_eq!(run(&mut i, "put the number of movies"), "1");
    assert_eq!(run(&mut i, "put the rect of window \"Card Trick.mp4\""), "10,20,330,260");
    assert_eq!(run(&mut i, "put the rect of movie 1"), "0,0,320,240");
    // Its frames are opaque.
    let frame = lock(&i.world).movies[0].player.frame().expect("a first frame");
    assert!(frame.rgba.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
    assert_eq!(run(&mut i, "put the defaultRect of first movie"), "0,0,320,240");
    // The movie's scale, which QuickTime Tools sets; its matrix says the same.
    run(&mut i, "set the scale of movie 1 to \"double\"");
    assert_eq!(run(&mut i, "put the scale of movie 1 && the rect of movie 1"), "2,2 0,0,640,480");
    assert_eq!(run(&mut i, "put line 1 of the matrix of movie 1"), "2,0,0");
    run(&mut i, "set the movieScale of movie 1 to \"1,1\"");
    assert_eq!(run(&mut i, "put the rect of movie 1"), "0,0,320,240");
    assert_eq!(run(&mut i, "put the name of window \"Card Trick\""), "Card Trick.mp4");
    let id = run(&mut i, "put the id of movie 1");
    assert_eq!(run(&mut i, &format!("put the number of movie id {id}")), "1");
    assert_eq!(run(&mut i, "put rate of movie 1"), "0");
    assert_eq!(run(&mut i, "put the duration of movie 1"), "4,1.4");
    // Times in SystemTime (seconds and ticks) or MovieTime (600 a second).
    run(&mut i, "set the currTime of movie 1 to \"2,30\"");
    assert_eq!(run(&mut i, "put the currTime of movie 1"), "2,30");
    run(&mut i, "set the timeFormat of movie 1 to MovieTime");
    assert_eq!(run(&mut i, "put the currTime of movie 1"), "1500");
    run(&mut i, "set the timeFormat of movie 1 to SystemTime");
    // The window's rect is the card's; the movie's is the window's.
    run(&mut i, "set the loc of window \"Card Trick.mp4\" to 0,0");
    assert_eq!(run(&mut i, "put the rect of window \"Card Trick.mp4\""), "0,0,320,240");
    run(&mut i, "set the visible of controller of movie 1 to true");
    assert_eq!(run(&mut i, "put the bottom of window \"Card Trick.mp4\""), "256");
    assert_eq!(run(&mut i, "put the controllerVisible of movie 1"), "true");
    // Playing, pausing and rewinding, with movieRateChanged to the card.
    while rx.try_recv().is_ok() {}
    run(&mut i, "play movie 1");
    assert_eq!(run(&mut i, "put the rate of movie 1"), "1");
    let msg = rx.try_recv().expect("movieRateChanged");
    assert!(matches!(msg, Event::Send { ref message, ref args, .. } if message == "movieRateChanged" && args[2] == "1"));
    // On a busy machine the seek to 2.5 can take a while to land before
    // the movie moves on, so it's given time, and the time that passes is
    // measured rather than assumed.
    let started = std::time::Instant::now();
    let now = |i: &mut Interp| crate::runtime::movies::parse_system_time(&run(i, "put the currTime of movie 1")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    while now(&mut i) <= 2.6 && started.elapsed().as_secs() < 10 {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    run(&mut i, "pause movie 1");
    let played = started.elapsed().as_secs_f64();
    let t = now(&mut i);
    assert!(t > 2.6 && t < 2.5 + played + 0.1, "played on from 2.5 to {t} in {played}");
    run(&mut i, "send \"rewind\" to movie 1");
    assert_eq!(run(&mut i, "put the currTime of movie 1"), "0");
    run(&mut i, "set the soundVolume of movie 1 to 100");
    run(&mut i, "set the mute of movie 1 to true");
    assert_eq!(run(&mut i, "put the soundVolume of movie 1"), "-100");
    run(&mut i, "set the fastIdle of movie 1 to true");
    assert_eq!(run(&mut i, "put the fastIdle of movie 1"), "true");
    assert!(run(&mut i, "put the frobnitz of movie 1").contains("no property"));
    // The end: it stops there, or loops, or closes.
    run(&mut i, "set the endTime of movie 1 to \"0,12\"");
    run(&mut i, "play movie 1");
    std::thread::sleep(std::time::Duration::from_millis(400));
    let tick = lock(&i.world).movies[0].tick();
    assert_eq!(tick, crate::runtime::movies::Tick::Stopped);
    assert_eq!(run(&mut i, "put the rate of movie 1"), "0");
    run(&mut i, "hide movie 1");
    assert_eq!(run(&mut i, "put the visible of movie 1"), "false");
    assert!(run(&mut i, "put the windows").contains("Card Trick.mp4"));
    run(&mut i, "close movie 1");
    assert_eq!(run(&mut i, "put the number of movies"), "0");
    // Dropped on the card window, a movie opens near its corner.
    i.handle_event(Event::OpenMovie(file.into()));
    assert_eq!(run(&mut i, "put the rect of last movie && the layer of movie 1"), "0,0,320,240 nonFloating");
    assert_eq!(run(&mut i, "put the rect of window \"Card Trick.mp4\""), "20,20,340,260");
    run(&mut i, "close window \"Card Trick.mp4\"");
    assert_eq!(run(&mut i, "put the number of movies"), "0");
    // A movie that isn't there.
    run(&mut i, "Movie \"No Such Movie\"");
    run(&mut i, "put the result");
    assert_eq!(lock(&i.world).msg, "Can't find the movie \"No Such Movie\"");
}

/// AddColor's color pictures, in HyperCard 2.4.1's own stacks (when they're
/// here): kept with the stack as it's imported, for its items to show.
#[test]
fn addcolor_pictures_from_hypercard_241() {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/apple/hypercard_241_stacks"));
    for (stack, want) in [("HyperCard Stacks/New Features", 1), ("Color Tools", 7)] {
        let path = dir.join(stack);
        if !path.exists() {
            continue;
        }
        let s = crate::import::load_any(&path, None).unwrap();
        let items: Vec<(usize, String, bool)> = s
            .cards
            .iter()
            .enumerate()
            .flat_map(|(n, c)| c.colors.iter().map(move |i| (n + 1, i)))
            .chain(s.backgrounds.iter().flat_map(|b| b.colors.iter().map(|i| (0, i))))
            .filter_map(|(n, i)| match &i.target {
                crate::model::color::ColorTarget::Picture { name, file, .. } => Some((n, name.clone(), *file)),
                _ => None,
            })
            .collect();
        assert_eq!(items.len(), want, "{stack}: {items:?}");
        for (card, name, file) in &items {
            let p = s.color_picture(name, *file);
            assert!(p.is_some() || *file, "{stack}: card {card}: {name} wasn't kept");
        }
    }
}

/// What the Color Tools stack (HyperCard 2.4.1's) needed: `lock recent`,
/// Enviro, the message box as a window, and pictures placed at a point.
#[test]
fn color_tools_needs() {
    let mut i = interp_with(stack());
    // lock recent keeps cards off the Recent list, until scripts finish.
    assert_eq!(run(&mut i, "put the lockRecent"), "false");
    lock(&i.world).recent.clear();
    run(&mut i, "lock recent\ngo next card\nput the lockRecent");
    assert_eq!(lock(&i.world).msg, "true");
    assert!(lock(&i.world).recent.is_empty());
    assert_eq!(run(&mut i, "put the lockRecent"), "false");
    run(&mut i, "set lockRecent to true\nunlock recent\ngo next card");
    assert_eq!(lock(&i.world).recent.len(), 1);
    // A bare `dial` goes to the stack's own dial handler, as Phone Dialer's.
    set_script(&i, ObjRef::Stack, "on dial n\n  put \"asked\" && n into msg\nend dial");
    assert_eq!(run(&mut i, "dial"), "asked ");
    // With a number, the handler still gets it first, as any command, and
    // gets the number itself.
    assert_eq!(run(&mut i, "put \"555-1212\" into phoneNumber\ndial phoneNumber"), "asked 555-1212");
    set_script(&i, ObjRef::Stack, "");
    // Enviro: the deepest screen's colors, in bits.
    assert_eq!(run(&mut i, "enviro GetLargestDepth\nput the result"), "32");
    // The message box, as a window.
    run(&mut i, "set the loc of window \"message\" to 24,293");
    assert_eq!(run(&mut i, "put the loc of window \"Message\""), "24,293");
    run(&mut i, "set the visible of window \"message\" to false");
    assert_eq!(run(&mut i, "put the visible of window \"msg\""), "false");
    // A picture at a point takes its own size.
    let mut p = Picture::new(30, 20);
    p.set(0, 0, crate::model::Pixel::BLACK);
    lock(&i.world).stack.color_pictures.insert(crate::model::color_picture_key("•Horse", false), std::sync::Arc::new(p));
    run(&mut i, "AddColor \"addPict\",\"cd\",\"•Horse\",\"43,94\",\"t\"");
    assert_eq!(run(&mut i, "addColor getObjectBounds,cd,1\nput the result"), "43,94,73,114");
}

/// HyperCard 2.4.1's own stacks stay working: every stack listed in
/// tests/hc241_clean.txt (written by tools/hc241.py from a full run) is
/// opened and every scripted button on every card clicked, without a
/// script error. The stacks are Apple's and not in git, so this runs
/// wherever examples/apple/hypercard_241_stacks is, and passes elsewhere.
/// Clicks stopped by the two-second limit (timed demos) aren't errors.
/// Stacks marked `slow:` (over a minute) run with RANDOMCARD_HC241_SLOW=1.
#[test]
fn hypercard_241_stacks_stay_clean() {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/apple/hypercard_241_stacks"));
    if !dir.exists() {
        return;
    }
    let slow = std::env::var("RANDOMCARD_HC241_SLOW").is_ok();
    let names: Vec<&str> = include_str!("../../tests/hc241_clean.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| match l.strip_prefix("slow:") {
            Some(name) => slow.then_some(name.trim()),
            None => Some(l),
        })
        .collect();
    let threads: Vec<_> = names
        .iter()
        .map(|name| {
            let path = dir.join(name);
            let name = name.to_string();
            std::thread::Builder::new()
                .stack_size(super::SCRIPT_STACK)
                .spawn(move || {
                    let r = exercise_one(&path, false);
                    (name, r.errors)
                })
                .unwrap()
        })
        .collect();
    let mut broken = Vec::new();
    for t in threads {
        let (name, errors) = t.join().expect("a stack's run panicked");
        if !errors.is_empty() {
            broken.push(format!("{name}: {}", errors.join("; ")));
        }
    }
    assert!(broken.is_empty(), "HyperCard 2.4.1 stacks that were clean have errors:\n{}", broken.join("\n"));
}

#[test]
fn cards_resize() {
    // Setting the rect of a card sizes every card (Phone Dialer's Shrink
    // Window), and the card is sent sizeWindow.
    let mut s = stack();
    s.cards[0].picture.set(100, 200, crate::model::Pixel::BLACK);
    s.script = "on sizeWindow\n  put the rect of this card into the msg box\nend sizeWindow".into();
    let mut i = interp_with(s);
    run(&mut i, "set the rect of this card to 40,50,552,117");
    assert_eq!(run(&mut i, "put the rect of card 3"), "0,0,512,67");
    assert_eq!(lock(&i.world).msg, "0,0,512,67");
    assert_eq!(run(&mut i, "put the height of card window"), "67");
    // Growing again brings back what was painted below the small card.
    run(&mut i, "set the height of this card to 302");
    assert_eq!(run(&mut i, "put the height of this card"), "302");
    assert_eq!(lock(&i.world).stack.cards[0].picture.get(100, 200), crate::model::Pixel::BLACK);
    run(&mut i, "set the width of card 2 to 10");
    assert_eq!(run(&mut i, "put the width of this card"), "64");
    assert!(run(&mut i, "set the rect of this card to 1,2").starts_with("ERROR"));
    // Stack Info's Resize.
    i.handle_event(Event::ResizeCards(640, 480));
    assert_eq!(run(&mut i, "put the rect of this card"), "0,0,640,480");
}

#[test]
fn a_card_window_smaller_than_its_card() {
    // HyperTalk Reference's madScroller: a small window, scrolled about.
    let mut s = stack();
    s.script = "on sizeWindow\n  put the rect of card window into the msg box\nend sizeWindow".into();
    let mut i = interp_with(s);
    run(&mut i, "set the rect of card window to 10,40,182,104");
    assert_eq!(run(&mut i, "put the rect of card window"), "10,40,182,104");
    assert_eq!(lock(&i.world).msg, "10,40,182,104");
    assert_eq!(run(&mut i, "put the rect of this card"), "0,0,512,342");
    run(&mut i, "set the scroll of card window to 100,50");
    assert_eq!(run(&mut i, "put the scroll of card window"), "100,50");
    // No further than shows the card's far edge.
    run(&mut i, "set the scroll of card window to bottomRight of this card");
    assert_eq!(run(&mut i, "put the scroll of card window"), "340,278");
    // Moving keeps the size; the width and height size it.
    run(&mut i, "set the topLeft of card window to 0,0");
    assert_eq!(run(&mut i, "put the rect of card window"), "0,0,172,64");
    run(&mut i, "set the height of card window to 100");
    assert_eq!(run(&mut i, "put the rect of card window"), "0,0,172,100");
    // Never bigger than the card.
    run(&mut i, "set the rect of card window to 0,0,2000,2000");
    assert_eq!(run(&mut i, "put the rect of card window"), "0,0,512,342");
    assert_eq!(run(&mut i, "put the scroll of card window"), "0,0");
    // The user resizing the window.
    lock(&i.world).msg.clear();
    i.handle_event(Event::SizeCardWindow(300, 200));
    assert_eq!(lock(&i.world).msg, "0,0,300,200");
    // Resizing the cards gives the window their size.
    run(&mut i, "set the rect of this card to 0,0,400,300");
    assert_eq!(run(&mut i, "put the rect of card window"), "0,0,400,300");
}

#[test]
fn the_card_window_moves_and_zooms() {
    // moveWindow follows sizeWindow when a script moves and sizes it.
    let mut s = stack();
    s.script = "on sizeWindow\n  put \"size \" after msg\nend sizeWindow\non moveWindow\n  put \"move\" after msg\nend moveWindow".into();
    let mut i = interp_with(s);
    lock(&i.world).screen_rect = Rect::new(0, 0, 1024, 768);
    run(&mut i, "set the rect of card window to 20,30,220,130");
    assert_eq!(lock(&i.world).msg, "size move");
    lock(&i.world).msg.clear();
    run(&mut i, "set the loc of card window to 20,30");
    assert_eq!(lock(&i.world).msg, "");
    // Zoomed: the whole card, centered on the screen, and back again.
    assert_eq!(run(&mut i, "put the zoomed of card window"), "false");
    run(&mut i, "set the zoomed of card window to true");
    assert_eq!(run(&mut i, "put the zoomed of card window & \":\" & the rect of card window"), "true:256,213,768,555");
    run(&mut i, "set the zoomed of card window to false");
    assert_eq!(run(&mut i, "put the zoomed of card window & \":\" & the rect of card window"), "false:20,30,220,130");
    // The user moving the window sends moveWindow and unzooms it.
    run(&mut i, "set the zoomed of card window to true");
    lock(&i.world).msg.clear();
    i.handle_event(Event::CardWindowMoved(5, 6));
    assert_eq!(lock(&i.world).msg, "move");
    assert_eq!(run(&mut i, "put the zoomed of card window & \":\" & the topLeft of card window"), "false:5,6");
}

#[test]
fn the_destination() {
    // Where navigation is going, as the close and suspend messages are sent;
    // otherwise the current stack. The Help stacks ask it in closeStack.
    let dir = std::env::temp_dir().join(format!("randomcard-destination-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let logger = |name: &str| {
        ["closeCard", "closeStack", "suspendStack"]
            .map(|m| format!("on {m}\n  global log\n  put \"{name} {m} \" & the destination & \",\" after log\nend {m}"))
            .join("\n")
    };
    let mut lib = stack();
    lib.name = "Lib".into();
    lib.script = logger("Lib");
    let lib_path = dir.join("Lib.rcstack");
    lib.path = Some(lib_path.clone());
    lib.save(&lib_path).unwrap();
    let mut main = stack();
    main.name = "Main".into();
    main.script = logger("Main");
    let main_path = dir.join("Main.rcstack");
    main.path = Some(main_path.clone());
    main.save(&main_path).unwrap();
    let (lib_at, main_at) = (crate::runtime::paths::shown(&lib_path), crate::runtime::paths::shown(&main_path));

    let mut i = interp_with(main);
    let log = |i: &mut Interp| run(i, "global log\nput log\nput empty into log");
    // Not navigating: the current stack.
    assert_eq!(run(&mut i, "put the destination"), main_at);
    assert_eq!(run(&mut i, "put destination()"), main_at);
    // Within a stack, its own path.
    run(&mut i, "go to card 2");
    assert_eq!(log(&mut i), format!("Main closeCard {main_at},"));
    // To another stack, that one's.
    run(&mut i, "go to stack \"Lib\"");
    assert_eq!(log(&mut i), format!("Main closeCard {lib_at},Main closeStack {lib_at},"));
    assert_eq!(run(&mut i, "put the destination"), lib_at);
    // A new window, and bringing one forward.
    run(&mut i, "go to stack \"Main\" in a new window");
    assert_eq!(log(&mut i), format!("Lib suspendStack {main_at},"));
    run(&mut i, "go to stack \"Lib\"");
    assert_eq!(log(&mut i), format!("Main suspendStack {lib_at},"));
    // Closing a window: the one left showing is where it's going.
    run(&mut i, "close window \"Main\"");
    assert_eq!(log(&mut i), format!("Main closeCard {lib_at},Main closeStack {lib_at},"));
    // A variable can still be called destination.
    assert_eq!(run(&mut i, "put \"Two\" into destination\nput destination"), "Two");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn apple_palettes() {
    // GeneralPalette, FieldPalette and StackTemplatePal, Apple's palette
    // XCMDs, built in: each draws the picture the stack carries by its name.
    let mut s = stack();
    for (name, w, h) in [("FieldPalette", 212, 88), ("StackTemplatePal", 189, 69), ("GeneralPalette", 212, 33)] {
        let key = crate::model::color_picture_key(name, false);
        s.color_pictures.insert(key, std::sync::Arc::new(crate::model::Picture::new(w, h)));
    }
    s.script = "on paletteDismiss\n  put \"dismissed\" into msg\nend paletteDismiss\non createStack\n  put \"create\" into msg\nend createStack".into();
    let mut i = interp_with(s);
    lock(&i.world).window_origin = (100, 50);
    // Opened at a point on the screen; the window is on the card.
    run(&mut i, "FieldPalette \"Categories\", 150, 250");
    assert_eq!(run(&mut i, "put the result"), "");
    assert_eq!(run(&mut i, "put the rect of window \"FieldPalette\""), "50,200,262,288");
    assert_eq!(run(&mut i, "global HaveFieldPalette, FieldPaletteLoc\nput HaveFieldPalette && FieldPaletteLoc"), "true 150,250");
    assert_eq!(run(&mut i, "put the id of window \"FieldPalette\""), "4000");
    assert_eq!(run(&mut i, "put the commands of window \"FieldPalette\""), "goPrev\ngoCategories\ngoNext");
    // Its three lines are drawn on it.
    let before = lock(&i.world).script_windows[0].palette.clone().unwrap();
    run(&mut i, "set the fields of window \"FieldPalette\" to \"Books\" & return & \"2 of 10\" & return & \"Spiral\"");
    assert_eq!(run(&mut i, "put line 2 of the fields of window \"FieldPalette\""), "2 of 10");
    let after = lock(&i.world).script_windows[0].palette.clone().unwrap();
    assert_ne!(before.picture, after.picture);
    // Only one at a time; a problem goes in the result.
    run(&mut i, "FieldPalette \"Again\", 0, 0");
    assert_eq!(run(&mut i, "put the result"), "Error:  There already is a FieldPalette window.");
    // Moved, then sent back to its Loc.
    run(&mut i, "set the loc of window \"FieldPalette\" to 5,5");
    run(&mut i, "send \"move\" to window \"FieldPalette\"");
    assert_eq!(run(&mut i, "put the loc of window \"FieldPalette\""), "50,200");
    // Closing: paletteDismiss, and the globals say where it was.
    run(&mut i, "set the loc of window \"FieldPalette\" to 10,20");
    run(&mut i, "close window \"FieldPalette\"");
    assert_eq!(lock(&i.world).msg, "dismissed");
    assert_eq!(run(&mut i, "global HaveFieldPalette, FieldPaletteLoc\nput HaveFieldPalette && FieldPaletteLoc"), "false 110,70");
    // StackTemplatePal takes two names; its top button sends createStack.
    run(&mut i, "StackTemplatePal \"Create Stack\", \"Templates\", 100, 50");
    assert_eq!(run(&mut i, "put the commands of window \"StackTemplatePal\""), "showInfo\ncreateStack\ngoHome\ngoCategories\ngoPrev\ngoNext");
    i.handle_event(Event::PaletteButton("StackTemplatePal".into(), 1));
    assert_eq!(lock(&i.world).msg, "create");
    // GeneralPalette with a title is taller; with too few names, an error.
    run(&mut i, "GeneralPalette \"Contents\", \"Art\", 100, 50");
    assert_eq!(run(&mut i, "put the height of window \"GeneralPalette\""), "53");
    run(&mut i, "close window \"GeneralPalette\"\nGeneralPalette 100, 50");
    assert_eq!(run(&mut i, "put the result"), "Error:  Incorrect number of parameters.");
    // No picture, no palette.
    lock(&i.world).stack.color_pictures.clear();
    run(&mut i, "GeneralPalette \"Contents\", 100, 50");
    assert_eq!(run(&mut i, "put the result"), "Error:  Can't access GeneralPalette PICT.");
}

#[test]
fn number_of_what_a_variable_names() {
    // `number of theObjs` counts what the variable names, as Power Tools'
    // listObjects has it; a variable naming one object still gives its number.
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "put \"cd btns\" into theObjs\nput number of theObjs"), "1");
    assert_eq!(run(&mut i, "put \"cards\" into what\nput the number of what"), "3");
    assert_eq!(run(&mut i, "put \"card 2\" into theCard\nput the number of theCard"), "2");
    assert_eq!(run(&mut i, "put \"cd btn\" into theObj\nput number of (theObj & \"s\")"), "1");
}

#[test]
fn show_list() {
    // ShowList, from Apple's Power Tools: up to four named buttons (a ^
    // draws a line above one), single or multiple choice, typing or not.
    let args: Vec<String> =
        ["a\nb\nc", "Pick:", "true", "false", "OK,Cancel^,Add", "2", "30", "40"].map(String::from).to_vec();
    let d = super::externals::show_list_dialog(&args);
    assert_eq!(d.items, ["a", "b", "c"]);
    assert_eq!(d.mode, 3);
    assert!(d.no_typing);
    assert_eq!(d.selected, [1]);
    assert_eq!(d.at, Some((30, 40)));
    assert_eq!(d.buttons, [("OK".to_string(), false), ("Cancel".to_string(), true), ("Add".to_string(), false)]);
    // It answers the button, then the chosen lines.
    let mut i = interp_with(stack());
    i.canned_answers.push(DialogReply::List(vec![0, 2], "OK".into()));
    assert_eq!(run(&mut i, "get showList(\"a\" & return & \"b\" & return & \"c\", \"Pick:\", true, true, \"OK,Cancel\", 0)\nput it"), "OK\n1,3");
    // The Tour's List Objects flashes the button chosen.
    let mut t = interp_with(Stack::demo());
    run(&mut t, "go to card \"Debugging\"");
    t.canned_answers.push(DialogReply::List(vec![0], "Flash".into()));
    assert_eq!(click(&mut t, "Debugging", "List Objects"), None);
    assert_eq!(run(&mut t, "put cd fld \"Answer\""), "ShowList answered \"Flash\" and line 1.");
}

#[test]
fn picture_windows() {
    // The picture command (HyperCard's Picture XCMD): a picture from a file
    // or the stack's own, in a window named for it.
    let dir = std::env::temp_dir().join(format!("randomcard-picture-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    image::RgbaImage::from_pixel(40, 30, image::Rgba([200, 0, 0, 255])).save(dir.join("Klee.png")).unwrap();
    let mut s = stack();
    s.path = Some(dir.join("Pics.rcstack"));
    s.pictures.insert(26760, crate::model::NamedPicture { name: "Pelican".into(), picture: crate::model::Picture::new(20, 10) });
    s.script = "on openPicture n, i\n  put \"open\" && n && i & \",\" after msg\nend openPicture\non closePicture n, i\n  put \"close\" && n && i & \",\" after msg\nend closePicture".into();
    let mut i = interp_with(s);
    assert_eq!(run(&mut i, "picture \"Klee.png\", file, shadow"), "open Klee.png 1001,");
    assert_eq!(run(&mut i, "put the result"), "");
    assert_eq!(run(&mut i, "put the owner of window \"Klee.png\" && the pictureWidth of window \"Klee.png\""), "Picture 40");
    // In the middle of the card, at its own size.
    assert_eq!(run(&mut i, "put the rect of window \"Klee.png\""), "236,156,276,186");
    // From the stack's pictures, by number or name; the window takes the
    // picture's name. An invisible one is still a window.
    run(&mut i, "picture 26760, resource, windoid, false");
    assert_eq!(run(&mut i, "put the visible of window \"Pelican\" && there is a window \"Pelican\""), "false true");
    assert_eq!(run(&mut i, "put the number of pictures"), "2");
    // Closed by number, front first, or by name.
    lock(&i.world).msg.clear();
    assert_eq!(run(&mut i, "close picture 1"), "close Pelican 1002,");
    run(&mut i, "close picture \"Klee.png\"");
    assert_eq!(run(&mut i, "put the number of pictures"), "0");
    run(&mut i, "open picture \"Klee.png\"");
    assert_eq!(run(&mut i, "put the number of pictures"), "1");
    // A window by its id.
    let id = run(&mut i, "put the id of window \"Klee.png\"");
    assert_eq!(run(&mut i, &format!("put the name of window id {id}")), "Klee.png");
    run(&mut i, &format!("hide window id {id}"));
    assert_eq!(run(&mut i, "put the visible of window \"Klee.png\""), "false");
    // Problems go in the result.
    run(&mut i, "picture \"Nowhere.png\"");
    assert!(run(&mut i, "put the result").starts_with("Couldn't display picture."));
    run(&mut i, "picture \"Klee.png\", file, sideways");
    assert!(run(&mut i, "put the result").starts_with("Couldn't display picture."));
    run(&mut i, "picture \"Gull\", resource");
    assert!(run(&mut i, "put the result").starts_with("Couldn't display picture."));
    // TNPict, from the Technical Notes stacks, shows its file the same way.
    run(&mut i, "TNPict \"Klee.png\"");
    assert_eq!(run(&mut i, "put the result & the owner of window \"Klee.png\""), "Picture");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn picture_window_properties() {
    // A picture window's own properties: where it is on the screen, what
    // part of the picture it shows and how big, zooming, and clicks.
    let dir = std::env::temp_dir().join(format!("randomcard-picprops-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    image::RgbaImage::from_pixel(400, 300, image::Rgba([0, 0, 0, 255])).save(dir.join("Big.png")).unwrap();
    let mut s = stack();
    s.path = Some(dir.join("Pics.rcstack"));
    s.script = "on mouseDownInPicture n, p\n  put \"down\" && n && p into msg\nend mouseDownInPicture".into();
    let mut i = interp_with(s);
    {
        let mut w = lock(&i.world);
        w.window_origin = (100, 50);
        w.screen_rect = crate::model::Rect::new(0, 0, 1024, 768);
    }
    run(&mut i, "picture \"Big.png\"");
    assert_eq!(run(&mut i, "put the name of picture 1"), "Big.png");
    assert_eq!(run(&mut i, "put the globalLoc of picture 1"), "156,71");
    run(&mut i, "set the globalLoc of picture 1 to 200,100");
    assert_eq!(run(&mut i, "put the loc of window \"Big.png\""), "100,50");
    // Made smaller than the picture, it scrolls, within the picture.
    run(&mut i, "set the rect of window \"Big.png\" to 0,0,100,100");
    run(&mut i, "set the scroll of window \"Big.png\" to 50,500");
    assert_eq!(run(&mut i, "put the scroll of window \"Big.png\""), "50,200");
    // Twice the size shows half as much, so it can scroll further.
    run(&mut i, "set the scale of picture 1 to 1\nset the scroll of picture 1 to 999,999");
    assert_eq!(run(&mut i, "put the scale of picture 1 && the scroll of picture 1"), "1 350,250");
    // Zoomed out, as big as the picture at its scale allows; in again, back.
    run(&mut i, "set the scale of picture 1 to 0\nset the zoom of picture 1 to out");
    assert_eq!(run(&mut i, "put the zoom of picture 1 && the rect of picture 1"), "out 0,0,400,300");
    run(&mut i, "set the zoom of picture 1 to in");
    assert_eq!(run(&mut i, "put the zoom of picture 1 && the rect of picture 1"), "in 0,0,100,100");
    // The screen words center it on the screen.
    run(&mut i, "set the loc of picture 1 to cardScreen");
    assert_eq!(run(&mut i, "put the globalRect of picture 1"), "462,334,562,434");
    // Clicks come to the card at their point in the picture.
    i.handle_event(Event::PictureMouse { name: "Big.png".into(), down: true, at: (3, 4) });
    assert_eq!(lock(&i.world).msg, "down Big.png 3,4");
    assert_eq!(run(&mut i, "put the number of windows > the number of pictures"), "true");
    // Windows by number, as the windows lists them, front first.
    assert_eq!(run(&mut i, "put the name of window 1 && the owner of window 1"), "Big.png Picture");
    run(&mut i, "close window 1");
    assert_eq!(run(&mut i, "put the number of pictures"), "0");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn tour_pictures() {
    // The Tour's Pictures card: a picture the Tour carries, in a window
    // that's scaled, zoomed and centered, and that tells the card.
    let mut i = interp_with(Stack::demo());
    lock(&i.world).screen_rect = crate::model::Rect::new(0, 0, 1024, 768);
    run(&mut i, "go to card \"Pictures\"");
    assert_eq!(click(&mut i, "Pictures", "Show Picture"), None);
    assert_eq!(run(&mut i, "put field \"Picture Messages\""), "openPicture Rainbow 1001");
    assert_eq!(run(&mut i, "put the rect of window \"Rainbow\""), "60,130,208,210");
    assert_eq!(click(&mut i, "Pictures", "Bigger"), None);
    assert_eq!(click(&mut i, "Pictures", "Zoom"), None);
    assert_eq!(run(&mut i, "put field \"Picture Messages\""), "zoom out, the rect 60,130,356,290");
    assert_eq!(click(&mut i, "Pictures", "Center"), None);
    assert!(run(&mut i, "put field \"Picture Messages\"").starts_with("Centered on the screen"));
    i.handle_event(Event::PictureMouse { name: "Rainbow".into(), down: true, at: (70, 40) });
    assert_eq!(run(&mut i, "put field \"Picture Messages\""), "mouseDownInPicture Rainbow at 70,40");
    // Leaving closes it.
    run(&mut i, "go to next card");
    assert_eq!(run(&mut i, "put the number of pictures"), "0");
}

#[test]
fn a_chunk_of_a_value_names_an_object() {
    // `the scroll of last item of fieldList`: an item of a variable names
    // the field, as Apple's Multiple Scrolling Fields has it. A chunk of a
    // field still has its own text properties.
    let mut i = interp_with(stack());
    run(&mut i, "put \"bg field id 1,bg field 1\" into fieldList");
    run(&mut i, "set the textSize of last item of fieldList to 18");
    assert_eq!(run(&mut i, "put the textSize of bg field 1"), "18");
    assert_eq!(run(&mut i, "put the textSize of last item of fieldList"), "18");
    run(&mut i, "set the textSize of (item 2 of fieldList) to 9");
    assert_eq!(run(&mut i, "put the textSize of item 2 of fieldList"), "9");
    run(&mut i, "put \"Hello world\" into bg field 1\nset the textStyle of word 2 of bg field 1 to bold");
    assert_eq!(run(&mut i, "put the textStyle of word 2 of bg field 1"), "bold");
}

#[test]
fn resources_lists_a_stacks_resources() {
    // Power Tools' Resources XFCN: "type id size name" a line, from what
    // the stack keeps of its resource fork.
    let mut s = stack();
    s.icons.insert(128, crate::model::Icon { name: "Star".into(), picture: crate::model::Picture::new(32, 32) });
    s.pictures.insert(200, crate::model::NamedPicture { name: "Klee".into(), picture: crate::model::Picture::new(10, 10) });
    s.palettes.insert(300, crate::model::Palette { name: "Tools".into(), picture: crate::model::Picture::new(4, 4), buttons: vec![] });
    let mut i = interp_with(s);
    assert_eq!(run(&mut i, "put Resources()"), "ICON 128 128 Star\nPLTE 300 24 Tools\nPICT 300 16 Tools\nPICT 200 100 Klee");
    assert_eq!(run(&mut i, "put line 1 of Resources(line 1 of the stacks)"), "ICON 128 128 Star");
    // `choose the browse tool`, as Power Tools has it.
    run(&mut i, "choose the button tool");
    assert_eq!(run(&mut i, "put the tool"), "button tool");
    run(&mut i, "choose the browse tool");
    assert_eq!(run(&mut i, "put the tool"), "browse tool");
}

#[test]
fn scripts_copy_and_paste_parts() {
    // Power Tools' overview: a background field selected, Command-C, then
    // Command-V pastes it onto the card, with its text, for the next line.
    let mut i = interp_with(stack());
    run(&mut i, "put \"Overview\" into bg field \"Name\"");
    run(&mut i, "select bg field \"Name\"\ntype \"C\" with cmdKey\ntype \"V\" with shiftKey, cmdKey");
    assert_eq!(run(&mut i, "put the number of card fields && card field \"Name\""), "1 Overview");
    assert_eq!(run(&mut i, "set the scroll of card field \"Name\" to 0\nput the tool"), "field tool");
    // The pasted field is selected: Cut takes it away again, and Paste
    // brings it back.
    run(&mut i, "doMenu \"Cut Field\"");
    assert_eq!(run(&mut i, "put the number of card fields"), "0");
    run(&mut i, "doMenu \"Paste Field\"");
    assert_eq!(run(&mut i, "put the number of card fields"), "1");
    // Other Command keys choose their menu items.
    run(&mut i, "choose browse tool\ntype \"3\" with commandKey");
    assert_eq!(run(&mut i, "put the number of this card"), "2");
}

#[test]
fn resource_mover() {
    // Power Tools' CopyRes, ResRemove and EditResource, on stacks' icons,
    // sounds, palettes, pictures and songs. Another stack's changes are
    // saved to its file.
    let dir = std::env::temp_dir().join(format!("randomcard-resmover-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let lib_path = dir.join("Lib.rcstack");
    let mut lib = stack();
    lib.name = "Lib".into();
    lib.icons.insert(128, crate::model::Icon { name: "Taken".into(), picture: crate::model::Picture::new(32, 32) });
    lib.path = Some(lib_path.clone());
    lib.save(&lib_path).unwrap();
    let mut s = stack();
    s.icons.insert(128, crate::model::Icon { name: "Star".into(), picture: crate::model::Picture::new(32, 32) });
    s.palettes.insert(300, crate::model::Palette { name: "Tools".into(), picture: crate::model::Picture::new(4, 4), buttons: vec![] });
    let mut i = interp_with(s);
    let lib = lib_path.display().to_string();
    // Copied, and renumbered where the id is taken; a palette with its PICT.
    run(&mut i, &format!("CopyRes \"ICON\", 128, \"\", \"{lib}\""));
    assert_eq!(run(&mut i, "put the result"), "");
    run(&mut i, &format!("CopyRes \"PLTE\", 300, \"\", \"{lib}\"\nCopyRes \"PICT\", 300, \"\", \"{lib}\""));
    assert_eq!(run(&mut i, &format!("put Resources(\"{lib}\")")), "ICON 128 128 Taken\nICON 129 128 Star\nPLTE 300 24 Tools\nPICT 300 16 Tools");
    let saved = Stack::load(&lib_path).unwrap();
    assert_eq!(saved.icons[&129].name, "Star");
    // Renamed and renumbered, then removed.
    run(&mut i, &format!("EditResource \"{lib}\", \"ICON\", 129, \"Big Star\", 500"));
    assert_eq!(run(&mut i, &format!("put line 2 of Resources(\"{lib}\")")), "ICON 500 128 Big Star");
    run(&mut i, &format!("ResRemove \"{lib}\", \"PLTE\", 300"));
    assert_eq!(run(&mut i, &format!("put the number of lines of Resources(\"{lib}\")")), "2");
    // Problems go in the result.
    run(&mut i, "ResRemove \"\", \"ICON\", 999");
    assert_eq!(run(&mut i, "put the result"), "Error: there's no ICON 999.");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn small_things_apples_stacks_use() {
    // `delete menuItem x from menu 3` (Audio Help), `select line 0` of a
    // popup (New Features), and the textHeight of painted text (Graph Maker).
    let mut i = interp_with(stack());
    run(&mut i, "create menu \"Things\"\nput \"One,Two\" into menu \"Things\"\ndelete menuItem \"One\" from menu \"Things\"");
    assert_eq!(run(&mut i, "put the number of menuItems of menu \"Things\""), "1");
    run(&mut i, "set the style of card button 1 to popup\nset the contents of card button 1 to \"a\" & return & \"b\"\nselect line 2 of card button 1\nselect line 0 of card button 1");
    assert_eq!(run(&mut i, "put the selectedLine of card button 1"), "");
    run(&mut i, "set the textSize to 12");
    assert_eq!(run(&mut i, "put the textHeight"), "16");
    run(&mut i, "set the textHeight to 20");
    assert_eq!(run(&mut i, "put the textHeight"), "20");
    run(&mut i, "set the textSize to 9");
    assert_eq!(run(&mut i, "put the textHeight"), "12");
}

#[test]
fn passing_reaches_a_stack_just_put_in_use() {
    // The HyperTalk Reference starts using HyperCard Help in its openStack
    // and passes openStack on: the library, now in the path, gets it.
    let dir = std::env::temp_dir().join(format!("randomcard-passuse-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut lib = stack();
    lib.name = "Lib".into();
    lib.script = "on openStack\n  global seen\n  put \"lib\" into seen\nend openStack".into();
    let lp = dir.join("Lib.rcstack");
    lib.path = Some(lp.clone());
    lib.save(&lp).unwrap();
    let mut s = stack();
    s.script = format!("on openStack\n  start using stack \"{}\"\n  pass openStack\nend openStack", lp.display());
    let mut i = interp_with(s);
    run(&mut i, "send openStack to this card");
    assert_eq!(run(&mut i, "global seen\nput seen"), "lib");
    // start using empties the result when it works.
    assert_eq!(run(&mut i, "put the result"), "");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn no_applescript_and_no_apple_events() {
    // The AppleScript stacks and the Apple Event Primer ask about
    // AppleScript and other programs: there are none, and they're told so.
    let mut s = stack();
    s.script = "on errorDialog msg\n  global caught\n  put msg into caught\nend errorDialog".into();
    let mut i = interp_with(s);
    assert_eq!(run(&mut i, "put there is a scriptingLanguage AppleScript"), "false");
    assert_eq!(run(&mut i, "put there is no scriptingLanguage \"AppleScript\""), "true");
    assert_eq!(run(&mut i, "put the version of scriptingLanguage \"AppleScript\""), "0");
    assert_eq!(run(&mut i, "request \"name of stack\" of program \"Mac:HyperCard\"\nput the result"), "Can't find program \"Mac:HyperCard\"");
    assert!(run(&mut i, "send \"gossip\" to program \"Mac:HyperCard\" without reply\nput the result").starts_with("Can't find program"));
    run(&mut i, "request appleEvent data\nreply \"done\" with keyword \"errn\"");
    assert_eq!(run(&mut i, "put it"), "");
    assert_eq!(run(&mut i, "do \"beep\" as AppleScript\nput the result"), "There is no scripting language \"AppleScript\"");
    // With error dialogs locked, an error goes to errorDialog instead.
    run(&mut i, "lock error dialogs\nput 1 / zzz into x");
    assert!(run(&mut i, "global caught\nput caught").contains("zzz"));
    assert!(lock(&i.world).requests.iter().all(|r| !matches!(r, UiRequest::ScriptError { .. })));
    assert_eq!(run(&mut i, "put the lockErrorDialogs"), "false");
    assert_eq!(run(&mut i, "put there is a movie \"Intro\""), "false");
    // Empty is in nothing, as HyperCard answers.
    assert_eq!(run(&mut i, "put (empty is in \"abc\") && (\"abc\" contains empty) && (empty is not in \"abc\")"), "false false true");
}

#[test]
fn send_domenu_to_hypercard_skips_the_stacks_handler() {
    // AppleScript Library's doMenu handler sends "doMenu Delete Card" to
    // HyperCard: that's the menu item, not the handler again.
    let mut s = stack();
    s.script = "on doMenu what\n  if what is \"Next\" then send \"doMenu Next\" to HyperCard\n  else pass doMenu\nend doMenu".into();
    let mut i = interp_with(s);
    run(&mut i, "doMenu \"Next\"");
    assert_eq!(run(&mut i, "put the number of this card"), "2");
}

#[test]
fn new_background_edits_the_background() {
    // As in HyperCard 2.4.1, New Background leaves you editing it, so
    // Readymade Buttons' Help button gets background fields.
    let mut i = interp_with(stack());
    run(&mut i, "doMenu \"New Background\"\ndoMenu \"New Field\"");
    assert_eq!(run(&mut i, "put the editBkgnd && the number of bg flds && the number of cd flds"), "true 1 0");
}

#[test]
fn the_navigator_palette() {
    // HyperCard's Navigator came with the application: a stack without a
    // palette of that name gets RandomCard's, with the same menu commands.
    let mut i = interp_with(stack());
    run(&mut i, "palette \"Navigator\", \"10,10\"");
    assert_eq!(run(&mut i, "put the number of lines of the commands of window \"Navigator\""), "11");
    assert_eq!(run(&mut i, "put line 7 of the commands of window \"Navigator\""), "doMenu \"Next\"");
    i.handle_event(Event::PaletteButton("Navigator".into(), 6));
    assert_eq!(run(&mut i, "put the number of this card"), "2");
}

#[test]
fn quicktime_tools_questions() {
    // QuickTime Tools asks the version of QuickTime, a stack's report
    // templates, and sets a property named by an expression.
    let mut i = interp_with(stack());
    assert_eq!(run(&mut i, "put the version of QuickTime"), "3.0");
    // Dividing by zero, as HyperCard 2.4.1 answers it.
    assert_eq!(run(&mut i, "put (1/0) && (-2/0) && (0/0) && (1/empty)"), "INF -INF -NAN(000) INF");
    assert!(run(&mut i, "put 5 div 0").contains("Can't DIV by zero"));
    assert_eq!(run(&mut i, "put the reportTemplates of this stack"), "");
    run(&mut i, "put \"textSize\" into prop\nset (prop) of bg field 1 to 18");
    assert_eq!(run(&mut i, "put the textSize of bg field 1"), "18");
}

#[test]
fn a_script_types_into_find() {
    // The French Help stack's demo: Find, the letters typed one by one,
    // then Enter, as a person would.
    let mut i = interp_with(stack());
    run(&mut i, "put \"zebra\" into bg field 1 of card 3");
    let card = lock(&i.world).card_id;
    set_script(&i, ObjRef::Card(card), "on findIt\n  doMenu \"Find…\"\n  type \"z\"\n  type \"e\"\n  enterKey\nend findIt");
    run(&mut i, "findIt");
    assert_eq!(run(&mut i, "put the number of this card"), "3");
    // XCMD.Flush's flush drops events waiting, as flush keys does.
    run(&mut i, "flush");
}

#[test]
#[ignore]
fn scratch_dia() {
    let p = std::path::Path::new("examples/apple/devcds/ADC Developer CD (Volume 11 - \"The Silence Of The ROMs\")_mdf/Start Here:Read CD License 1st/Developer Info Assistant (DIA)/DIA Copy Me to Hard Disk/Developer Info Assistant/Developer Info Assistant");
    let s = crate::import::load_any(p, None).unwrap();
    for (n, l) in s.script.lines().enumerate().skip(15).take(14) { println!("S{:3} {l}", n + 1); }
    let bg = s.backgrounds.iter().find(|b| b.id == 7727).unwrap();
    for (n, l) in bg.script.lines().enumerate().take(6) { println!("B{:3} {l}", n + 1); }
}

#[test]
fn quoted_names_are_names() {
    // In HyperCard a quoted name is a name even when it looks like a number:
    // the Developer Info Assistant's Setup card has buttons "1200" and
    // "2400". A number, or a variable or expression that gives one, counts.
    let mut i = interp_with(stack());
    {
        let mut w = lock(&i.world);
        w.stack.cards[0].parts.push(Part::new_button(30, "1200", Rect::new(10, 10, 60, 30)));
        w.stack.cards[0].parts.push(Part::new_button(31, "2", Rect::new(70, 10, 120, 30)));
    }
    assert_eq!(run(&mut i, "put the id of cd btn \"1200\" && the id of cd btn \"2\" && the id of cd btn 2"), "30 31 30");
    assert_eq!(run(&mut i, "put \"2\" into x\nput the id of cd btn x && the id of cd btn (\"2\") && the id of cd btn (1 + 1)"), "30 30 30");
    assert!(run(&mut i, "put the id of cd btn \"7\"").contains("No such"));
}

/// A dialog as a stack's DLOG and DITL give it, for Dialoger.
fn dialog_template(name: &str, items: &[(crate::model::dialogs::DialogItemKind, [i32; 4], bool, &str, i16)]) -> crate::model::dialogs::DialogTemplate {
    use crate::model::dialogs::{DialogItem, DialogTemplate};
    DialogTemplate {
        name: name.into(),
        title: name.into(),
        rect: Rect::new(100, 80, 360, 260),
        proc_id: 1,
        close_box: false,
        items: items
            .iter()
            .map(|(kind, r, enabled, text, id)| DialogItem { kind: *kind, rect: Rect::new(r[0], r[1], r[2], r[3]), enabled: *enabled, text: (*text).into(), id: *id })
            .collect(),
    }
}

#[test]
fn dialoger_dialogs() {
    // As Tools Advisor asks which features a story mentions: check boxes
    // set from a global, OK or Cancel, and the answer a line an item.
    use crate::model::dialogs::DialogItemKind as K;
    let mut i = interp_with(stack());
    {
        let mut w = lock(&i.world);
        w.stack.dialogs.insert(
            1552,
            dialog_template(
                "Dev Features",
                &[
                    (K::Button, [147, 61, 165, 133], true, "OK", 0),
                    (K::Button, [148, 150, 167, 227], true, "Cancel", 0),
                    (K::StaticText, [8, 6, 220, 27], false, "Strengths ^0", 0),
                    (K::CheckBox, [31, 25, 144, 47], true, "Learning Curve", 0),
                    (K::CheckBox, [52, 25, 153, 68], true, "Support", 0),
                    (K::User, [10, 100, 200, 160], true, "", 1),
                ],
            ),
        );
    }
    let call = "put Dialoger(\"Dev Features\", \"3◊StyledText Chicago,12,u.\", \"4◊1\", \"5◊0\", \"6◊userList Geneva,9\" & return & \"Ada\" & return & \"C\") into r";
    // Support turned on, then OK: OK's line has its name.
    i.canned_answers.extend([DialogReply::Hit(5), DialogReply::Hit(1)]);
    assert_eq!(run(&mut i, &format!("{call}\nput r")), "OK\n\nStrengths ^0\ntrue\ntrue\n");
    // Cancel.
    i.canned_answers.push(DialogReply::Hit(2));
    assert_eq!(run(&mut i, &format!("{call}\nput line 2 of r && line 4 of r")), "Cancel true");
    // With no one to answer, it's canceled rather than waiting.
    assert_eq!(run(&mut i, &format!("{call}\nput line 2 of r")), "Cancel");
    assert_eq!(run(&mut i, "put there is a window \"Dev Features\""), "false");
    // A dialog the stack doesn't have.
    assert_eq!(run(&mut i, "put Dialoger(\"Nothing\")"), "Error: unable to find DLOG \"Nothing\"");
    // A floating one: Dialoger returns at once and it stays open, read with
    // dContents. The second argument fills ^0; one after it with no number
    // is the item at its place, as the Developer Info Assistant's Progress
    // puts its text in item 3.
    run(&mut i, "get Dialoger(\"Dev Features\" & return & \"float\", \"Mentioned\", \"Story\", \"4◊true\", \"6◊userList Geneva,9\" & return & \"Ada\" & return & \"C\")");
    assert_eq!(run(&mut i, "put dContents(\"Dev Features\") && dContents(\"Nope\")"), "1 0");
    assert_eq!(run(&mut i, "put dContents(\"Dev Features\", 4) && dContents(\"Dev Features\", 6, \"all\")"), "true Ada\nC");
    assert_eq!(run(&mut i, "put dContents(\"Dev Features\", 3)"), "Story");
    assert_eq!(run(&mut i, "put dContents(\"Dev Features\", 0, \"kind\")"), "Float");
    let d = lock(&i.world).script_windows.iter().find(|t| t.name == "Dev Features").and_then(|t| t.dialoger.clone()).unwrap();
    assert_eq!(d.shown_text("Strengths ^0"), "Strengths Mentioned");
    // Its clicks: a check box turns; OK closes it.
    i.handle_event(Event::DialogerHit { name: "Dev Features".into(), item: 4 });
    assert_eq!(run(&mut i, "put dContents(\"Dev Features\", 4)"), "false");
    i.handle_event(Event::DialogerHit { name: "Dev Features".into(), item: 1 });
    assert_eq!(run(&mut i, "put dContents(\"Dev Features\")"), "0");
    run(&mut i, "get Dialoger(\"Dev Features\" & return & \"float\")");
    run(&mut i, "close window \"Dev Features\"");
    assert_eq!(run(&mut i, "put dContents(\"Dev Features\")"), "0");
    // The Developer Info Assistant's About box: its picture is a push
    // button that closes it.
    lock(&i.world).stack.dialogs.insert(13467, dialog_template("AboutThisStack", &[(K::Picture, [-1, -1, 349, 203], true, "", 2666), (K::StaticText, [200, 9, 329, 36], false, "Version 1.0", 0)]));
    i.canned_answers.push(DialogReply::Hit(1));
    assert_eq!(run(&mut i, "put Dialoger(\"AboutThisStack\" & return & \"DefaultItem 1\", \"1◊PictButton 1,1,Push\", \"2◊StyledText Palatino,10,R.\" & return & \"Version 4.1\")"), "1\nVersion 4.1");
}

#[test]
fn dialoger_hit_handlers() {
    // The Developer Info Assistant's ListMaker: a hit handler that adds,
    // deletes and moves lines with dChanger, and reads the list on OK, while
    // the dialog is still open.
    use crate::model::dialogs::DialogItemKind as K;
    let mut i = interp_with(stack());
    {
        let mut w = lock(&i.world);
        w.stack.dialogs.insert(
            3741,
            dialog_template(
                "ListMaker",
                &[
                    (K::Button, [194, 166, 274, 184], true, "OK", 0),
                    (K::Button, [194, 189, 274, 207], true, "Cancel", 0),
                    (K::EditText, [13, 179, 180, 195], true, "", 0),
                    (K::User, [10, 27, 183, 174], true, "", 1),
                    (K::Button, [192, 84, 277, 102], true, "Add", 0),
                    (K::Button, [192, 107, 277, 125], true, "Delete", 0),
                    (K::Button, [192, 25, 277, 43], true, "Up", 0),
                    (K::StaticText, [11, 11, 182, 27], false, "^0", 0),
                ],
            ),
        );
        let card = w.card_id;
        w.stack.card_mut(card).unwrap().script = "on ListMakerHits dialog, i\n  global gLast, gLog\n  put i & \",\" after gLog\n  if i = 0 then dChanger dialog, \"5◊Inactivate\"\n  else if i = 1 then put dContents(dialog, 4, \"all\") into gLast\n  else if i = 5 then\n    put dContents(dialog, 4) into ln\n    if ln is empty then put 32000 into ln\n    dChanger dialog, \"3◊\", \"4◊Add\" && ln & return & \"Carol\" & return & \"deselect 0,32000\"\n  else if i = 6 then\n    dChanger dialog, \"4◊Delete\" && dContents(dialog, 4)\n  else if i = 7 then\n    get dContents(dialog, 4)\n    dChanger dialog, \"4◊swap\" && it - 1 & \",\" & it & return & \"Select\" && it - 1\n  end if\nend ListMakerHits\n\nfunction MakeList thePrompt, theList\n  global gLast\n  get Dialoger(\"ListMaker\", thePrompt, \"0◊on ListMakerHits\", \"4◊userList Chicago,12,SingleSelection\" & return & theList)\n  return gLast\nend MakeList".into();
    }
    // With Bob (line 2) chosen, Add makes Carol line 2 and chooses her; Up
    // moves her above Ann; Delete takes her out; OK.
    let card = lock(&i.world).card_id;
    {
        let mut w = lock(&i.world);
        let s = &mut w.stack.card_mut(card).unwrap().script;
        *s = s.replace("if i = 0 then dChanger dialog, \"5◊Inactivate\"", "if i = 0 then dChanger dialog, \"4◊Select 2\"");
        *s = s.replace("deselect 0,32000\"", "Select 2\"");
    }
    i.canned_answers.extend([DialogReply::Hit(5), DialogReply::Hit(7), DialogReply::Hit(6), DialogReply::Hit(1)]);
    run(&mut i, "global gLog\nput empty into gLog");
    assert_eq!(run(&mut i, "put MakeList(\"Your friends:\", \"Ann\" & return & \"Bob\")"), "Ann\nBob");
    assert_eq!(run(&mut i, "global gLog\nput gLog"), "0,5,7,6,1,");

    // Package Maker: OK doesn't close it (noDismiss) until the handler says.
    lock(&i.world).stack.dialogs.insert(
        17238,
        dialog_template(
            "Package Maker",
            &[(K::Button, [126, 205, 206, 223], true, "OK", 0), (K::Button, [15, 205, 95, 223], true, "Cancel", 0), (K::CheckBox, [18, 177, 205, 193], true, "Compress", 0)],
        ),
    );
    lock(&i.world).stack.card_mut(card).unwrap().script = "on PackHits dialog, itemHit\n  global gN\n  if itemHit = 0 then dChanger dialog, \"1◊noDismiss\"\n  else if itemHit = 1 then\n    add 1 to gN\n    if gN = 2 then dChanger dialog, \"0◊dismiss\"\n  end if\nend PackHits".into();
    i.canned_answers.extend([DialogReply::Hit(1), DialogReply::Hit(3), DialogReply::Hit(1)]);
    assert_eq!(run(&mut i, "global gN\nput 0 into gN\nput Dialoger(\"Package Maker\" & return & \"on PackHits\") into r\nput line 1 of r && line 3 of r && gN"), "OK true 2");

    // A floating Help window: its handler sets it up, its pop-up's choice
    // reaches the handler, and other scripts change it with dChanger.
    lock(&i.world).stack.dialogs.insert(
        15808,
        dialog_template("Help", &[(K::Button, [140, 214, 220, 232], true, "ok", 0), (K::StaticText, [8, 36, 253, 195], false, "", 0), (K::User, [69, 9, 206, 27], true, "", 2)]),
    );
    lock(&i.world).stack.card_mut(card).unwrap().script = "on HelpHits dialog, itemHit\n  if itemHit = 0 then dChanger dialog, \"2◊General help\"\n  else if itemHit = 3 then dChanger dialog, \"2◊About\" && dContents(dialog, 3)\nend HelpHits".into();
    run(&mut i, "get Dialoger(\"Help\" & return & \"on HelpHits\" & return & \"float\", \"\", \"3◊userPop\" & return & \"General\" & return & \"Sending\")");
    assert_eq!(run(&mut i, "put dContents(\"Help\", 2)"), "General help");
    run(&mut i, "dChanger \"Help\", \"3◊Select 2\"");
    i.handle_event(Event::DialogerHit { name: "Help".into(), item: 3 });
    assert_eq!(run(&mut i, "put dContents(\"Help\", 2)"), "About Sending");
    run(&mut i, "dChanger \"Help\", \"0◊dismiss\"");
    assert_eq!(run(&mut i, "put there is a window \"Help\""), "false");
    assert_eq!(run(&mut i, "dChanger \"Help\", \"2◊x\"\nput the result"), "Error: no such dialog exists: Help");
}

#[test]
fn text_compares_exactly() {
    // As HyperCard 2.4.1 answers (checked in it): spaces and returns count,
    // case doesn't, and numbers compare by value.
    assert_eq!(eval("(space is return) && (space is empty) && (\"a \" is \"a\") && (\"a\" is \"a \") && (\" a\" is \"a\") && (tab is empty) && (return is empty)"), "false false false false false false false");
    assert_eq!(eval("(char 1 of empty is space) && (\"ABC\" is \"abc\") && (\"5\" = 5.0) && (\" 5\" = 5)"), "false true true true");
}

#[test]
fn image_parts() {
    // An image is a field for pictures: its contents are where the picture
    // comes from, and a background one keeps a source per card.
    let dir = std::env::temp_dir().join(format!("randomcard-images-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("poster.png");
    std::fs::write(&file, crate::paint::image_file::encode_png(30, 45, &[255u8; 30 * 45 * 4]).unwrap()).unwrap();
    let mut s = stack();
    s.backgrounds[0].parts.push(Part::new_image(40, "Poster", Rect::new(300, 20, 400, 170)));
    s.pictures.insert(128, crate::model::NamedPicture { name: "Logo".into(), picture: Picture::new(12, 8) });
    let mut i = interp_with(s);
    // Old scripts' variables called image still work.
    assert_eq!(run(&mut i, "put 5 into image\nadd 1 to image\nput image"), "6");
    assert_eq!(run(&mut i, "put the number of images && the number of bkgnd images"), "1 1");
    run(&mut i, &format!("put \"{}\" into image \"Poster\"", file.display()));
    assert_eq!(run(&mut i, "put the pictureWidth of image \"Poster\" && the pictureHeight of image 1"), "30 45");
    assert_eq!(run(&mut i, "put image \"Poster\" is the source of image \"Poster\""), "true");
    // Each card its own picture.
    run(&mut i, "go next card\nset the source of bkgnd image \"Poster\" to \"Logo\"");
    assert_eq!(run(&mut i, "put the pictureWidth of image \"Poster\" && image \"Poster\""), "12 Logo");
    run(&mut i, "go prev card");
    assert_eq!(run(&mut i, "put the pictureWidth of image \"Poster\""), "30");
    // How it fills its rect.
    assert_eq!(run(&mut i, "put the fit of image 1"), "fit");
    run(&mut i, "set the fit of image 1 to fill");
    assert_eq!(run(&mut i, "put the fit of image 1"), "fill");
    assert!(run(&mut i, "set the fit of image 1 to squash").starts_with("ERROR"));
    // A picture that isn't there shows nothing, and has no size.
    run(&mut i, "put \"nowhere.png\" into image 1");
    assert_eq!(run(&mut i, "put the pictureWidth of image 1"), "");
    // New Image makes one.
    run(&mut i, "doMenu \"New Image\"");
    assert_eq!(run(&mut i, "put the number of card images"), "1");
    let _ = std::fs::remove_dir_all(&dir);
    // Fit: all of it, centered; fill: cropped to cover.
    use crate::model::ImageFit;
    let r = Rect::new(0, 0, 100, 100);
    assert_eq!(ImageFit::Fit.place(50.0, 100.0, r), ([25.0, 0.0, 75.0, 100.0], [0.0, 0.0, 50.0, 100.0]));
    assert_eq!(ImageFit::Fill.place(50.0, 100.0, r), ([0.0, 0.0, 100.0, 100.0], [0.0, 25.0, 50.0, 75.0]));
}

#[test]
fn images_from_the_web() {
    // A picture from the web loads in the background: the image has no
    // picture until it comes, then shows it; a missing one fails, with why.
    use std::io::{Read, Write};
    let png = crate::paint::image_file::encode_png(20, 10, &[255u8; 20 * 10 * 4]).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut buf = [0u8; 2048];
            let n = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            if request.starts_with("GET /poster.png") {
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", png.len());
                let _ = stream.write_all(&png);
            } else {
                let _ = write!(stream, "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            }
        }
    });
    let mut s = stack();
    s.backgrounds[0].parts.push(Part::new_image(40, "Poster", Rect::new(300, 20, 400, 170)));
    let mut i = interp_with(s);
    let wait = |url: &str| {
        let start = std::time::Instant::now();
        while matches!(super::images::web_state(url), Some(super::images::Image::Loading)) && start.elapsed().as_secs() < 10 {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    };
    let good = format!("{base}/poster.png");
    run(&mut i, &format!("put \"{good}\" into image \"Poster\""));
    // Asking for its size starts it loading; it has none yet.
    run(&mut i, "get the pictureWidth of image \"Poster\"");
    wait(&good);
    assert_eq!(run(&mut i, "put the pictureWidth of image \"Poster\" && the pictureHeight of image \"Poster\""), "20 10");
    let bad = format!("{base}/missing.png");
    run(&mut i, &format!("put \"{bad}\" into image \"Poster\"\nget the pictureWidth of image \"Poster\""));
    wait(&bad);
    assert_eq!(super::images::web_state(&bad), Some(super::images::Image::Failed("404 Not Found".into())));
    let arrived = super::images::arrived();
    assert!(arrived.contains(&good) && arrived.contains(&bad), "{arrived:?}");
}

#[test]
fn the_image_tool() {
    // Its own tool, number 19 after HyperCard's 18, and New Image in the
    // Objects menu chooses it.
    let mut i = interp_with(stack());
    run(&mut i, "choose image tool");
    assert_eq!(run(&mut i, "put the tool"), "image tool");
    run(&mut i, "choose tool 19");
    assert_eq!(run(&mut i, "put the tool"), "image tool");
    run(&mut i, "choose browse tool\ndoMenu \"New Image\"");
    assert_eq!(run(&mut i, "put the tool && the number of card images"), "image tool 1");
    // HyperCard's first five Info items stay where they were.
    assert_eq!(run(&mut i, "put menuItem 3 of menu \"Objects\" & \"/\" & menuItem 6 of menu \"Objects\""), "Card Info…/Image Info…");
    // An image's styles: no frame, a frame, a shadow.
    run(&mut i, "set the style of card image 1 to shadow");
    assert_eq!(run(&mut i, "put the style of card image 1"), "shadow");
}
