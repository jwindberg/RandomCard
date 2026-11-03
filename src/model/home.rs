//! The Home stack RandomCard makes the first time it runs, as HyperCard came
//! with one. It opens at startup, Go ▸ Home returns to it, and its script is
//! part of every stack's message path.

use super::*;

const HOME_SCRIPT: &str = r#"-- This is the Home stack's script. It comes after every stack (and the
-- stacks in use) in the message path, so handlers and functions you put
-- here work in any stack: try `put homeVersion()` in the message box.

function homeVersion
  return "Home 1.0"
end homeVersion
"#;

pub fn build() -> Stack {
    let mut s = Stack::new("Home");
    s.script = HOME_SCRIPT.into();
    let mut next = s.next_id;
    let mut id = || {
        next += 1;
        next - 1
    };
    let card = &mut s.cards[0];
    card.name = "Home".into();

    let label = |id: Id, rect: Rect, text: &str, size: u32| {
        let mut p = Part::new_field(id, "", rect);
        p.style = PartStyle::Transparent;
        p.lock_text = true;
        p.text_size = size;
        p.text_align = TextAlign::Center;
        p.contents = text.into();
        p
    };
    let mut title = label(id(), Rect::new(20, 20, 492, 60), "Home", 28);
    title.name = "Title".into();
    card.parts.push(title);
    let mut about = label(
        id(),
        Rect::new(40, 66, 472, 120),
        &format!(
            "Your Home stack. Its script is in every stack's message path, so handlers you put there \
             (Objects ▸ Stack Script) work anywhere. Go ▸ Home ({}) brings you back.",
            super::shortcut("H")
        ),
        12,
    );
    about.name = "About".into();
    card.parts.push(about);

    let buttons = [
        ("Take the Tour", "on mouseUp\n  visual effect dissolve\n  go to stack \"Tour\"\nend mouseUp"),
        ("Open a Stack…", "on mouseUp\n  doMenu \"Open Stack…\"\nend mouseUp"),
        ("Import a HyperCard Stack…", "on mouseUp\n  doMenu \"Import HyperCard Stack…\"\nend mouseUp"),
        ("New Stack…", "on mouseUp\n  doMenu \"New Stack…\"\nend mouseUp"),
        ("User Preferences", "on mouseUp\n  visual effect dissolve\n  go to card \"User Preferences\"\nend mouseUp"),
    ];
    for (i, (name, script)) in buttons.iter().enumerate() {
        let y = 136 + i as i32 * 38;
        let mut b = Part::new_button(id(), name, Rect::new(156, y, 356, y + 30));
        b.style = PartStyle::RoundRect;
        b.script = (*script).into();
        card.parts.push(b);
    }
    s.next_id = next;
    add_preference_cards(&mut s);
    s
}

/// The user levels, as the User Preferences card names its buttons.
const LEVELS: [&str; 5] = ["Browsing", "Typing", "Painting", "Authoring", "Scripting"];

/// The search-path cards: their names, and what each one's paths are for.
const SEARCH_PATHS: [(&str, &str); 3] = [
    ("Stacks", "Folders to look in for a stack that isn't beside the one asking for it, one per line."),
    ("Applications", "Folders to look in for applications, one per line."),
    ("Documents", "Folders to look in for documents, one per line."),
];

/// Give a Home stack HyperCard's own cards if it lacks them: "User
/// Preferences", and "Stacks", "Applications" and "Documents" with their
/// search paths. Stacks made as replacements for Home's script go to them
/// for the user's name, level and paths (their getHomeInfo handlers). The
/// level is kept in step with Settings; see `show_level`. Answers whether
/// it added them.
pub fn add_preference_cards(s: &mut Stack) -> bool {
    if s.cards.iter().any(|c| c.name.eq_ignore_ascii_case("User Preferences")) {
        return false;
    }
    let mut next = s.next_id;
    let mut id = || {
        next += 1;
        next - 1
    };
    let (w, h) = (s.width, s.height);
    let text = |id: Id, rect: Rect, contents: &str, size: u32| {
        let mut p = Part::new_field(id, "", rect);
        p.style = PartStyle::Transparent;
        p.lock_text = true;
        p.text_size = size;
        p.contents = contents.into();
        p
    };
    let go_home = |id: Id| {
        let mut b = Part::new_button(id, "Home", Rect::new(20, h as i32 - 44, 110, h as i32 - 16));
        b.style = PartStyle::RoundRect;
        b.script = "on mouseUp\n  visual effect dissolve\n  go home\nend mouseUp".into();
        b
    };

    // User Preferences, on Home's own background.
    let mut prefs = Card {
        id: id(),
        name: "User Preferences".into(),
        background: s.cards[0].background,
        picture: Picture::new(w, h),
        show_pict: true,
        ..Card::default()
    };
    let mut title = text(id(), Rect::new(20, 16, 492, 46), "User Preferences", 20);
    title.text_style = "bold".into();
    prefs.parts.push(title);
    prefs.parts.push(text(id(), Rect::new(20, 60, 120, 80), "Your name:", 12));
    let mut name = Part::new_field(id(), "User Name", Rect::new(120, 58, 340, 80));
    name.style = PartStyle::Rectangle;
    name.text_size = 12;
    name.contents = std::env::var("USER").or_else(|_| std::env::var("USERNAME")).unwrap_or_default();
    prefs.parts.push(name);
    prefs.parts.push(text(id(), Rect::new(20, 96, 200, 116), "User level:", 12));
    for (i, level) in LEVELS.iter().enumerate().rev() {
        let y = 120 + (4 - i as i32) * 24;
        let mut b = Part::new_button(id(), level, Rect::new(40, y, 200, y + 20));
        b.style = PartStyle::Radio;
        b.family = 1;
        b.hilite = i == 4;
        b.script = format!("on mouseUp\n  set userLevel to {}\nend mouseUp", i + 1);
        prefs.parts.push(b);
    }
    // The level as a number, as HyperCard's Home kept it.
    let mut number = Part::new_field(id(), "User Level", Rect::new(200, 96, 240, 116));
    number.visible = false;
    number.lock_text = true;
    number.contents = "5".into();
    prefs.parts.push(number);
    for (i, option) in ["Text Arrows", "Power Keys", "Blind Typing"].iter().enumerate() {
        let y = 120 + i as i32 * 24;
        let mut b = Part::new_button(id(), option, Rect::new(260, y, 460, y + 20));
        b.style = PartStyle::CheckBox;
        b.hilite = true;
        b.script = format!("on mouseUp\n  set {} to the hilite of me\nend mouseUp", option.replace(' ', ""));
        prefs.parts.push(b);
    }
    let mut paths = Part::new_button(id(), "Search Paths", Rect::new(130, h as i32 - 44, 260, h as i32 - 16));
    paths.style = PartStyle::RoundRect;
    paths.script = "on mouseUp\n  visual effect dissolve\n  go to card \"Stacks\"\nend mouseUp".into();
    prefs.parts.push(paths);
    prefs.parts.push(go_home(id()));
    s.cards.push(prefs);

    // The search paths: one background, a card for each kind of file.
    let bg_id = id();
    let mut bg = Background { id: bg_id, name: "Search Paths".into(), picture: Picture::new(w, h), show_pict: true, ..Background::default() };
    let field_id = id();
    let mut field = Part::new_field(field_id, "Paths", Rect::new(20, 90, 492, h as i32 - 60));
    field.style = PartStyle::Scrolling;
    field.text_size = 12;
    bg.parts.push(field);
    let mut kinds = Part::new_button(id(), "Next Kind", Rect::new(130, h as i32 - 44, 260, h as i32 - 16));
    kinds.style = PartStyle::RoundRect;
    kinds.script = "on mouseUp\n  if the short name of this card is \"Documents\" then go to card \"Stacks\"\n  else go next card\nend mouseUp".into();
    bg.parts.push(kinds);
    bg.parts.push(go_home(id()));
    s.backgrounds.push(bg);
    for (kind, about) in SEARCH_PATHS {
        let mut c = Card { id: id(), name: kind.into(), background: bg_id, picture: Picture::new(w, h), show_pict: true, ..Card::default() };
        let mut title = text(id(), Rect::new(20, 16, 492, 46), &format!("Search Paths: {kind}"), 20);
        title.text_style = "bold".into();
        c.parts.push(title);
        c.parts.push(text(id(), Rect::new(20, 50, 492, 86), about, 12));
        c.bg_text.insert(field_id, String::new());
        s.cards.push(c);
    }
    s.next_id = next;
    true
}

/// Show the user level on the User Preferences card: the number in its
/// "User Level" field and the level's button hilited.
pub fn show_level(s: &mut Stack, level: u8) {
    let Some(card) = s.cards.iter_mut().find(|c| c.name.eq_ignore_ascii_case("User Preferences")) else { return };
    for p in &mut card.parts {
        if p.name.eq_ignore_ascii_case("User Level") {
            p.contents = level.to_string();
        } else if p.family == 1 && let Some(i) = LEVELS.iter().position(|l| l.eq_ignore_ascii_case(&p.name)) {
            p.hilite = i + 1 == level as usize;
        }
    }
}

/// The folders on one of Home's search-path cards ("Stacks",
/// "Applications" or "Documents").
pub fn search_paths(s: &Stack, kind: &str) -> Vec<String> {
    let Some(card) = s.cards.iter().find(|c| c.name.eq_ignore_ascii_case(kind)) else { return vec![] };
    let Some(field) = s.background(card.background).and_then(|b| b.parts.iter().find(|p| p.name.eq_ignore_ascii_case("Paths"))) else {
        return vec![];
    };
    let text = card.bg_text.get(&field.id).cloned().unwrap_or_default();
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
}
