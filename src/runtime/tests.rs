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
    assert_eq!(run(&mut i, "put field \"Page\""), "Card 1 of 15");
    let current = |i: &Interp| {
        let w = lock(&i.world);
        w.stack.card(w.card_id).unwrap().name.clone()
    };
    // The stack's own Tour menu.
    assert_eq!(run(&mut i, "put the number of menuItems of menu \"Tour\""), "14");
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
    // AddColor on, then off, on the card it's clicked on.
    run(&mut i, "go to card \"Drawing\"");
    assert_eq!(click(&mut i, "Drawing", "Color the House"), None);
    let drawing_colors = |i: &Interp| lock(&i.world).stack.cards.iter().find(|c| c.name == "Drawing").unwrap().colors.len();
    assert_eq!(drawing_colors(&i), 3);
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
    assert_eq!(current(&i), "Grace Hopper");
    i.handle_event(Event::PaletteButton("Tour Palette".into(), 2));
    assert_eq!(current(&i), "Contents");
    run(&mut i, "go to card \"Palettes\"");
    assert_eq!(click(&mut i, "Palettes", "Close Palette"), None);
    assert_eq!(run(&mut i, "put field \"Said\" of card \"Palettes\""), "closePalette Tour Palette");
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
    assert_eq!(run(&mut i, "put the number of cards"), "15");
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
    assert!(run(&mut i, "put the menus").starts_with("File\nEdit\nGo"));
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
    // Cards of other stacks can only be reached by going there.
    assert!(run(&mut i, "put the name of card 1 of stack \"Lib\"").starts_with("ERROR"));

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
/// home` leaves the stack instead of falling back to card 1.
fn give_home(i: &Interp) {
    let home = std::env::temp_dir().join(format!("randomcard-harness-home-{}.rcstack", std::process::id()));
    if !home.exists() {
        let mut h = Stack::home();
        h.path = Some(home.clone());
        h.save(&home).unwrap();
    }
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
    give_home(&i);
    let abort = i.abort.clone();
    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    // A click waiting on something that never comes (a loop until the user
    // types a number) is stopped after two seconds. Answers whether it was.
    let with_watchdog = |abort: &Arc<AtomicBool>, f: &mut dyn FnMut()| {
        let done = Arc::new(AtomicBool::new(false));
        let (d, a) = (done.clone(), abort.clone());
        let watchdog = std::thread::spawn(move || {
            for _ in 0..40 {
                std::thread::sleep(std::time::Duration::from_millis(50));
                if d.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
            }
            a.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        let start = std::time::Instant::now();
        f();
        done.store(true, std::sync::atomic::Ordering::SeqCst);
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
    // Windows, tool windows and the stack's size.
    assert_eq!(run(&mut i, "put there is a window \"message box\""), "true");
    assert_eq!(run(&mut i, "put there is no window \"Describe Problem\""), "true");
    i.handle_event(Event::MsgBox("hide tool window".into()));
    assert!(lock(&i.world).requests.drain(..).any(|r| matches!(r, UiRequest::ToolWindow(false))));
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
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
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
    // line of text on its first card.
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
        texts.flat_map(|t| t.lines().map(str::to_string).collect::<Vec<_>>()).find(|l| l.chars().filter(|c| c.is_alphanumeric()).count() > 3).unwrap_or_default()
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
    assert_eq!(run(&mut i, "put the scroll of card window"), "0,20");
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
