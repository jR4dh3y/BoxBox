//! `org.gtk.vfs.MountOperation`: the object a GVfs backend calls while it
//! mounts, to ask for a password or a yes/no decision. Each question goes to
//! the window as a `Prompt` and waits for its answer.

use async_channel::{Receiver, Sender};

/// What the user typed. Fields the question did not ask for stay empty.
#[derive(Clone, Debug, Default)]
pub struct Answer {
    pub user: String,
    pub domain: String,
    pub password: String,
}

/// A question from a backend. Dropping `reply` cancels the mount.
pub enum Prompt {
    Password {
        message: String,
        /// The user name to offer, when one is needed.
        user: Option<String>,
        domain: Option<String>,
        reply: Sender<Answer>,
    },
    /// Such as an unknown SSH host key: `choices` are the buttons, first is "go on".
    Question { message: String, choices: Vec<String>, reply: Sender<u32> },
}

pub type Prompts = Sender<Prompt>;

// GAskPasswordFlags
const NEED_USERNAME: u32 = 1 << 1;
const NEED_DOMAIN: u32 = 1 << 2;

pub struct MountOperation {
    pub prompts: Prompts,
}

impl MountOperation {
    /// None when the window is gone or the user cancelled.
    async fn ask<T>(&self, prompt: Prompt, reply: Receiver<T>) -> Option<T> {
        self.prompts.send(prompt).await.ok()?;
        reply.recv().await.ok()
    }
}

#[zbus::interface(name = "org.gtk.vfs.MountOperation")]
impl MountOperation {
    /// Returns handled, aborted, password, user, domain, anonymous and how long to keep it (never).
    #[zbus(name = "AskPassword")]
    async fn ask_password(&self, message: String, default_user: String, default_domain: String, flags: u32) -> (bool, bool, String, String, String, bool, u32) {
        let (reply, answer) = async_channel::bounded(1);
        let prompt = Prompt::Password {
            message,
            user: (flags & NEED_USERNAME != 0).then_some(default_user),
            domain: (flags & NEED_DOMAIN != 0).then_some(default_domain),
            reply,
        };
        match self.ask(prompt, answer).await {
            Some(answer) => (true, false, answer.password, answer.user, answer.domain, false, 0),
            None => (true, true, String::new(), String::new(), String::new(), false, 0),
        }
    }

    /// Returns handled, aborted and the chosen button.
    #[zbus(name = "AskQuestion")]
    async fn ask_question(&self, message: String, choices: Vec<String>) -> (bool, bool, u32) {
        let (reply, answer) = async_channel::bounded(1);
        match self.ask(Prompt::Question { message, choices, reply }, answer).await {
            Some(choice) => (true, false, choice),
            None => (true, true, 0),
        }
    }
}
