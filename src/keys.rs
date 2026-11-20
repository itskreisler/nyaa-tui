use ratatui::crossterm::event::KeyCode::{
    self, Backspace, Char, Delete, Down, End, Enter, Esc, Home, Left, Right, Up,
};

use crate::nyaa::Field;

#[derive(Clone, Copy)]
pub enum Action {
    Down,
    Up,
    Top,
    Bottom,
    Focus(Field),
    Uploader,
    Filter,
    Sort,
    Zen,
    Copy,
    Download,
    Reload,
    Details,
    Close,
    Quit,
    Submit,
    Cancel,
    Delete,
    Remove,
    Left,
    Right,
    Home,
    End,
}

/// Keys that trigger `action`, the first one is shown in hints.
pub struct Bind {
    pub keys: &'static [KeyCode],
    pub hint: &'static str,
    pub action: Action,
}

const fn bind(keys: &'static [KeyCode], hint: &'static str, action: Action) -> Bind {
    Bind { keys, hint, action }
}

const COPY: Bind = bind(&[Char('c')], "copy magnet", Action::Copy);
const DOWNLOAD: Bind = bind(&[Char('d')], "download", Action::Download);
const RELOAD: Bind = bind(&[Char('r')], "reload", Action::Reload);

/// Header binds that change the query fields, then footer binds for the rest.
pub type Keymap = &'static [&'static [Bind]; 2];

pub const NORMAL: Keymap = &[
    &[
        bind(&[Char('/')], "search", Action::Focus(Field::Text)),
        bind(&[Char('u')], "uploader", Action::Uploader),
        bind(&[Char('U')], "type uploader", Action::Focus(Field::User)),
        bind(&[Char('f')], "filter", Action::Filter),
        bind(&[Char('s')], "sort", Action::Sort),
    ],
    &[
        bind(&[Char('j'), Down], "down", Action::Down),
        bind(&[Char('k'), Up], "up", Action::Up),
        bind(&[Char('g'), Home], "top", Action::Top),
        bind(&[Char('G'), End], "bottom", Action::Bottom),
        bind(&[Enter], "details", Action::Details),
        COPY,
        DOWNLOAD,
        RELOAD,
        bind(&[Char('z')], "zen", Action::Zen),
        bind(&[Char('q'), Esc], "quit", Action::Quit),
    ],
];

pub const DETAILS: Keymap = &[
    &[],
    &[
        bind(&[Esc, Enter], "close", Action::Close),
        COPY,
        DOWNLOAD,
        bind(&[Char('q')], "quit", Action::Quit),
    ],
];

pub const SEARCH: Keymap = &[
    &[
        bind(&[Enter], "search", Action::Submit),
        bind(&[Esc], "cancel", Action::Cancel),
        bind(&[Backspace], "delete", Action::Delete),
    ],
    &[
        bind(&[Left], "left", Action::Left),
        bind(&[Right], "right", Action::Right),
        bind(&[Home], "start", Action::Home),
        bind(&[End], "end", Action::End),
        bind(&[Delete], "remove", Action::Remove),
    ],
];
