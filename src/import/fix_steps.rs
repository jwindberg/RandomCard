//! Fix steps: changes to an old stack's scripts for things RandomCard
//! doesn't carry on purpose, such as Mac colon paths on Linux and Windows,
//! or a prefixed external's old name. They're listed in FIX_STEPS.tsv at the
//! top of the project, one change a line: the stack's file name, the text
//! to find, what it becomes, and why. A change applies to every script in
//! the stack where its text appears, and only to our copy: the stack on the
//! disc isn't touched.

use crate::model::Stack;

const TABLE: &str = include_str!("../../FIX_STEPS.tsv");

/// One change, as listed (its reason is for people, not used here).
pub struct FixStep {
    pub stack: &'static str,
    pub old: &'static str,
    pub new: &'static str,
}

/// The listed lines: lines starting with # are comments.
fn lines() -> impl Iterator<Item = &'static str> {
    TABLE.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
}

/// Every fix step in FIX_STEPS.tsv.
pub fn all() -> Vec<FixStep> {
    lines()
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some(FixStep { stack: f.next()?, old: f.next()?, new: f.next()? })
        })
        .collect()
}

/// Apply the fix steps listed for a stack file (by its name) to its scripts.
/// Answers how many changes were made.
pub fn apply(stack: &mut Stack, file_name: &str) -> usize {
    let steps: Vec<FixStep> = all().into_iter().filter(|s| s.stack == file_name).collect();
    if steps.is_empty() {
        return 0;
    }
    let mut made = 0;
    let mut fix = |script: &mut String| {
        for s in &steps {
            let n = script.matches(s.old).count();
            if n > 0 {
                *script = script.replace(s.old, s.new);
                made += n;
            }
        }
    };
    fix(&mut stack.script);
    for bg in &mut stack.backgrounds {
        fix(&mut bg.script);
        for p in &mut bg.parts {
            fix(&mut p.script);
        }
    }
    for card in &mut stack.cards {
        fix(&mut card.script);
        for p in &mut card.parts {
            fix(&mut p.script);
        }
    }
    made
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_reads() {
        for l in lines() {
            let f: Vec<&str> = l.split('\t').collect();
            assert_eq!(f.len(), 4, "stack, old, new, why: {l}");
            assert!(f.iter().all(|x| !x.trim().is_empty()), "an empty column: {l}");
            assert_ne!(f[1], f[2], "a fix step that changes nothing: {l}");
            // Applying it again changes nothing more.
            assert!(!f[2].contains(f[1]), "the new text holds the old: {l}");
        }
        assert_eq!(all().len(), lines().count());
    }

    #[test]
    fn applies_to_its_stack_only() {
        let mut s = Stack::new("Developer Info Assistant");
        s.script = "if last char of tempDIA is \":\" then exit repeat".into();
        let mut other = s.clone();
        assert_eq!(apply(&mut s, "Developer Info Assistant"), 1);
        assert!(s.script.contains("the pathSeparator"));
        assert_eq!(apply(&mut other, "Something Else"), 0);
    }
}
