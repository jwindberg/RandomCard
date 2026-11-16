//! Movies on Linux, with GStreamer. Its libraries are loaded when the first
//! movie opens rather than linked, so RandomCard starts on a system without
//! them, and building it needs nothing of GStreamer's. A movie is a playbin
//! (which finds the decoders, hardware ones first, and plays the sound)
//! with an app sink taking its pictures as RGBA.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use libloading::Library;

use super::Frame;

type Ptr = *mut c_void;

const STATE_NULL: c_int = 1;
const STATE_PAUSED: c_int = 3;
const STATE_PLAYING: c_int = 4;
const STATE_CHANGE_FAILURE: c_int = 0;
const FORMAT_TIME: c_int = 3;
const SEEK_FLUSH_ACCURATE: c_int = 1 | 2;
const SEEK_TYPE_NONE: c_int = 0;
const SEEK_TYPE_SET: c_int = 1;
const MAP_READ: c_int = 1;
const MESSAGE_ERROR: c_int = 1 << 1;
const SECOND: f64 = 1e9;

#[repr(C)]
struct MapInfo {
    memory: Ptr,
    flags: c_int,
    data: *const u8,
    size: usize,
    maxsize: usize,
    user_data: [Ptr; 4],
    reserved: [Ptr; 4],
}

#[repr(C)]
struct GError {
    domain: u32,
    code: c_int,
    message: *const c_char,
}

/// GstAppSinkCallbacks: seven pointers, those after the third optional.
#[repr(C)]
struct Callbacks {
    eos: Option<unsafe extern "C" fn(Ptr, Ptr)>,
    new_preroll: Option<unsafe extern "C" fn(Ptr, Ptr) -> c_int>,
    new_sample: Option<unsafe extern "C" fn(Ptr, Ptr) -> c_int>,
    rest: [Ptr; 4],
}

/// The GStreamer functions movies use.
struct Gst {
    _libs: Vec<Library>,
    init_check: unsafe extern "C" fn(Ptr, Ptr, *mut *mut GError) -> c_int,
    factory_make: unsafe extern "C" fn(*const c_char, *const c_char) -> Ptr,
    object_set: unsafe extern "C" fn(Ptr, *const c_char, ...),
    object_unref: unsafe extern "C" fn(Ptr),
    caps_from_string: unsafe extern "C" fn(*const c_char) -> Ptr,
    mini_object_unref: unsafe extern "C" fn(Ptr),
    filename_to_uri: unsafe extern "C" fn(*const c_char, *mut *mut GError) -> *mut c_char,
    set_state: unsafe extern "C" fn(Ptr, c_int) -> c_int,
    get_state: unsafe extern "C" fn(Ptr, *mut c_int, *mut c_int, u64) -> c_int,
    seek: unsafe extern "C" fn(Ptr, f64, c_int, c_int, c_int, i64, c_int, i64) -> c_int,
    query_position: unsafe extern "C" fn(Ptr, c_int, *mut i64) -> c_int,
    query_duration: unsafe extern "C" fn(Ptr, c_int, *mut i64) -> c_int,
    get_bus: unsafe extern "C" fn(Ptr) -> Ptr,
    bus_pop: unsafe extern "C" fn(Ptr) -> Ptr,
    bus_pop_filtered: unsafe extern "C" fn(Ptr, c_int) -> Ptr,
    parse_error: unsafe extern "C" fn(Ptr, *mut *mut GError, *mut *mut c_char),
    error_free: unsafe extern "C" fn(*mut GError),
    free: unsafe extern "C" fn(Ptr),
    sample_get_buffer: unsafe extern "C" fn(Ptr) -> Ptr,
    sample_get_caps: unsafe extern "C" fn(Ptr) -> Ptr,
    caps_get_structure: unsafe extern "C" fn(Ptr, u32) -> Ptr,
    structure_get_int: unsafe extern "C" fn(Ptr, *const c_char, *mut c_int) -> c_int,
    buffer_map: unsafe extern "C" fn(Ptr, *mut MapInfo, c_int) -> c_int,
    buffer_unmap: unsafe extern "C" fn(Ptr, *mut MapInfo),
    pull_sample: unsafe extern "C" fn(Ptr) -> Ptr,
    pull_preroll: unsafe extern "C" fn(Ptr) -> Ptr,
    set_callbacks: unsafe extern "C" fn(Ptr, *const Callbacks, Ptr, Ptr),
}

// The function pointers are plain C functions GStreamer lets any thread call.
unsafe impl Send for Gst {}
unsafe impl Sync for Gst {}

fn gst() -> Result<&'static Gst, String> {
    static GST: OnceLock<Result<Gst, String>> = OnceLock::new();
    GST.get_or_init(load).as_ref().map_err(Clone::clone)
}

fn load() -> Result<Gst, String> {
    let missing = "Movies need GStreamer, which isn't installed. Install the gstreamer1.0-plugins-good and gstreamer1.0-libav packages (or your system's GStreamer plugins)";
    let open = |name: &str| unsafe { Library::new(name) }.map_err(|_| missing.to_string());
    let (core, app, gobject, glib) =
        (open("libgstreamer-1.0.so.0")?, open("libgstapp-1.0.so.0")?, open("libgobject-2.0.so.0")?, open("libglib-2.0.so.0")?);
    macro_rules! get {
        ($lib:expr, $name:literal) => {
            *unsafe { $lib.get(concat!($name, "\0").as_bytes()) }.map_err(|_| format!("This GStreamer has no {}", $name))?
        };
    }
    let gst = Gst {
        init_check: get!(core, "gst_init_check"),
        factory_make: get!(core, "gst_element_factory_make"),
        object_set: get!(gobject, "g_object_set"),
        object_unref: get!(core, "gst_object_unref"),
        caps_from_string: get!(core, "gst_caps_from_string"),
        mini_object_unref: get!(core, "gst_mini_object_unref"),
        filename_to_uri: get!(core, "gst_filename_to_uri"),
        set_state: get!(core, "gst_element_set_state"),
        get_state: get!(core, "gst_element_get_state"),
        seek: get!(core, "gst_element_seek"),
        query_position: get!(core, "gst_element_query_position"),
        query_duration: get!(core, "gst_element_query_duration"),
        get_bus: get!(core, "gst_element_get_bus"),
        bus_pop: get!(core, "gst_bus_pop"),
        bus_pop_filtered: get!(core, "gst_bus_pop_filtered"),
        parse_error: get!(core, "gst_message_parse_error"),
        error_free: get!(glib, "g_error_free"),
        free: get!(glib, "g_free"),
        sample_get_buffer: get!(core, "gst_sample_get_buffer"),
        sample_get_caps: get!(core, "gst_sample_get_caps"),
        caps_get_structure: get!(core, "gst_caps_get_structure"),
        structure_get_int: get!(core, "gst_structure_get_int"),
        buffer_map: get!(core, "gst_buffer_map"),
        buffer_unmap: get!(core, "gst_buffer_unmap"),
        pull_sample: get!(app, "gst_app_sink_pull_sample"),
        pull_preroll: get!(app, "gst_app_sink_pull_preroll"),
        set_callbacks: get!(app, "gst_app_sink_set_callbacks"),
        _libs: vec![core, app, gobject, glib],
    };
    let mut err = std::ptr::null_mut();
    if unsafe { (gst.init_check)(std::ptr::null_mut(), std::ptr::null_mut(), &mut err) } == 0 {
        return Err(format!("GStreamer didn't start: {}", unsafe { gst.take_error(err) }));
    }
    Ok(gst)
}

impl Gst {
    /// A GError's message, freeing it.
    unsafe fn take_error(&self, err: *mut GError) -> String {
        if err.is_null() {
            return "unknown error".into();
        }
        let text = unsafe { CStr::from_ptr((*err).message) }.to_string_lossy().into_owned();
        unsafe { (self.error_free)(err) };
        text
    }
}

/// What the app sink's callbacks fill in, on GStreamer's threads.
struct Sink {
    frame: Mutex<Option<Arc<Frame>>>,
    serial: AtomicU64,
    /// The newest frame's time in the movie, in nanoseconds (NO_TIME if
    /// none): where the movie is, as shown. GStreamer's own position
    /// doesn't move while it plays backward.
    pts: AtomicU64,
    eos: AtomicBool,
    /// A 256-color movie's own colors, for the standard ones GStreamer
    /// gives it (see `palette`).
    colors: Option<std::collections::HashMap<[u8; 3], [u8; 3]>>,
}

const NO_TIME: u64 = u64::MAX;

/// A GstBuffer's presentation time: after its GstMiniObject (64 bytes on a
/// 64-bit system) and buffer pool pointer, as GStreamer 1's ABI fixes it.
unsafe fn buffer_pts(buffer: Ptr) -> u64 {
    const OFFSET: usize = if cfg!(target_pointer_width = "64") { 72 } else { 40 };
    unsafe { std::ptr::read_unaligned((buffer as *const u8).add(OFFSET) as *const u64) }
}

unsafe extern "C" fn on_eos(_sink: Ptr, data: Ptr) {
    let sink = unsafe { &*(data as *const Sink) };
    sink.eos.store(true, Ordering::SeqCst);
}

unsafe extern "C" fn on_preroll(appsink: Ptr, data: Ptr) -> c_int {
    let Ok(gst) = gst() else { return 0 };
    unsafe { take_frame(gst, (gst.pull_preroll)(appsink), &*(data as *const Sink)) };
    0
}

unsafe extern "C" fn on_sample(appsink: Ptr, data: Ptr) -> c_int {
    let Ok(gst) = gst() else { return 0 };
    unsafe { take_frame(gst, (gst.pull_sample)(appsink), &*(data as *const Sink)) };
    0
}

/// Copy a sample's picture out as the newest frame, and let it go.
unsafe fn take_frame(gst: &Gst, sample: Ptr, sink: &Sink) {
    if sample.is_null() {
        return;
    }
    unsafe {
        let structure = (gst.caps_get_structure)((gst.sample_get_caps)(sample), 0);
        let (mut w, mut h) = (0, 0);
        (gst.structure_get_int)(structure, c"width".as_ptr(), &mut w);
        (gst.structure_get_int)(structure, c"height".as_ptr(), &mut h);
        let buffer = (gst.sample_get_buffer)(sample);
        let mut map: MapInfo = std::mem::zeroed();
        let need = w.max(0) as usize * h.max(0) as usize * 4;
        if !buffer.is_null() && (gst.buffer_map)(buffer, &mut map, MAP_READ) != 0 {
            if map.size >= need && need > 0 {
                let mut rgba = std::slice::from_raw_parts(map.data, need).to_vec();
                // Movies are opaque, but some old codecs (QuickTime's
                // 8-bit Graphics) come out with an alpha of 0.
                for px in rgba.as_chunks_mut::<4>().0 {
                    px[3] = 255;
                }
                if let Some(colors) = &sink.colors {
                    super::palette::remap(&mut rgba, colors);
                }
                let serial = sink.serial.fetch_add(1, Ordering::SeqCst) + 1;
                let frame = Arc::new(Frame { width: w as u32, height: h as u32, rgba, serial });
                *sink.frame.lock().unwrap_or_else(|e| e.into_inner()) = Some(frame);
                sink.pts.store(buffer_pts(buffer), Ordering::SeqCst);
            }
            (gst.buffer_unmap)(buffer, &mut map);
        }
        (gst.mini_object_unref)(sample);
    }
}

/// Where the movie stands, as last set.
struct State {
    rate: f64,
    /// Where it was paused or sent, while it isn't playing.
    held: f64,
    /// The rate GStreamer was last sent off at.
    sent: f64,
}

pub struct Player {
    gst: &'static Gst,
    playbin: Ptr,
    bus: Ptr,
    sink: *const Sink,
    duration: f64,
    size: (u32, u32),
    state: Mutex<State>,
}

// GStreamer's elements are safe to use from any thread; State is locked.
unsafe impl Send for Player {}
unsafe impl Sync for Player {}

impl Player {
    pub fn open(path: &Path) -> Result<Player, String> {
        let gst = gst()?;
        let file = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| "Bad file name".to_string())?;
        unsafe {
            let mut err = std::ptr::null_mut();
            let uri = (gst.filename_to_uri)(file.as_ptr(), &mut err);
            if uri.is_null() {
                return Err(gst.take_error(err));
            }
            let playbin = (gst.factory_make)(c"playbin".as_ptr(), std::ptr::null());
            let appsink = (gst.factory_make)(c"appsink".as_ptr(), std::ptr::null());
            if playbin.is_null() || appsink.is_null() {
                (gst.free)(uri as Ptr);
                return Err("GStreamer's playbin or appsink is missing: install the gstreamer1.0-plugins-base package".into());
            }
            let caps = (gst.caps_from_string)(c"video/x-raw,format=RGBA".as_ptr());
            let end: *const c_char = std::ptr::null();
            (gst.object_set)(appsink, c"caps".as_ptr(), caps, c"max-buffers".as_ptr(), 1 as c_int, c"drop".as_ptr(), 1 as c_int, end);
            (gst.mini_object_unref)(caps);
            let sink = Box::into_raw(Box::new(Sink { frame: Mutex::new(None), serial: AtomicU64::new(0), pts: AtomicU64::new(NO_TIME), eos: AtomicBool::new(false), colors: super::palette::remap_for(path) }));
            let callbacks = Callbacks { eos: Some(on_eos), new_preroll: Some(on_preroll), new_sample: Some(on_sample), rest: [std::ptr::null_mut(); 4] };
            (gst.set_callbacks)(appsink, &callbacks, sink as Ptr, std::ptr::null_mut());
            (gst.object_set)(playbin, c"uri".as_ptr(), uri, c"video-sink".as_ptr(), appsink, end);
            // Tests keep quiet, still playing in time.
            if cfg!(test) {
                let quiet = (gst.factory_make)(c"fakesink".as_ptr(), std::ptr::null());
                (gst.object_set)(quiet, c"sync".as_ptr(), 1 as c_int, end);
                (gst.object_set)(playbin, c"audio-sink".as_ptr(), quiet, end);
            }
            (gst.free)(uri as Ptr);
            let bus = (gst.get_bus)(playbin);
            let mut player = Player { gst, playbin, bus, sink, duration: 0.0, size: (0, 0), state: Mutex::new(State { rate: 0.0, held: 0.0, sent: 1.0 }) };
            // Paused, it decodes the first frame, and knows its length.
            (gst.set_state)(playbin, STATE_PAUSED);
            let (mut now, mut pending) = (0, 0);
            if (gst.get_state)(playbin, &mut now, &mut pending, 10 * SECOND as u64) == STATE_CHANGE_FAILURE {
                return Err(player.error().unwrap_or_else(|| "It isn't a movie RandomCard can play".into()));
            }
            let mut ns = 0i64;
            if (gst.query_duration)(playbin, FORMAT_TIME, &mut ns) != 0 {
                player.duration = ns.max(0) as f64 / SECOND;
            }
            if let Some(f) = player.frame() {
                player.size = (f.width, f.height);
            }
            player.drain();
            Ok(player)
        }
    }

    /// The first error on the bus, if any.
    fn error(&self) -> Option<String> {
        unsafe {
            let msg = (self.gst.bus_pop_filtered)(self.bus, MESSAGE_ERROR);
            if msg.is_null() {
                return None;
            }
            let (mut err, mut debug) = (std::ptr::null_mut(), std::ptr::null_mut());
            (self.gst.parse_error)(msg, &mut err, &mut debug);
            (self.gst.free)(debug as Ptr);
            (self.gst.mini_object_unref)(msg);
            Some(self.gst.take_error(err))
        }
    }

    /// Let the bus's messages go: with no main loop to take them, they'd
    /// pile up for as long as the movie is open.
    fn drain(&self) {
        unsafe {
            loop {
                let msg = (self.gst.bus_pop)(self.bus);
                if msg.is_null() {
                    break;
                }
                (self.gst.mini_object_unref)(msg);
            }
        }
    }

    fn sink(&self) -> &Sink {
        unsafe { &*self.sink }
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
        let state = self.state();
        if state.rate == 0.0 {
            return state.held;
        }
        let pts = self.sink().pts.load(Ordering::SeqCst);
        if pts != NO_TIME {
            return (pts as f64 / SECOND).min(self.duration);
        }
        let mut ns = 0i64;
        if unsafe { (self.gst.query_position)(self.playbin, FORMAT_TIME, &mut ns) } != 0 {
            return (ns.max(0) as f64 / SECOND).min(self.duration);
        }
        state.held
    }

    /// A flushing seek at this rate from this time: forward to the end, or
    /// backward to the start.
    fn send(&self, rate: f64, secs: f64) {
        let at = (secs * SECOND) as i64;
        self.sink().eos.store(false, Ordering::SeqCst);
        // Where it's going, until a frame from there comes.
        self.sink().pts.store(at.max(0) as u64, Ordering::SeqCst);
        unsafe {
            if rate < 0.0 {
                (self.gst.seek)(self.playbin, rate, FORMAT_TIME, SEEK_FLUSH_ACCURATE, SEEK_TYPE_SET, 0, SEEK_TYPE_SET, at);
            } else {
                let rate = if rate == 0.0 { 1.0 } else { rate };
                (self.gst.seek)(self.playbin, rate, FORMAT_TIME, SEEK_FLUSH_ACCURATE, SEEK_TYPE_SET, at, SEEK_TYPE_NONE, -1);
            }
        }
    }

    pub fn seek(&self, secs: f64) {
        let now = self.time();
        let mut state = self.state();
        if (now - secs).abs() < 1e-6 && state.rate == 0.0 {
            return;
        }
        state.held = secs;
        let rate = if state.rate == 0.0 { state.sent } else { state.rate };
        state.sent = rate;
        self.send(rate, secs);
    }

    pub fn set_rate(&self, rate: f64) {
        let now = self.time();
        let mut state = self.state();
        if rate == state.rate {
            return;
        }
        state.rate = rate;
        state.held = now;
        unsafe {
            if rate == 0.0 {
                (self.gst.set_state)(self.playbin, STATE_PAUSED);
                return;
            }
            // A new speed or direction starts over from where it is; so
            // does playing again after the end.
            if rate != state.sent || self.sink().eos.load(Ordering::SeqCst) {
                state.sent = rate;
                self.send(rate, now);
            }
            (self.gst.set_state)(self.playbin, STATE_PLAYING);
        }
    }

    pub fn set_volume(&self, level: f64) {
        let end: *const c_char = std::ptr::null();
        unsafe { (self.gst.object_set)(self.playbin, c"volume".as_ptr(), level, end) };
    }

    pub fn frame(&self) -> Option<Arc<Frame>> {
        self.sink().frame.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn finished(&self) -> bool {
        self.drain();
        let done = self.sink().eos.load(Ordering::SeqCst);
        if done {
            let mut state = self.state();
            if state.rate != 0.0 {
                state.held = if state.rate < 0.0 { 0.0 } else { self.duration };
                state.rate = 0.0;
                unsafe { (self.gst.set_state)(self.playbin, STATE_PAUSED) };
            }
        }
        done
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        unsafe {
            // Once it's stopped, no callback can come, and the sink can go.
            (self.gst.set_state)(self.playbin, STATE_NULL);
            self.drain();
            (self.gst.object_unref)(self.bus);
            (self.gst.object_unref)(self.playbin);
            drop(Box::from_raw(self.sink as *mut Sink));
        }
    }
}
