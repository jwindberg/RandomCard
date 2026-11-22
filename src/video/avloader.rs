//! Movies from a connection that signs in, on a Mac. AVPlayer has no public
//! way to send a connection's headers, so the movie's address is given to
//! it under a scheme of our own, which it can't load: it asks this
//! loader, an AVAssetResourceLoaderDelegate, for each stretch of the movie
//! it wants, and the loader gets those bytes with RandomCard's own HTTP
//! client, with the connection's credentials, following no redirects.

use std::io::Read;
use std::sync::{Arc, Mutex};

use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
use objc2::{AnyThread, DefinedClass, Message, define_class, msg_send};
use objc2_av_foundation::{
    AVAssetResourceLoader, AVAssetResourceLoaderDelegate, AVAssetResourceLoadingDataRequest, AVAssetResourceLoadingRequest,
};
use objc2_foundation::{NSData, NSDictionary, NSError, NSLocalizedDescriptionKey, NSString};

use crate::web::http;

/// The scheme a signed-in movie's address is given to AVPlayer under.
pub(super) const SCHEME: &str = "randomcard-movie";

/// How much is handed to AVPlayer at a time.
const CHUNK: usize = 256 * 1024;

/// Where a movie comes from, and what it is.
pub(super) struct Source {
    /// The address sent: with a connection's query keys in it.
    pub sent: String,
    /// The address shown in errors: without them.
    pub shown: String,
    pub headers: Vec<(String, String)>,
    /// Its Uniform Type Identifier, as AVFoundation wants its type.
    pub uti: &'static str,
    pub length: u64,
    /// The server sends ranges, so AVPlayer can seek in it.
    pub ranges: bool,
    /// Why the last stretch couldn't be got, for the error that follows.
    pub failed: Mutex<Option<String>>,
}

impl Source {
    fn hide(&self, error: String) -> String {
        if self.sent == self.shown { error } else { error.replace(&self.sent, &self.shown) }
    }
}

pub(super) struct Ivars {
    source: Arc<Source>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the loader
    // doesn't implement Drop.
    #[unsafe(super(NSObject))]
    #[name = "RandomCardMovieLoader"]
    #[ivars = Ivars]
    pub(super) struct Loader;

    unsafe impl NSObjectProtocol for Loader {}

    unsafe impl AVAssetResourceLoaderDelegate for Loader {
        /// Each stretch is got on a thread of its own, so the loader's
        /// queue is free for the next, and the request is finished there.
        #[unsafe(method(resourceLoader:shouldWaitForLoadingOfRequestedResource:))]
        fn should_wait(&self, _loader: &AVAssetResourceLoader, request: &AVAssetResourceLoadingRequest) -> bool {
            let job = Job(request.retain());
            let source = self.ivars().source.clone();
            std::thread::spawn(move || job.run(&source));
            true
        }
    }
);

impl Loader {
    pub(super) fn new(source: Arc<Source>) -> Retained<Loader> {
        let this = Self::alloc().set_ivars(Ivars { source });
        // SAFETY: NSObject's init, on a freshly allocated loader.
        unsafe { msg_send![super(this), init] }
    }

    pub(super) fn source(&self) -> &Source {
        &self.ivars().source
    }
}

/// One loading request, kept until it's finished.
struct Job(Retained<AVAssetResourceLoadingRequest>);

// SAFETY: AVFoundation lets a loading request be answered and finished from
// any thread; only this job's thread uses it.
unsafe impl Send for Job {}

impl Job {
    fn run(self, source: &Source) {
        let req = &self.0;
        autoreleasepool(|_| {
            // SAFETY: Answering the loading request: its type and length,
            // then its bytes, then finishing it (unless AVPlayer cancelled
            // it, which needs no answer).
            unsafe {
                if let Some(info) = req.contentInformationRequest() {
                    info.setContentType(Some(&NSString::from_str(source.uti)));
                    info.setContentLength(source.length as i64);
                    info.setByteRangeAccessSupported(source.ranges);
                }
                let Some(data) = req.dataRequest() else {
                    req.finishLoading();
                    return;
                };
                let start = data.currentOffset().max(0) as u64;
                let end = if data.requestsAllDataToEndOfResource() {
                    source.length
                } else {
                    (data.requestedOffset().max(0) as u64).saturating_add(data.requestedLength().max(0) as u64).min(source.length)
                };
                match feed(req, &data, source, start, end) {
                    Ok(true) => req.finishLoading(),
                    Ok(false) => {}
                    Err(why) => {
                        let why = source.hide(why);
                        *source.failed.lock().unwrap_or_else(|e| e.into_inner()) = Some(why.clone());
                        req.finishLoadingWithError(Some(&error(&why)));
                    }
                }
            }
        });
    }
}

/// Hand AVPlayer the bytes from `start` up to `end`. Ok(false) when it
/// stopped wanting them.
///
/// # Safety
/// `req` and `data` are a live loading request and its data request.
unsafe fn feed(
    req: &AVAssetResourceLoadingRequest,
    data: &AVAssetResourceLoadingDataRequest,
    source: &Source,
    start: u64,
    end: u64,
) -> Result<bool, String> {
    if start >= end {
        return Ok(true);
    }
    let mut reply = http::get_range(&source.sent, &source.headers, start, Some(end - 1))?;
    // A server that sends all of it, not the range: up to the start.
    if reply.start > start {
        return Err("The server sent the wrong part of the movie".into());
    }
    let skip = start - reply.start;
    std::io::copy(&mut (&mut reply.body).take(skip), &mut std::io::sink()).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; CHUNK];
    let mut left = end - start;
    while left > 0 {
        // SAFETY: Asking whether the live request was cancelled.
        if unsafe { req.isCancelled() } {
            return Ok(false);
        }
        let want = (left as usize).min(CHUNK);
        let n = reply.body.read(&mut buf[..want]).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("The movie ended early".into());
        }
        // SAFETY: Handing the live data request the bytes that follow.
        autoreleasepool(|_| unsafe { data.respondWithData(&NSData::with_bytes(&buf[..n])) });
        left -= n as u64;
    }
    Ok(true)
}

/// An error AVPlayer can carry, saying why.
fn error(why: &str) -> Retained<NSError> {
    let text = NSString::from_str(why);
    // SAFETY: The standard localized-description key, with a string.
    unsafe {
        let info = NSDictionary::<NSString, AnyObject>::from_slices(&[NSLocalizedDescriptionKey], &[text.as_ref()]);
        NSError::errorWithDomain_code_userInfo(&NSString::from_str("RandomCard"), 1, Some(&info))
    }
}

/// The Uniform Type Identifier AVFoundation wants for a MIME type it plays.
pub(super) fn uti(mime: &str) -> Option<&'static str> {
    let mime = mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    Some(match mime.as_str() {
        "video/mp4" => "public.mpeg-4",
        "video/quicktime" => "com.apple.quicktime-movie",
        "video/x-m4v" => "com.apple.m4v-video",
        "video/mpeg" => "public.mpeg",
        "video/mp2t" => "public.mpeg-2-transport-stream",
        "audio/mp4" | "audio/x-m4a" | "audio/m4a" => "com.apple.m4a-audio",
        "audio/mpeg" | "audio/mp3" => "public.mp3",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "com.microsoft.waveform-audio",
        "audio/aiff" | "audio/x-aiff" => "public.aiff-audio",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn types_as_avfoundation_wants_them() {
        assert_eq!(super::uti("video/mp4"), Some("public.mpeg-4"));
        assert_eq!(super::uti("Video/QuickTime; charset=binary"), Some("com.apple.quicktime-movie"));
        assert_eq!(super::uti("application/octet-stream"), None);
        assert_eq!(super::uti("application/vnd.apple.mpegurl"), None);
    }
}
