use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::{env, fs};

/// Clipboard commands tried in order, the first one that succeeds wins.
const CLIPBOARDS: [&[&str]; 4] = [
    &["wl-copy"],
    &["xclip", "-selection", "clipboard"],
    &["pbcopy"],
    &["clip"],
];

/// URL openers tried in order, the first one that succeeds wins.
const OPENERS: [&[&str]; 2] = [&["xdg-open"], &["open"]];

/// Termux paths, also visible from proot-distro.
const TERMUX_HOME: &str = "/data/data/com.termux/files/home";
const TERMUX_CLIPBOARD: &str = "/data/data/com.termux/files/usr/bin/termux-clipboard-set";
const TERMUX_OPEN_URL: &str = "/data/data/com.termux/files/usr/bin/termux-open-url";
const TERMUX_CLIPBOARD_CMD: &[&str] = &[TERMUX_CLIPBOARD];
const TERMUX_OPEN_URL_CMD: &[&str] = &[TERMUX_OPEN_URL];

/// Equivalent to checking `test -d $TERMUX_HOME` and `test -f $TERMUX_CLIPBOARD`.
fn has_termux() -> bool {
    Path::new(TERMUX_HOME).is_dir() && Path::new(TERMUX_CLIPBOARD).is_file()
}

fn spawn(argv: &[&str]) -> io::Result<Child> {
    Command::new(argv[0])
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

/// Pipes `text` into `argv`, true when it exits cleanly.
fn pipe(argv: &[&str], text: &str) -> bool {
    spawn(argv).is_ok_and(|mut child| {
        let written = child
            .stdin
            .take()
            .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
        child.wait().is_ok_and(|status| written && status.success())
    })
}

/// Runs `argv` with `arg`, true when it exits cleanly.
fn run(argv: &[&str], arg: &str) -> bool {
    Command::new(argv[0])
        .args(&argv[1..])
        .arg(arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn copy(text: &str) -> io::Result<()> {
    let mut clipboards = Vec::with_capacity(CLIPBOARDS.len() + 1);
    if has_termux() {
        clipboards.push(TERMUX_CLIPBOARD_CMD);
    }
    clipboards.extend_from_slice(&CLIPBOARDS);
    clipboards
        .iter()
        .any(|argv| pipe(argv, text))
        .then_some(())
        .ok_or_else(|| io::Error::other("no working clipboard command"))
}

pub fn open_url(url: &str) -> io::Result<()> {
    let mut openers = Vec::with_capacity(OPENERS.len() + 1);
    if has_termux() {
        openers.push(TERMUX_OPEN_URL_CMD);
    }
    openers.extend_from_slice(&OPENERS);
    openers
        .iter()
        .any(|argv| run(argv, url))
        .then_some(())
        .ok_or_else(|| io::Error::other("no working url opener"))
}

fn home() -> String {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap_or_default()
}

/// XDG download dir, `$HOME/Downloads` (or `%USERPROFILE%`) when `xdg-user-dir` is missing.
fn downloads() -> PathBuf {
    Command::new("xdg-user-dir")
        .arg("DOWNLOAD")
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| PathBuf::from(home()).join("Downloads"), PathBuf::from)
}

pub fn save(name: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    let dir = downloads();
    fs::create_dir_all(&dir)?;
    let path = dir.join(name);
    fs::write(&path, bytes)?;
    Ok(path)
}
