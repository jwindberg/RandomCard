//! Movies on macOS, with AVFoundation. `AVAssetReader` decodes video frames
//! (in hardware through VideoToolbox where it can) as 32-bit BGRA pixel
//! buffers, while `AVSampleBufferRenderSynchronizer` and
//! `AVSampleBufferAudioRenderer` play the sound and drive the movie's clock
//! without needing an AppKit main-thread run loop (so headless tests play in
//! real time too).

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{AnyThread, msg_send};
use objc2_av_foundation::{
    AVAssetReader, AVAssetReaderStatus, AVAssetReaderTrackOutput, AVAssetTrack, AVMediaTypeAudio,
    AVMediaTypeVideo, AVQueuedSampleBufferRendering, AVSampleBufferAudioRenderer,
    AVSampleBufferRenderSynchronizer, AVURLAsset,
};
use objc2_core_media::{
    CMSampleBuffer, CMTime, CMTimeFlags, CMTimeRange, kCMTimePositiveInfinity,
};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow,
    CVPixelBufferGetHeight, CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress,
    CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress, kCVPixelBufferPixelFormatTypeKey,
    kCVPixelFormatType_32BGRA, kCVReturnSuccess,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

use super::Frame;

/// Preferred timescale for `CMTime`: divisible by 24, 25, 30, 60, 600 and 1000.
const TIMESCALE: i32 = 600_000;

/// Orthogonal rotation from a video track's `preferredTransform` (such as
/// portrait phone recordings).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Rot {
    None,
    Cw90,
    Cw180,
    Cw270,
}

impl Rot {
    fn from_track(track: &AVAssetTrack) -> Self {
        // SAFETY: Reads the value-type `CGAffineTransform` of the video track.
        let t = unsafe { track.preferredTransform() };
        if t.b > 0.5 && t.c < -0.5 {
            Rot::Cw90
        } else if t.b < -0.5 && t.c > 0.5 {
            Rot::Cw270
        } else if t.a < -0.5 && t.d < -0.5 {
            Rot::Cw180
        } else {
            Rot::None
        }
    }
}

/// One decoded video sample before it becomes a `Frame`.
#[derive(Clone)]
struct RawPic {
    pts: f64,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// Sequential forward video decoder kept open across `frame()` calls.
struct VideoStream {
    reader: Retained<AVAssetReader>,
    output: Retained<AVAssetReaderTrackOutput>,
    rot: Rot,
    /// Already-decoded sample whose presentation time hasn't arrived yet.
    peek: Option<RawPic>,
    eof: bool,
}

impl Drop for VideoStream {
    fn drop(&mut self) {
        // SAFETY: Cancels any background read-ahead on our owned `AVAssetReader`.
        unsafe {
            if self.reader.status() == AVAssetReaderStatus::Reading {
                self.reader.cancelReading();
            }
        }
    }
}

/// Short batch of decoded frames used to step smoothly through reverse playback.
struct ReverseWindow {
    start: f64,
    end: f64,
    frames: Vec<RawPic>,
    shown_pts: f64,
}

/// Wall-clock timeline for negative rates (`AVSampleBufferRenderSynchronizer`
/// only runs at rates `>= 0`).
struct ManualClock {
    anchor_pos: f64,
    anchor_time: Instant,
}

impl ManualClock {
    fn new(pos: f64) -> Self {
        Self { anchor_pos: pos, anchor_time: Instant::now() }
    }

    fn position(&self, rate: f64, duration: f64) -> f64 {
        (self.anchor_pos + self.anchor_time.elapsed().as_secs_f64() * rate).clamp(0.0, duration)
    }
}

/// Audio reader state fed to `AVSampleBufferAudioRenderer` on a helper thread.
enum AudioJob {
    Idle,
    Feed {
        reader: Retained<AVAssetReader>,
        output: Retained<AVAssetReaderTrackOutput>,
    },
    Stop,
}

struct AudioShared {
    renderer: Retained<AVSampleBufferAudioRenderer>,
    job: AudioJob,
}

// SAFETY: Access to `AudioShared` is serialized by `Mutex<AudioShared>`, and
// `AVSampleBufferAudioRenderer` / `AVAssetReader` support cross-thread handoff
// when calls are not concurrent.
unsafe impl Send for AudioShared {}

struct AudioFeeder {
    shared: Arc<(Mutex<AudioShared>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl AudioFeeder {
    fn new(renderer: Retained<AVSampleBufferAudioRenderer>) -> Self {
        let shared = Arc::new((Mutex::new(AudioShared { renderer, job: AudioJob::Idle }), Condvar::new()));
        let pair = shared.clone();
        let thread = std::thread::spawn(move || audio_loop(&pair));
        Self { shared, thread: Some(thread) }
    }

    /// Flush queued audio and start feeding samples from `reader` / `output`.
    fn start(&self, reader: Retained<AVAssetReader>, output: Retained<AVAssetReaderTrackOutput>) {
        let (lock, cvar) = &*self.shared;
        let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        cancel_audio_job(&mut guard.job);
        // SAFETY: `flush` and `enqueueSampleBuffer:` are serialized under `lock`.
        unsafe {
            guard.renderer.flush();
            let mut eof = false;
            while guard.renderer.isReadyForMoreMediaData() {
                let Some(sb) = output.copyNextSampleBuffer() else {
                    eof = true;
                    break;
                };
                guard.renderer.enqueueSampleBuffer(&sb);
            }
            if eof {
                if reader.status() == AVAssetReaderStatus::Reading {
                    reader.cancelReading();
                }
                guard.job = AudioJob::Idle;
            } else {
                guard.job = AudioJob::Feed { reader, output };
            }
        }
        cvar.notify_all();
    }

    /// Stop feeding and silence any buffered audio right away.
    fn silence(&self) {
        let (lock, cvar) = &*self.shared;
        let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        cancel_audio_job(&mut guard.job);
        // SAFETY: Serialized under `lock`.
        unsafe { guard.renderer.flush() };
        cvar.notify_all();
    }

    fn set_volume(&self, volume: f32) {
        let (lock, _) = &*self.shared;
        let guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: Sets gain on the player's own audio renderer.
        unsafe { guard.renderer.setVolume(volume) };
    }
}

impl Drop for AudioFeeder {
    fn drop(&mut self) {
        {
            let (lock, cvar) = &*self.shared;
            let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
            cancel_audio_job(&mut guard.job);
            // SAFETY: Flush buffered samples before shutting down.
            unsafe { guard.renderer.flush() };
            guard.job = AudioJob::Stop;
            cvar.notify_all();
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

fn cancel_audio_job(job: &mut AudioJob) {
    if let AudioJob::Feed { reader, .. } = std::mem::replace(job, AudioJob::Idle) {
        // SAFETY: Cancels any remaining read-ahead on the old audio reader.
        unsafe {
            if reader.status() == AVAssetReaderStatus::Reading {
                reader.cancelReading();
            }
        }
    }
}

fn audio_loop(pair: &(Mutex<AudioShared>, Condvar)) {
    let (lock, cvar) = pair;
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        match &guard.job {
            AudioJob::Stop => break,
            AudioJob::Idle => {
                guard = cvar.wait(guard).unwrap_or_else(|e| e.into_inner());
            }
            AudioJob::Feed { .. } => {
                let done = autoreleasepool(|_| {
                    let shared = &mut *guard;
                    let AudioJob::Feed { reader, output } = &shared.job else { return false };
                    // SAFETY: `renderer` and `output` are owned by `AudioShared` under `lock`.
                    unsafe {
                        while shared.renderer.isReadyForMoreMediaData() {
                            let Some(sb) = output.copyNextSampleBuffer() else {
                                if reader.status() == AVAssetReaderStatus::Reading {
                                    reader.cancelReading();
                                }
                                return true;
                            };
                            shared.renderer.enqueueSampleBuffer(&sb);
                        }
                    }
                    false
                });
                if done {
                    guard.job = AudioJob::Idle;
                } else {
                    let (g, _) = cvar
                        .wait_timeout(guard, Duration::from_millis(20))
                        .unwrap_or_else(|e| e.into_inner());
                    guard = g;
                }
            }
        }
    }
}

/// Objective-C media objects kept behind `Player`'s mutex.
struct Media {
    asset: Retained<AVURLAsset>,
    video_track: Option<(Retained<AVAssetTrack>, Rot)>,
    audio_track: Option<Retained<AVAssetTrack>>,
    sync: Retained<AVSampleBufferRenderSynchronizer>,
    audio: Option<AudioFeeder>,
    stream: Option<VideoStream>,
    shown_pts: f64,
    reverse: Option<ReverseWindow>,
}

// SAFETY: All Objective-C objects in `Media` are accessed exclusively under
// `Player`'s `Mutex<State>`, never concurrently from multiple threads.
unsafe impl Send for Media {}

struct State {
    media: Media,
    rate: f64,
    /// Exact position in seconds while paused (`rate == 0.0`).
    held: f64,
    ended: bool,
    /// Guards against `setRate:time:` updating `CMTimebase` asynchronously:
    /// holds `(target_secs, set_at)` briefly until `sync.currentTime()` catches up.
    sync_jump: Option<(f64, Instant)>,
    manual: Option<ManualClock>,
    frame: Option<Arc<Frame>>,
    serial: u64,
}

impl State {
    /// Current playback position in seconds.
    fn pos(&mut self, duration: f64) -> f64 {
        if self.rate == 0.0 {
            return self.held;
        }
        if let Some(manual) = &self.manual {
            return manual.position(self.rate, duration);
        }
        // SAFETY: Queries the synchronizer's `CMTimebase` position.
        let raw = valid_seconds(unsafe { self.media.sync.currentTime() });
        if let Some((target, when)) = self.sync_jump {
            let elapsed = when.elapsed();
            if elapsed < Duration::from_millis(80) {
                let expected = (target + elapsed.as_secs_f64() * self.rate).clamp(0.0, duration);
                if raw.is_none_or(|s| (s - expected).abs() > 0.25) {
                    return expected;
                }
            }
            self.sync_jump = None;
        }
        raw.map(|s| s.clamp(0.0, duration)).unwrap_or(self.held)
    }
}

/// A movie: a file, read here, or one on the web, streamed by AVPlayer.
pub enum Player {
    File(Box<FilePlayer>),
    Stream(super::avplayer::StreamPlayer),
}

impl Player {
    pub fn open(path: &Path) -> Result<Player, String> {
        FilePlayer::open(path).map(|p| Player::File(Box::new(p)))
    }

    pub fn open_url(url: &str) -> Result<Player, String> {
        super::avplayer::StreamPlayer::open(url).map(Player::Stream)
    }

    pub fn size(&self) -> (u32, u32) {
        match self {
            Player::File(p) => p.size(),
            Player::Stream(p) => p.size(),
        }
    }

    pub fn duration(&self) -> f64 {
        match self {
            Player::File(p) => p.duration(),
            Player::Stream(p) => p.duration(),
        }
    }

    pub fn time(&self) -> f64 {
        match self {
            Player::File(p) => p.time(),
            Player::Stream(p) => p.time(),
        }
    }

    pub fn seek(&self, secs: f64) {
        match self {
            Player::File(p) => p.seek(secs),
            Player::Stream(p) => p.seek(secs),
        }
    }

    pub fn set_rate(&self, rate: f64) {
        match self {
            Player::File(p) => p.set_rate(rate),
            Player::Stream(p) => p.set_rate(rate),
        }
    }

    pub fn set_volume(&self, level: f64) {
        match self {
            Player::File(p) => p.set_volume(level),
            Player::Stream(p) => p.set_volume(level),
        }
    }

    pub fn frame(&self) -> Option<Arc<Frame>> {
        match self {
            Player::File(p) => p.frame(),
            Player::Stream(p) => p.frame(),
        }
    }

    pub fn finished(&self) -> bool {
        match self {
            Player::File(p) => p.finished(),
            Player::Stream(p) => p.finished(),
        }
    }
}

/// A movie file, read with AVAssetReader.
pub struct FilePlayer {
    duration: f64,
    size: (u32, u32),
    state: Mutex<State>,
}

impl FilePlayer {
    pub fn open(path: &Path) -> Result<FilePlayer, String> {
        if !path.is_file() {
            return Err("Can't find that movie file".into());
        }
        autoreleasepool(|_| {
            let path_str = NSString::from_str(&path.to_string_lossy());
            let url = NSURL::fileURLWithPath(&path_str);
            // SAFETY: Initializes an immutable `AVURLAsset` for a local file URL.
            let asset = unsafe { AVURLAsset::URLAssetWithURL_options(&url, None) };

            // SAFETY: Synchronous asset inspection on a local file.
            if !unsafe { asset.isPlayable() } {
                return Err(reader_error_for(&asset));
            }

            let video_tracks = media_tracks(&asset, true);
            let video_track = if let Some(vt) = video_tracks.into_iter().next() {
                // Classic QuickTime codecs (SMC, Animation, Cinepak) and MPEG-1/2
                // have a video track that AVFoundation marks unplayable/undecodable.
                // SAFETY: Queries track playability on the loaded asset.
                if !unsafe { vt.isPlayable() && vt.isDecodable() } {
                    return Err("It isn't a movie RandomCard can play".into());
                }
                let rot = Rot::from_track(&vt);
                Some((vt, rot))
            } else {
                None
            };

            let audio_track = media_tracks(&asset, false)
                .into_iter()
                // SAFETY: Queries audio track playability on the loaded asset.
                .find(|t| unsafe { t.isPlayable() && t.isDecodable() });

            if video_track.is_none() && audio_track.is_none() {
                return Err("It isn't a movie RandomCard can play".into());
            }

            // SAFETY: Reads the asset's `CMTime` duration.
            let asset_dur = valid_seconds(unsafe { asset.duration() }).unwrap_or(0.0).max(0.0);
            // QuickTime and MP4 files store their authoritative movie duration in
            // `mvhd`, whereas `AVURLAsset` trims encoder-delay samples from AAC.
            let duration = mp4_mvhd_duration(path).unwrap_or(asset_dur).max(asset_dur);

            let (size, first_frame, stream, shown_pts) = if let Some((vt, rot)) = &video_track {
                let (stream, pic) = open_video_at(&asset, vt, *rot, 0.0)?;
                let (w, h, pts) = (pic.width, pic.height, pic.pts);
                let frame = Arc::new(Frame { width: w, height: h, rgba: pic.rgba, serial: 1 });
                ((w, h), Some(frame), Some(stream), pts)
            } else {
                // Verify the audio track actually opens before reporting success.
                if let Some(at) = &audio_track {
                    let (reader, _) = open_audio_at(&asset, at, 0.0)?;
                    // SAFETY: Cleanly stop the probe reader.
                    unsafe { reader.cancelReading() };
                }
                ((0, 0), None, None, 0.0)
            };

            // SAFETY: Creates the render synchronizer and (outside tests) audio renderer.
            let (sync, audio) = unsafe {
                let sync = AVSampleBufferRenderSynchronizer::new();
                sync.setDelaysRateChangeUntilHasSufficientMediaData(false);
                let audio = if cfg!(test) || audio_track.is_none() {
                    None
                } else {
                    let r: Option<Retained<AVSampleBufferAudioRenderer>> =
                        msg_send![AVSampleBufferAudioRenderer::alloc(), init];
                    r.map(|renderer| {
                        sync.addRenderer(ProtocolObject::from_ref(&*renderer));
                        AudioFeeder::new(renderer)
                    })
                };
                (sync, audio)
            };

            let serial = u64::from(first_frame.is_some());
            Ok(FilePlayer {
                duration,
                size,
                state: Mutex::new(State {
                    media: Media {
                        asset,
                        video_track,
                        audio_track,
                        sync,
                        audio,
                        stream,
                        shown_pts,
                        reverse: None,
                    },
                    rate: 0.0,
                    held: 0.0,
                    ended: false,
                    sync_jump: None,
                    manual: None,
                    frame: first_frame,
                    serial,
                }),
            })
        })
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn time(&self) -> f64 {
        let mut state = self.state();
        autoreleasepool(|_| state.pos(self.duration))
    }

    pub fn seek(&self, secs: f64) {
        let secs = secs.clamp(0.0, self.duration);
        let mut guard = self.state();
        let state = &mut *guard;
        autoreleasepool(|_| {
            if (state.held - secs).abs() < 1e-6 && state.rate == 0.0 && !state.ended {
                return;
            }
            state.held = secs;
            state.ended = false;
            state.media.reverse = None;

            if let Some((vt, rot)) = &state.media.video_track
                && let Ok((stream, pic)) = open_video_at(&state.media.asset, vt, *rot, secs)
            {
                state.media.stream = Some(stream);
                state.media.shown_pts = pic.pts;
                store_pic(&mut state.serial, &mut state.frame, pic);
            }

            if state.rate == 0.0 {
                state.sync_jump = None;
                if let Some(audio) = &state.media.audio {
                    audio.silence();
                }
                // SAFETY: Moves the synchronizer's timebase while keeping rate 0.
                unsafe { state.media.sync.setRate_time(0.0, cm_time(secs)) };
            } else if state.rate > 0.0 {
                start_audio(&state.media, secs);
                state.sync_jump = Some((secs, Instant::now()));
                // SAFETY: Resumes forward rendering from the new position.
                unsafe { state.media.sync.setRate_time(state.rate as f32, cm_time(secs)) };
            } else {
                state.manual = Some(ManualClock::new(secs));
            }
        });
    }

    pub fn set_rate(&self, rate: f64) {
        let mut guard = self.state();
        let state = &mut *guard;
        autoreleasepool(|_| {
            if rate == state.rate && !state.ended {
                return;
            }
            let now = state.pos(self.duration);
            let prev_rate = state.rate;
            state.held = now;
            state.rate = rate;
            state.manual = None;
            state.sync_jump = None;

            if rate == 0.0 {
                // SAFETY: Stops the synchronizer's clock immediately.
                unsafe { state.media.sync.setRate_time(0.0, cm_time(now)) };
                if let Some(audio) = &state.media.audio {
                    audio.silence();
                }
                return;
            }

            state.ended = false;
            if rate > 0.0 {
                state.media.reverse = None;
                // Reopen the forward video stream if coming out of reverse or EOF.
                let need_reopen = prev_rate < 0.0
                    || state.media.stream.as_ref().is_none_or(|s| s.eof || (now - state.media.shown_pts).abs() > 0.5);
                if need_reopen
                    && let Some((vt, rot)) = &state.media.video_track
                    && let Ok((stream, pic)) = open_video_at(&state.media.asset, vt, *rot, now)
                {
                    state.media.stream = Some(stream);
                    state.media.shown_pts = pic.pts;
                    store_pic(&mut state.serial, &mut state.frame, pic);
                }
                start_audio(&state.media, now);
                state.sync_jump = Some((now, Instant::now()));
                // SAFETY: Starts the synchronizer's timebase at `rate` (`> 0`).
                unsafe { state.media.sync.setRate_time(rate as f32, cm_time(now)) };
            } else {
                // Reverse playback: silence audio and drive position from `ManualClock`.
                // SAFETY: Stops forward timebase before switching to reverse clock.
                unsafe { state.media.sync.setRate_time(0.0, cm_time(now)) };
                if let Some(audio) = &state.media.audio {
                    audio.silence();
                }
                state.media.stream = None;
                state.manual = Some(ManualClock::new(now));
            }
        });
    }

    pub fn set_volume(&self, level: f64) {
        let state = self.state();
        let vol = level.clamp(0.0, 1.0) as f32;
        if let Some(audio) = &state.media.audio {
            autoreleasepool(|_| audio.set_volume(vol));
        }
    }

    pub fn frame(&self) -> Option<Arc<Frame>> {
        let mut guard = self.state();
        let state = &mut *guard;
        state.media.video_track.as_ref()?;
        if state.rate == 0.0 {
            return state.frame.clone();
        }
        autoreleasepool(|_| {
            let now = state.pos(self.duration);
            if state.rate > 0.0 {
                if let Some(stream) = &mut state.media.stream
                    && let Some(pic) = advance_stream(stream, now)
                {
                    state.media.shown_pts = pic.pts;
                    store_pic(&mut state.serial, &mut state.frame, pic);
                }
            } else if let Some((vt, rot)) = &state.media.video_track
                && let Some(pic) = reverse_pic_at(&state.media.asset, vt, *rot, &mut state.media.reverse, now)
            {
                state.media.shown_pts = pic.pts;
                store_pic(&mut state.serial, &mut state.frame, pic);
            }
            state.frame.clone()
        })
    }

    pub fn finished(&self) -> bool {
        let mut guard = self.state();
        let state = &mut *guard;
        if state.ended {
            return true;
        }
        if state.rate == 0.0 {
            return false;
        }
        autoreleasepool(|_| {
            let now = state.pos(self.duration);
            let done = if state.rate > 0.0 {
                self.duration <= 0.0 || now >= self.duration
            } else {
                now <= 0.0
            };
            if done {
                let end_pos = if state.rate < 0.0 { 0.0 } else { self.duration };
                state.ended = true;
                state.held = end_pos;
                state.rate = 0.0;
                state.manual = None;
                state.sync_jump = None;
                state.media.reverse = None;
                // SAFETY: Stops the synchronizer at the boundary.
                unsafe { state.media.sync.setRate_time(0.0, cm_time(end_pos)) };
                if let Some(audio) = &state.media.audio {
                    audio.silence();
                }
                if let Some((vt, rot)) = &state.media.video_track
                    && let Ok((stream, pic)) = open_video_at(&state.media.asset, vt, *rot, end_pos)
                {
                    state.media.stream = Some(stream);
                    state.media.shown_pts = pic.pts;
                    store_pic(&mut state.serial, &mut state.frame, pic);
                }
            }
            done
        })
    }
}

impl Drop for FilePlayer {
    fn drop(&mut self) {
        let mut state = self.state();
        autoreleasepool(|_| {
            // SAFETY: Stops the synchronizer before dropping audio and readers.
            unsafe { state.media.sync.setRate(0.0) };
            state.media.audio.take();
            state.media.stream.take();
        });
    }
}

fn store_pic(serial: &mut u64, slot: &mut Option<Arc<Frame>>, pic: RawPic) {
    *serial += 1;
    *slot = Some(Arc::new(Frame { width: pic.width, height: pic.height, rgba: pic.rgba, serial: *serial }));
}

/// Start feeding compressed audio samples from `secs` into the audio renderer.
fn start_audio(media: &Media, secs: f64) {
    let (Some(audio), Some(track)) = (&media.audio, &media.audio_track) else { return };
    if let Ok((reader, output)) = open_audio_at(&media.asset, track, secs) {
        audio.start(reader, output);
    } else {
        audio.silence();
    }
}

/// Open an audio `AVAssetReader` starting at `secs`.
fn open_audio_at(
    asset: &AVURLAsset,
    track: &AVAssetTrack,
    secs: f64,
) -> Result<(Retained<AVAssetReader>, Retained<AVAssetReaderTrackOutput>), String> {
    // SAFETY: Creates an `AVAssetReader` and compressed pass-through track output.
    unsafe {
        let reader = AVAssetReader::assetReaderWithAsset_error(asset).map_err(|e| e.to_string())?;
        reader.setTimeRange(CMTimeRange { start: cm_time(secs), duration: kCMTimePositiveInfinity });
        let output = AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(track, None);
        output.setAlwaysCopiesSampleData(false);
        if !reader.canAddOutput(&output) {
            return Err("It isn't a movie RandomCard can play".into());
        }
        reader.addOutput(&output);
        if !reader.startReading() {
            return Err(reader_status_error(&reader));
        }
        Ok((reader, output))
    }
}

/// Open a BGRA video `AVAssetReader` around `secs` and return the stream along
/// with the exact decoded picture covering `secs`.
fn open_video_at(
    asset: &AVURLAsset,
    track: &AVAssetTrack,
    rot: Rot,
    secs: f64,
) -> Result<(VideoStream, RawPic), String> {
    let lead = (secs - 0.25).max(0.0);
    if let Ok(found) = open_video_from(asset, track, rot, lead, secs) {
        return Ok(found);
    }
    if lead > 0.0 {
        return open_video_from(asset, track, rot, 0.0, secs);
    }
    Err(reader_error_for(asset))
}

fn open_video_from(
    asset: &AVURLAsset,
    track: &AVAssetTrack,
    rot: Rot,
    from: f64,
    target: f64,
) -> Result<(VideoStream, RawPic), String> {
    let (reader, output) = new_video_reader(asset, track, from, None)?;
    let mut stream = VideoStream { reader, output, rot, peek: None, eof: false };
    let mut best = pull_next_pic(&mut stream).ok_or_else(|| reader_status_error(&stream.reader))?;
    while let Some(next) = pull_next_pic(&mut stream) {
        if next.pts <= target + 1e-4 {
            best = next;
        } else {
            stream.peek = Some(next);
            break;
        }
    }
    Ok((stream, best))
}

/// Create and start a 32-bit BGRA `AVAssetReader` over `[start .. start + span]`.
fn new_video_reader(
    asset: &AVURLAsset,
    track: &AVAssetTrack,
    start: f64,
    span: Option<f64>,
) -> Result<(Retained<AVAssetReader>, Retained<AVAssetReaderTrackOutput>), String> {
    let fmt = NSNumber::numberWithUnsignedInt(kCVPixelFormatType_32BGRA);
    // SAFETY: `kCVPixelBufferPixelFormatTypeKey` is a `CFString`, toll-free bridged with `NSString`.
    let fmt_key: &NSString = unsafe { &*(kCVPixelBufferPixelFormatTypeKey as *const _ as *const NSString) };
    let settings = NSDictionary::<NSString, AnyObject>::from_slices(&[fmt_key], &[fmt.as_ref()]);
    // SAFETY: Configures an `AVAssetReader` for BGRA pixel buffers on `track`.
    unsafe {
        let reader = AVAssetReader::assetReaderWithAsset_error(asset).map_err(|e| e.to_string())?;
        let duration = match span {
            Some(d) => cm_time(d.max(0.001)),
            None => kCMTimePositiveInfinity,
        };
        reader.setTimeRange(CMTimeRange { start: cm_time(start), duration });
        let output = AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(track, Some(&settings));
        output.setAlwaysCopiesSampleData(false);
        if !reader.canAddOutput(&output) {
            return Err("It isn't a movie RandomCard can play".into());
        }
        reader.addOutput(&output);
        if !reader.startReading() {
            return Err(reader_status_error(&reader));
        }
        Ok((reader, output))
    }
}

/// Pull samples from `stream` up to `now`, returning the newest picture due for
/// display (or `None` when the current frame is still on screen).
fn advance_stream(stream: &mut VideoStream, now: f64) -> Option<RawPic> {
    if let Some(peek) = &stream.peek
        && peek.pts > now + 1e-4
    {
        return None;
    }
    let mut latest = stream.peek.take();
    while !stream.eof {
        let Some(pic) = pull_next_pic(stream) else { break };
        if pic.pts <= now + 1e-4 {
            latest = Some(pic);
        } else {
            stream.peek = Some(pic);
            break;
        }
    }
    latest
}

/// Decode a short window around `now` when needed and step through it in reverse.
fn reverse_pic_at(
    asset: &AVURLAsset,
    track: &AVAssetTrack,
    rot: Rot,
    cache: &mut Option<ReverseWindow>,
    now: f64,
) -> Option<RawPic> {
    let covered = cache.as_ref().is_some_and(|w| now >= w.start && now <= w.end && !w.frames.is_empty());
    if !covered {
        let start = (now - 0.55).max(0.0);
        let end = now + 0.05;
        let Ok((reader, output)) = new_video_reader(asset, track, start, Some(end - start + 0.1)) else {
            return None;
        };
        let mut stream = VideoStream { reader, output, rot, peek: None, eof: false };
        let mut frames = Vec::new();
        while let Some(pic) = pull_next_pic(&mut stream) {
            frames.push(pic);
        }
        frames.sort_by(|a, b| a.pts.total_cmp(&b.pts));
        *cache = Some(ReverseWindow { start, end, frames, shown_pts: f64::NAN });
    }
    let win = cache.as_mut()?;
    let idx = win.frames.partition_point(|f| f.pts <= now + 1e-4);
    let pick = &win.frames[idx.saturating_sub(1)];
    if (pick.pts - win.shown_pts).abs() < 1e-6 {
        return None;
    }
    win.shown_pts = pick.pts;
    Some(pick.clone())
}

/// Pull the next non-empty video `CMSampleBuffer` from `stream` and unpack its
/// BGRA `CVPixelBuffer` into tightly packed opaque RGBA.
fn pull_next_pic(stream: &mut VideoStream) -> Option<RawPic> {
    loop {
        // SAFETY: Synchronous sample pull on our exclusively-owned `AVAssetReaderTrackOutput`.
        let Some(sbuf) = (unsafe { stream.output.copyNextSampleBuffer() }) else {
            stream.eof = true;
            return None;
        };
        if let Some(pic) = sample_to_pic(&sbuf, stream.rot) {
            return Some(pic);
        }
    }
}

fn sample_to_pic(sbuf: &CMSampleBuffer, rot: Rot) -> Option<RawPic> {
    // SAFETY: Queries presentation timestamp and `CVImageBuffer` on a live `CMSampleBuffer`.
    let (pts, buf) = unsafe {
        let pts = valid_seconds(sbuf.output_presentation_time_stamp())
            .or_else(|| valid_seconds(sbuf.presentation_time_stamp()))
            .unwrap_or(0.0)
            .max(0.0);
        (pts, sbuf.image_buffer()?)
    };
    let (width, height, rgba) = copy_bgra_to_rgba(&buf, rot)?;
    Some(RawPic { pts, width, height, rgba })
}

/// Lock `buf`, strip per-row stride padding, swap BGRA to RGBA with alpha 255,
/// and apply any orthogonal track rotation.
pub(super) fn copy_bgra_to_rgba(buf: &CVPixelBuffer, rot: Rot) -> Option<(u32, u32, Vec<u8>)> {
    let lock = CVPixelBufferLockFlags::ReadOnly;
    // SAFETY: `buf` is a valid `CVPixelBuffer` retained for the duration of this call.
    if unsafe { CVPixelBufferLockBaseAddress(buf, lock) } != kCVReturnSuccess {
        return None;
    }
    let w = CVPixelBufferGetWidth(buf);
    let h = CVPixelBufferGetHeight(buf);
    let stride = CVPixelBufferGetBytesPerRow(buf);
    let base = CVPixelBufferGetBaseAddress(buf) as *const u8;
    let out = if !base.is_null() && w > 0 && h > 0 && stride >= w * 4 {
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h {
            // SAFETY: Base address is locked read-only and spans `h` rows of `stride` bytes.
            let src_row = unsafe { std::slice::from_raw_parts(base.add(y * stride), w * 4) };
            match rot {
                Rot::None => {
                    let dst_row = &mut rgba[y * w * 4..(y + 1) * w * 4];
                    for (dst, src) in dst_row.as_chunks_mut::<4>().0.iter_mut().zip(src_row.as_chunks::<4>().0) {
                        *dst = [src[2], src[1], src[0], 255];
                    }
                }
                Rot::Cw180 => {
                    let dy = h - 1 - y;
                    for (x, src) in src_row.as_chunks::<4>().0.iter().enumerate() {
                        let off = (dy * w + (w - 1 - x)) * 4;
                        rgba[off..off + 4].copy_from_slice(&[src[2], src[1], src[0], 255]);
                    }
                }
                Rot::Cw90 => {
                    let dx = h - 1 - y;
                    for (x, src) in src_row.as_chunks::<4>().0.iter().enumerate() {
                        let off = (x * h + dx) * 4;
                        rgba[off..off + 4].copy_from_slice(&[src[2], src[1], src[0], 255]);
                    }
                }
                Rot::Cw270 => {
                    for (x, src) in src_row.as_chunks::<4>().0.iter().enumerate() {
                        let off = ((w - 1 - x) * h + y) * 4;
                        rgba[off..off + 4].copy_from_slice(&[src[2], src[1], src[0], 255]);
                    }
                }
            }
        }
        let (ow, oh) = match rot {
            Rot::None | Rot::Cw180 => (w as u32, h as u32),
            Rot::Cw90 | Rot::Cw270 => (h as u32, w as u32),
        };
        Some((ow, oh, rgba))
    } else {
        None
    };
    // SAFETY: Unlocks with the same `ReadOnly` flag passed to `CVPixelBufferLockBaseAddress`.
    unsafe { CVPixelBufferUnlockBaseAddress(buf, lock) };
    out
}

pub(super) fn cm_time(secs: f64) -> CMTime {
    // SAFETY: Pure CoreMedia value constructor.
    unsafe { CMTime::with_seconds(secs.max(0.0), TIMESCALE) }
}

pub(super) fn valid_seconds(t: CMTime) -> Option<f64> {
    let flags = t.flags;
    if !flags.contains(CMTimeFlags::Valid) || flags.intersects(CMTimeFlags::ImpliedValueFlagsMask) {
        return None;
    }
    // SAFETY: Pure CoreMedia `CMTime` conversion.
    let s = unsafe { t.seconds() };
    s.is_finite().then_some(s)
}

#[allow(deprecated)]
fn media_tracks(asset: &AVURLAsset, video: bool) -> Vec<Retained<AVAssetTrack>> {
    // SAFETY: `AVMediaTypeVideo` / `AVMediaTypeAudio` are static framework constants.
    unsafe {
        let Some(kind) = (if video { AVMediaTypeVideo } else { AVMediaTypeAudio }) else {
            return Vec::new();
        };
        asset.tracksWithMediaType(kind).to_vec()
    }
}

fn reader_error_for(asset: &AVURLAsset) -> String {
    // SAFETY: Probes `AVAssetReader` initialization to surface AVFoundation's `NSError`.
    match unsafe { AVAssetReader::assetReaderWithAsset_error(asset) } {
        Err(e) => e.to_string(),
        Ok(_) => "It isn't a movie RandomCard can play".into(),
    }
}

fn reader_status_error(reader: &AVAssetReader) -> String {
    // SAFETY: Reads `error` on a failed `AVAssetReader`.
    match unsafe { reader.error() } {
        Some(e) => e.to_string(),
        None => "It isn't a movie RandomCard can play".into(),
    }
}

/// Read the movie header (`mvhd`) duration of a QuickTime or MP4 file without
/// reading `mdat`, so movie durations match QuickTime's and GStreamer's.
fn mp4_mvhd_duration(path: &Path) -> Option<f64> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let mut pos = 0u64;
    while pos + 8 <= len {
        f.seek(SeekFrom::Start(pos)).ok()?;
        let mut head = [0u8; 8];
        f.read_exact(&mut head).ok()?;
        let raw_size = u32::from_be_bytes(head[..4].try_into().ok()?) as u64;
        let (hdr, size) = match raw_size {
            0 => (8u64, len - pos),
            1 => {
                let mut ext = [0u8; 8];
                f.read_exact(&mut ext).ok()?;
                (16u64, u64::from_be_bytes(ext))
            }
            n => (8u64, n),
        };
        if size < hdr || pos + size > len {
            return None;
        }
        if &head[4..8] == b"moov" {
            let body_len = size - hdr;
            if body_len > 64 * 1024 * 1024 {
                return None;
            }
            let mut moov = vec![0u8; body_len as usize];
            f.read_exact(&mut moov).ok()?;
            return parse_mvhd(&moov);
        }
        pos += size;
    }
    None
}

fn parse_mvhd(moov: &[u8]) -> Option<f64> {
    let mut at = 0usize;
    while at + 8 <= moov.len() {
        let size = u32::from_be_bytes(moov[at..at + 4].try_into().ok()?) as usize;
        if size < 8 || at + size > moov.len() {
            return None;
        }
        if &moov[at + 4..at + 8] == b"mvhd" {
            let body = &moov[at + 8..at + size];
            let version = *body.first()?;
            let (scale, dur) = match version {
                0 if body.len() >= 20 => (
                    u32::from_be_bytes(body[12..16].try_into().ok()?),
                    u32::from_be_bytes(body[16..20].try_into().ok()?) as u64,
                ),
                1 if body.len() >= 28 => (
                    u32::from_be_bytes(body[20..24].try_into().ok()?),
                    u64::from_be_bytes(body[24..32].try_into().ok()?),
                ),
                _ => return None,
            };
            if scale == 0 || dur == 0 || dur == u32::MAX as u64 || dur == u64::MAX {
                return None;
            }
            return Some(dur as f64 / scale as f64);
        }
        at += size;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOUR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/model/tour_movie/Card Trick.mp4");

    #[test]
    fn rejects_missing_and_non_movies() {
        assert!(Player::open(Path::new("/no/such/movie.mp4")).is_err());
        assert!(Player::open(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))).is_err());
    }

    #[test]
    fn mvhd_header_duration() {
        let d = mp4_mvhd_duration(Path::new(TOUR)).expect("mvhd duration");
        assert_eq!(crate::runtime::movies::system_time(d), "4,1.4");
    }

    #[test]
    fn frames_seeking_forward_and_reverse() {
        let Ok(p) = Player::open(Path::new(TOUR)) else {
            // VideoToolbox XPC is unavailable inside restricted sandboxes.
            return;
        };
        assert_eq!(p.size(), (320, 240));
        assert_eq!(crate::runtime::movies::system_time(p.duration()), "4,1.4");
        assert_eq!(p.time(), 0.0);
        assert!(!p.finished());

        // First frame is ready immediately, cached by Arc, opaque, top-down,
        // unskewed, and in RGBA order rather than BGRA.
        let f0 = p.frame().expect("first frame");
        let f0_again = p.frame().expect("cached first frame");
        assert!(Arc::ptr_eq(&f0, &f0_again));
        assert_eq!(f0.serial, 1);
        assert_eq!(f0.rgba.len(), 320 * 240 * 4);
        assert!(f0.rgba.as_chunks::<4>().0.iter().all(|px| px[3] == 255));
        let px = |f: &Frame, x: usize, y: usize| -> [u8; 4] {
            let i = (y * f.width as usize + x) * 4;
            f.rgba[i..i + 4].try_into().unwrap()
        };
        // Top-left is dark green felt; bottom-left is bright; center is blue
        // card back (B >> R); (207, 183) is warm gold (R >> B).
        let tl = px(&f0, 10, 10);
        let bl = px(&f0, 10, 230);
        let mid = px(&f0, 160, 120);
        let gold = px(&f0, 207, 183);
        assert!(tl[1] > tl[0] && tl[1] < 130, "top-left felt: {tl:?}");
        assert!(bl[0] > 170 && bl[1] > 220, "bottom-left bright: {bl:?}");
        assert!(mid[2] > 130 && mid[0] < 60, "center blue card back (R/B not swapped): {mid:?}");
        assert!(gold[0] > gold[2] + 50, "gold detail (R/B not swapped): {gold:?}");

        // Seeking while paused updates time and decodes the sought frame.
        p.seek(2.0);
        assert!((p.time() - 2.0).abs() < 1e-6);
        let f_seek = p.frame().expect("sought frame");
        assert_eq!(f_seek.serial, 2);
        assert_ne!(f_seek.rgba, f0.rgba);

        // Forward playback advances both time and frames.
        // The clock follows the real time that passes, which on a busy
        // machine can be well over the sleeps' total, so that's measured.
        let started = Instant::now();
        p.set_rate(1.0);
        let mut last_serial = f_seek.serial;
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(25));
            last_serial = p.frame().unwrap().serial;
        }
        p.set_rate(0.0);
        let played = started.elapsed().as_secs_f64();
        assert!(last_serial > f_seek.serial + 2, "frames advanced forward: {last_serial}");
        assert!(p.time() > 2.1 && (p.time() - (2.0 + played)).abs() < 0.15, "time moved forward: {} in {played}", p.time());

        // Reverse playback moves time and frames backward, then finishes at 0.
        p.seek(2.5);
        let before_rev = p.frame().unwrap().serial;
        let started = Instant::now();
        p.set_rate(-1.0);
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(25));
            let _ = p.frame();
        }
        p.set_rate(0.0);
        let played = started.elapsed().as_secs_f64();
        let after_rev = p.frame().unwrap().serial;
        let expected = (2.5 - played).max(0.0);
        assert!(p.time() < 2.35 && (p.time() - expected).abs() < 0.15, "time moved backward: {} in {played}", p.time());
        assert!(after_rev > before_rev + 2, "frames stepped backward: {before_rev} -> {after_rev}");

        p.seek(0.15);
        p.set_rate(-2.0);
        std::thread::sleep(Duration::from_millis(150));
        assert!(p.finished());
        assert_eq!(p.time(), 0.0);

        // Playing forward past the end stops at duration and reports finished.
        let dur = p.duration();
        p.seek(dur - 0.15);
        p.set_rate(1.0);
        std::thread::sleep(Duration::from_millis(250));
        assert!(p.finished());
        assert!((p.time() - dur).abs() < 1e-6);
    }

    #[test]
    fn audio_feeder_lifecycle() {
        autoreleasepool(|_| unsafe {
            let url = NSURL::fileURLWithPath(&NSString::from_str(TOUR));
            let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
            let Some(at) = media_tracks(&asset, false).into_iter().next() else { return };
            let Ok((reader, output)) = open_audio_at(&asset, &at, 0.0) else { return };
            let r: Option<Retained<AVSampleBufferAudioRenderer>> =
                msg_send![AVSampleBufferAudioRenderer::alloc(), init];
            let Some(renderer) = r else { return };
            renderer.setMuted(true);
            let feeder = AudioFeeder::new(renderer);
            feeder.set_volume(0.0);
            feeder.start(reader, output);
            std::thread::sleep(Duration::from_millis(30));
            feeder.silence();
        });
    }
}
