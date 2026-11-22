//! Movies on the web, on a Mac. The file player reads movies with
//! AVAssetReader, which reads only files; a movie on the web streams through
//! AVPlayer instead. AVPlayer plays the sound itself, and the pictures come
//! from an AVPlayerItemVideoOutput as the UI asks for them.
//!
//! AVPlayer and its item are made on the main thread, as AppKit wants. After
//! that, calls come straight from whichever thread asks: going back to the
//! main thread for them could wait on a UI that's waiting on the script.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use objc2::AnyThread;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::AnyObject;
use objc2_av_foundation::{
    AVPlayer, AVPlayerItem, AVPlayerItemStatus, AVPlayerItemVideoOutput, AVURLAsset, AVURLAssetOverrideMIMETypeKey,
};
use objc2_core_video::{kCVPixelBufferPixelFormatTypeKey, kCVPixelFormatType_32BGRA};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

use super::Frame;
use super::avfoundation::{Rot, cm_time, copy_bgra_to_rgba, valid_seconds};

/// How long a movie on the web may take to be ready to play.
const OPEN_WAIT: Duration = Duration::from_secs(20);

/// The player, its item and the output that hands over its pictures.
struct Objects {
    player: Retained<AVPlayer>,
    item: Retained<AVPlayerItem>,
    output: Retained<AVPlayerItemVideoOutput>,
}

// SAFETY: Made on the main thread; AVPlayer, its item and the output are
// then used from the script and UI threads, as AVFoundation allows for
// playback control, time and video output.
unsafe impl Send for Objects {}
unsafe impl Sync for Objects {}

/// The newest picture, and how many there have been.
struct Shown {
    frame: Option<Arc<Frame>>,
    serial: u64,
}

pub struct StreamPlayer {
    objects: Objects,
    size: (u32, u32),
    duration: f64,
    shown: Mutex<Shown>,
}

impl StreamPlayer {
    /// Open a movie at an http or https address, paused at its start.
    pub fn open(url: &str) -> Result<StreamPlayer, String> {
        let address = url.to_string();
        // A server that doesn't say what the movie is (and an address with
        // no extension) leaves AVPlayer unable to open it ("Cannot Open"):
        // its first bytes say, and AVPlayer is told. When the peek fails,
        // AVPlayer tries anyway, and says why it can't.
        let mime = crate::web::http::peek(url, None, 16, Duration::from_secs(10))
            .ok()
            .and_then(|(ct, head)| super::web_movie_type(&ct, &head));
        let objects = dispatch2::run_on_main(move |mtm| {
            autoreleasepool(|_| {
                // SAFETY: Making AVFoundation objects on the main thread, with
                // BGRA pixel buffers asked of the output.
                unsafe {
                    let url = NSURL::URLWithString(&NSString::from_str(&address)).ok_or("That isn't a web address".to_string())?;
                    let options = mime.map(|m| {
                        let m = NSString::from_str(m);
                        NSDictionary::<NSString, AnyObject>::from_slices(&[AVURLAssetOverrideMIMETypeKey], &[m.as_ref()])
                    });
                    let asset = AVURLAsset::URLAssetWithURL_options(&url, options.as_deref());
                    let item = AVPlayerItem::playerItemWithAsset(&asset, mtm);
                    let fmt = NSNumber::numberWithUnsignedInt(kCVPixelFormatType_32BGRA);
                    let fmt_key: &NSString = &*(kCVPixelBufferPixelFormatTypeKey as *const _ as *const NSString);
                    let attrs = NSDictionary::<NSString, AnyObject>::from_slices(&[fmt_key], &[fmt.as_ref()]);
                    let output = AVPlayerItemVideoOutput::initWithPixelBufferAttributes(AVPlayerItemVideoOutput::alloc(), Some(&attrs));
                    item.addOutput(&output);
                    let player = AVPlayer::playerWithPlayerItem(Some(&item), mtm);
                    Ok::<Objects, String>(Objects { player, item, output })
                }
            })
        })?;
        // Ready, or why not, as it loads.
        let start = Instant::now();
        loop {
            // SAFETY: Reading the item's status and error.
            let status = unsafe { objects.item.status() };
            if status == AVPlayerItemStatus::ReadyToPlay {
                break;
            }
            if status == AVPlayerItemStatus::Failed {
                let why = unsafe { objects.item.error() }
                    .map(|e| e.localizedDescription().to_string())
                    .unwrap_or_else(|| "The movie couldn't be played".into());
                return Err(why);
            }
            if start.elapsed() > OPEN_WAIT {
                return Err("The movie took too long to open".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        // SAFETY: Reading the ready item's size and length, and going to its
        // start, which brings its first picture.
        let (size, duration) = unsafe {
            let s = objects.item.presentationSize();
            objects.player.seekToTime_toleranceBefore_toleranceAfter(cm_time(0.0), cm_time(0.0), cm_time(0.0));
            ((s.width.max(0.0) as u32, s.height.max(0.0) as u32), valid_seconds(objects.item.duration()).unwrap_or(0.0))
        };
        Ok(StreamPlayer { objects, size, duration, shown: Mutex::new(Shown { frame: None, serial: 0 }) })
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn time(&self) -> f64 {
        // SAFETY: Reading the player's time.
        valid_seconds(unsafe { self.objects.player.currentTime() }).unwrap_or(0.0)
    }

    pub fn seek(&self, secs: f64) {
        // SAFETY: An exact seek, so the controller and scripts see the time
        // they asked for.
        unsafe { self.objects.player.seekToTime_toleranceBefore_toleranceAfter(cm_time(secs), cm_time(0.0), cm_time(0.0)) }
    }

    /// Backward only where the stream allows it; otherwise it pauses.
    pub fn set_rate(&self, rate: f64) {
        // SAFETY: Setting the playback rate.
        unsafe {
            let rate = if rate < 0.0 && !self.objects.item.canPlayReverse() { 0.0 } else { rate };
            self.objects.player.setRate(rate as f32);
        }
    }

    pub fn set_volume(&self, level: f64) {
        // SAFETY: Setting the player's volume, 0 to 1.
        unsafe { self.objects.player.setVolume(level.clamp(0.0, 1.0) as f32) }
    }

    /// The newest picture: a new one when the output has it.
    pub fn frame(&self) -> Option<Arc<Frame>> {
        let mut shown = self.shown.lock().unwrap_or_else(|e| e.into_inner());
        autoreleasepool(|_| {
            // SAFETY: Asking the output for the picture due now, and copying
            // it out of its locked pixel buffer.
            unsafe {
                let now = self.objects.item.currentTime();
                if (shown.frame.is_none() || self.objects.output.hasNewPixelBufferForItemTime(now))
                    && let Some(buf) = self.objects.output.copyPixelBufferForItemTime_itemTimeForDisplay(now, std::ptr::null_mut())
                    && let Some((width, height, rgba)) = copy_bgra_to_rgba(&buf, Rot::None)
                {
                    shown.serial += 1;
                    shown.frame = Some(Arc::new(Frame { width, height, rgba, serial: shown.serial }));
                }
            }
        });
        shown.frame.clone()
    }

    /// It has played to its end and stopped.
    pub fn finished(&self) -> bool {
        // SAFETY: Reading the player's rate.
        let stopped = unsafe { self.objects.player.rate() } == 0.0;
        self.duration > 0.0 && stopped && self.time() >= self.duration - 0.05
    }
}

impl Drop for StreamPlayer {
    fn drop(&mut self) {
        // SAFETY: Stopping it, so its sound stops with the window.
        unsafe { self.objects.player.pause() }
    }
}
