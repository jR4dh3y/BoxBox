//! `--gallery`: every shared component, in the cells and states of
//! `frontend/src/routes/dev/gallery/gallery.json`, which the web app's
//! `/dev/gallery` renders too. `scripts/compare-gallery.py` diffs the two.

use gpui::{div, prelude::*, px, Context, SharedString, Window};
use serde::Deserialize;
use serde_json::Value;

use crate::components::{
    Button, ButtonSize, ButtonVariant, ContextMenu, Input, MenuItem, Modal, ProgressBar, ProgressSize,
    ProgressVariant, Select, Toast, ToastKind, Toggle,
};
use crate::theme::*;

const MANIFEST: &str = include_str!("../../../frontend/src/routes/dev/gallery/gallery.json");

#[derive(Deserialize)]
pub struct Manifest {
    pub width: f32,
    pub height: f32,
    cells: Vec<Cell>,
}

#[derive(Deserialize)]
struct Cell {
    id: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    kind: String,
    props: Value,
}

pub fn manifest() -> Manifest {
    serde_json::from_str(MANIFEST).expect("gallery.json is valid")
}

pub struct Gallery {
    manifest: Manifest,
    font: SharedString,
}

impl Gallery {
    pub fn new(font: SharedString) -> Self {
        Self { manifest: manifest(), font }
    }
}

fn text(props: &Value, key: &str) -> String {
    props.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn flag(props: &Value, key: &str) -> bool {
    props.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn render_cell(cell: &Cell) -> gpui::AnyElement {
    let p = &cell.props;
    let id = SharedString::from(cell.id.clone());
    match cell.kind.as_str() {
        "button" => {
            let variant = match text(p, "variant").as_str() {
                "secondary" => ButtonVariant::Secondary,
                "ghost" => ButtonVariant::Ghost,
                "danger" => ButtonVariant::Danger,
                _ => ButtonVariant::Primary,
            };
            let size = match text(p, "size").as_str() {
                "sm" => ButtonSize::Sm,
                "lg" => ButtonSize::Lg,
                "icon" => ButtonSize::Icon,
                _ => ButtonSize::Md,
            };
            let mut button = Button::new(id).variant(variant).size(size).disabled(flag(p, "disabled"));
            if let Some(icon) = p.get("icon").and_then(Value::as_str) {
                button = button.icon(icon.to_string());
            }
            if let Some(label) = p.get("label").and_then(Value::as_str) {
                button = button.label(label.to_string());
            }
            div().flex().child(button).into_any_element()
        }
        "input" => Input::new(text(p, "value"))
            .placeholder(text(p, "placeholder"))
            .disabled(flag(p, "disabled"))
            .into_any_element(),
        "toggle" => div()
            .flex()
            .child(Toggle::new(id, flag(p, "checked")).disabled(flag(p, "disabled")))
            .into_any_element(),
        "select" => Select::new(id, text(p, "value"))
            .swatch(p.get("swatch").and_then(Value::as_str).and_then(|hex| u32::from_str_radix(hex.trim_start_matches('#'), 16).ok()))
            .into_any_element(),
        "progress" => ProgressBar::new(p.get("value").and_then(Value::as_f64).unwrap_or_default() as f32)
            .size(if text(p, "size") == "sm" { ProgressSize::Sm } else { ProgressSize::Md })
            .variant(match text(p, "variant").as_str() {
                "info" => ProgressVariant::Info,
                "success" => ProgressVariant::Success,
                "warning" => ProgressVariant::Warning,
                "danger" => ProgressVariant::Danger,
                _ => ProgressVariant::Default,
            })
            .into_any_element(),
        "context-menu" => {
            let items = p
                .get("items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|item| {
                    if flag(item, "separator") {
                        return MenuItem::Separator;
                    }
                    let mut entry = MenuItem::action(SharedString::from(text(item, "id")), text(item, "label"))
                        .disabled(flag(item, "disabled"));
                    if let Some(icon) = item.get("icon").and_then(Value::as_str) {
                        entry = entry.icon(icon.to_string());
                    }
                    if let Some(keys) = item.get("shortcut").and_then(Value::as_str) {
                        entry = entry.shortcut(keys.to_string());
                    }
                    entry
                })
                .collect();
            div().flex().items_start().child(ContextMenu::new(items)).into_any_element()
        }
        // The web dialog focuses its field when it opens.
        "modal" => Modal::new()
            .title(text(p, "title"))
            .body(Input::new(text(p, "value")).focused(true))
            .footer(Button::new("modal-cancel").variant(ButtonVariant::Secondary).label(text(p, "cancel")))
            .footer(Button::new("modal-confirm").label(text(p, "confirm")))
            .into_any_element(),
        "toasts" => div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .w_full()
            .children(p.get("toasts").and_then(Value::as_array).into_iter().flatten().enumerate().map(|(index, toast)| {
                let kind = match text(toast, "type").as_str() {
                    "error" => ToastKind::Error,
                    "info" => ToastKind::Info,
                    "warning" => ToastKind::Warning,
                    _ => ToastKind::Success,
                };
                Toast::new(SharedString::from(format!("toast-{index}")), kind, text(toast, "message"))
            }))
            .into_any_element(),
        other => div().text_color(c(DANGER)).child(format!("unknown kind {other}")).into_any_element(),
    }
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(self.manifest.width))
            .h(px(self.manifest.height))
            .bg(c(SURFACE_PRIMARY))
            .font_family(self.font.clone())
            .children(self.manifest.cells.iter().map(|cell| {
                div()
                    .absolute()
                    .left(px(cell.x))
                    .top(px(cell.y))
                    .w(px(cell.w))
                    .h(px(cell.h))
                    .overflow_hidden()
                    .child(render_cell(cell))
            }))
    }
}
