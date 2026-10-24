//! The debugger, as in HyperCard: a script stops at a checkpoint (set in the
//! script editor, or with `debug checkpoint`), shows where it is, and waits
//! to be stepped, traced, continued or aborted. Also the message watcher's
//! log of messages sent.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::interp::{Ctl, Interp, R};
use super::world::{DebugCmd, DebugPause, MESSAGE_LOG_LEN, WatchedMessage, same_script};

/// How the debugger runs a script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stepping {
    /// Stop only at checkpoints.
    Run,
    /// Stop at the next line run with at most this many handlers running.
    Over(usize),
    /// Stop at the next line run.
    Into,
    /// Show every line, without stopping.
    Trace,
}

static MESSAGE_SEQ: AtomicU64 = AtomicU64::new(0);

/// Messages too frequent to be worth watching unless asked for.
pub const FREQUENT_MESSAGES: &[&str] = &["idle", "mousewithin", "mousestilldown"];

impl Interp {
    /// Called before each line of a handler runs.
    pub(super) fn debug_hook(&mut self, line: usize) -> R<()> {
        let depth = self.frames.len();
        let stop = std::mem::take(&mut self.pause_next)
            || match self.stepping {
                Stepping::Run => false,
                Stepping::Over(d) => depth <= d,
                Stepping::Into | Stepping::Trace => true,
            }
            || {
                let me = self.frame().me;
                let w = self.w();
                !w.checkpoints.is_empty() && w.checkpoints.iter().any(|(o, l)| *l == line && same_script(*o, me))
            };
        if stop { self.pause(line) } else { Ok(()) }
    }

    fn publish(&mut self, line: usize, paused: bool) {
        let f = self.frame();
        let mut locals: Vec<(String, String)> = f.vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        locals.sort();
        let pause = DebugPause {
            obj: f.me,
            handler: f.message.clone(),
            line,
            locals,
            // The first frame is the event's own, not a handler's.
            calls: self.frames.iter().skip(1).map(|f| f.message.clone()).collect(),
            paused,
        };
        let globals = self.sorted_globals();
        let mut w = self.w();
        w.debug = Some(pause);
        w.globals_view = globals;
    }

    fn sorted_globals(&self) -> Vec<(String, String)> {
        let mut g: Vec<(String, String)> = self.globals.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        g.sort();
        g
    }

    /// Stop at a line and do what the debugger says.
    fn pause(&mut self, line: usize) -> R<()> {
        self.paused_lines.push(line);
        if self.stepping == Stepping::Trace {
            self.publish(line, false);
            let ticks = self.w().trace_delay.max(2);
            self.sleep(Duration::from_millis(ticks as u64 * 1000 / 60))?;
            // A command while tracing takes over (Step stops the trace).
            if let Some(cmd) = self.debug_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
                return self.obey(cmd, line);
            }
            return Ok(());
        }
        self.publish(line, true);
        if let Some(rx) = &self.debug_rx {
            // Anything sent before this stop is stale.
            while rx.try_recv().is_ok() {}
        }
        loop {
            let cmd = match &self.debug_rx {
                Some(rx) => match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(cmd) => cmd,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if self.abort.load(Ordering::SeqCst) {
                            return Err(Ctl::ExitToHyperCard);
                        }
                        continue;
                    }
                    Err(_) => return Err(Ctl::ExitToHyperCard),
                },
                // Tests answer from a list; with nothing to say, carry on.
                None if self.canned_debug.is_empty() => DebugCmd::Go,
                None => self.canned_debug.remove(0),
            };
            match cmd {
                DebugCmd::SetVar { name, value } => {
                    self.debug_set_var(&name, value);
                    self.publish(line, true);
                }
                cmd => return self.obey(cmd, line),
            }
        }
    }

    fn obey(&mut self, cmd: DebugCmd, line: usize) -> R<()> {
        self.stepping = match cmd {
            DebugCmd::Step => Stepping::Over(self.frames.len()),
            DebugCmd::StepInto => Stepping::Into,
            DebugCmd::Trace => Stepping::Trace,
            DebugCmd::Go => Stepping::Run,
            DebugCmd::SetVar { name, value } => {
                self.debug_set_var(&name, value);
                return Ok(());
            }
        };
        if self.stepping == Stepping::Run {
            self.w().debug = None;
        } else {
            self.publish(line, false);
        }
        Ok(())
    }

    /// Change a variable from the variable watcher: a local of the handler
    /// stopped in, or a global.
    fn debug_set_var(&mut self, name: &str, value: String) {
        let name = name.to_lowercase();
        let local = self.frames.last().is_some_and(|f| f.vars.contains_key(&name) && !f.globals.contains(&name));
        if local {
            self.frame_mut().vars.insert(name, value);
        } else {
            self.globals.insert(name, value);
        }
    }

    /// Between events, the variable watcher can still change globals.
    pub(super) fn debug_commands_while_idle(&mut self) {
        let Some(rx) = &self.debug_rx else { return };
        let cmds: Vec<DebugCmd> = rx.try_iter().collect();
        let mut changed = false;
        for cmd in cmds {
            if let DebugCmd::SetVar { name, value } = cmd {
                self.globals.insert(name.to_lowercase(), value);
                changed = true;
            }
        }
        if changed {
            let g = self.sorted_globals();
            self.w().globals_view = g;
        }
    }

    /// When the last handler finishes, the debugger lets go.
    pub(super) fn end_debugging(&mut self) {
        self.stepping = Stepping::Run;
        self.pause_next = false;
        let g = self.sorted_globals();
        let mut w = self.w();
        w.debug = None;
        if w.globals_view != g {
            w.globals_view = g;
        }
    }

    /// Log a message for the message watcher, if it's open.
    pub(super) fn watch_message(&mut self, name: &str) -> Option<u64> {
        let depth = self.frames.len().saturating_sub(1);
        let mut w = self.w();
        if !w.watch_messages {
            return None;
        }
        let seq = MESSAGE_SEQ.fetch_add(1, Ordering::Relaxed);
        if w.message_log.len() >= MESSAGE_LOG_LEN {
            w.message_log.pop_front();
        }
        w.message_log.push_back(WatchedMessage { seq, name: name.to_string(), depth, handled: false });
        Some(seq)
    }

    pub(super) fn message_handled(&mut self, seq: Option<u64>) {
        let Some(seq) = seq else { return };
        if let Some(m) = self.w().message_log.iter_mut().rev().find(|m| m.seq == seq) {
            m.handled = true;
        }
    }
}
