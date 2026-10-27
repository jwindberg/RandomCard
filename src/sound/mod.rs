//! Sound for HyperTalk's `play`, `dial` and `beep`: turning sampled sounds and
//! note strings like "c4q e g# a5h." into audio, and playing it.

use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use crate::model::Sound;

/// HyperCard's default tempo, in quarter notes per minute.
pub const DEFAULT_TEMPO: f32 = 120.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Note {
    /// MIDI note number, or None for a rest.
    pub midi: Option<u8>,
    /// Length in quarter notes.
    pub beats: f32,
}

/// Parse HyperTalk notes: a name (a–g, or r for rest) or a MIDI number, an
/// optional accidental (# or b), octave and duration (w h q e s t x), then
/// optional dots and a 3 for triplets. Octave and duration carry over from
/// the previous note; the first note defaults to octave 4, quarter note.
pub fn parse_notes(text: &str) -> Vec<Note> {
    let (mut octave, mut beats) = (4i32, 1.0f32);
    let mut out = Vec::new();
    for token in text.split_whitespace() {
        let t = token.to_ascii_lowercase();
        let mut chars = t.chars().peekable();
        let mut midi: Option<i32>;
        match chars.peek().copied() {
            Some(c @ 'a'..='g') => {
                chars.next();
                let semitone = match c {
                    'c' => 0,
                    'd' => 2,
                    'e' => 4,
                    'f' => 5,
                    'g' => 7,
                    'a' => 9,
                    _ => 11,
                };
                let mut accidental = 0;
                while let Some(&a) = chars.peek() {
                    match a {
                        '#' => accidental += 1,
                        'b' => accidental -= 1,
                        _ => break,
                    }
                    chars.next();
                }
                if let Some(d) = chars.peek().and_then(|c| c.to_digit(10)) {
                    octave = d as i32;
                    chars.next();
                }
                midi = Some((octave + 1) * 12 + semitone + accidental);
            }
            Some('r') => {
                chars.next();
                midi = None;
            }
            Some(c) if c.is_ascii_digit() => {
                let mut n = 0;
                while let Some(d) = chars.peek().and_then(|c| c.to_digit(10)) {
                    n = n * 10 + d as i32;
                    chars.next();
                }
                midi = Some(n);
                octave = n / 12 - 1;
            }
            _ => continue,
        }
        if let Some(&d) = chars.peek() {
            let length = match d {
                'w' => Some(4.0),
                'h' => Some(2.0),
                'q' => Some(1.0),
                'e' => Some(0.5),
                's' => Some(0.25),
                't' => Some(0.125),
                'x' => Some(0.0625),
                _ => None,
            };
            if let Some(l) = length {
                beats = l;
                chars.next();
            }
        }
        let mut this = beats;
        let mut dot = beats / 2.0;
        for c in chars {
            match c {
                '.' => {
                    this += dot;
                    dot /= 2.0;
                }
                '3' => this *= 2.0 / 3.0,
                _ => {}
            }
        }
        midi = midi.map(|m| m.clamp(0, 127));
        out.push(Note { midi: midi.map(|m| m as u8), beats: this });
    }
    out
}

const FADE_SECONDS: f32 = 0.004;

/// Render a sound, either as-is or as a sequence of notes, to mono f32 samples
/// at the sound's own rate.
pub fn render(sound: &Sound, notes: &[Note], tempo: f32) -> Vec<f32> {
    let src: Vec<f32> = sound.samples.iter().map(|s| *s as f32 / 32768.0).collect();
    if src.is_empty() {
        return Vec::new();
    }
    if notes.is_empty() {
        return src;
    }
    let tempo = if tempo > 0.0 { tempo } else { DEFAULT_TEMPO };
    let looped = sound.loop_end > sound.loop_start + 1 && (sound.loop_end as usize) <= src.len();
    let fade = ((sound.rate * FADE_SECONDS) as usize).max(1);
    let mut out = Vec::new();
    for note in notes {
        let len = (note.beats * 60.0 / tempo * sound.rate) as usize;
        let Some(midi) = note.midi else {
            out.resize(out.len() + len, 0.0);
            continue;
        };
        let ratio = 2f32.powf((midi as f32 - sound.base_note as f32) / 12.0);
        let mut pos = 0f32;
        let start = out.len();
        for _ in 0..len {
            if looped && pos >= sound.loop_end as f32 {
                pos -= (sound.loop_end - sound.loop_start) as f32;
            }
            let i = pos as usize;
            if i + 1 >= src.len() {
                break;
            }
            // Linear interpolation between neighbouring samples.
            let f = pos - i as f32;
            out.push(src[i] * (1.0 - f) + src[i + 1] * f);
            pos += ratio;
        }
        // Short fades at both ends avoid clicks between notes.
        let written = out.len() - start;
        let n = fade.min(written / 2);
        for k in 0..n {
            let g = k as f32 / n as f32;
            out[start + k] *= g;
            out[start + written - 1 - k] *= g;
        }
        out.resize(start + len, 0.0);
    }
    out
}

const SYNTH_RATE: f32 = 22050.0;

/// RandomCard's own stand-ins for HyperCard's built-in voices, used when a
/// stack doesn't carry its own copy of the sound.
pub fn builtin(name: &str) -> Option<Sound> {
    let name = name.trim().to_ascii_lowercase();
    let seconds = match name.as_str() {
        "boing" => 0.7,
        "harpsichord" => 1.6,
        "flute" => 1.2,
        _ => return None,
    };
    let n = (seconds * SYNTH_RATE) as usize;
    let base = 261.63; // middle C
    let mut samples = Vec::with_capacity(n);
    let mut phase = 0f32;
    for i in 0..n {
        let t = i as f32 / SYNTH_RATE;
        let v = match name.as_str() {
            "boing" => {
                // A springy sine: pitch bends up and settles, with a wobble.
                let bend = 1.0 + 0.6 * (-t * 9.0).exp() * (t * 38.0).sin();
                phase += TAU * 293.66 * bend / SYNTH_RATE;
                phase.sin() * (-t * 4.5).exp()
            }
            "harpsichord" => {
                // Bright plucked string: decaying harmonics, the upper ones faster.
                (1..=8)
                    .map(|h| {
                        let h = h as f32;
                        (TAU * base * h * t).sin() * (-t * (2.5 + h * 1.3)).exp() / h.powf(0.8)
                    })
                    .sum::<f32>()
                    * 0.45
                    * (t * 400.0).min(1.0)
            }
            _ => {
                // Breathy sine with gentle vibrato.
                let vibrato = 1.0 + 0.004 * (TAU * 5.5 * t).sin();
                phase += TAU * base * vibrato / SYNTH_RATE;
                let attack = (t * 12.0).min(1.0);
                let breath = (rand::random::<f32>() - 0.5) * 0.03;
                (phase.sin() + 0.18 * (2.0 * phase).sin() + breath) * 0.6 * attack
            }
        };
        samples.push((v.clamp(-1.0, 1.0) * 28000.0) as i16);
    }
    // The flute sustains by looping a steady stretch of whole cycles.
    let (loop_start, loop_end) = if name == "flute" {
        let period = SYNTH_RATE / base;
        let start = (0.4 * SYNTH_RATE) as u32;
        (start, start + (period * 40.0).round() as u32)
    } else {
        (0, 0)
    };
    let base_note = if name == "boing" { 62 } else { 60 };
    Some(Sound { name, rate: SYNTH_RATE, base_note, loop_start, loop_end, samples })
}

/// Touch tones for `dial`: 120 ms per digit with 60 ms gaps.
pub fn dial_tones(number: &str) -> Vec<f32> {
    const ROWS: [f32; 4] = [697.0, 770.0, 852.0, 941.0];
    const COLS: [f32; 3] = [1209.0, 1336.0, 1477.0];
    let mut out = Vec::new();
    for c in number.chars() {
        let key = match c {
            '1'..='9' => {
                let d = c as usize - '1' as usize;
                (d / 3, d % 3)
            }
            '*' => (3, 0),
            '0' => (3, 1),
            '#' => (3, 2),
            _ => continue,
        };
        let (row, col) = (ROWS[key.0], COLS[key.1]);
        let n = (0.12 * SYNTH_RATE) as usize;
        for i in 0..n {
            let t = i as f32 / SYNTH_RATE;
            out.push(((TAU * row * t).sin() + (TAU * col * t).sin()) * 0.3);
        }
        out.resize(out.len() + (0.06 * SYNTH_RATE) as usize, 0.0);
    }
    out
}

/// A short system beep.
pub fn beep_tone() -> Vec<f32> {
    let n = (0.12 * SYNTH_RATE) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / SYNTH_RATE;
            // A square-ish tone, like the classic Mac "Simple Beep".
            let s = (TAU * 880.0 * t).sin();
            s.signum() * 0.15 * (1.0 - i as f32 / n as f32)
        })
        .collect()
}

/// Plays sounds one after another, and knows what is playing for `the sound`.
/// Audio output is opened on first use; without a device, sounds are timed
/// but silent, so scripts that wait for them still work.
pub struct Player {
    enabled: bool,
    device: Option<(rodio::MixerDeviceSink, rodio::Player)>,
    tried: bool,
    /// Names of queued sounds with the times they finish.
    queue: Vec<(String, Instant)>,
}

impl Player {
    /// A player that never touches the audio device (for tests).
    pub fn silent() -> Player {
        Player { enabled: false, device: None, tried: false, queue: Vec::new() }
    }

    pub fn new() -> Player {
        Player { enabled: true, ..Player::silent() }
    }

    fn output(&mut self) -> Option<&rodio::Player> {
        if self.enabled && !self.tried {
            self.tried = true;
            if let Ok(mut sink) = rodio::DeviceSinkBuilder::open_default_sink() {
                sink.log_on_drop(false);
                let player = rodio::Player::connect_new(sink.mixer());
                self.device = Some((sink, player));
            }
        }
        self.device.as_ref().map(|(_, p)| p)
    }

    /// Queue mono samples after whatever is already playing.
    pub fn play(&mut self, name: &str, samples: Vec<f32>, rate: f32) {
        if samples.is_empty() || rate <= 0.0 {
            return;
        }
        let duration = Duration::from_secs_f32(samples.len() as f32 / rate);
        let now = Instant::now();
        let start = self.queue.last().map(|(_, end)| *end).filter(|end| *end > now).unwrap_or(now);
        self.queue.retain(|(_, end)| *end > now);
        self.queue.push((name.to_string(), start + duration));
        if let (Some(player), Some(rate)) = (self.output(), std::num::NonZero::new(rate.round() as u32)) {
            let channels = std::num::NonZero::new(1).unwrap();
            player.append(rodio::buffer::SamplesBuffer::new(channels, rate, samples));
        }
    }

    pub fn stop(&mut self) {
        self.queue.clear();
        if let Some((_, p)) = &self.device {
            // clear() also pauses the player; resume it for the next sound.
            p.clear();
            p.play();
        }
    }

    /// `the sound`: the name of the sound playing now, or "done".
    pub fn current(&mut self) -> String {
        let now = Instant::now();
        self.queue.retain(|(_, end)| *end > now);
        self.queue.first().map(|(n, _)| n.clone()).unwrap_or_else(|| "done".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes() {
        let n = parse_notes("c4q e g#5h r 60e d.");
        let midi: Vec<Option<u8>> = n.iter().map(|n| n.midi).collect();
        assert_eq!(midi, vec![Some(60), Some(64), Some(80), None, Some(60), Some(62)]);
        let beats: Vec<f32> = n.iter().map(|n| n.beats).collect();
        assert_eq!(beats, vec![1.0, 1.0, 2.0, 2.0, 0.5, 0.75]);
        assert_eq!(parse_notes("eb3 bb")[1].midi, Some(58));
        assert_eq!(parse_notes("c3")[0].beats, 1.0);
        assert!((parse_notes("cq3")[0].beats - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn render_lengths_and_pitch() {
        let sound = builtin("harpsichord").unwrap();
        assert_eq!(render(&sound, &[], 120.0).len(), sound.samples.len());
        // Two quarter notes at 120 bpm last one second.
        let out = render(&sound, &parse_notes("c4 c5"), 120.0);
        assert_eq!(out.len(), 22050);
        // A flute note longer than its sample sustains on the loop.
        let flute = builtin("flute").unwrap();
        let long = render(&flute, &parse_notes("c4w"), 60.0);
        let tail = &long[long.len() - 2000..long.len() - 500];
        assert!(tail.iter().any(|v| v.abs() > 0.1));
    }

    #[test]
    fn timing_without_a_device() {
        let mut p = Player::silent();
        assert_eq!(p.current(), "done");
        p.play("boing", vec![0.0; 2205], 22050.0);
        assert_eq!(p.current(), "boing");
        p.stop();
        assert_eq!(p.current(), "done");
        assert_eq!(dial_tones("1-2").len(), 2 * (2646 + 1323));
    }
}
