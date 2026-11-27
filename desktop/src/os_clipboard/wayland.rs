//! Data-control publication with a compositor acknowledgement before the next write.
use std::fs::File;
use std::io::Write;
use std::sync::mpsc;

use anyhow::{Context as _, Result};
use wayland_client::globals::{registry_queue_init, GlobalList, GlobalListContents};
use wayland_client::protocol::{wl_registry::WlRegistry, wl_seat::WlSeat};
use wayland_client::{delegate_noop, Connection, Dispatch, Proxy, QueueHandle};

type Formats = Vec<(&'static str, Vec<u8>)>;

pub(super) fn publish(formats: Formats) -> Result<()> {
    let (ready, published) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("clipboard".into())
        .spawn(move || {
            let result = (|| -> Result<()> {
                let connection = Connection::connect_to_env()?;
                let (globals, mut queue) = registry_queue_init::<State>(&connection)?;
                let seats: Vec<_> = globals.contents().with_list(|list| {
                    list.iter()
                        .filter(|global| global.interface == WlSeat::interface().name)
                        .map(|global| {
                            globals.registry().bind::<WlSeat, _, _>(
                                global.name,
                                1,
                                &queue.handle(),
                                (),
                            )
                        })
                        .collect()
                });
                anyhow::ensure!(!seats.is_empty(), "the compositor has no clipboard seat");
                if globals.contents().with_list(|list| {
                    list.iter()
                        .any(|global| global.interface == ext::Manager::interface().name)
                }) {
                    ext::offer(&globals, &queue.handle(), &seats, &formats)?;
                } else {
                    wlr::offer(&globals, &queue.handle(), &seats, &formats)?;
                }
                let mut state = State {
                    formats,
                    sources: seats.len(),
                };
                queue.roundtrip(&mut state)?;
                ready.send(Ok(())).ok();
                while state.sources != 0 {
                    queue.blocking_dispatch(&mut state)?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                if ready.send(Err(error)).is_err() {
                    log::debug!("Stopped serving files from the clipboard");
                }
            }
        })?;
    published.recv().context("clipboard publisher stopped")?
}

struct State {
    formats: Formats,
    sources: usize,
}

delegate_noop!(State: ignore WlSeat);

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

// The two clipboard protocols have the same regular-selection requests and events.
macro_rules! clipboard_protocol {
    ($module:ident, $manager:path, $device:path, $offer:path, $source:path) => {
        mod $module {
            use super::*;
            use $manager as manager;
            use $device as device;
            use $offer as offer;
            use $source as source;

            pub(super) use manager::Manager;
            use device::Device;
            use offer::Offer;
            use source::Source;

            pub(super) fn offer(globals: &GlobalList, handle: &QueueHandle<State>, seats: &[WlSeat], formats: &Formats) -> Result<()> {
                let manager: Manager = globals.bind(handle, 1..=1, ()).context("the compositor has no data-control protocol")?;
                for seat in seats {
                    let device = manager.get_data_device(seat, handle, ());
                    let source = manager.create_data_source(handle, ());
                    for (mime, _) in formats {
                        source.offer((*mime).to_owned());
                    }
                    device.set_selection(Some(&source));
                }
                Ok(())
            }

            delegate_noop!(State: ignore Manager);
            delegate_noop!(State: ignore Offer);

            impl Dispatch<Device, ()> for State {
                fn event(_: &mut Self, device: &Device, event: device::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
                    match event {
                        device::Event::DataOffer { id } => id.destroy(),
                        device::Event::Finished => device.destroy(),
                        _ => {}
                    }
                }

                wayland_client::event_created_child!(State, Device, [
                    device::EVT_DATA_OFFER_OPCODE => (Offer, ())
                ]);
            }

            impl Dispatch<Source, ()> for State {
                fn event(state: &mut Self, source: &Source, event: source::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
                    match event {
                        source::Event::Send { mime_type, fd } => {
                            if let Some((_, bytes)) = state.formats.iter().find(|(mime, _)| *mime == mime_type) {
                                use std::os::fd::AsRawFd as _;
                                // The pipe belongs to this request. Serve the entire payload before closing it.
                                let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
                                if flags >= 0 && flags & libc::O_NONBLOCK != 0 {
                                    unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags & !libc::O_NONBLOCK) };
                                }
                                if let Err(error) = File::from(fd).write_all(bytes) {
                                    log::debug!("Clipboard request ended: {error}");
                                }
                            }
                        }
                        source::Event::Cancelled => {
                            source.destroy();
                            state.sources -= 1;
                        }
                        _ => {}
                    }
                }
            }
        }
    };
}

mod ext_protocol {
    use wayland_protocols::ext::data_control::v1::client;
    pub(super) mod manager {
        pub(in super::super) use super::client::ext_data_control_manager_v1::ExtDataControlManagerV1 as Manager;
    }
    pub(super) mod device {
        pub(in super::super) use super::client::ext_data_control_device_v1::{
            Event, ExtDataControlDeviceV1 as Device, EVT_DATA_OFFER_OPCODE,
        };
    }
    pub(super) mod source {
        pub(in super::super) use super::client::ext_data_control_source_v1::{
            Event, ExtDataControlSourceV1 as Source,
        };
    }
    pub(super) mod offer {
        pub(in super::super) use super::client::ext_data_control_offer_v1::ExtDataControlOfferV1 as Offer;
    }
}

mod wlr_protocol {
    use wayland_protocols_wlr::data_control::v1::client;
    pub(super) mod manager {
        pub(in super::super) use super::client::zwlr_data_control_manager_v1::ZwlrDataControlManagerV1 as Manager;
    }
    pub(super) mod device {
        pub(in super::super) use super::client::zwlr_data_control_device_v1::{
            Event, ZwlrDataControlDeviceV1 as Device, EVT_DATA_OFFER_OPCODE,
        };
    }
    pub(super) mod source {
        pub(in super::super) use super::client::zwlr_data_control_source_v1::{
            Event, ZwlrDataControlSourceV1 as Source,
        };
    }
    pub(super) mod offer {
        pub(in super::super) use super::client::zwlr_data_control_offer_v1::ZwlrDataControlOfferV1 as Offer;
    }
}

clipboard_protocol!(
    ext,
    super::ext_protocol::manager,
    super::ext_protocol::device,
    super::ext_protocol::offer,
    super::ext_protocol::source
);
clipboard_protocol!(
    wlr,
    super::wlr_protocol::manager,
    super::wlr_protocol::device,
    super::wlr_protocol::offer,
    super::wlr_protocol::source
);
