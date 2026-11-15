//! A layer over a pane's list or grid for the pointer gestures that are not on
//! one item: Ctrl+wheel zoom and the rubber band. It draws the band too. It
//! takes no clicks itself, so rows and tiles under it work as before.

use gpui::{
    canvas, prelude::*, px, quad, BorderStyle, ContentMask, Context, DispatchPhase, HitboxBehavior, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ScrollWheelEvent,
};

use crate::theme::*;
use crate::workspace::operations::PaneTarget;
use crate::workspace::Workspace;

impl Workspace {
    pub(crate) fn pointer_layer(&self, target: PaneTarget, window: &gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = cx.entity().downgrade();
        let band = self.band_rect(target, window);
        let viewport = self.pane_layout(target, window).map(|(_, viewport, _)| viewport);
        let held = self.band.as_ref().is_some_and(|band| band.target == target);
        let accent = c(self.accent_hex());
        canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |_, hitbox, window, _| {
                if let (Some(band), Some(viewport)) = (band, viewport) {
                    window.with_content_mask(Some(ContentMask { bounds: viewport }), |window| {
                        window.paint_quad(quad(band, px(2.), alpha_of(accent, 0.15), px(1.), alpha_of(accent, 0.8), BorderStyle::Solid));
                    });
                }
                // Before the list scrolls: Ctrl+wheel zooms instead.
                window.on_mouse_event({
                    let (workspace, hitbox) = (workspace.clone(), hitbox.clone());
                    move |event: &ScrollWheelEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture && event.modifiers.control && hitbox.is_hovered(window) {
                            cx.stop_propagation();
                            workspace.update(cx, |this, cx| this.zoom_wheel(target, event.delta, window, cx)).ok();
                        }
                    }
                });
                window.on_mouse_event({
                    let (workspace, hitbox) = (workspace.clone(), hitbox.clone());
                    move |event: &MouseDownEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left && hitbox.is_hovered(window) {
                            workspace.update(cx, |this, cx| this.band_press(target, event.position, event.modifiers, window, cx)).ok();
                        }
                    }
                });
                // While held, the band follows the pointer anywhere, even outside the window.
                if held {
                    window.on_mouse_event({
                        let workspace = workspace.clone();
                        move |event: &MouseMoveEvent, phase, window, cx| {
                            if phase == DispatchPhase::Bubble {
                                workspace.update(cx, |this, cx| this.band_move(event.position, window, cx)).ok();
                            }
                        }
                    });
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                            workspace.update(cx, |this, cx| this.band_release(cx)).ok();
                        }
                    });
                }
            },
        )
        .absolute()
        .inset_0()
    }
}

fn alpha_of(color: gpui::Rgba, a: f32) -> gpui::Rgba {
    gpui::Rgba { a: color.a * a, ..color }
}
