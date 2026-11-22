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
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::runtime::ProtocolObject;
use objc2_foundation::{NSDate, NSDictionary, NSNumber, NSRunLoop, NSString, NSURL};

use crate::web::http;

use super::{Frame, avloader};
use super::avfoundation::{Rot, cm_time, copy_bgra_to_rgba, valid_seconds};

/// How long a movie on the web may take to be ready to play.
const OPEN_WAIT: Duration = Duration::from_secs(20);

/// The player, its item and the output that hands over its pictures.
struct Objects {
    player: Retained<AVPlayer>,
    item: Retained<AVPlayerItem>,
    output: Retained<AVPlayerItemVideoOutput>,
    /// A signed-in movie's loader and its queue: AVFoundation holds the
    /// loader weakly, so the player keeps it.
    _loader: Option<Retained<avloader::Loader>>,
    _queue: Option<DispatchRetained<DispatchQueue>>,
}

// SAFETY: Made on the main thread; AVPlayer, its item and the output are
// then used from the script and UI threads, as AVFoundation allows for
// playback control, time and video output.
unsafe impl Send for Objects {}
unsafe impl Sync for Objects {}

/// How AVPlayer gets the movie.
enum How {
    /// From its address, told its type when the server doesn't say.
    Direct { address: String, mime: Option<&'static str> },
    /// Through the loader, which signs in.
    Loader(Arc<avloader::Source>),
}

/// What a signed-in movie is, and how long, from its first bytes: the
/// loader must tell AVPlayer both before it asks for any of it.
fn probe(shown: &str, sent: &str, headers: Vec<(String, String)>) -> Result<avloader::Source, String> {
    use std::io::Read;
    let mut reply = http::get_range(sent, &headers, 0, Some(15))?;
    let mut head = Vec::with_capacity(16);
    (&mut reply.body).take(16).read_to_end(&mut head).map_err(|e| e.to_string())?;
    if super::web_movie_type(&reply.content_type, &head) == Some("application/vnd.apple.mpegurl")
        || reply.content_type.to_ascii_lowercase().contains("mpegurl")
    {
        return Err("A live stream from a connection that signs in doesn't play on a Mac yet".into());
    }
    let mime = super::web_movie_type(&reply.content_type, &head).unwrap_or(&reply.content_type);
    let uti = avloader::uti(mime).ok_or("The server didn't say what kind of movie it is")?;
    let length = reply.total.ok_or("The server didn't say how long the movie is")?;
    Ok(avloader::Source {
        sent: sent.to_string(),
        shown: shown.to_string(),
        headers,
        uti,
        length,
        ranges: reply.status == 206,
        failed: Mutex::new(None),
    })
}

/// A moment, while AVPlayer loads. On the main thread (in tests), its run
/// loop runs, as AVPlayer's news comes there.
fn wait() {
    if objc2::MainThreadMarker::new().is_some() {
        NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.02));
    } else {
        std::thread::sleep(Duration::from_millis(20));
    }
}

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
    /// Open a movie at an http or https address, paused at its start, with
    /// a connection's credentials if it has them: query keys go in the
    /// address AVPlayer gets, and headers mean the movie comes through the
    /// loader (see avloader), which sends them.
    pub fn open(url: &str, credentials: Option<&http::Credentials>) -> Result<StreamPlayer, String> {
        let (sent, headers) = http::with_credentials(url, credentials)?;
        let hide = |e: String| if sent == url { e } else { e.replace(&sent, url) };
        let how = if headers.is_empty() {
            // A server that doesn't say what the movie is (and an address
            // with no extension) leaves AVPlayer unable to open it ("Cannot
            // Open"): its first bytes say, and AVPlayer is told. When the
            // peek fails, AVPlayer tries anyway, and says why it can't.
            let mime = match http::peek(url, credentials, 16, Duration::from_secs(10)) {
                Ok((ct, head)) => super::web_movie_type(&ct, &head),
                // A server that wants signing in says so better than AVPlayer.
                Err(e) if e.starts_with("401") || e.starts_with("403") => return Err(e),
                Err(_) => None,
            };
            How::Direct { address: sent.clone(), mime }
        } else {
            How::Loader(Arc::new(probe(url, &sent, headers).map_err(hide)?))
        };
        let objects = dispatch2::run_on_main(move |mtm| {
            autoreleasepool(|_| {
                // SAFETY: Making AVFoundation objects on the main thread, with
                // BGRA pixel buffers asked of the output.
                unsafe {
                    let bad = || "That isn't a web address".to_string();
                    let (asset, loader, queue) = match how {
                        How::Direct { address, mime } => {
                            let url = NSURL::URLWithString(&NSString::from_str(&address)).ok_or_else(bad)?;
                            let options = mime.map(|m| {
                                let m = NSString::from_str(m);
                                NSDictionary::<NSString, AnyObject>::from_slices(&[AVURLAssetOverrideMIMETypeKey], &[m.as_ref()])
                            });
                            (AVURLAsset::URLAssetWithURL_options(&url, options.as_deref()), None, None)
                        }
                        How::Loader(source) => {
                            // The address under our own scheme, so AVPlayer
                            // asks the loader for it.
                            let rest = source.shown.split_once("://").map(|(_, r)| r).ok_or_else(bad)?;
                            let ours = format!("{}://{rest}", avloader::SCHEME);
                            let url = NSURL::URLWithString(&NSString::from_str(&ours)).ok_or_else(bad)?;
                            let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
                            let loader = avloader::Loader::new(source);
                            let queue = DispatchQueue::new("RandomCard movie loader", None);
                            asset.resourceLoader().setDelegate_queue(Some(ProtocolObject::from_ref(&*loader)), Some(&queue));
                            (asset, Some(loader), Some(queue))
                        }
                    };
                    let item = AVPlayerItem::playerItemWithAsset(&asset, mtm);
                    let fmt = NSNumber::numberWithUnsignedInt(kCVPixelFormatType_32BGRA);
                    let fmt_key: &NSString = &*(kCVPixelBufferPixelFormatTypeKey as *const _ as *const NSString);
                    let attrs = NSDictionary::<NSString, AnyObject>::from_slices(&[fmt_key], &[fmt.as_ref()]);
                    let output = AVPlayerItemVideoOutput::initWithPixelBufferAttributes(AVPlayerItemVideoOutput::alloc(), Some(&attrs));
                    item.addOutput(&output);
                    let player = AVPlayer::playerWithPlayerItem(Some(&item), mtm);
                    // Apple's advice for a loader that loads the media itself:
                    // AVPlayer's guesses at what's coming don't hold for it.
                    if loader.is_some() {
                        player.setAutomaticallyWaitsToMinimizeStalling(false);
                    }
                    Ok::<Objects, String>(Objects { player, item, output, _loader: loader, _queue: queue })
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
                // The loader's own reason first: AVPlayer's for it is vaguer.
                let loaded = objects._loader.as_ref().and_then(|l| l.source().failed.lock().ok().and_then(|f| f.clone()));
                let why = loaded
                    .or_else(|| unsafe { objects.item.error() }.map(|e| e.localizedDescription().to_string()))
                    .unwrap_or_else(|| "The movie couldn't be played".into());
                return Err(hide(why));
            }
            if start.elapsed() > OPEN_WAIT {
                return Err("The movie took too long to open".into());
            }
            wait();
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
