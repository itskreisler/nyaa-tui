use std::sync::mpsc::{self, Sender};
use std::{io, iter, thread};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::widgets::TableState;

use crate::keys::{Action, DETAILS, Keymap, NORMAL, SEARCH};
use crate::nyaa::{self, Field, Filter, Query, Sort, Torrent, UPLOADERS};
use crate::{os, ui};

/// Outcome shown next to the results badge until the next key press.
pub type Note = Result<String, String>;

pub enum Msg {
    Input(Event),
    Results(Query, Result<Vec<Torrent>, String>),
    Note(Note),
}

#[derive(Default)]
pub enum Status {
    #[default]
    Loading,
    Done,
    Failed(String),
}

#[derive(Default)]
pub struct App {
    pub query: Query,
    /// Draft of the field being edited, `Some` while editing.
    pub input: Option<(Field, String)>,
    pub torrents: Vec<Torrent>,
    pub table: TableState,
    pub status: Status,
    pub note: Option<Note>,
    pub details: bool,
    pub sort: Sort,
    quit: bool,
}

impl App {
    pub fn run(mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let (tx, rx) = mpsc::channel();
        let input = tx.clone();
        thread::spawn(move || {
            iter::repeat_with(event::read)
                .map_while(Result::ok)
                .try_for_each(|event| input.send(Msg::Input(event)))
        });
        self.search(&tx);
        while !self.quit {
            terminal.draw(|frame| ui::render(frame, &mut self))?;
            let Ok(msg) = rx.recv() else { break };
            let before = self.query.clone();
            self.update(msg, &tx);
            (self.query != before).then(|| self.search(&tx));
        }
        Ok(())
    }

    pub const fn keys(&self) -> Keymap {
        match (&self.input, self.details) {
            (Some(_), _) => SEARCH,
            (_, true) => DETAILS,
            _ => NORMAL,
        }
    }

    pub fn selected(&self) -> Option<&Torrent> {
        self.table.selected().and_then(|at| self.torrents.get(at))
    }

    fn search(&mut self, tx: &Sender<Msg>) {
        self.status = Status::Loading;
        let (query, tx) = (self.query.clone(), tx.clone());
        thread::spawn(move || {
            let results = nyaa::search(&query).map_err(|err| err.to_string());
            tx.send(Msg::Results(query, results))
        });
    }

    fn copy(&mut self) {
        self.note = self.selected().map(|torrent| {
            os::copy(&torrent.magnet())
                .map(|()| "copied magnet".into())
                .map_err(|err| err.to_string())
        });
    }

    fn download(&mut self, tx: &Sender<Msg>) {
        let Some(torrent) = self.selected() else {
            return;
        };
        let (link, name, tx) = (
            torrent.link(),
            format!("{}.torrent", torrent.id),
            tx.clone(),
        );
        self.note = Some(Ok("downloading".into()));
        thread::spawn(move || {
            let saved = nyaa::torrent(&link)
                .and_then(|bytes| Ok(os::save(&name, &bytes)?))
                .map(|path| format!("saved {}", path.display()))
                .map_err(|err| err.to_string());
            tx.send(Msg::Note(saved))
        });
    }

    fn update(&mut self, msg: Msg, tx: &Sender<Msg>) {
        match msg {
            Msg::Input(event) => self.handle(&event, tx),
            Msg::Results(query, _) if query != self.query => {}
            Msg::Results(_, Ok(torrents)) => self.load(torrents),
            Msg::Results(_, Err(err)) => self.status = Status::Failed(err),
            Msg::Note(note) => self.note = Some(note),
        }
    }

    fn load(&mut self, torrents: Vec<Torrent>) {
        self.torrents = torrents;
        self.status = Status::Done;
        self.resort();
    }

    fn resort(&mut self) {
        self.sort.apply(&mut self.torrents);
        self.table = TableState::default().with_selected((!self.torrents.is_empty()).then_some(0));
    }

    fn handle(&mut self, event: &Event, tx: &Sender<Msg>) {
        let Some(key) = event.as_key_press_event() else {
            return;
        };
        self.note = None;
        let bind = self
            .keys()
            .iter()
            .copied()
            .flatten()
            .find(|bind| bind.keys.contains(&key.code));
        match (bind, key.code) {
            (Some(bind), _) => self.apply(bind.action, tx),
            (None, KeyCode::Char(c)) => self.input.as_mut().map_or((), |(_, text)| text.push(c)),
            _ => {}
        }
    }

    fn apply(&mut self, action: Action, tx: &Sender<Msg>) {
        let query = &mut self.query;
        match action {
            Action::Down => self.table.select_next(),
            Action::Up => self.table.select_previous(),
            Action::Top => self.table.select_first(),
            Action::Bottom => self.table.select_last(),
            Action::Focus(field) => self.input = Some((field, query.field(field).clone())),
            Action::Uploader => query.user = nyaa::next(UPLOADERS, query.user.as_str()).into(),
            Action::Filter => query.filter = nyaa::next(&Filter::ALL, query.filter),
            Action::Sort => {
                self.sort = nyaa::next(&Sort::ALL, self.sort);
                self.resort();
            }
            Action::Copy => self.copy(),
            Action::Download => self.download(tx),
            Action::Details => self.details = self.selected().is_some(),
            Action::Close => self.details = false,
            Action::Quit => self.quit = true,
            Action::Submit => {
                if let Some((field, text)) = self.input.take() {
                    *query.field(field) = text;
                }
            }
            Action::Cancel => self.input = None,
            Action::Delete => {
                self.input.as_mut().and_then(|(_, text)| text.pop());
            }
        }
    }
}
