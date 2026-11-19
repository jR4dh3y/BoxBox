//! Motion: short eased transitions for things that appear (menus, dialogs,
//! notices, tabs, a folder's contents) and for hover and press feedback.
//! Things appear as macOS shows them: they fade in where they stand, quickest
//! for menus, a little slower for popovers and dialogs, and never slide.
//!
//! GPUI swaps `.hover()` and `.active()` styles instantly, so `Interact` tracks
//! hover and press itself and paints an eased fill behind its child and a state
//! layer over it. Everything is off when the user or the desktop turns
//! animations off (`Motion`). Each running transition redraws the window every
//! frame, which costs about 3.5 ms in a release build with 3000 items listed.

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
/// A menu, which should be there almost as the button goes down.
pub const MENU: Duration = Duration::from_millis(100);
/// A hover card, a notice or a side panel.
pub const POPOVER: Duration = Duration::from_millis(150);
/// A dialog and the shade behind it.
pub const DIALOG: Duration = Duration::from_millis(180);
/// A folder's contents after navigating.
pub const CONTENT: Duration = Duration::from_millis(120);
/// A new tab sliding open.
pub const TAB: Duration = Duration::from_millis(160);

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

/// Fast at first, settling gently.
pub fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}

/// `element` fading in where it stands, over `duration`. Nothing moves: a
/// margin animated under an `anchored` menu shifts it as it settles. The
/// animation replays whenever the element appears again, since GPUI drops
/// the state of an element that was not drawn.
pub fn enter<E: IntoElement + Styled + 'static>(id: impl Into<ElementId>, element: E, duration: Duration, cx: &App) -> AnyElement {
    if !enabled(cx) {
        return element.into_any_element();
    }
    element.with_animation(id, Animation::new(duration).with_easing(ease_out), |element, t| element.opacity(t)).into_any_element()
}

/// The hover fill of rows and list-like controls: a light wash, so it shows on
/// the panels' dark grey, over a wallpaper and in a transparent window alike.
pub fn hover_fill() -> Rgba {
    Rgba { r: 1., g: 1., b: 1., a: 0.07 }
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
        Self { from: 0., to: 0., start: Instant::now(), duration: Duration::ZERO }
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
    radius: Pixels,
    disabled: bool,
    press: bool,
}

/// Wraps `child`; give it `fill` for a hover background.
pub fn interact(id: impl Into<ElementId>, child: impl IntoElement) -> Interact {
    Interact { id: id.into(), child: Some(child.into_any_element()), base: None, fill: None, radius: px(0.), disabled: false, press: true }
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
        let track = window.with_element_state(id.expect("Interact has an id"), |state: Option<Rc<RefCell<Track>>>, _| {
            let track = state.unwrap_or_else(|| {
                Rc::new(RefCell::new(Track { hovered: false, pressed: false, hover: Transition::at_rest(), press: Transition::at_rest() }))
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
            let (hover, press) = (state.hover.value(now), state.press.value(now));
            let corners = Corners::all(self.radius);
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
            if state.hover.running(now) || state.press.running(now) {
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
