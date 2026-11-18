use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Position, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Cell, Paragraph, Row, Table};

use crate::app::{App, Note, Status};
use crate::keys::Bind;
use crate::nyaa::{Field, Torrent};
use crate::{popup, title};

/// Name, width and color per column, `None` keeps the row's trusted/remake color.
const COLUMNS: [(&str, Constraint, Option<Color>); 5] = [
    ("title", Constraint::Fill(1), None),
    ("size", Constraint::Length(10), Some(Color::LightBlue)),
    ("date", Constraint::Length(17), Some(Color::LightMagenta)),
    ("seed", Constraint::Length(5), Some(Color::LightGreen)),
    ("leech", Constraint::Length(5), Some(Color::LightRed)),
];

pub fn render(frame: &mut Frame, app: &mut App) {
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

    let line = query_line(app);
    if let Some((field, _)) = &app.input {
        frame.set_cursor_position(cursor(header, &line, *field));
    }
    let bar = Paragraph::new(line).block(panel("nyaa · anime english-translated"));
    frame.render_widget(bar, header);
    let note = app.note.as_ref().map_or_default(note);
    let table = results(&app.torrents, &app.status, &app.query.user, note);
    frame.render_stateful_widget(table, body, &mut app.table);
    if let Some(torrent) = app.selected().filter(|_| app.details) {
        popup::render(frame, body, torrent);
    }
}

fn note(note: &Note) -> Line<'_> {
    match note {
        Ok(text) => format!(" {text} ").dark_gray().into(),
        Err(err) => format!(" {err} ").light_red().into(),
    }
}

/// End of the edited field's value span.
fn cursor(area: Rect, line: &Line, field: Field) -> Position {
    let width: usize = line.spans[..2 * (field as usize + 1)]
        .iter()
        .map(Span::width)
        .sum();
    Position::new(
        area.x + 1 + u16::try_from(width).unwrap_or(u16::MAX),
        area.y + 1,
    )
}

fn present(text: &str) -> Option<&str> {
    (!text.is_empty()).then_some(text)
}

/// Draft while `field` is edited, else the saved value if any.
fn shown<'a>(app: &'a App, field: Field, saved: &'a str) -> Option<&'a str> {
    match &app.input {
        Some((edited, draft)) if *edited == field => Some(draft),
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

fn results<'a>(torrents: &'a [Torrent], status: &Status, user: &str, note: Line<'a>) -> Table<'a> {
    let header = Row::new(COLUMNS.map(|(name, ..)| name)).bold().light_cyan();
    let rows = torrents.iter().map(|torrent| row(torrent, user));
    Table::new(rows, COLUMNS.map(|(_, width, _)| width))
        .header(header)
        .row_highlight_style(Style::new().reversed())
        .highlight_symbol("▌")
        .block(
            panel("results")
                .title(Line::from(badge(torrents.len(), status)).right_aligned())
                .title(note.right_aligned()),
        )
}

fn row<'a>(torrent: &'a Torrent, user: &str) -> Row<'a> {
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
    let colored = cells
        .into_iter()
        .zip(COLUMNS)
        .map(|(cell, (.., fg))| cell.style(Style { fg, ..Style::new() }));
    Row::new(colored).style(style)
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
