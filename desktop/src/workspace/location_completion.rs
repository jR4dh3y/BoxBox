//! Tab completion for the location field, as a terminal completes a path: the
//! folders inside the typed one whose names start with what follows its last
//! separator. Tab fills as far as they agree, then steps through them.

use std::{fs, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Result;
use gpui::{Context, Window};

use super::Workspace;
use crate::api::Api;

/// The folders the list under the field shows before it scrolls.
pub(crate) const SHOWN: usize = 8;

/// The folders that can follow what is typed.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Completion {
    /// The text up to and including its last separator.
    pub base: String,
    /// What follows it, which every name starts with.
    pub prefix: String,
    pub names: Vec<String>,
    /// The name Tab put in the field, while stepping through them.
    pub index: Option<usize>,
}

impl Completion {
    pub fn new(text: &str, folders: Vec<String>, show_hidden: bool) -> Self {
        let (base, prefix) = split(text);
        Self { base: base.to_owned(), prefix: prefix.to_owned(), names: matching(folders, prefix, show_hidden), index: None }
    }

    /// Whether it was made for `text`, or `text` is a name Tab stepped to.
    pub fn is_for(&self, text: &str) -> bool {
        text == format!("{}{}", self.base, self.prefix) || self.stepped_to(text)
    }

    pub fn stepped_to(&self, text: &str) -> bool {
        self.index.is_some_and(|index| text == format!("{}{}", self.base, self.names[index]))
    }

    /// The field's text after Tab (Shift+Tab when `back`): the one folder, then
    /// as far as they agree, then each in turn.
    pub fn next(&mut self, text: &str, back: bool) -> Option<String> {
        let count = self.names.len();
        if count == 0 {
            return None;
        }
        if self.stepped_to(text) {
            return self.step(back);
        }
        if let [name] = self.names.as_slice() {
            return Some(format!("{}{name}{}", self.base, separator(&self.base)));
        }
        let shared = common_prefix(&self.names);
        if shared.chars().count() > self.prefix.chars().count() {
            return Some(format!("{}{shared}", self.base));
        }
        self.step(back)
    }

    /// The field's text after Down (Up when `back`): the next name in the list,
    /// from either end, wrapping around.
    pub fn step(&mut self, back: bool) -> Option<String> {
        let count = self.names.len();
        if count == 0 {
            return None;
        }
        let index = match (self.index, back) {
            (Some(index), false) => (index + 1) % count,
            (Some(index), true) => (index + count - 1) % count,
            (None, false) => 0,
            (None, true) => count - 1,
        };
        self.index = Some(index);
        Some(format!("{}{}", self.base, self.names[index]))
    }
}

/// `/home/me/Dow` into `/home/me/` and `Dow`.
pub(crate) fn split(text: &str) -> (&str, &str) {
    let cut = text.rfind(|ch| ch == '/' || (cfg!(windows) && ch == '\\')).map_or(0, |at| at + 1);
    text.split_at(cut)
}

/// The separator the typed path already uses, for after a completed folder.
fn separator(base: &str) -> char {
    if cfg!(windows) && base.contains('\\') && !base.contains('/') { '\\' } else { '/' }
}

/// The names starting with `prefix`, by case when any do, else ignoring it, in
/// name order. Hidden ones only when shown or asked for with a leading dot.
fn matching(folders: Vec<String>, prefix: &str, show_hidden: bool) -> Vec<String> {
    let visible = |name: &String| show_hidden || prefix.starts_with('.') || !name.starts_with('.');
    let folders: Vec<String> = folders.into_iter().filter(visible).collect();
    let exact: Vec<&String> = folders.iter().filter(|name| name.starts_with(prefix)).collect();
    let mut names: Vec<String> = if exact.is_empty() {
        let lower = prefix.to_lowercase();
        folders.iter().filter(|name| name.to_lowercase().starts_with(&lower)).cloned().collect()
    } else {
        exact.into_iter().cloned().collect()
    };
    names.sort_by_key(|name| name.to_lowercase());
    names
}

fn common_prefix(names: &[String]) -> String {
    let Some((first, rest)) = names.split_first() else { return String::new() };
    let mut end = first.len();
    for name in rest {
        end = first.char_indices().zip(name.chars()).take_while(|((_, a), b)| a == b).last().map_or(0, |((at, ch), _)| at + ch.len_utf8()).min(end);
    }
    first[..end].to_owned()
}

/// `~` and `~/…` on this computer.
pub(crate) fn expand_home(text: &str) -> String {
    match (text.strip_prefix('~'), std::env::home_dir()) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\') => format!("{}{rest}", home.display()),
        _ => text.to_owned(),
    }
}

/// The folders in `base`: a real folder for a typed real path on this
/// computer, else the provider's own folder, or its drives at the top.
fn read_folders(api: &Api, base: &str) -> Result<Vec<String>> {
    let real = PathBuf::from(expand_home(base));
    if api.is_local() && real.is_absolute() {
        let entries = fs::read_dir(&real)?.flatten();
        let folders = entries.filter(|entry| entry.path().is_dir()).filter_map(|entry| entry.file_name().into_string().ok());
        return Ok(folders.collect());
    }
    let folder = base.trim_end_matches('/');
    if folder.is_empty() {
        return Ok(api.roots()?.into_iter().map(|root| root.name).collect());
    }
    Ok(api.list(folder)?.into_iter().filter(|item| item.is_dir).map(|item| item.name).collect())
}

impl Workspace {
    /// Lists what can follow the field's text, after a pause in typing, then
    /// `then` runs with it when the text has not changed meanwhile.
    pub(crate) fn refresh_location_completion(&mut self, delay: Duration, window: &Window, cx: &mut Context<Self>, then: Option<fn(&mut Self, &mut Window, &mut Context<Self>)>) {
        let Some(editor) = self.location_editor.as_ref() else { return };
        let Some(api) = self.pane_by_id(editor.target_id).map(|pane| pane.api.clone()) else { return };
        let text = self.location_field.read(cx).text().to_owned();
        self.completion_generation += 1;
        let generation = self.completion_generation;
        let show_hidden = self.preferences.show_hidden_files;
        cx.spawn_in(window, async move |this, cx| {
            if !delay.is_zero() {
                cx.background_executor().timer(delay).await;
            }
            let (base, _) = split(&text);
            let (api, base): (Arc<Api>, String) = (api, base.to_owned());
            let folders = cx.background_executor().spawn(async move { read_folders(&api, &base) }).await;
            this.update_in(cx, |this, window, cx| {
                if this.completion_generation != generation || this.location_field.read(cx).text() != text {
                    return;
                }
                this.location_completion = folders.ok().map(|folders| Completion::new(&text, folders, show_hidden));
                // A new list starts at its top.
                this.location_scroll.set_offset(gpui::Point::default());
                if let Some(then) = then {
                    then(this, window, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Tab in the location field; Shift+Tab steps back.
    pub(crate) fn complete_location(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        let then = if back { Self::complete_location_back } else { Self::complete_location_forward };
        self.move_in_completion(Completion::next, back, then, window, cx);
    }

    /// Down in the location field picks the next folder in the list; Up the one before.
    pub(crate) fn step_location(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) {
        let then = if back { Self::step_location_back } else { Self::step_location_forward };
        let step = |completion: &mut Completion, _: &str, back| completion.step(back);
        self.move_in_completion(step, back, then, window, cx);
    }

    /// Applies `pick` to the folders for the field's text, listing them first
    /// when they are not yet known, then `retry` runs once they are.
    fn move_in_completion(
        &mut self,
        pick: impl FnOnce(&mut Completion, &str, bool) -> Option<String>,
        back: bool,
        retry: fn(&mut Self, &mut Window, &mut Context<Self>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = self.location_field.read(cx).text().to_owned();
        let Some(completion) = self.location_completion.as_mut().filter(|completion| completion.is_for(&text)) else {
            self.refresh_location_completion(Duration::ZERO, window, cx, Some(retry));
            return;
        };
        let Some(next) = pick(completion, &text, back) else { return };
        let stepping = completion.stepped_to(&next);
        if let Some(index) = completion.index.filter(|_| stepping) {
            self.location_scroll.scroll_to_item(index);
        }
        self.fill_location(next, !stepping, window, cx);
    }

    fn complete_location_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.complete_location(false, window, cx);
    }

    fn complete_location_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.complete_location(true, window, cx);
    }

    fn step_location_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step_location(false, window, cx);
    }

    fn step_location_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.step_location(true, window, cx);
    }

    /// Puts `text` in the field with the cursor at its end. `relist` reads the
    /// folders for it, as after entering a folder.
    pub(crate) fn fill_location(&mut self, text: String, relist: bool, window: &mut Window, cx: &mut Context<Self>) {
        let end = text.len();
        self.location_field.update(cx, |field, cx| field.set_text(text, end..end, cx));
        if relist {
            self.refresh_location_completion(Duration::ZERO, window, cx, None);
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::{common_prefix, split, Completion};

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn tab_fills_the_one_folder_then_as_far_as_they_agree_then_steps() {
        let mut one = Completion::new("/home/me/Dow", names(&["Downloads", "Documents", ".cache"]), false);
        assert_eq!(one.next("/home/me/Dow", false).as_deref(), Some("/home/me/Downloads/"));

        let mut many = Completion::new("/home/me/D", names(&["Downloads", "Documents", "Desktop"]), false);
        assert_eq!(many.names, ["Desktop", "Documents", "Downloads"]);
        // Nothing more they share, so Tab steps, and Shift+Tab goes back.
        assert_eq!(many.next("/home/me/D", false).as_deref(), Some("/home/me/Desktop"));
        assert_eq!(many.next("/home/me/Desktop", false).as_deref(), Some("/home/me/Documents"));
        assert_eq!(many.next("/home/me/Documents", true).as_deref(), Some("/home/me/Desktop"));
        assert!(many.is_for("/home/me/Desktop") && !many.is_for("/home/me/Desk"));

        let mut shared = Completion::new("/srv/Pro", names(&["Projects-old", "Projects-new"]), false);
        assert_eq!(shared.next("/srv/Pro", false).as_deref(), Some("/srv/Projects-"));
    }

    #[test]
    fn down_and_up_step_from_either_end_and_wrap() {
        let mut list = Completion::new("/home/me/", names(&["a", "b", "c"]), false);
        assert_eq!(list.step(true).as_deref(), Some("/home/me/c"));
        assert_eq!(list.step(false).as_deref(), Some("/home/me/a"));
        assert_eq!(list.step(false).as_deref(), Some("/home/me/b"));
        assert!(list.stepped_to("/home/me/b") && list.is_for("/home/me/b"));
    }

    #[test]
    fn case_matters_only_when_something_matches_by_it_and_hidden_ones_need_a_dot() {
        assert_eq!(Completion::new("~/dow", names(&["Downloads"]), false).names, ["Downloads"]);
        assert_eq!(Completion::new("~/D", names(&["Downloads", "dev"]), false).names, ["Downloads"]);
        assert!(Completion::new("~/", names(&[".cache"]), false).names.is_empty());
        assert_eq!(Completion::new("~/.c", names(&[".cache"]), false).names, [".cache"]);
        assert_eq!(Completion::new("~/", names(&[".cache"]), true).names, [".cache"]);
    }

    #[test]
    fn paths_split_at_their_last_separator() {
        assert_eq!(split("/home/me/Dow"), ("/home/me/", "Dow"));
        assert_eq!(split("home"), ("", "home"));
        assert_eq!(common_prefix(&names(&["Résumé-a", "Résumé-b"])), "Résumé-");
    }
}
