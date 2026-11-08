//! Sound for HyperTalk's `play`, `dial` and `beep`: turning sampled sounds and
//! note strings like "c4q e g# a5h." into audio, and playing it.

use std::f32::consts::TAU;
use std::time::{Duration, Instant};

use crate::model::Sound;

pub mod song;

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
            // Linear interpolation between neighboring samples.
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

/// The sound channels HyperCard 2.4 had (`the soundChannel`, 1 to 8).
pub const CHANNELS: usize = 8;

/// One sound channel: sounds play on it one after another.
#[derive(Default)]
struct Channel {
    player: Option<rodio::Player>,
    /// Names of queued sounds with the times they finish.
    queue: Vec<(String, Instant)>,
}

impl Channel {
    fn playing(&mut self, now: Instant) -> Option<&str> {
        self.queue.retain(|(_, end)| *end > now);
        self.queue.first().map(|(n, _)| n.as_str())
    }

    fn stop(&mut self) {
        self.queue.clear();
        if let Some(p) = &self.player {
            // clear() also pauses the player; resume it for the next sound.
            p.clear();
            p.play();
        }
    }
}

/// Plays sounds on eight channels, one after another on each, and knows
/// what is playing for `the sound`. `play` uses the current channel (`the
/// soundChannel`), so a script switching channels plays sounds together, as
/// in HyperCard 2.4. Audio output is opened on first use; without a device,
/// sounds are timed but silent, so scripts that wait for them still work.
pub struct Player {
    enabled: bool,
    device: Option<rodio::MixerDeviceSink>,
    channels: Vec<Channel>,
    /// `the soundChannel`, from 1.
    channel: usize,
    /// The song channel, beside sound effects. Songs aren't in `the
    /// sound`: they loop, and `wait until the sound is done` waits for
    /// sound effects.
    music: Option<rodio::Player>,
    /// The song playing, for `play stop "name"`.
    song: Option<String>,
    /// `the soundVolume`, 0 to 7.
    volume: u8,
    tried: bool,
}

impl Player {
    /// A player that never touches the audio device (for tests).
    pub fn silent() -> Player {
        Player {
            enabled: false,
            device: None,
            tried: false,
            channels: (0..CHANNELS).map(|_| Channel::default()).collect(),
            channel: 1,
            music: None,
            song: None,
            volume: 7,
        }
    }

    pub fn new() -> Player {
        Player { enabled: true, ..Player::silent() }
    }

    /// Whether this player makes sound at all (not in tests or the stack
    /// exerciser).
    pub fn audible(&self) -> bool {
        self.enabled
    }

    fn open(&mut self) {
        if self.enabled && !self.tried {
            self.tried = true;
            if let Ok(mut sink) = rodio::DeviceSinkBuilder::open_default_sink() {
                sink.log_on_drop(false);
                self.device = Some(sink);
            }
        }
    }

    /// `the soundChannel`.
    pub fn channel(&self) -> usize {
        self.channel
    }

    pub fn set_channel(&mut self, n: usize) {
        self.channel = n.clamp(1, CHANNELS);
    }

    /// Queue mono samples after whatever is already playing on the current
    /// channel.
    pub fn play(&mut self, name: &str, samples: Vec<f32>, rate: f32) {
        if samples.is_empty() || rate <= 0.0 {
            return;
        }
        let duration = Duration::from_secs_f32(samples.len() as f32 / rate);
        let now = Instant::now();
        self.open();
        let gain = self.gain();
        let device = self.device.as_ref();
        let ch = &mut self.channels[self.channel - 1];
        let start = ch.queue.last().map(|(_, end)| *end).filter(|end| *end > now).unwrap_or(now);
        ch.queue.retain(|(_, end)| *end > now);
        ch.queue.push((name.to_string(), start + duration));
        if let Some(sink) = device {
            let player = ch.player.get_or_insert_with(|| {
                let p = rodio::Player::connect_new(sink.mixer());
                p.set_volume(gain);
                p
            });
            if let Some(rate) = std::num::NonZero::new(rate.round() as u32) {
                let mono = std::num::NonZero::new(1).unwrap();
                player.append(rodio::buffer::SamplesBuffer::new(mono, rate, samples));
            }
        }
    }

    /// Play a song on its own channel, looping, in place of any other.
    pub fn play_song(&mut self, name: &str, data: &[u8]) -> Result<(), String> {
        self.end_song();
        self.song = Some(name.to_string());
        self.open();
        let Some(sink) = &self.device else { return Ok(()) };
        let source = song::SongSource::new(data, 44_100)?;
        let player = rodio::Player::connect_new(sink.mixer());
        player.set_volume(self.gain());
        player.append(source);
        self.music = Some(player);
        Ok(())
    }

    /// `play stop "name"`: stop that song, if it's the one playing.
    pub fn stop_song(&mut self, name: &str) {
        if self.song.as_deref().is_some_and(|s| s.eq_ignore_ascii_case(name.trim())) {
            self.end_song();
        }
    }

    fn end_song(&mut self) {
        if let Some(p) = self.music.take() {
            p.stop();
        }
        self.song = None;
    }

    /// `the soundVolume`, 0 (silent) to 7 (full), as the Mac's sound
    /// levels went: RandomCard's own, never the computer's.
    pub fn volume(&self) -> u8 {
        self.volume
    }

    pub fn set_volume(&mut self, level: u8) {
        self.volume = level.min(7);
        let gain = self.gain();
        for p in self.channels.iter().filter_map(|c| c.player.as_ref()).chain(self.music.as_ref()) {
            p.set_volume(gain);
        }
    }

    fn gain(&self) -> f32 {
        self.volume as f32 / 7.0
    }

    /// `play stop`: the current channel's sounds; a song plays on (`play
    /// stop "name"` stops it).
    pub fn stop(&mut self) {
        self.channels[self.channel - 1].stop();
    }

    /// `stop sound`: every channel's sounds.
    pub fn stop_all(&mut self) {
        for c in &mut self.channels {
            c.stop();
        }
    }

    /// `the sound`: the name of the sound playing now on the current
    /// channel, or "done". (Shattered Stone's channelDone switches channels
    /// to ask after each one.)
    pub fn current(&mut self) -> String {
        let now = Instant::now();
        let ch = &mut self.channels[self.channel - 1];
        ch.playing(now).map(str::to_string).unwrap_or_else(|| "done".into())
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

    #[test]
    fn channels() {
        // Apple's chord: one note on each of three channels, together.
        let mut p = Player::silent();
        assert_eq!(p.channel(), 1);
        for (ch, name) in [(1, "c"), (2, "e"), (3, "g")] {
            p.set_channel(ch);
            p.play(name, vec![0.0; 22050], 22050.0);
        }
        assert_eq!(p.current(), "g");
        // play stop: the current channel only.
        p.stop();
        assert_eq!(p.current(), "done");
        p.set_channel(1);
        assert_eq!(p.current(), "c");
        // stop sound: all of them.
        p.stop_all();
        assert_eq!(p.current(), "done");
        p.set_channel(12);
        assert_eq!(p.channel(), CHANNELS);
    }
}
