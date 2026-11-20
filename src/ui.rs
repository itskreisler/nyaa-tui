use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Position, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Paragraph, Row, Table};

use crate::app::{App, Draft, Note, Status};
use crate::keys::Bind;
use crate::nyaa::{Field, Torrent};
use crate::{popup, title};

/// One results column. `zen` columns stay visible in zen mode.
#[derive(Clone, Copy)]
struct Col {
    name: &'static str,
    width: Constraint,
    fg: Option<Color>,
    zen: bool,
}

const COLUMNS: [Col; 5] = [
    Col {
        name: "title",
        width: Constraint::Fill(1),
        fg: None,
        zen: true,
    },
    Col {
        name: "size",
        width: Constraint::Length(10),
        fg: Some(Color::LightBlue),
        zen: false,
    },
    Col {
        name: "date",
        width: Constraint::Length(17),
        fg: Some(Color::LightMagenta),
        zen: false,
    },
    Col {
        name: "seed",
        width: Constraint::Length(5),
        fg: Some(Color::LightGreen),
        zen: true,
    },
    Col {
        name: "leech",
        width: Constraint::Length(5),
        fg: Some(Color::LightRed),
        zen: false,
    },
];

const fn keep(col: &Col, zen: bool) -> bool {
    !zen || col.zen
}

pub fn render(frame: &mut Frame, app: &mut App) {
    let (header, body) = place(frame, app);
    search(frame, header, app);
    let mark = app.note.as_ref().map_or_default(note);
    let table = results(&app.torrents, &app.status, &app.query.user, mark, app.zen);
    frame.render_stateful_widget(table, body, &mut app.table);
    if app.torrents.is_empty() && matches!(app.status, Status::Loading) {
        spinner(frame, body, app.spin);
    }
    if let Some(torrent) = app.selected().filter(|_| app.details) {
        popup::render(frame, body, torrent);
    }
}

/// Header and results. Normal mode also draws both key rows.
fn place(frame: &mut Frame, app: &App) -> (Rect, Rect) {
    if app.zen {
        let [header, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(frame.area());
        return (header, body);
    }
    let [top, header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let &[fields, actions] = app.keys();
    hints(frame, top, fields);
    hints(frame, footer, actions);
    (header, body)
}

fn search(frame: &mut Frame, area: Rect, app: &App) {
    let line = query_line(app);
    if let Some(draft) = &app.input {
        frame.set_cursor_position(cursor(area, &line, draft, app.zen));
    }
    if app.zen {
        return zen_bar(frame, area, line, feedback(app));
    }
    frame.render_widget(
        Paragraph::new(line).block(panel("nyaa · anime english-translated")),
        area,
    );
}

/// Search line on the left, note or status on the right.
fn zen_bar(frame: &mut Frame, area: Rect, line: Line, mark: Line) {
    let width = u16::try_from(mark.width()).unwrap_or(u16::MAX);
    let [text, slot] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(width)]).areas(area);
    frame.render_widget(Paragraph::new(line), text);
    frame.render_widget(Paragraph::new(mark), slot);
}

fn feedback(app: &App) -> Line<'_> {
    if let Some(item) = &app.note {
        return note(item);
    }
    match &app.status {
        Status::Loading => " loading ".light_yellow().into(),
        Status::Failed(err) => format!(" {err} ").light_red().into(),
        Status::Done => Line::default(),
    }
}

fn note(note: &Note) -> Line<'_> {
    match note {
        Ok(text) => format!(" {text} ").dark_gray().into(),
        Err(err) => format!(" {err} ").light_red().into(),
    }
}

/// Insertion point inside the edited field. Zen has no border inset.
fn cursor(area: Rect, line: &Line, draft: &Draft, zen: bool) -> Position {
    let before: usize = line.spans[..=2 * (draft.field as usize)]
        .iter()
        .map(Span::width)
        .sum();
    let typed = Span::raw(&draft.text[..draft.at]).width();
    let inset = u16::from(!zen);
    Position::new(
        area.x + inset + u16::try_from(before + typed).unwrap_or(u16::MAX),
        area.y + inset,
    )
}

fn present(text: &str) -> Option<&str> {
    (!text.is_empty()).then_some(text)
}

/// Draft while `field` is edited, else the saved value if any.
fn shown<'a>(app: &'a App, field: Field, saved: &'a str) -> Option<&'a str> {
    match &app.input {
        Some(draft) if draft.field == field => Some(&draft.text),
        _ => present(saved),
    }
}

pub fn panel(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().light_magenta())
        .title(format!(" {title} ").bold().light_cyan())
}

/// Label and value spans per slot, the edited field shows its draft.
fn query_line(app: &App) -> Line<'_> {
    let query = &app.query;
    let fields = [
        ("search", shown(app, Field::Text, &query.text)),
        ("uploader", shown(app, Field::User, &query.user)),
        ("filter", Some(query.filter.label())),
        ("sort", Some(app.sort.label())),
    ];
    fields
        .into_iter()
        .flat_map(|(key, value)| {
            [
                format!(" {key} ").dark_gray(),
                value.unwrap_or("any").bold().light_yellow(),
            ]
        })
        .collect()
}

fn results<'a>(
    torrents: &'a [Torrent],
    status: &Status,
    user: &str,
    note: Line<'a>,
    zen: bool,
) -> Table<'a> {
    let shown = COLUMNS.into_iter().filter(move |col| keep(col, zen));
    let header = Row::new(shown.clone().map(|col| col.name))
        .bold()
        .light_cyan();
    let rows = torrents.iter().map(|torrent| row(torrent, user, zen));
    let table = Table::new(rows, shown.map(|col| col.width))
        .header(header)
        .row_highlight_style(Style::new().reversed())
        .highlight_symbol("▌");
    if zen {
        return table;
    }
    table.block(
        panel("results")
            .title(Line::from(badge(torrents.len(), status)).right_aligned())
            .title(note.right_aligned()),
    )
}

fn row<'a>(torrent: &'a Torrent, user: &str, zen: bool) -> Row<'a> {
    let style = match (torrent.trusted, torrent.remake) {
        (_, true) => Style::new().light_red(),
        (true, _) => Style::new().light_green(),
        _ => Style::new(),
    };
    let cells: [Cell; 5] = [
        title::short(&torrent.title, user).into(),
        torrent.size.as_str().into(),
        torrent.date.as_str().into(),
        torrent.seeders.to_string().into(),
        torrent.leechers.to_string().into(),
    ];
    let colored = cells.into_iter().zip(COLUMNS).filter_map(|(cell, col)| {
        let fg = (zen && col.name == "seed")
            .then_some(Color::Yellow)
            .or(col.fg);
        keep(&col, zen).then(|| cell.style(Style { fg, ..Style::new() }))
    });
    Row::new(colored).style(style)
}

const SPIN: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Braille spinner centered in the empty results panel.
fn spinner(frame: &mut Frame, area: Rect, step: u8) {
    let line = Line::from(vec![
        SPIN[usize::from(step) % SPIN.len()].bold().light_yellow(),
        " loading".dark_gray(),
    ]);
    let width = u16::try_from(line.width()).unwrap_or(u16::MAX);
    let spot = area.centered(Constraint::Length(width), Constraint::Length(1));
    frame.render_widget(line, spot);
}

fn badge(count: usize, status: &Status) -> Span<'static> {
    match status {
        Status::Loading => " loading ".light_yellow(),
        Status::Done => format!(" {count} ").dark_gray(),
        Status::Failed(err) => format!(" {err} ").light_red(),
    }
}

fn hint(bind: &Bind) -> Line<'static> {
    Line::from(vec![
        format!(" {} ", bind.keys[0])
            .bold()
            .fg(Color::Rgb(0, 0, 0))
            .bg(Color::Magenta),
        format!(" {} ", bind.hint).dark_gray(),
    ])
}

/// Hints spread evenly across `area`.
fn hints(frame: &mut Frame, area: Rect, binds: &[Bind]) {
    let lines: Vec<Line> = binds.iter().map(hint).collect();
    let widths = lines
        .iter()
        .map(|line| Constraint::Length(u16::try_from(line.width()).unwrap_or(u16::MAX)));
    let cells = Layout::horizontal(widths)
        .flex(Flex::SpaceEvenly)
        .split(area);
    for (line, cell) in lines.into_iter().zip(cells.iter()) {
        frame.render_widget(line, *cell);
    }
}
