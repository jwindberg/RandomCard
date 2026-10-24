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
        "Your Home stack. Its script is in every stack's message path, so handlers you put there \
         (Objects ▸ Stack Script) work anywhere. Go ▸ Home (Ctrl+H) brings you back.",
        12,
    );
    about.name = "About".into();
    card.parts.push(about);

    let buttons = [
        ("Take the Tour", "on mouseUp\n  visual effect dissolve\n  go to stack \"Tour\"\nend mouseUp"),
        ("Open a Stack…", "on mouseUp\n  doMenu \"Open Stack…\"\nend mouseUp"),
        ("Import a HyperCard Stack…", "on mouseUp\n  doMenu \"Import HyperCard Stack…\"\nend mouseUp"),
        ("New Stack…", "on mouseUp\n  doMenu \"New Stack…\"\nend mouseUp"),
    ];
    for (i, (name, script)) in buttons.iter().enumerate() {
        let y = 140 + i as i32 * 44;
        let mut b = Part::new_button(id(), name, Rect::new(156, y, 356, y + 30));
        b.style = PartStyle::RoundRect;
        b.script = (*script).into();
        card.parts.push(b);
    }
    s.next_id = next;
    s
}
