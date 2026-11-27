//! Wayland drag source. A drag must be started by the client that owns the pressed surface, with
//! the serial of that press, and GPUI 0.2 keeps both private. So this joins GPUI's own
//! connection (the `wl_display` its window hands out) with a second event queue, the way a
//! clipboard helper shares a toolkit's connection: it is the same client, so a second `wl_seat`
//! binding sees the same button serials. Its events are read on a dedicated thread.

use std::fs::File;
use std::io::Write as _;
use std::os::fd::OwnedFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use anyhow::{anyhow, bail, ensure, Context as _, Result};
use gpui::Window;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use wayland_backend::client::{Backend, ObjectId};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::{DndAction, WlDataDeviceManager};
use wayland_client::protocol::wl_data_offer::WlDataOffer;
use wayland_client::protocol::wl_data_source::{self, WlDataSource};
use wayland_client::protocol::wl_pointer::{self, ButtonState, WlPointer};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::{self, Capability, WlSeat};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{
    delegate_noop, event_created_child, Connection, Dispatch, Proxy, QueueHandle, WEnum,
};

use super::icon::DragIcon;
use super::Prepare;

const URI_LIST: &str = "text/uri-list";
/// `wl_data_source.set_actions` and `wl_data_device.release` need version 3 and 2.
const DATA_DEVICE_VERSION: u32 = 3;

/// The dragged files, resolved (or downloaded) once, when a target first asks for them.
pub struct Payload {
    prepare: Mutex<Option<Prepare>>,
    uri_list: OnceLock<Result<String, String>>,
}

impl Payload {
    pub fn new(prepare: Prepare) -> Self {
        Self {
            prepare: Mutex::new(Some(prepare)),
            uri_list: OnceLock::new(),
        }
    }

    /// Blocks while the files are prepared; later callers get the same answer.
    fn uri_list(&self) -> &Result<String, String> {
        self.uri_list.get_or_init(|| {
            let prepare = self.prepare.lock().ok().and_then(|mut slot| slot.take());
            let paths: Result<Vec<PathBuf>> = match prepare {
                Some(prepare) => prepare(),
                None => Err(anyhow!("the dragged files were already taken")),
            };
            paths
                .map(|paths| super::uri_list(&paths))
                .map_err(|error| format!("{error:#}"))
        })
    }
}

/// One drag: its data, and the data device and picture that live only as long as the drag does.
struct Drag {
    device: WlDataDevice,
    payload: Arc<Payload>,
    icon: Option<Icon>,
}

/// The surface that follows the pointer, and the buffer it shows.
struct Icon {
    surface: WlSurface,
    buffer: WlBuffer,
}

impl Icon {
    fn destroy(&self) {
        self.surface.destroy();
        self.buffer.destroy();
    }
}

struct Globals {
    queue: QueueHandle<State>,
    seat: WlSeat,
    manager: WlDataDeviceManager,
    /// For the drag picture; without them the drag still works, unpictured.
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
}

struct Shared {
    conn: Connection,
    /// Set by the event thread once the registry has answered.
    globals: OnceLock<Globals>,
    /// Serial of the last button press, which the compositor checks against its pointer grab.
    press: AtomicU32,
}

pub struct Source(Arc<Shared>);

impl Source {
    /// Joins the connection behind `window` and starts the thread that reads this queue.
    pub fn connect(window: &Window) -> Result<Self> {
        let Ok(RawDisplayHandle::Wayland(display)) =
            window.display_handle().map(|handle| handle.as_raw())
        else {
            bail!("the window is not on Wayland");
        };
        // Safety: GPUI keeps this `wl_display` open until the app quits, and the event thread
        // only sleeps in `poll` after that.
        let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
        let shared = Arc::new(Shared {
            conn: Connection::from_backend(backend),
            globals: OnceLock::new(),
            press: AtomicU32::new(0),
        });
        let thread_shared = shared.clone();
        thread::Builder::new()
            .name("wayland-drag-source".into())
            .spawn(move || {
                if let Err(error) = run(thread_shared) {
                    log::warn!("dragging to other apps stopped: {error:#}");
                }
            })?;
        Ok(Self(shared))
    }

    /// Asks the compositor to take over the drag that began with the last press on `window`.
    /// Only queues requests; the data is served from the event thread.
    pub fn start(&self, window: &Window, payload: Payload, icon: Option<DragIcon>) -> Result<()> {
        let shared = &self.0;
        let globals = shared
            .globals
            .get()
            .context("the compositor has no drag-and-drop support")?;
        let serial = shared.press.load(Ordering::Relaxed);
        if serial == 0 {
            bail!("no pointer press was seen");
        }
        let Ok(RawWindowHandle::Wayland(handle)) =
            HasWindowHandle::window_handle(window).map(|handle| handle.as_raw())
        else {
            bail!("the window is not on Wayland");
        };
        // Safety: the pointer is the live `wl_surface` of a window that is borrowed here.
        let origin =
            unsafe { ObjectId::from_ptr(WlSurface::interface(), handle.surface.as_ptr().cast()) }
                .map_err(|_| anyhow!("the window has no wl_surface"))?;
        let origin = WlSurface::from_id(&shared.conn, origin)
            .map_err(|_| anyhow!("the window surface is gone"))?;

        let device = globals
            .manager
            .get_data_device(&globals.seat, &globals.queue, ());
        let icon = icon.and_then(|icon| {
            picture(globals, &icon).map_err(|error| log::warn!("cannot show the drag picture: {error:#}")).ok()
        });
        let drag = Drag {
            device: device.clone(),
            payload: Arc::new(payload),
            icon: icon.as_ref().map(|(icon, _)| Icon { surface: icon.surface.clone(), buffer: icon.buffer.clone() }),
        };
        let source = globals.manager.create_data_source(&globals.queue, drag);
        source.offer(URI_LIST.to_string());
        source.set_actions(DndAction::Copy);
        device.start_drag(Some(&source), &origin, icon.as_ref().map(|(icon, _)| &icon.surface), serial);
        // The surface takes its role from start_drag; its first buffer comes after.
        if let Some((icon, scale)) = &icon {
            if *scale > 1 {
                icon.surface.set_buffer_scale(*scale as i32);
            }
            icon.surface.attach(Some(&icon.buffer), 0, 0);
            icon.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
            icon.surface.commit();
        }
        shared.conn.flush().context("cannot reach the compositor")
    }
}

/// A surface and a shared-memory buffer holding `icon`, and the buffer scale to set.
fn picture(globals: &Globals, icon: &DragIcon) -> Result<(Icon, u32)> {
    let (Some(compositor), Some(shm)) = (&globals.compositor, &globals.shm) else {
        bail!("the compositor offers no shared-memory buffers");
    };
    let stride = icon.width * 4;
    let length = (stride * icon.height) as usize;
    ensure!(icon.bgra.len() == length, "the picture has {} bytes, not {length}", icon.bgra.len());
    // Safety: a fresh anonymous file, owned from here on.
    let fd = unsafe { libc::memfd_create(c"boxbox-drag-icon".as_ptr(), libc::MFD_CLOEXEC) };
    ensure!(fd >= 0, "cannot make the picture buffer: {}", std::io::Error::last_os_error());
    let fd = unsafe { <OwnedFd as std::os::fd::FromRawFd>::from_raw_fd(fd) };
    let mut file = File::from(fd);
    file.write_all(&icon.bgra)?;
    let pool = shm.create_pool(std::os::fd::AsFd::as_fd(&file), length as i32, &globals.queue, ());
    let buffer = pool.create_buffer(0, icon.width as i32, icon.height as i32, stride as i32, wl_shm::Format::Argb8888, &globals.queue, ());
    // The buffer keeps the memory; the pool is not needed again.
    pool.destroy();
    let surface = compositor.create_surface(&globals.queue, ());
    let scale = if compositor.version() >= 3 { icon.scale.max(1) } else { 1 };
    Ok((Icon { surface, buffer }, scale))
}

fn run(shared: Arc<Shared>) -> Result<()> {
    let (list, mut queue) = registry_queue_init::<State>(&shared.conn)?;
    let handle = queue.handle();
    // Version 1 is enough to hear button presses.
    let seat: WlSeat = list.bind(&handle, 1..=1, ())?;
    let manager: WlDataDeviceManager =
        list.bind(&handle, DATA_DEVICE_VERSION..=DATA_DEVICE_VERSION, ())?;
    let compositor: Option<WlCompositor> = list.bind(&handle, 1..=4, ()).ok();
    let shm: Option<WlShm> = list.bind(&handle, 1..=1, ()).ok();
    let globals = Globals {
        queue: handle,
        seat,
        manager,
        compositor,
        shm,
    };
    if shared.globals.set(globals).is_err() {
        bail!("the drag source was started twice");
    }
    let mut state = State {
        shared,
        pointer: None,
        offers: Vec::new(),
    };
    loop {
        queue.blocking_dispatch(&mut state)?;
    }
}

struct State {
    shared: Arc<Shared>,
    pointer: Option<WlPointer>,
    /// Offers the compositor mirrors to our data device during a drag. They are GPUI's to
    /// answer, so these copies are only held and destroyed once the drag is over.
    offers: Vec<WlDataOffer>,
}

impl State {
    fn finish(&mut self, source: &WlDataSource, drag: &Drag) {
        source.destroy();
        if let Some(icon) = &drag.icon {
            icon.destroy();
        }
        drag.device.release();
        for offer in self.offers.drain(..) {
            offer.destroy();
        }
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        else {
            return;
        };
        // When the last mouse is unplugged the compositor silences the old pointer
        // object, so one plugged in later needs a new one.
        if !capabilities.contains(Capability::Pointer) {
            state.pointer = None;
        } else if state.pointer.is_none() {
            state.pointer = Some(seat.get_pointer(handle, ()));
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_pointer::Event::Button {
            serial,
            state: WEnum::Value(ButtonState::Pressed),
            ..
        } = event
        {
            state.shared.press.store(serial, Ordering::Relaxed);
        }
    }
}

impl Dispatch<WlDataDevice, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_device::Event::DataOffer { id } = event {
            state.offers.push(id);
        }
    }

    event_created_child!(State, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ()),
    ]);
}

impl Dispatch<WlDataSource, Drag> for State {
    fn event(
        state: &mut Self,
        source: &WlDataSource,
        event: wl_data_source::Event,
        drag: &Drag,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_source::Event::Send { mime_type, fd } if mime_type == URI_LIST => {
                serve(drag.payload.clone(), fd);
            }
            wl_data_source::Event::Cancelled | wl_data_source::Event::DndFinished => {
                state.finish(source, drag);
            }
            _ => {}
        }
    }
}

delegate_noop!(State: WlDataDeviceManager);
delegate_noop!(State: WlCompositor);
delegate_noop!(State: WlShmPool);
delegate_noop!(State: ignore WlShm);
delegate_noop!(State: ignore WlBuffer);
delegate_noop!(State: ignore WlSurface);
delegate_noop!(State: ignore WlDataOffer);

/// Writes on a thread of its own: preparing can download, and a target may read slowly.
fn serve(payload: Arc<Payload>, fd: OwnedFd) {
    thread::spawn(move || match payload.uri_list() {
        Ok(list) => {
            if let Err(error) = File::from(fd).write_all(list.as_bytes()) {
                log::warn!("cannot send the dragged files: {error}");
            }
        }
        Err(error) => log::warn!("cannot prepare dragged files: {error}"),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_prepares_once_and_keeps_the_answer() {
        let payload = Payload::new(Box::new(|| Ok(vec![PathBuf::from("/tmp/a b.txt")])));
        let expected = Ok("file:///tmp/a%20b.txt\r\n".to_string());
        assert_eq!(payload.uri_list(), &expected);
        assert_eq!(payload.uri_list(), &expected);
    }

    #[test]
    fn payload_reports_a_failed_preparation() {
        let payload = Payload::new(Box::new(|| bail!("offline")));
        assert_eq!(payload.uri_list(), &Err("offline".to_string()));
    }
}
