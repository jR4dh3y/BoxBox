//! The tab strip in `BrowseWorkspace.svelte`.

use gpui::{div, prelude::*, px, Animation, AnimationExt as _, Context, Window};
#[cfg(windows)]
use gpui::WindowControlArea;

use super::{icon, TextScale};
use crate::components::motion;
use crate::theme::*;
use crate::workspace::operations::PaneTarget;
use crate::workspace::{NewTab, ToggleSplit, Workspace};

/// Wider than any tab, so the clip never cuts one that has finished opening.
const TAB_MAX_WIDTH: f32 = 260.;

impl Workspace {
    pub(crate) fn render_tabs(&self, _window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let closable = self.tabs.len() > 1;
        let animate = motion::enabled(cx);
        let tabs = self.tabs.iter().enumerate().map(|(index, pane)| {
            // The folder's name as the path bar shows it, so a device root reads as the device.
            let title = self.breadcrumbs(PaneTarget(pane.id)).pop().map_or_else(|| "Overview".to_string(), |crumb| crumb.label);
            let active = index == self.active_tab;
            let tab = div()
                .flex()
                .min_w_0()
                .items_center()
                .rounded(px(4.))
                .when(active, |tab| tab.bg(c(SURFACE_ELEVATED)))
                .child(
                    div()
                        .id(("tab", index))
                        .max_w(px(176.))
                        .px(px(8.))
                        .py(px(4.))
                        .text_xs_tw()
                        .truncate()
                        .cursor_pointer()
                        .child(title)
                        .on_click(cx.listener(move |this, _, _, cx| this.activate_tab(index, cx))),
                )
                .when(closable, |tab| {
                    tab.child(
                        motion::interact(
                            ("close-tab-feedback", pane.id),
                            div()
                                .id(("close-tab", index))
                                .p(px(4.))
                                .cursor_pointer()
                                .child(icon("x", 12., c(TEXT_PRIMARY)))
                                .on_click(cx.listener(move |this, _, _, cx| this.close_tab_at(index, cx))),
                        )
                        .fill(c(SURFACE_TERTIARY))
                        .rounded(4.),
                    )
                });
            // A tab opened after launch slides out of the + button, as in File Pilot:
            // a clip widens from nothing while the tab stays pinned to its right
            // edge, against +, so the tab is drawn out of it and + moves aside.
            let tab = match animate && pane.id >= self.first_new_pane {
                true => div()
                    .flex()
                    .justify_end()
                    .overflow_hidden()
                    .child(tab.flex_none())
                    .with_animation(("tab-open", pane.id), Animation::new(motion::TAB).with_easing(motion::ease_out), |clip, t| {
                        clip.max_w(px(TAB_MAX_WIDTH * t)).opacity(0.3 + 0.7 * t)
                    })
                    .into_any_element(),
                false => tab.into_any_element(),
            };
            motion::interact(("tab-feedback", pane.id), tab).fill(c(SURFACE_SECONDARY)).rounded(4.)
        });
        let tool = |id: &'static str, glyph: &'static str| {
            div().id(id).debug_selector(move || id.into()).p(px(4.)).cursor_pointer().child(icon(glyph, 14., c(TEXT_SECONDARY)))
        };

        // Taffy drops `gap` in a row with an auto margin, so a spacer stands in for `ml-auto`.
        let bar = div()
            .h(px(32.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(4.))
            .border_b_1()
            .border_color(c(BORDER_SECONDARY))
            .bg(self.shell_background())
            .px(px(8.))
            .child(div().id("tabs").flex().items_center().gap(px(4.)).min_w_0().overflow_x_scroll().children(tabs))
            .child(
                motion::interact("new-tab-feedback", tool("new-tab", "plus").on_click(cx.listener(|this, _, window, cx| this.new_tab(&NewTab, window, cx))))
                    .fill(c(SURFACE_SECONDARY))
                    .rounded(4.),
            )
            .child({
                let spacer = div().flex_1().h_full();
                #[cfg(windows)]
                let spacer = spacer.min_w(px(32.)).window_control_area(WindowControlArea::Drag);
                spacer
            })
            .child(
                motion::interact(
                    "toggle-split-feedback",
                    tool("toggle-split", "columns-2").on_click(cx.listener(|this, _, window, cx| this.toggle_split(&ToggleSplit, window, cx))),
                )
                .fill(c(SURFACE_SECONDARY))
                .rounded(4.),
            );
        #[cfg(windows)]
        let bar = bar.when(self.integrated_window_controls && !_window.is_fullscreen(), |bar| {
            let maximized = _window.is_maximized();
            bar.pr_0()
                // Leave the top resize edge available on a restored window.
                .when(!maximized, |bar| bar.h(px(37.)).pt(px(5.)))
                .child(window_controls(maximized))
        });
        bar
    }
}

#[cfg(windows)]
fn window_controls(maximized: bool) -> impl IntoElement {
    use WindowControlArea::{Close, Max, Min};

    let maximize_icon = if maximized { "window-restore" } else { "window-maximize" };
    let buttons = [
        ("minimize-window", "window-minimize", Min),
        ("maximize-window", maximize_icon, Max),
        ("close-window", "x", Close),
    ];
    div().flex().flex_none().h_full().children(buttons.map(|(id, glyph, area)| {
        let hover = if matches!(area, Close) { c(0xc42b1c) } else { c(SURFACE_ELEVATED) };
        div()
            .id(id)
            .w(px(46.))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .window_control_area(area)
            .hover(move |button| button.bg(hover))
            .active(crate::components::pressed)
            .child(icon(glyph, 14., c(TEXT_PRIMARY)))
    }))
}
