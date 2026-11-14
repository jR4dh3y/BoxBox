//! Opt-in profiler. `BOXBOX_PROFILE=trace.json` records a Chrome trace that
//! ui.perfetto.dev opens, and `boxbox-gpui --profile-summary trace.json` ranks it.
//!
//! Spans time work on any thread. Journeys time what the user feels: from the action
//! (a click, a keypress) to the first frame painted after its result is applied.
//! With profiling off every call is one relaxed load.

pub mod summary;

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};

use gpui::{canvas, AnyElement, IntoElement, Styled};
use serde_json::{json, Value};

/// The thread that runs GPUI; journeys and frames live there.
const MAIN_TID: u64 = 1;

struct Recorder {
    epoch: Instant,
    events: mpsc::Sender<Value>,
}

static RECORDER: OnceLock<Option<Recorder>> = OnceLock::new();
static NEXT_TID: AtomicU64 = AtomicU64::new(MAIN_TID);
static NEXT_JOURNEY: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static TID: Cell<u64> = const { Cell::new(0) };
    /// Journeys whose result is applied, waiting for the frame that shows it.
    static READY: RefCell<Vec<Journey>> = const { RefCell::new(Vec::new()) };
    static FRAME_START: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// Starts recording when `BOXBOX_PROFILE` names a file. Call first, on the main thread.
pub fn init() {
    let process_start = Instant::now();
    let recorder = std::env::var_os("BOXBOX_PROFILE").filter(|path| !path.is_empty()).and_then(|path| {
        let path = Path::new(&path).to_path_buf();
        match std::fs::File::create(&path) {
            Ok(file) => {
                eprintln!("Profiling to {}; open it in ui.perfetto.dev or run --profile-summary", path.display());
                let (events, received) = mpsc::channel();
                std::thread::Builder::new().name("profile-writer".into()).spawn(move || write_events(file, received)).ok()?;
                Some(Recorder { epoch: process_start, events })
            }
            Err(error) => {
                eprintln!("BOXBOX_PROFILE: cannot create {}: {error}", path.display());
                None
            }
        }
    });
    let enabled = recorder.is_some();
    RECORDER.get_or_init(|| recorder);
    if enabled {
        tid();
        Journey { id: 0, name: "startup", detail: None, start: process_start }.ready();
    }
}

fn recorder() -> Option<&'static Recorder> {
    RECORDER.get().and_then(Option::as_ref)
}

pub fn enabled() -> bool {
    recorder().is_some()
}

/// Streams the JSON array format. A trace cut short by a crash still loads,
/// since both viewers accept an array without its closing bracket.
fn write_events(file: std::fs::File, events: mpsc::Receiver<Value>) {
    let mut out = std::io::BufWriter::new(file);
    let _ = out.write_all(b"[\n");
    while let Ok(event) = events.recv() {
        let _ = writeln!(out, "{event},");
        while let Ok(event) = events.try_recv() {
            let _ = writeln!(out, "{event},");
        }
        let _ = out.flush();
    }
}

fn emit(recorder: &Recorder, event: Value) {
    let _ = recorder.events.send(event);
}

fn micros(recorder: &Recorder, at: Instant) -> f64 {
    at.saturating_duration_since(recorder.epoch).as_secs_f64() * 1e6
}

/// A stable small id for this thread, announcing its name the first time.
fn tid() -> u64 {
    TID.with(|tid| {
        if tid.get() == 0 {
            let id = NEXT_TID.fetch_add(1, Ordering::Relaxed);
            tid.set(id);
            if let Some(recorder) = recorder() {
                let name = match id {
                    MAIN_TID => "main (UI)".to_string(),
                    _ => std::thread::current().name().map_or_else(|| format!("worker {id}"), str::to_string),
                };
                emit(recorder, json!({"ph": "M", "name": "thread_name", "pid": 1, "tid": id, "args": {"name": name}}));
            }
        }
        tid.get()
    })
}

/// Times the enclosing scope; recorded when dropped.
#[must_use = "a span measures until it is dropped"]
pub struct Span {
    open: Option<(&'static str, Instant, Option<String>)>,
}

pub fn span(name: &'static str) -> Span {
    Span { open: enabled().then(|| (name, Instant::now(), None)) }
}

impl Span {
    /// What the span worked on, such as a path. Built only while profiling.
    pub fn with(mut self, detail: impl FnOnce() -> String) -> Self {
        if let Some((_, _, slot)) = &mut self.open {
            *slot = Some(detail());
        }
        self
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        let (Some((name, start, detail)), Some(recorder)) = (self.open.take(), recorder()) else {
            return;
        };
        let duration = start.elapsed();
        let mut event = json!({
            "ph": "X", "name": name, "cat": "span", "pid": 1, "tid": tid(),
            "ts": micros(recorder, start), "dur": duration.as_secs_f64() * 1e6,
        });
        if let Some(detail) = detail {
            event["args"] = json!({"detail": detail});
        }
        emit(recorder, event);
    }
}

/// A user action, from input to the frame that shows its result. Dropping one
/// unfinished (a newer request replaced it) records nothing.
#[must_use = "call ready() once the result is applied"]
pub struct Journey {
    id: u64,
    name: &'static str,
    detail: Option<String>,
    start: Instant,
}

pub fn journey(name: &'static str, detail: impl FnOnce() -> String) -> Option<Journey> {
    enabled().then(|| Journey {
        id: NEXT_JOURNEY.fetch_add(1, Ordering::Relaxed),
        name,
        detail: Some(detail()),
        start: Instant::now(),
    })
}

impl Journey {
    /// The result is in the UI state; the next painted frame ends the journey.
    /// Call on the UI thread, alongside the `notify` that redraws.
    pub fn ready(self) {
        READY.with(|ready| ready.borrow_mut().push(self));
    }
}

/// Ends a journey, if one is being recorded, at the next painted frame.
pub fn ready(journey: Option<Journey>) {
    if let Some(journey) = journey {
        journey.ready();
    }
}

/// Call first in the root view's render.
pub fn frame_started() {
    if enabled() {
        FRAME_START.with(|start| start.set(Some(Instant::now())));
    }
}

/// Paints last in the window, so it runs once the frame's layout and paint are done.
pub fn frame_marker() -> Option<AnyElement> {
    enabled().then(|| canvas(|_, _, _| {}, |_, _, _, _| frame_painted()).absolute().size_0().into_any_element())
}

fn frame_painted() {
    let Some(recorder) = recorder() else { return };
    let now = Instant::now();
    if let Some(start) = FRAME_START.with(Cell::take) {
        let duration = now - start;
        emit(recorder, json!({
            "ph": "X", "name": "frame", "cat": "frame", "pid": 1, "tid": MAIN_TID,
            "ts": micros(recorder, start), "dur": duration.as_secs_f64() * 1e6,
        }));
        if duration > Duration::from_millis(16) {
            log::debug!("slow frame: {duration:?}");
        }
    }
    for journey in READY.with(|ready| std::mem::take(&mut *ready.borrow_mut())) {
        let args = json!({"detail": journey.detail});
        // Async events give each journey its own row, since journeys overlap.
        for (phase, at) in [("b", journey.start), ("e", now)] {
            emit(recorder, json!({
                "ph": phase, "name": journey.name, "cat": "journey", "id": journey.id,
                "pid": 1, "tid": MAIN_TID, "ts": micros(recorder, at), "args": args,
            }));
        }
    }
}
