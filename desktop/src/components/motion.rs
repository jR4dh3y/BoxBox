//! Motion: short eased transitions for things that appear (menus, dialogs,
//! notices, tabs, a folder's contents), for hover, press and selection
//! feedback, and for things that fold, turn or fill.
//!
//! Movement is offset only, never size, so nothing beside a moving thing
//! reflows: a folder's contents arrive from the side the pane moved to,
//! menus drop and dialogs rise a few pixels into place, notices rise into
//! their stack and the Details panel slides in from the right. A new tab
//! slides out of the + button. Sidebar sections fold through a clip, sort
//! arrows and chevrons turn, and usage bars grow to their value.
//!
//! GPUI swaps `.hover()` and `.active()` styles instantly, so `Interact` tracks
//! hover, press and selection itself and paints eased fills behind its child
//! and a state layer over it. Everything is off when the user or the desktop
//! turns animations off (`Motion`). Each running transition redraws the
//! window every frame: about 3.3 ms a frame in a release build with 3000
//! items listed, against 2.4 ms at rest, and opening a folder is no slower.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    fill, px, Animation, AnimationExt as _, AnyElement, App, Bounds, Corners, DispatchPhase, Element, ElementId, GlobalElementId, Global,
    Hitbox, HitboxBehavior, InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Rgba, ScrollWheelEvent, Styled, Window,
};

/// Hover fills come in and leave a little slower, as desktop toolkits do.
const HOVER_IN: Duration = Duration::from_millis(130);
const HOVER_OUT: Duration = Duration::from_millis(220);
const PRESS_IN: Duration = Duration::from_millis(40);
const PRESS_OUT: Duration = Duration::from_millis(180);
/// A selection washes in quickly, so a click answers at once, and leaves softly.
const SELECT_IN: Duration = Duration::from_millis(90);
const SELECT_OUT: Duration = Duration::from_millis(200);
/// A menu, which should be there almost as the button goes down.
pub const MENU: Duration = Duration::from_millis(100);
/// A hover card, a notice or a side panel.
pub const POPOVER: Duration = Duration::from_millis(150);
/// A dialog and the shade behind it.
pub const DIALOG: Duration = Duration::from_millis(180);
/// A folder's contents after navigating.
pub const CONTENT: Duration = Duration::from_millis(120);
/// A new tab sliding out of the + button.
pub const TAB: Duration = Duration::from_millis(160);
/// A chevron or sort arrow turning over.
pub const TURN: Duration = Duration::from_millis(180);
/// A sidebar section folding open or shut.
pub const FOLD: Duration = Duration::from_millis(200);
/// A notice or other thing sliding away once dismissed.
pub const LEAVE: Duration = Duration::from_millis(220);
/// A usage bar growing to its value.
pub const GROW: Duration = Duration::from_millis(520);
/// A folder's contents arriving from the side the pane moved to.
pub const ARRIVE: Duration = Duration::from_millis(220);
/// How far they travel: enough to read as a direction, short enough not to blur.
const ARRIVE_DISTANCE: f32 = 28.;

/// Whether things move: Settings → Appearance → Animations and the desktop's own
/// `enable-animations`, read from the settings portal.
#[derive(Clone, Copy)]
pub struct Motion {
    pub enabled: bool,
}

impl Global for Motion {}

pub fn enabled(cx: &App) -> bool {
    cx.try_global::<Motion>().is_none_or(|motion| motion.enabled)
}

/// The desktop's animation setting, when its settings portal has one. Blocks on D-Bus.
#[cfg(target_os = "linux")]
pub fn desktop_enabled() -> Option<bool> {
    let connection = zbus::blocking::connection::Builder::session().ok()?.method_timeout(Duration::from_secs(1)).build().ok()?;
    let reply = connection
        .call_method(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            Some("org.freedesktop.portal.Settings"),
            "ReadOne",
            &("org.gnome.desktop.interface", "enable-animations"),
        )
        .ok()?;
    let value: zbus::zvariant::OwnedValue = reply.body().deserialize().ok()?;
    bool::try_from(value).ok()
}

/// Windows' "Animation effects" setting (Settings > Accessibility > Visual effects).
#[cfg(windows)]
pub fn desktop_enabled() -> Option<bool> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION};
    let mut enabled: i32 = 0;
    let read = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut enabled).cast(), 0) };
    (read != 0).then_some(enabled != 0)
}

/// Fast at first, settling gently.
pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

/// `element` fading in while it settles from `(dx, dy)` pixels away, over
/// `duration`. The animation replays whenever the element appears again,
/// since GPUI drops the state of an element that was not drawn. The
/// offset is relative positioning, which moves only what is drawn: an
/// `anchored` menu keeps its place and nothing beside it reflows. Only for an
/// element in the normal flow: it is made relative.
pub fn enter_from<E: IntoElement + Styled + 'static>(id: impl Into<ElementId>, element: E, duration: Duration, (dx, dy): (f32, f32), cx: &App) -> AnyElement {
    if !enabled(cx) {
        return element.into_any_element();
    }
    element
        .with_animation(id, Animation::new(duration).with_easing(ease_out_quint), move |element, t| {
            let element = element.opacity(t);
            if dx == 0. && dy == 0. {
                return element;
            }
            element.relative().left(px(dx * (1. - t))).top(px(dy * (1. - t)))
        })
        .into_any_element()
}

/// How far a change that happened at `since` has eased in over `duration`,
/// 0 to 1; asks for the next frame until it is done. 1 with motion off.
pub fn progress(since: Option<Instant>, duration: Duration, window: &Window, cx: &App) -> f32 {
    let Some(since) = since.filter(|_| enabled(cx)) else { return 1. };
    let t = since.elapsed().as_secs_f32() / duration.as_secs_f32();
    if t >= 1. {
        return 1.;
    }
    window.request_animation_frame();
    ease_out_quint(t)
}

/// A chevron or arrow turning to `degrees`, from where `from` says it was,
/// when `id` changes; at rest it is simply turned.
pub fn turn(id: impl Into<ElementId>, icon: gpui::Svg, from: f32, degrees: f32, cx: &App) -> AnyElement {
    let rotate = |angle: f32| gpui::Transformation::rotate(gpui::radians(angle.to_radians()));
    if !enabled(cx) || from == degrees {
        return icon.with_transformation(rotate(degrees)).into_any_element();
    }
    icon.with_animation(id, Animation::new(TURN).with_easing(ease_out), move |icon, t| icon.with_transformation(rotate(from + (degrees - from) * t)))
        .into_any_element()
}

/// The hover fill of rows and list-like controls: a light wash, so it shows on
/// the panels' dark grey, over a wallpaper and in a transparent window alike.
/// Below about 12% it vanishes over a busy wallpaper.
pub fn hover_fill() -> Rgba {
    Rgba { r: 1., g: 1., b: 1., a: 0.12 }
}

/// `element` fading up from 60%, for content that replaced what was there.
pub fn reveal<E: IntoElement + Styled + 'static>(id: impl Into<ElementId>, element: E, cx: &App) -> AnyElement {
    if !enabled(cx) {
        return element.into_any_element();
    }
    element
        .with_animation(id, Animation::new(CONTENT).with_easing(ease_out), |element, t| element.opacity(0.6 + 0.4 * t))
        .into_any_element()
}

/// Which side a folder's contents come from, as pages do in Nautilus: a deeper
/// folder from the right, a shallower one from the left.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Arrival {
    /// Somewhere unrelated, or another provider: the contents only fade up.
    #[default]
    InPlace,
    FromRight,
    FromLeft,
}

impl Arrival {
    /// The way from `from` to `to`, two virtual paths; the Overview is `""`.
    pub fn between(from: &str, to: &str, same_provider: bool) -> Self {
        let under = |child: &str, parent: &str| parent.is_empty() && !child.is_empty() || child.strip_prefix(parent).is_some_and(|rest| rest.starts_with('/'));
        match (same_provider, under(to, from), under(from, to)) {
            (true, true, _) => Self::FromRight,
            (true, _, true) => Self::FromLeft,
            _ => Self::InPlace,
        }
    }
}

/// `element` sliding in from `arrival`'s side while it fades up; it only moves,
/// so nothing beside it reflows. Plays once per `id`.
pub fn arrive<E: IntoElement + Styled + 'static>(id: impl Into<ElementId>, element: E, arrival: Arrival, cx: &App) -> AnyElement {
    let from = match arrival {
        Arrival::FromRight => ARRIVE_DISTANCE,
        Arrival::FromLeft => -ARRIVE_DISTANCE,
        Arrival::InPlace => return element.into_any_element(),
    };
    if !enabled(cx) {
        return element.into_any_element();
    }
    element
        .with_animation(id, Animation::new(ARRIVE).with_easing(ease_out_quint), move |element, t| element.left(px(from * (1. - t))).opacity(0.25 + 0.75 * t))
        .into_any_element()
}

/// How long after a folder's first listing lands its items still cascade in;
/// items drawn later, by scrolling, simply appear.
pub const CASCADE_WINDOW: Duration = Duration::from_millis(450);
const CASCADE: Duration = Duration::from_millis(200);
const CASCADE_STEP: Duration = Duration::from_millis(14);
/// Items past this many come in with the last of them.
const CASCADE_ITEMS: usize = 16;

/// `element`, the `index`th item of a listing that just arrived, fading up and
/// settling a few pixels a little after the one before it.
pub fn cascade<E: IntoElement + Styled + 'static>(id: impl Into<ElementId>, element: E, index: usize, cx: &App) -> AnyElement {
    if !enabled(cx) {
        return element.into_any_element();
    }
    let delay = (CASCADE_STEP * index.min(CASCADE_ITEMS) as u32).as_secs_f32();
    let (own, total) = (CASCADE.as_secs_f32(), CASCADE.as_secs_f32() + delay);
    element
        .with_animation(id, Animation::new(Duration::from_secs_f32(total)), move |item, t| {
            let local = ease_out_quint(((t * total - delay) / own).clamp(0., 1.));
            item.opacity(local).relative().top(px(6. * (1. - local)))
        })
        .into_any_element()
}

/// Quicker off the mark than `ease_out` and longer to settle, for movement.
pub fn ease_out_quint(t: f32) -> f32 {
    1. - (1. - t).powi(5)
}

fn mix(from: Rgba, to: Rgba, t: f32) -> Rgba {
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    Rgba { r: lerp(from.r, to.r), g: lerp(from.g, to.g), b: lerp(from.b, to.b), a: lerp(from.a, to.a) }
}

/// A value easing toward a target.
#[derive(Clone, Copy)]
struct Transition {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
}

impl Transition {
    fn at_rest() -> Self {
        Self::resting(0.)
    }

    fn resting(value: f32) -> Self {
        Self { from: value, to: value, start: Instant::now(), duration: Duration::ZERO }
    }

    fn value(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.start);
        if self.duration.is_zero() || elapsed >= self.duration {
            return self.to;
        }
        self.from + (self.to - self.from) * ease_out(elapsed.as_secs_f32() / self.duration.as_secs_f32())
    }

    fn running(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start) < self.duration
    }

    /// Heads for `to` from wherever the value is now, so a quick in-and-out never jumps.
    fn go(&mut self, to: f32, duration: Duration, now: Instant) {
        if self.to == to {
            return;
        }
        self.from = self.value(now);
        self.to = to;
        self.start = now;
        self.duration = duration;
    }
}

/// One element's hover and press, kept across frames.
pub struct Track {
    hovered: bool,
    pressed: bool,
    hover: Transition,
    press: Transition,
    select: Transition,
    /// The last selection colour, which a deselected element fades out in.
    selection: Rgba,
}

impl Track {
    fn set_hovered(&mut self, hovered: bool, animate: bool) -> bool {
        if self.hovered == hovered {
            return false;
        }
        self.hovered = hovered;
        let duration = if !animate { Duration::ZERO } else if hovered { HOVER_IN } else { HOVER_OUT };
        self.hover.go(if hovered { 1. } else { 0. }, duration, Instant::now());
        true
    }

    fn set_pressed(&mut self, pressed: bool, animate: bool) -> bool {
        if self.pressed == pressed {
            return false;
        }
        self.pressed = pressed;
        let duration = if !animate { Duration::ZERO } else if pressed { PRESS_IN } else { PRESS_OUT };
        self.press.go(if pressed { 1. } else { 0. }, duration, Instant::now());
        true
    }
}

/// Hover and press feedback for `child`, which keeps its own layout and handlers:
/// a fill behind it that eases in on hover, and a light layer over it while pressed.
pub struct Interact {
    id: ElementId,
    child: Option<AnyElement>,
    base: Option<Rgba>,
    fill: Option<Rgba>,
    selection: Option<Rgba>,
    radius: Pixels,
    disabled: bool,
    press: bool,
}

/// Wraps `child`; give it `fill` for a hover background.
pub fn interact(id: impl Into<ElementId>, child: impl IntoElement) -> Interact {
    Interact { id: id.into(), child: Some(child.into_any_element()), base: None, fill: None, selection: None, radius: px(0.), disabled: false, press: true }
}

impl Interact {
    /// The background shown at full hover. A child with its own opaque background
    /// hides it: give that background as `base` instead.
    pub fn fill(mut self, color: Rgba) -> Self {
        self.fill = Some(color);
        self
    }

    /// The background at rest, blended into `fill` on hover; the child draws none.
    pub fn base(mut self, color: Rgba) -> Self {
        self.base = Some(color);
        self
    }

    /// The selection colour while selected, eased in under the hover fill and
    /// out again when deselected; the child draws none.
    pub fn selected(mut self, color: Option<Rgba>) -> Self {
        self.selection = color;
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.radius = px(radius);
        self
    }

    /// Leaves out the press layer, for rows whose selection already answers a
    /// click and which a drag would keep pressed.
    pub fn without_press(mut self) -> Self {
        self.press = false;
        self
    }

    /// No feedback, as for a disabled control.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// A white layer over pressed controls; it reads on the dark theme and on accent buttons.
const PRESS_LAYER: f32 = 0.1;

impl IntoElement for Interact {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Interact {
    type RequestLayoutState = (AnyElement, Rc<RefCell<Track>>);
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, id: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, Self::RequestLayoutState) {
        let selection = self.selection;
        let track = window.with_element_state(id.expect("Interact has an id"), |state: Option<Rc<RefCell<Track>>>, _| {
            // An element first drawn selected, such as a row scrolled into view, is already so.
            let track = state.unwrap_or_else(|| {
                Rc::new(RefCell::new(Track {
                    hovered: false,
                    pressed: false,
                    hover: Transition::at_rest(),
                    press: Transition::at_rest(),
                    select: Transition::resting(if selection.is_some() { 1. } else { 0. }),
                    selection: selection.unwrap_or_default(),
                }))
            });
            (track.clone(), track)
        });
        let mut child = self.child.take().expect("laid out once");
        let layout = child.request_layout(window, cx);
        (layout, (child, track))
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, (child, _): &mut Self::RequestLayoutState, window: &mut Window, cx: &mut App) -> Hitbox {
        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        child.prepaint(window, cx);
        hitbox
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (child, track): &mut Self::RequestLayoutState,
        hitbox: &mut Hitbox,
        window: &mut Window,
        cx: &mut App,
    ) {
        let animate = enabled(cx);
        let now = Instant::now();
        {
            let mut state = track.borrow_mut();
            // Content can move under a still pointer (scrolling), which no pointer event reports.
            let hovered = !self.disabled && hitbox.is_hovered(window);
            state.set_hovered(hovered, animate);
            if self.disabled || !self.press {
                state.set_pressed(false, false);
            }
            if let Some(color) = self.selection {
                state.selection = color;
            }
            let selecting = self.selection.is_some();
            let target = if selecting { 1. } else { 0. };
            state.select.go(target, if !animate { Duration::ZERO } else if selecting { SELECT_IN } else { SELECT_OUT }, now);
            let (hover, press, select) = (state.hover.value(now), state.press.value(now), state.select.value(now));
            let corners = Corners::all(self.radius);
            if select > 0. {
                window.paint_quad(fill(bounds, Rgba { a: state.selection.a * select, ..state.selection }).corner_radii(corners));
            }
            let background = match (self.base, self.fill) {
                (Some(base), Some(hovered)) => Some(mix(base, hovered, hover)),
                (Some(base), None) => Some(base),
                (None, Some(hovered)) => Some(Rgba { a: hovered.a * hover, ..hovered }),
                (None, None) => None,
            };
            if let Some(color) = background.filter(|color| color.a > 0.) {
                window.paint_quad(fill(bounds, color).corner_radii(corners));
            }
            child.paint(window, cx);
            if press > 0. {
                window.paint_quad(fill(bounds, Rgba { r: 1., g: 1., b: 1., a: PRESS_LAYER * press }).corner_radii(corners));
            }
            if state.hover.running(now) || state.press.running(now) || state.select.running(now) {
                window.request_animation_frame();
            }
        }
        if self.disabled {
            return;
        }
        // Capture runs before any child stops a bubbling event, so clicks still register here.
        let (hover_track, hover_box) = (track.clone(), hitbox.clone());
        window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
            if phase == DispatchPhase::Capture && hover_track.borrow_mut().set_hovered(hover_box.is_hovered(window), enabled(cx)) {
                window.refresh();
            }
        });
        let (scroll_track, scroll_box) = (track.clone(), hitbox.clone());
        window.on_mouse_event(move |_: &ScrollWheelEvent, phase, window, cx| {
            if phase == DispatchPhase::Capture && scroll_track.borrow_mut().set_hovered(scroll_box.is_hovered(window), enabled(cx)) {
                window.refresh();
            }
        });
        if !self.press {
            return;
        }
        let (down_track, down_box) = (track.clone(), hitbox.clone());
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if phase == DispatchPhase::Capture && event.button == MouseButton::Left && down_box.is_hovered(window) && down_track.borrow_mut().set_pressed(true, enabled(cx)) {
                window.refresh();
            }
        });
        let up_track = track.clone();
        window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
            if phase == DispatchPhase::Capture && up_track.borrow_mut().set_pressed(false, enabled(cx)) {
                window.refresh();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contents_arrive_from_the_side_the_pane_moved_to() {
        assert_eq!(Arrival::between("home", "home/docs", true), Arrival::FromRight);
        assert_eq!(Arrival::between("", "home", true), Arrival::FromRight, "out of the Overview is deeper");
        assert_eq!(Arrival::between("home/docs/a", "home", true), Arrival::FromLeft);
        assert_eq!(Arrival::between("home/docs", "", true), Arrival::FromLeft);
        assert_eq!(Arrival::between("home/docs", "home/documents", true), Arrival::InPlace, "a sibling is not under it");
        assert_eq!(Arrival::between("home", "home/docs", false), Arrival::InPlace, "another provider only fades");
    }

    #[test]
    fn transitions_ease_toward_the_target_from_where_they_are() {
        let start = Instant::now();
        let mut value = Transition::at_rest();
        value.go(1., Duration::from_millis(100), start);
        assert_eq!(value.value(start), 0.);
        let half = value.value(start + Duration::from_millis(50));
        assert!(half > 0.5 && half < 1., "eased out: past halfway at half time, got {half}");
        assert_eq!(value.value(start + Duration::from_millis(100)), 1.);
        // Reversing midway starts from the current value, not from 1.
        let midway = start + Duration::from_millis(50);
        value.go(0., Duration::from_millis(100), midway);
        assert_eq!(value.value(midway), half);
        assert!(!value.running(midway + Duration::from_millis(100)));
        let mut instant = Transition::at_rest();
        instant.go(1., Duration::ZERO, start);
        assert_eq!(instant.value(start), 1., "with motion off a change is immediate");
    }
}
