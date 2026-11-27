//! `org.freedesktop.impl.portal.FileChooser` on the session bus: the backend
//! xdg-desktop-portal calls when the user routes Open and Save dialogs to
//! BoxBox. Only the portal frontend may call it; it grants sandboxed apps
//! access to what is chosen.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{bail, Context as _, Result};
use zbus::message::Header;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::Connection;

use super::request::{Kind, Request, Response};

pub const BUS_NAME: &str = "org.freedesktop.impl.portal.desktop.boxbox";
const OBJECT: &str = "/org/freedesktop/portal/desktop";
const FRONTEND: &str = "org.freedesktop.portal.Desktop";
/// More open dialogs than this is an app misbehaving.
const MOST_OPEN: u64 = 16;

/// A dialog to show, or one the app no longer wants.
pub enum Event {
    Open { id: u64, request: Box<Request>, reply: async_channel::Sender<Response> },
    Close(u64),
}

struct FileChooser {
    events: async_channel::Sender<Event>,
    next: AtomicU64,
    open: std::sync::Arc<AtomicU64>,
}

/// Whether the call comes from the portal frontend, not some other client.
async fn from_frontend(connection: &Connection, header: &Header<'_>) -> bool {
    let Some(sender) = header.sender() else { return false };
    let Ok(proxy) = zbus::fdo::DBusProxy::new(connection).await else { return false };
    let Ok(name) = zbus::names::BusName::try_from(FRONTEND) else { return false };
    proxy.get_name_owner(name).await.is_ok_and(|owner| owner.as_str() == sender.as_str())
}

impl FileChooser {
    #[allow(clippy::too_many_arguments)]
    async fn run(&self, connection: &Connection, header: &Header<'_>, kind: Kind, handle: OwnedObjectPath, app_id: String, title: String, options: HashMap<String, OwnedValue>) -> (u32, HashMap<String, OwnedValue>) {
        let failed = (2, HashMap::new());
        if !from_frontend(connection, header).await {
            log::warn!("File chooser call from {:?}, not the portal frontend", header.sender());
            return failed;
        }
        if !handle.as_str().starts_with("/org/freedesktop/portal/desktop/request/") {
            return failed;
        }
        let request = match Request::parse(kind, &app_id, &title, &options) {
            Ok(request) => request,
            Err(error) => {
                log::warn!("Refused a file chooser request from {app_id}: {error:#}");
                return failed;
            }
        };
        if self.open.fetch_add(1, Ordering::SeqCst) >= MOST_OPEN {
            self.open.fetch_sub(1, Ordering::SeqCst);
            return failed;
        }
        let _open = Decrement(self.open.clone());
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (reply, answer) = async_channel::bounded(1);
        let closer = RequestObject { id, events: self.events.clone(), reply: reply.clone() };
        if connection.object_server().at(&handle, closer).await.is_err() {
            return failed;
        }
        let response = match self.events.send(Event::Open { id, request: Box::new(request), reply }).await {
            Ok(()) => answer.recv().await.unwrap_or_else(|_| Response::cancelled()),
            Err(_) => Response::cancelled(),
        };
        let _ = connection.object_server().remove::<RequestObject, _>(&handle).await;
        (response.code, response.results())
    }
}

struct Decrement(std::sync::Arc<AtomicU64>);
impl Drop for Decrement {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

#[zbus::interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    #[allow(clippy::too_many_arguments)]
    async fn open_file(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        handle: OwnedObjectPath,
        app_id: String,
        _parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        self.run(connection, &header, Kind::Open, handle, app_id, title, options).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_file(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        handle: OwnedObjectPath,
        app_id: String,
        _parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        self.run(connection, &header, Kind::Save, handle, app_id, title, options).await
    }

    #[allow(clippy::too_many_arguments)]
    async fn save_files(
        &self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(header)] header: Header<'_>,
        handle: OwnedObjectPath,
        app_id: String,
        _parent_window: String,
        title: String,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, HashMap<String, OwnedValue>) {
        self.run(connection, &header, Kind::SaveFiles, handle, app_id, title, options).await
    }

    #[zbus(property)]
    fn version(&self) -> u32 {
        4
    }
}

/// The request handle: the frontend calls Close when the app gives up.
struct RequestObject {
    id: u64,
    events: async_channel::Sender<Event>,
    reply: async_channel::Sender<Response>,
}

#[zbus::interface(name = "org.freedesktop.impl.portal.Request")]
impl RequestObject {
    async fn close(&self, #[zbus(connection)] connection: &Connection, #[zbus(header)] header: Header<'_>) -> zbus::fdo::Result<()> {
        if !from_frontend(connection, &header).await {
            return Err(zbus::fdo::Error::AccessDenied("only the portal frontend may close a request".into()));
        }
        let _ = self.reply.try_send(Response::cancelled());
        let _ = self.events.send(Event::Close(self.id)).await;
        Ok(())
    }
}

/// Serves the chooser while the returned connection lives. Fails when another
/// BoxBox already has the name, so that one keeps answering.
pub fn serve(events: async_channel::Sender<Event>) -> Result<zbus::blocking::Connection> {
    use zbus::fdo::{RequestNameFlags, RequestNameReply};
    let chooser = FileChooser { events, next: AtomicU64::new(1), open: Default::default() };
    let connection = zbus::blocking::connection::Builder::session()?.serve_at(OBJECT, chooser)?.build().context("cannot reach the session bus")?;
    match connection.request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into())? {
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => Ok(connection),
        _ => bail!("another BoxBox already serves the file chooser"),
    }
}
