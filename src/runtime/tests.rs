//! End-to-end interpreter tests: scripts run against a stack with no UI.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use super::interp::Interp;
use super::world::{DialogReply, Event, ObjRef, UiRequest, World, lock};
use crate::model::{Card, Layer, Part, Picture, Rect, Stack};

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
fn demo_stack_runs() {
    let mut i = interp_with(Stack::demo());
    i.handle_event(Event::Startup);
    assert_eq!(run(&mut i, "put field \"Page\""), "Card 1 of 8");
    let current = |i: &Interp| {
        let w = lock(&i.world);
        w.stack.card(w.card_id).unwrap().name.clone()
    };
    assert_eq!(click(&mut i, "Home", "Take the Tour"), None);
    assert_eq!(current(&i), "Basics");
    for b in ["Count Words", "Chunks", "Do Some Math", "What Time Is It?"] {
        i.canned_answers.push(DialogReply::Button("OK".into()));
        assert_eq!(click(&mut i, "Basics", b), None, "{b}");
    }
    i.canned_answers.push(DialogReply::Text(Some("Dan".into())));
    i.canned_answers.push(DialogReply::Button("OK".into()));
    assert_eq!(click(&mut i, "Basics", "Say Hello"), None);
    assert_eq!(click(&mut i, "Home", "Dice & Effects"), None);
    assert_eq!(click(&mut i, "Dice", "Roll"), None);
    assert!(run(&mut i, "put field \"Result\" of card \"Dice\"").starts_with("You rolled"));
    // A fast bounce runs to completion.
    run(&mut i, "go to card \"Animation\"\nset the hilite of button \"Fast\" to true");
    assert_eq!(click(&mut i, "Animation", "Bounce"), None);
    assert_eq!(click(&mut i, "Animation", "Show the ball"), None);
    // Address book on its own background.
    assert_eq!(click(&mut i, "Home", "Address Book"), None);
    assert_eq!(current(&i), "Grace Hopper");
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
    assert_eq!(run(&mut i, "put the number of cards"), "8");
    assert_eq!(click(&mut i, "Grace Hopper", "Home"), None);
    assert_eq!(current(&i), "Home");
}

#[test]
fn icons_by_id_or_name() {
    let mut s = stack();
    s.icons.insert(128, crate::model::Icon { name: "Star".into(), picture: Picture::new(32, 32) });
    let mut i = interp_with(s);
    assert_eq!(run(&mut i, "set the icon of button 1 to \"star\"\nput the icon of button 1"), "128");
    assert_eq!(run(&mut i, "set the icon of button 1 to 0\nput the icon of button 1"), "0");
    assert!(run(&mut i, "set the icon of button 1 to \"Nope\"").starts_with("ERROR: No such icon"));
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
    assert_eq!(pixel(&i, 30, 10), Pixel::Black);
    // Filled rectangles use the current pattern inside.
    run(&mut i, "choose rectangle tool\nset the filled to true\nset the pattern to 2\ndrag from 100,100 to 140,140");
    assert_eq!((pixel(&i, 100, 120), pixel(&i, 120, 120)), (Pixel::Black, Pixel::White));
    // click at with the bucket fills with the pattern (black).
    run(&mut i, "set the pattern to 1\nchoose bucket tool\nclick at 120,120");
    assert_eq!(pixel(&i, 120, 120), Pixel::Black);
    // The text tool: click to place, then type paints text.
    run(&mut i, "choose text tool\nset the textSize to 24\nclick at 200,200\ntype \"Hi\"");
    let w = lock(&i.world);
    let pic = &w.stack.cards[0].picture;
    let inked = (200..240).flat_map(|x| (200..230).map(move |y| (x, y))).filter(|(x, y)| pic.get(*x, *y) == Pixel::Black).count();
    assert!(inked > 20, "{inked}");
    drop(w);
    assert_eq!(run(&mut i, "put the pattern & \",\" & the textSize & \",\" & the tool"), "1,24,text tool");
    assert_eq!(run(&mut i, "set the polySides to 6\nset the brush to 9\nput the polySides && the brush"), "6 9");
}
