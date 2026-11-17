//! Command-line arguments. Anything starting with `-` is an option, so
//! `--help` prints help instead of opening a folder named `--help`.

use std::ffi::OsString;

pub const USAGE: &str = "\
Usage: boxbox-files [OPTIONS] [LOCATION]

Opens LOCATION: a folder, a file:// URI, or a file (opens its folder).
If BoxBox is already running, the location opens in that window.

Options:
  -h, --help                     Print this help and exit
  -V, --version                  Print the version and exit
      --enable-picker            Use BoxBox for other apps' Open and Save dialogs
      --disable-picker           Go back to the previous Open and Save dialogs
      --portal                   Serve the Open and Save dialogs (D-Bus starts this)
      --profile-summary <TRACE>  Summarise a BOXBOX_PROFILE trace and exit
      --                         Treat the next argument as a location,
                                 even if it starts with '-'

Environment:
  BOXBOX_URL      Browse a BoxBox server instead of local files
  BOXBOX_TOKEN    Bearer token for BOXBOX_URL
  BOXBOX_ROOT     Use this folder as the local root (for fixtures)
  BOXBOX_PROFILE  Record a profiling trace to this file
";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// Open the window at a location; empty means the default start.
    Open(String),
    Gallery,
    /// Serve the file chooser portal.
    Portal,
    /// Route other apps' file dialogs to BoxBox (`true`), or back.
    SetPicker(bool),
    ProfileSummary(String),
    Help,
    Version,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Error(pub String);

/// Parses the arguments after the program name.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, Error> {
    let mut args = args.into_iter().map(|arg| arg.into_string().map_err(|arg| Error(format!("argument is not valid UTF-8: {}", arg.to_string_lossy()))));
    let mut location: Option<String> = None;
    let set_location = |value: String, location: &mut Option<String>| match location {
        Some(_) => Err(Error(format!("unexpected extra argument '{value}'"))),
        None => {
            *location = Some(value);
            Ok(())
        }
    };
    while let Some(arg) = args.next() {
        let arg = arg?;
        match arg.as_str() {
            "-h" | "--help" | "-help" | "help" => return Ok(Command::Help),
            "-V" | "--version" => return Ok(Command::Version),
            "--gallery" => return Ok(Command::Gallery),
            "--portal" => return Ok(Command::Portal),
            "--enable-picker" => return Ok(Command::SetPicker(true)),
            "--disable-picker" => return Ok(Command::SetPicker(false)),
            "--profile-summary" => {
                let trace = args.next().transpose()?.ok_or_else(|| Error("--profile-summary needs a trace file".into()))?;
                return Ok(Command::ProfileSummary(trace));
            }
            "--" => {
                for rest in args.by_ref() {
                    set_location(rest?, &mut location)?;
                }
            }
            option if option.starts_with('-') && option != "-" => return Err(Error(format!("unknown option '{option}'"))),
            _ => set_location(arg, &mut location)?,
        }
    }
    Ok(Command::Open(location.unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<Command, Error> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn help_and_version_never_become_locations() {
        for help in ["--help", "-h", "help"] {
            assert_eq!(parse_strs(&[help]), Ok(Command::Help));
        }
        assert_eq!(parse_strs(&["--version"]), Ok(Command::Version));
        assert_eq!(parse_strs(&["/tmp", "--help"]), Ok(Command::Help));
    }

    #[test]
    fn locations_open() {
        assert_eq!(parse_strs(&[]), Ok(Command::Open(String::new())));
        assert_eq!(parse_strs(&["/home/me"]), Ok(Command::Open("/home/me".into())));
        assert_eq!(parse_strs(&["--", "--help"]), Ok(Command::Open("--help".into())));
        // A folder named "help" is still reachable by path.
        assert_eq!(parse_strs(&["./help"]), Ok(Command::Open("./help".into())));
    }

    #[test]
    fn bad_input_is_refused() {
        assert!(parse_strs(&["--nope"]).is_err());
        assert!(parse_strs(&["/a", "/b"]).is_err());
        assert!(parse_strs(&["--profile-summary"]).is_err());
    }
}
