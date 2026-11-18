use ratatui::Frame;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Clear, Paragraph, Wrap};

use crate::nyaa::{TRACKERS, Torrent};
use crate::ui::panel;

/// Label and value per line, in display order.
fn fields(torrent: &Torrent) -> impl Iterator<Item = (&'static str, String)> {
    let head = [
        ("title", torrent.title.clone()),
        ("size", torrent.size.clone()),
        ("date", torrent.date.clone()),
        ("comments", torrent.comments.to_string()),
        ("seeders", torrent.seeders.to_string()),
        ("leechers", torrent.leechers.to_string()),
    ];
    let trackers = TRACKERS.map(|tracker| ("tracker", tracker.to_owned()));
    let links = [("magnet", torrent.magnet()), ("download", torrent.link())];
    head.into_iter().chain(trackers).chain(links)
}

pub fn render(frame: &mut Frame, area: Rect, torrent: &Torrent) {
    let lines: Vec<Line> = fields(torrent)
        .map(|(label, value)| {
            Line::from(vec![
                format!("{label:>9} ").dark_gray(),
                value.light_yellow(),
            ])
        })
        .collect();
    let area = area.centered(Constraint::Percentage(80), Constraint::Percentage(80));
    let popup = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(panel("details"));
    frame.render_widget(Clear, area);
    frame.render_widget(popup, area);
}
