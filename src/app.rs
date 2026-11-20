use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::Duration;
use std::{io, iter, thread};

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode};
use ratatui::widgets::TableState;

use crate::keys::{Action, DETAILS, Keymap, NORMAL, SEARCH};
use crate::nyaa::{self, Field, Filter, Query, Sort, Torrent, UPLOADERS};
use crate::{os, ui};

/// Outcome shown next to the results badge until the next key press.
pub type Note = Result<String, String>;

const TICK: Duration = Duration::from_millis(80);

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

/// Field being typed. `at` is a byte offset on a char boundary.
pub struct Draft {
    pub field: Field,
    pub text: String,
    pub at: usize,
}

impl Draft {
    fn open(field: Field, text: String) -> Self {
        Self {
            field,
            at: text.len(),
            text,
        }
    }

    fn insert(&mut self, c: char) {
        self.text.insert(self.at, c);
        self.at += c.len_utf8();
    }

    fn backspace(&mut self) {
        let Some(c) = self.text[..self.at].chars().next_back() else {
            return;
        };
        self.at -= c.len_utf8();
        self.text.remove(self.at);
    }

    fn remove(&mut self) {
        let Some(_) = self.text[self.at..].chars().next() else {
            return;
        };
        self.text.remove(self.at);
    }

    fn left(&mut self) {
        let Some(c) = self.text[..self.at].chars().next_back() else {
            return;
        };
        self.at -= c.len_utf8();
    }

    fn right(&mut self) {
        let Some(c) = self.text[self.at..].chars().next() else {
            return;
        };
        self.at += c.len_utf8();
    }

    fn home(&mut self) {
        self.at = 0;
    }

    fn end(&mut self) {
        self.at = self.text.len();
    }
}

#[derive(Default)]
pub struct App {
    pub query: Query,
    /// Draft of the field being edited, `Some` while editing.
    pub input: Option<Draft>,
    pub torrents: Vec<Torrent>,
    pub table: TableState,
    pub status: Status,
    pub note: Option<Note>,
    pub details: bool,
    pub zen: bool,
    pub sort: Sort,
    /// Spinner step while a search is in flight.
    pub spin: u8,
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
            let Some(msg) = self.recv(&rx) else { continue };
            let before = self.query.clone();
            self.update(msg, &tx);
            (self.query != before).then(|| self.search(&tx));
        }
        Ok(())
    }

    /// Next message. `None` is a spinner tick, or quit after the channel closes.
    fn recv(&mut self, rx: &Receiver<Msg>) -> Option<Msg> {
        let pending = match self.status {
            Status::Loading => rx.recv_timeout(TICK),
            Status::Done | Status::Failed(_) => {
                rx.recv().map_err(|_| RecvTimeoutError::Disconnected)
            }
        };
        match pending {
            Ok(msg) => Some(msg),
            Err(RecvTimeoutError::Timeout) => {
                self.spin = self.spin.wrapping_add(1);
                None
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.quit = true;
                None
            }
        }
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
        self.table.select_next();
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
            (None, KeyCode::Char(c)) => self.edit(|draft| draft.insert(c)),
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
            Action::Focus(field) => {
                self.input = Some(Draft::open(field, query.field(field).clone()));
            }
            Action::Uploader => query.user = nyaa::next(UPLOADERS, query.user.as_str()).into(),
            Action::Filter => query.filter = nyaa::next(&Filter::ALL, query.filter),
            Action::Sort => {
                self.sort = nyaa::next(&Sort::ALL, self.sort);
                self.resort();
            }
            Action::Zen => self.zen = !self.zen,
            Action::Copy => self.copy(),
            Action::Download => self.download(tx),
            Action::Reload => self.search(tx),
            Action::Details => self.details = self.selected().is_some(),
            Action::Close => self.details = false,
            Action::Quit => self.quit = true,
            Action::Submit => {
                if let Some(draft) = self.input.take() {
                    *query.field(draft.field) = draft.text;
                }
            }
            Action::Cancel => self.input = None,
            Action::Delete => self.edit(Draft::backspace),
            Action::Remove => self.edit(Draft::remove),
            Action::Left => self.edit(Draft::left),
            Action::Right => self.edit(Draft::right),
            Action::Home => self.edit(Draft::home),
            Action::End => self.edit(Draft::end),
        }
    }

    fn edit(&mut self, change: impl FnOnce(&mut Draft)) {
        let Some(draft) = self.input.as_mut() else {
            return;
        };
        change(draft);
    }
}

#[cfg(test)]
mod tests {
    use super::Draft;
    use crate::nyaa::Field;

    #[test]
    fn caret_edits_in_the_middle() {
        let mut draft = Draft::open(Field::Text, "cat".into());
        draft.home();
        draft.right();
        draft.insert('o');
        assert_eq!((draft.text.as_str(), draft.at), ("coat", 2));
        draft.backspace();
        assert_eq!((draft.text.as_str(), draft.at), ("cat", 1));
        draft.end();
        draft.left();
        draft.remove();
        assert_eq!((draft.text.as_str(), draft.at), ("ca", 2));
        draft.home();
        draft.left();
        draft.backspace();
        assert_eq!((draft.text.as_str(), draft.at), ("ca", 0));
    }

    #[test]
    fn caret_steps_one_char() {
        let mut draft = Draft::open(Field::Text, "aあb".into());
        draft.left();
        draft.backspace();
        assert_eq!((draft.text.as_str(), draft.at), ("ab", 1));
        draft.home();
        draft.remove();
        assert_eq!((draft.text.as_str(), draft.at), ("b", 0));
    }
}
