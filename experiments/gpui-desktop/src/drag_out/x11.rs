//! XDND v5 source on a second X connection. GPUI's connection keeps the implicit pointer grab
//! from the button press, so the pointer is polled instead of grabbed. GPUI 0.2's X11 window
//! panics on `window_handle()`, so this app's windows are recognised by `_NET_WM_PID`.

use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context as _, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask, KeyButMask,
    PropMode, SelectionNotifyEvent, SelectionRequestEvent, Window, WindowClass,
    SELECTION_NOTIFY_EVENT,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, CURRENT_TIME, NONE};

const XDND_VERSION: u32 = 5;
const POLL: Duration = Duration::from_millis(16);
/// Status replies to the last position may still be in flight when the button is released.
const STATUS_GRACE: Duration = Duration::from_millis(150);
/// Covers downloading the files after the drop.
const FINISH_TIMEOUT: Duration = Duration::from_secs(300);

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        XdndAware,
        XdndProxy,
        XdndEnter,
        XdndPosition,
        XdndStatus,
        XdndLeave,
        XdndDrop,
        XdndFinished,
        XdndSelection,
        XdndActionCopy,
        TARGETS,
        _NET_WM_PID,
        URI_LIST: b"text/uri-list",
    }
}

pub type Prepare = Box<dyn FnOnce() -> Result<Vec<PathBuf>> + Send>;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Target {
    window: Window,
    /// Where messages go: the window itself or its `XdndProxy`.
    proxy: Window,
    version: u32,
}

/// The dragged files are downloaded only once a foreign drop target is hovered.
enum Data {
    Pending(Prepare),
    Staging(JoinHandle<Result<Vec<PathBuf>>>),
    Ready(Result<String, String>),
}

impl Data {
    fn start(&mut self) {
        if matches!(self, Data::Pending(_)) {
            let Data::Pending(prepare) = std::mem::replace(self, Data::Ready(Ok(String::new())))
            else {
                unreachable!()
            };
            *self = Data::Staging(thread::spawn(prepare));
        }
    }

    fn wait(&mut self) -> Result<String, String> {
        self.start();
        if matches!(self, Data::Staging(_)) {
            let Data::Staging(handle) = std::mem::replace(self, Data::Ready(Ok(String::new())))
            else {
                unreachable!()
            };
            let staged = handle
                .join()
                .map_err(|_| anyhow!("download thread panicked"))
                .and_then(|result| result);
            *self = Data::Ready(
                staged
                    .map(|paths| super::uri_list(&paths))
                    .map_err(|error| format!("{error:#}")),
            );
        }
        match self {
            Data::Ready(result) => result.clone(),
            _ => unreachable!(),
        }
    }
}

enum Reply {
    Status(Window, bool),
    Finished(Window),
    Other,
}

struct Session {
    conn: RustConnection,
    atoms: Atoms,
    root: Window,
    source: Window,
    pid: u32,
    data: Data,
}

/// Runs until the button is released and any accepted drop has finished.
pub fn drag(prepare: Prepare) -> Result<()> {
    let (conn, screen) = RustConnection::connect(None).context("cannot connect to X")?;
    let root = conn.setup().roots[screen].root;
    let atoms = Atoms::new(&conn)?.reply()?;
    let source = conn.generate_id()?;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        source,
        root,
        -1,
        -1,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        COPY_FROM_PARENT,
        &CreateWindowAux::new(),
    )?;
    conn.set_selection_owner(source, atoms.XdndSelection, CURRENT_TIME)?;
    conn.flush()?;
    let mut session = Session {
        conn,
        atoms,
        root,
        source,
        pid: std::process::id(),
        data: Data::Pending(prepare),
    };
    let result = session.run();
    session.conn.destroy_window(source).ok();
    session.conn.flush().ok();
    result
}

impl Session {
    fn run(&mut self) -> Result<()> {
        let mut current: Option<Target> = None;
        let mut accepted = false;
        let mut last = None;
        loop {
            while let Some(event) = self.conn.poll_for_event()? {
                if let Reply::Status(window, ok) = self.handle(event)? {
                    accepted = current.is_some_and(|target| target.window == window) && ok;
                }
            }
            let pointer = self.conn.query_pointer(self.root)?.reply()?;
            if u16::from(pointer.mask) & u16::from(KeyButMask::BUTTON1) == 0 {
                break;
            }
            let found = self.target_at(pointer.root_x, pointer.root_y)?;
            if found != current {
                log::debug!("drop target {:?}", found.map(|target| target.window));
                if let Some(old) = current {
                    self.send(old, self.atoms.XdndLeave, [self.source, 0, 0, 0, 0])?;
                }
                if let Some(new) = found {
                    self.data.start();
                    let enter = [self.source, new.version << 24, self.atoms.URI_LIST, 0, 0];
                    self.send(new, self.atoms.XdndEnter, enter)?;
                }
                (current, accepted, last) = (found, false, None);
            }
            let position = (pointer.root_x, pointer.root_y);
            if let Some(target) = current.filter(|_| last != Some(position)) {
                let packed = (u32::from(position.0 as u16) << 16) | u32::from(position.1 as u16);
                let data = [
                    self.source,
                    0,
                    packed,
                    CURRENT_TIME,
                    self.atoms.XdndActionCopy,
                ];
                self.send(target, self.atoms.XdndPosition, data)?;
                last = Some(position);
            }
            self.conn.flush()?;
            thread::sleep(POLL);
        }

        let Some(target) = current else {
            return Ok(());
        };
        let grace = Instant::now() + STATUS_GRACE;
        while Instant::now() < grace {
            match self.conn.poll_for_event()? {
                Some(event) => {
                    if let Reply::Status(window, ok) = self.handle(event)? {
                        accepted = window == target.window && ok;
                    }
                }
                None => thread::sleep(Duration::from_millis(5)),
            }
        }
        if !accepted {
            self.send(target, self.atoms.XdndLeave, [self.source, 0, 0, 0, 0])?;
            self.conn.flush()?;
            return Ok(());
        }
        self.send(
            target,
            self.atoms.XdndDrop,
            [self.source, 0, CURRENT_TIME, 0, 0],
        )?;
        self.conn.flush()?;
        let deadline = Instant::now() + FINISH_TIMEOUT;
        while Instant::now() < deadline {
            match self.conn.poll_for_event()? {
                Some(event) => {
                    if matches!(self.handle(event)?, Reply::Finished(window) if window == target.window)
                    {
                        return Ok(());
                    }
                }
                None => thread::sleep(POLL),
            }
        }
        bail!("the drop target did not finish")
    }

    fn handle(&mut self, event: Event) -> Result<Reply> {
        Ok(match event {
            Event::ClientMessage(message) => {
                let data = message.data.as_data32();
                if message.type_ == self.atoms.XdndStatus {
                    Reply::Status(data[0], data[1] & 1 == 1)
                } else if message.type_ == self.atoms.XdndFinished {
                    Reply::Finished(data[0])
                } else {
                    Reply::Other
                }
            }
            Event::SelectionRequest(request) => {
                self.answer(request)?;
                Reply::Other
            }
            _ => Reply::Other,
        })
    }

    fn answer(&mut self, request: SelectionRequestEvent) -> Result<()> {
        let atoms = self.atoms;
        // Obsolete requestors leave the property unset and expect the target name.
        let property = if request.property == NONE {
            request.target
        } else {
            request.property
        };
        let served = if request.selection != atoms.XdndSelection {
            false
        } else if request.target == atoms.TARGETS {
            let targets = [atoms.TARGETS, atoms.URI_LIST];
            self.conn.change_property32(
                PropMode::REPLACE,
                request.requestor,
                property,
                AtomEnum::ATOM,
                &targets,
            )?;
            true
        } else if request.target == atoms.URI_LIST {
            match self.data.wait() {
                Ok(list) => {
                    self.conn.change_property8(
                        PropMode::REPLACE,
                        request.requestor,
                        property,
                        atoms.URI_LIST,
                        list.as_bytes(),
                    )?;
                    true
                }
                Err(error) => {
                    log::warn!("cannot prepare dragged files: {error}");
                    false
                }
            }
        } else {
            false
        };
        let notify = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: request.time,
            requestor: request.requestor,
            selection: request.selection,
            target: request.target,
            property: if served { property } else { NONE },
        };
        self.conn
            .send_event(false, request.requestor, EventMask::NO_EVENT, notify)?;
        self.conn.flush()?;
        Ok(())
    }

    /// The deepest XDND-aware window under the pointer, unless it belongs to this app.
    fn target_at(&self, x: i16, y: i16) -> Result<Option<Target>> {
        let mut window = self.root;
        loop {
            let child = self
                .conn
                .translate_coordinates(self.root, window, x, y)?
                .reply()?
                .child;
            if child == NONE {
                return Ok(None);
            }
            window = child;
            if self.property(window, self.atoms._NET_WM_PID, AtomEnum::CARDINAL.into())
                == Some(self.pid)
            {
                return Ok(None);
            }
            let proxy = self
                .property(window, self.atoms.XdndProxy, AtomEnum::WINDOW.into())
                .unwrap_or(window);
            if let Some(version) = self.property(proxy, self.atoms.XdndAware, AtomEnum::ATOM.into())
            {
                return Ok((version >= 3).then_some(Target {
                    window,
                    proxy,
                    version: version.min(XDND_VERSION),
                }));
            }
        }
    }

    /// First 32-bit value of a property; windows can vanish mid-drag, so errors read as unset.
    fn property(&self, window: Window, property: Atom, kind: Atom) -> Option<u32> {
        let reply = self
            .conn
            .get_property(false, window, property, kind, 0, 1)
            .ok()?
            .reply()
            .ok()?;
        let value = reply.value32()?.next();
        value
    }

    fn send(&self, target: Target, kind: Atom, data: [u32; 5]) -> Result<()> {
        let message = ClientMessageEvent::new(32, target.window, kind, data);
        self.conn
            .send_event(false, target.proxy, EventMask::NO_EVENT, message)?;
        Ok(())
    }
}
