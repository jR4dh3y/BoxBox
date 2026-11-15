//! `Select` with its option list: the closed control, and a menu under it while open.

use gpui::{anchored, deferred, div, point, prelude::*, px, AnchoredPositionMode, Context, SharedString};

use crate::components::{ContextMenu, MenuItem, Select};
use crate::workspace::Workspace;

impl Workspace {
    /// A `width`-wide select over `options`. `on_change` runs with the picked value.
    pub(crate) fn select<T: Copy + PartialEq + 'static>(
        &self,
        id: &'static str,
        value: T,
        options: &'static [(T, &'static str)],
        width: f32,
        cx: &mut Context<Self>,
        on_change: fn(&mut Workspace, T, &mut Context<Workspace>),
    ) -> impl IntoElement {
        let label = options.iter().find(|(option, _)| *option == value).map_or("", |(_, label)| label);
        let open = self.open_select == Some(id);
        div()
            .relative()
            .w(px(width))
            .debug_selector(move || id.to_string())
            // Toggles on press. A press on the control while the list is open first closes
            // it as a press outside the list; that same press must not open it again.
            .child(Select::new(id, label).on_press(cx.listener(move |this, _, _, cx| {
                this.open_select = match (this.open_select, this.select_dismissed.take()) {
                    (_, Some(dismissed)) if dismissed == id => None,
                    (Some(open), _) if open == id => None,
                    _ => Some(id),
                };
                cx.notify();
            })))
            .when(open, |slot| {
                let items = options
                    .iter()
                    .enumerate()
                    .map(|(index, (option, label))| {
                        let option = *option;
                        let item_id = SharedString::from(format!("{id}-{index}"));
                        let item = MenuItem::action(item_id, *label);
                        // The current value carries a tick; the others keep the icon slot empty.
                        let item = if option == value { item.icon("check") } else { item };
                        item.on_select(cx.listener(move |this, _, _, cx| {
                                this.open_select = None;
                                on_change(this, option, cx);
                                cx.notify();
                            }))
                    })
                    .collect();
                // From the slot's top-left corner, not from below the control, where the
                // menu would otherwise sit in the flow.
                slot.child(div().absolute().top_0().left_0().child(
                    deferred(
                        anchored().position_mode(AnchoredPositionMode::Local).position(point(px(0.), px(34.))).snap_to_window().child(
                            div()
                                .id(SharedString::from(format!("{id}-menu")))
                                .occlude()
                                .min_w(px(width))
                                .on_mouse_down_out(cx.listener(move |this, _, _, cx| {
                                    this.open_select = None;
                                    this.select_dismissed = Some(id);
                                    // Only this press counts; later ones open the list again.
                                    let workspace = cx.entity().downgrade();
                                    cx.defer(move |cx| {
                                        workspace.update(cx, |this, _| this.select_dismissed = None).ok();
                                    });
                                    cx.notify();
                                }))
                                .child(ContextMenu::new(items)),
                        ),
                    )
                    .with_priority(2),
                ))
            })
    }
}
