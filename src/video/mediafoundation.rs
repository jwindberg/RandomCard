//! Movies on Windows, with Media Foundation's Media Engine: the player
//! behind the HTML video element, which finds the decoders (hardware ones
//! when there's a GPU), keeps the picture in time and plays the sound. It
//! runs in "frame server" mode: rather than drawing into a window, it hands
//! over each new picture, which is copied out as RGBA.
//!
//! The pictures come through a Direct3D 11 texture when the machine has a
//! GPU that decodes video, and otherwise (a virtual machine, say) through a
//! WIC bitmap, decoded in software.
//!
//! The Media Engine can't play most movies backward, so a negative rate
//! steps back through the movie by seeking, silently, keeping its own clock.

use std::cell::Cell;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HMODULE, RECT, S_OK};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Multithread, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Imaging::{CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICBitmap, IWICImagingFactory, WICBitmapCacheOnLoad, WICBitmapLockRead, WICRect};
use windows::Win32::Media::MediaFoundation::{
    CLSID_MFMediaEngineClassFactory, IMFAttributes, IMFDXGIDeviceManager, IMFMediaEngine, IMFMediaEngineClassFactory, IMFMediaEngineEx, IMFMediaEngineNotify, IMFMediaEngineNotify_Impl,
    MF_ACCESSMODE_READ, MF_FILEFLAGS_NONE, MF_MEDIA_ENGINE_CALLBACK, MF_MEDIA_ENGINE_DXGI_MANAGER, MF_MEDIA_ENGINE_PRELOAD_AUTOMATIC, MF_MEDIA_ENGINE_READY_HAVE_CURRENT_DATA,
    MF_MEDIA_ENGINE_SEEK_MODE_NORMAL, MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT, MF_OPENMODE_FAIL_IF_NOT_EXIST, MF_VERSION, MFARGB, MFCreateAttributes, MFCreateDXGIDeviceManager, MFCreateFile,
    MFSTARTUP_FULL, MFStartup,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx};
use windows_core::{BSTR, HSTRING, Interface, implement};

use super::Frame;

/// How long opening a movie may take before giving up.
const OPEN_WAIT: Duration = Duration::from_secs(10);

/// Where the engine's pictures are copied to, to be read back.
enum Target {
    /// A GPU texture the engine draws into, and a CPU-readable copy of it.
    D3d { context: ID3D11DeviceContext, texture: ID3D11Texture2D, staging: ID3D11Texture2D },
    /// A bitmap in memory, for software decoding.
    Wic(IWICBitmap),
}

/// Playing backward: the time it started from, and when.
struct Reverse {
    from: f64,
    since: Instant,
    rate: f64,
}

impl Reverse {
    fn position(&self) -> f64 {
        (self.from + self.since.elapsed().as_secs_f64() * self.rate).max(0.0)
    }
}

struct State {
    engine: IMFMediaEngine,
    ex: IMFMediaEngineEx,
    target: Option<Target>,
    /// The rate scripts asked for.
    rate: f64,
    /// Where it is while paused, exactly as paused or sought.
    held: f64,
    reverse: Option<Reverse>,
    ended: bool,
    frame: Option<Arc<Frame>>,
    serial: u64,
}

/// An open movie.
pub struct Player {
    size: (u32, u32),
    duration: f64,
    state: Mutex<State>,
}

// SAFETY: Media Foundation's objects are free-threaded, the D3D device is
// created with multithread protection, and everything else (the device
// context, the WIC bitmap, our own state) is only touched while holding the
// mutex. So the player can be used from the script and UI threads alike.
unsafe impl Send for Player {}
unsafe impl Sync for Player {}

/// The engine's event callback, which it requires. The player asks the
/// engine its state rather than waiting on events, so this ignores them.
#[implement(IMFMediaEngineNotify)]
struct Notify;

impl IMFMediaEngineNotify_Impl for Notify_Impl {
    fn EventNotify(&self, _event: u32, _param1: usize, _param2: u32) -> windows_core::Result<()> {
        Ok(())
    }
}

thread_local! {
    static COM: Cell<bool> = const { Cell::new(false) };
}

/// Make sure COM is running on this thread. A thread that already has it
/// (the UI thread, set up by the window system for drag and drop) keeps its
/// own kind, which is fine: Media Foundation's objects work from either.
fn ensure_com() {
    COM.with(|done| {
        if !done.get() {
            // SAFETY: Initializing COM for the calling thread; a thread that
            // already has COM in another mode just gets an error code back.
            let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            done.set(true);
        }
    });
}

/// Start Media Foundation, once per run.
fn startup() -> Result<(), String> {
    static STARTED: OnceLock<Result<(), String>> = OnceLock::new();
    STARTED
        .get_or_init(|| {
            // SAFETY: Called once, after COM is running on this thread.
            unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.map_err(|e| format!("Media Foundation didn't start: {}", e.message()))
        })
        .clone()
}

/// A Direct3D 11 device that can decode video, and a device manager to
/// share it with the engine. None when there's no such GPU.
fn d3d_device() -> Option<(ID3D11Device, ID3D11DeviceContext, IMFDXGIDeviceManager)> {
    let mut device = None;
    let mut context = None;
    // SAFETY: Out pointers to local Options, filled on success.
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    }
    .ok()?;
    let (device, context) = (device?, context?);
    // The engine decodes on its own threads with the same device.
    // SAFETY: A plain interface query and setter on a live device.
    // It returns the old setting, which doesn't matter.
    let _ = unsafe { device.cast::<ID3D11Multithread>().ok()?.SetMultithreadProtected(true) };
    let mut token = 0;
    let mut manager = None;
    // SAFETY: Out pointers to locals, then handing the manager our device.
    unsafe {
        MFCreateDXGIDeviceManager(&mut token, &mut manager).ok()?;
        manager.as_ref()?.ResetDevice(&device, token).ok()?;
    }
    Some((device, context, manager?))
}

/// The textures the engine's pictures go through on the GPU.
fn d3d_target(device: &ID3D11Device, context: ID3D11DeviceContext, (w, h): (u32, u32)) -> Option<Target> {
    let mut desc = D3D11_TEXTURE2D_DESC {
        Width: w,
        Height: h,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: 0,
    };
    let mut texture = None;
    // SAFETY: The description is valid and the out pointer is a local.
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }.ok()?;
    desc.Usage = D3D11_USAGE_STAGING;
    desc.BindFlags = 0;
    desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
    let mut staging = None;
    // SAFETY: As above.
    unsafe { device.CreateTexture2D(&desc, None, Some(&mut staging)) }.ok()?;
    Some(Target::D3d { context, texture: texture?, staging: staging? })
}

/// A bitmap in memory the engine's pictures are copied to.
fn wic_target((w, h): (u32, u32)) -> Option<Target> {
    // SAFETY: Creating WIC's factory and a bitmap from it; COM is running.
    unsafe {
        let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        factory.CreateBitmap(w, h, &GUID_WICPixelFormat32bppBGRA, WICBitmapCacheOnLoad).ok().map(Target::Wic)
    }
}

/// Copy BGRA rows with this stride into tightly packed, opaque RGBA.
fn to_rgba(src: &[u8], stride: usize, (w, h): (u32, u32)) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    let mut rgba = Vec::with_capacity(w * h * 4);
    for row in src.chunks(stride).take(h) {
        for px in row[..w * 4].as_chunks::<4>().0 {
            rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
        }
    }
    rgba
}

/// What a Media Engine error code means, for `the result`.
fn error_text(engine: &IMFMediaEngine) -> Option<String> {
    // SAFETY: A query on a live engine; it fails when there's no error.
    let err = unsafe { engine.GetError() }.ok()?;
    // SAFETY: A query on the error object just returned.
    Some(match unsafe { err.GetErrorCode() } {
        2 => "The movie couldn't be read".into(),
        3 => "The movie's pictures or sound couldn't be decoded".into(),
        _ => "It isn't a movie RandomCard can play".into(),
    })
}

impl State {
    /// Copy the engine's picture out, if there's a new one (or `force`).
    fn grab(&mut self, size: (u32, u32), force: bool) -> bool {
        let Some(target) = &self.target else { return false };
        let mut pts = 0i64;
        // Called through the vtable to tell S_OK (a new picture) from
        // S_FALSE (no new one), which the wrapper treats alike.
        // SAFETY: A call on a live engine with a pointer to a local.
        let hr = unsafe { (Interface::vtable(&self.engine).OnVideoStreamTick)(Interface::as_raw(&self.engine), &mut pts) };
        if hr != S_OK && !force {
            return false;
        }
        let rect = RECT { left: 0, top: 0, right: size.0 as i32, bottom: size.1 as i32 };
        let border = MFARGB { rgbBlue: 0, rgbGreen: 0, rgbRed: 0, rgbAlpha: 255 };
        let rgba = match target {
            Target::D3d { context, texture, staging } => {
                // SAFETY: The engine draws into our texture, which is copied
                // to the staging one and read while mapped; the slice covers
                // exactly the mapped rows.
                unsafe {
                    if self.engine.TransferVideoFrame(texture, None, &rect, Some(&border)).is_err() {
                        return false;
                    }
                    context.CopyResource(staging, texture);
                    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
                    if context.Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).is_err() {
                        return false;
                    }
                    let stride = mapped.RowPitch as usize;
                    let data = std::slice::from_raw_parts(mapped.pData as *const u8, stride * size.1 as usize);
                    let rgba = to_rgba(data, stride, size);
                    context.Unmap(staging, 0);
                    rgba
                }
            }
            Target::Wic(bitmap) => {
                // SAFETY: The engine draws into our bitmap, which is read
                // while locked; the slice is the buffer the lock gives.
                unsafe {
                    if self.engine.TransferVideoFrame(bitmap, None, &rect, Some(&border)).is_err() {
                        return false;
                    }
                    let whole = WICRect { X: 0, Y: 0, Width: size.0 as i32, Height: size.1 as i32 };
                    let Ok(lock) = bitmap.Lock(&whole, WICBitmapLockRead.0 as u32) else { return false };
                    let Ok(stride) = lock.GetStride() else { return false };
                    let (mut len, mut ptr) = (0u32, std::ptr::null_mut());
                    if lock.GetDataPointer(&mut len, &mut ptr).is_err() || ptr.is_null() {
                        return false;
                    }
                    to_rgba(std::slice::from_raw_parts(ptr, len as usize), stride as usize, size)
                }
            }
        };
        self.serial += 1;
        self.frame = Some(Arc::new(Frame { width: size.0, height: size.1, rgba, serial: self.serial }));
        true
    }

    fn seek_engine(&self, secs: f64) {
        // SAFETY: A call on a live engine. Normal mode is frame-accurate.
        let _ = unsafe { self.ex.SetCurrentTimeEx(secs, MF_MEDIA_ENGINE_SEEK_MODE_NORMAL) };
    }

    fn pause_engine(&self) {
        // SAFETY: A call on a live engine.
        let _ = unsafe { self.engine.Pause() };
    }

    fn time(&self) -> f64 {
        if let Some(r) = &self.reverse {
            r.position()
        } else if self.rate == 0.0 || self.ended {
            self.held
        } else {
            // SAFETY: A query on a live engine.
            unsafe { self.engine.GetCurrentTime() }
        }
    }
}

impl Player {
    pub fn open(path: &Path) -> Result<Player, String> {
        ensure_com();
        startup()?;
        let fail = |e: windows_core::Error| format!("The movie couldn't be opened: {}", e.message());
        let gpu = d3d_device();
        // SAFETY: Creating the engine, with attributes for frame-server mode
        // in BGRA, our callback and (with a GPU) the device to decode on, then
        // giving it the file.
        let engine = unsafe {
            let factory: IMFMediaEngineClassFactory = CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_INPROC_SERVER).map_err(fail)?;
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 3).map_err(fail)?;
            let attrs = attrs.ok_or("Media Foundation gave no attributes")?;
            let notify: IMFMediaEngineNotify = Notify.into();
            attrs.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &notify).map_err(fail)?;
            attrs.SetUINT32(&MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM.0 as u32).map_err(fail)?;
            if let Some((_, _, manager)) = &gpu {
                attrs.SetUnknown(&MF_MEDIA_ENGINE_DXGI_MANAGER, manager).map_err(fail)?;
            }
            factory.CreateInstance(0, &attrs).map_err(fail)?
        };
        let ex: IMFMediaEngineEx = engine.cast().map_err(fail)?;
        let state = State { engine, ex, target: None, rate: 0.0, held: 0.0, reverse: None, ended: false, frame: None, serial: 0 };
        // From here on, dropping the state on failure shuts the engine down.
        let mut player = Player { size: (0, 0), duration: 0.0, state: Mutex::new(state) };
        {
            let st = player.state.get_mut().unwrap();
            // A byte stream rather than a URL, so a file with no extension
            // (a classic Mac movie) still opens; the name is a hint to its
            // type.
            // SAFETY: Opening the file for reading and handing it over.
            unsafe {
                let stream = MFCreateFile(MF_ACCESSMODE_READ, MF_OPENMODE_FAIL_IF_NOT_EXIST, MF_FILEFLAGS_NONE, &HSTRING::from(path.as_os_str())).map_err(|_| "The movie file couldn't be read".to_string())?;
                let _ = st.engine.SetPreload(MF_MEDIA_ENGINE_PRELOAD_AUTOMATIC);
                let _ = st.engine.SetAutoPlay(false);
                st.ex.SetSourceFromByteStream(&stream, &BSTR::from(path.to_string_lossy().as_ref())).map_err(fail)?;
            }
            let start = Instant::now();
            loop {
                if let Some(e) = error_text(&st.engine) {
                    return Err(e);
                }
                // SAFETY: A query on a live engine.
                if unsafe { st.engine.GetReadyState() } >= MF_MEDIA_ENGINE_READY_HAVE_CURRENT_DATA.0 as u16 {
                    break;
                }
                if start.elapsed() > OPEN_WAIT {
                    return Err("The movie took too long to open".into());
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            // SAFETY: Queries on a live, loaded engine, with out pointers to
            // locals.
            unsafe {
                let d = st.engine.GetDuration();
                player.duration = if d.is_finite() { d.max(0.0) } else { 0.0 };
                if st.engine.HasVideo().as_bool() {
                    let (mut w, mut h) = (0u32, 0u32);
                    if st.engine.GetNativeVideoSize(Some(&mut w), Some(&mut h)).is_ok() {
                        player.size = (w, h);
                    }
                }
            }
            if player.size.0 > 0 && player.size.1 > 0 {
                st.target = gpu.and_then(|(device, context, _)| d3d_target(&device, context, player.size)).or_else(|| wic_target(player.size));
                // The first picture may come a moment after the data does.
                let start = Instant::now();
                while !st.grab(player.size, true) && start.elapsed() < Duration::from_secs(2) {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
        Ok(player)
    }

    fn state(&self) -> MutexGuard<'_, State> {
        ensure_com();
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    pub fn time(&self) -> f64 {
        self.state().time().min(self.duration)
    }

    pub fn seek(&self, secs: f64) {
        let mut st = self.state();
        st.ended = false;
        st.held = secs;
        if let Some(r) = &mut st.reverse {
            r.from = secs;
            r.since = Instant::now();
        }
        st.seek_engine(secs);
    }

    pub fn set_rate(&self, rate: f64) {
        let mut st = self.state();
        let now = st.time().min(self.duration);
        st.held = now;
        st.rate = rate;
        st.ended = false;
        st.reverse = None;
        if rate == 0.0 {
            st.pause_engine();
            st.seek_engine(now);
        } else if rate > 0.0 {
            // SAFETY: Calls on a live engine. Play is what takes the rate
            // from the default, so both are set.
            unsafe {
                let _ = st.engine.SetDefaultPlaybackRate(rate);
                let _ = st.engine.SetPlaybackRate(rate);
                let _ = st.engine.Play();
            }
        } else {
            st.pause_engine();
            st.reverse = Some(Reverse { from: now, since: Instant::now(), rate });
        }
    }

    pub fn set_volume(&self, level: f64) {
        // SAFETY: A call on a live engine.
        let _ = unsafe { self.state().engine.SetVolume(level.min(1.0)) };
    }

    pub fn frame(&self) -> Option<Arc<Frame>> {
        let mut st = self.state();
        // Playing backward, step to where its clock is, once the last step
        // has landed.
        if let Some(r) = &st.reverse {
            let at = r.position();
            // SAFETY: A query on a live engine.
            if !unsafe { st.engine.IsSeeking() }.as_bool() {
                st.seek_engine(at);
            }
        }
        if self.size.0 > 0 {
            st.grab(self.size, false);
        }
        st.frame.clone()
    }

    pub fn finished(&self) -> bool {
        let mut st = self.state();
        if st.ended {
            return true;
        }
        let done = if let Some(r) = &st.reverse {
            r.position() <= 0.0
        } else {
            // SAFETY: A query on a live engine.
            st.rate > 0.0 && unsafe { st.engine.IsEnded() }.as_bool()
        };
        if done {
            let at = if st.reverse.is_some() { 0.0 } else { self.duration };
            st.reverse = None;
            st.rate = 0.0;
            st.held = at;
            st.ended = true;
            st.pause_engine();
            st.seek_engine(at);
        }
        done
    }
}

impl Drop for State {
    fn drop(&mut self) {
        // Stops the sound and lets go of the file and decoders now, rather
        // than when the last reference to the engine goes.
        // SAFETY: Shutting down a live engine, which is used no more.
        unsafe {
            let _ = self.engine.Pause();
            let _ = self.engine.Shutdown();
        }
    }
}
