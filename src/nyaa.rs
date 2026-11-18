use std::cmp::Reverse;
use std::error::Error;

use roxmltree::{Document, Node};

const BASE: &str = "https://nyaa.si";

type Res<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub const TRACKERS: [&str; 5] = [
    "http://nyaa.tracker.wf:7777/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://exodus.desync.com:6969/announce",
    "udp://tracker.torrent.eu.org:451/announce",
];

pub const UPLOADERS: &[&str] = &[
    "",
    "subsplease",
    "Erai-raws",
    "Judas",
    "ember_encodes",
    "Yameii",
    "Tsundere-Raws",
];

/// Item after `cur` in `items`, wrapping around.
pub fn next<T: PartialEq + Copy>(items: &[T], cur: T) -> T {
    let pos = items
        .iter()
        .position(|item| *item == cur)
        .unwrap_or_default();
    items[(pos + 1) % items.len()]
}

/// nyaa `f` param: discriminant is the query value.
#[derive(Clone, Copy, Default, PartialEq)]
pub enum Filter {
    None = 0,
    NoRemakes = 1,
    #[default]
    Trusted = 2,
}

impl Filter {
    pub const ALL: [Self; 3] = [Self::None, Self::NoRemakes, Self::Trusted];

    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "no filter",
            Self::NoRemakes => "no remakes",
            Self::Trusted => "trusted only",
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq)]
pub enum Sort {
    #[default]
    Date,
    Seeders,
    Size,
}

impl Sort {
    pub const ALL: [Self; 3] = [Self::Date, Self::Seeders, Self::Size];

    /// Sorts in place, largest first, since nyaa RSS ignores `s`.
    pub fn apply(self, torrents: &mut [Torrent]) {
        match self {
            Self::Date => torrents.sort_by_key(|t| Reverse(t.id)),
            Self::Seeders => torrents.sort_by_key(|t| Reverse(t.seeders)),
            Self::Size => torrents.sort_by(|a, b| b.bytes().total_cmp(&a.bytes())),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Seeders => "seeders",
            Self::Size => "size",
        }
    }
}

/// Typed query field, discriminant is its slot in the query bar.
#[derive(Clone, Copy, PartialEq)]
pub enum Field {
    Text = 0,
    User = 1,
}

#[derive(Clone, Default, PartialEq)]
pub struct Query {
    pub text: String,
    pub user: String,
    pub filter: Filter,
}

impl Query {
    pub const fn field(&mut self, field: Field) -> &mut String {
        match field {
            Field::Text => &mut self.text,
            Field::User => &mut self.user,
        }
    }

    /// RSS search URL, empty params omitted.
    pub fn url(&self) -> String {
        let filter = (self.filter as u8).to_string();
        let params = [
            ("page", "rss"),
            ("c", "1_2"),
            ("f", filter.as_str()),
            ("q", self.text.trim()),
            ("u", self.user.trim()),
        ];
        let query: Vec<_> = params
            .iter()
            .filter(|(_, value)| !value.is_empty())
            .map(|(key, value)| format!("{key}={}", encode(value)))
            .collect();
        format!("{BASE}/?{}", query.join("&"))
    }
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b' ' => "+".into(),
            b if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) => char::from(b).to_string(),
            b => format!("%{b:02X}"),
        })
        .collect()
}

pub struct Torrent {
    pub id: u32,
    pub title: String,
    pub size: String,
    pub date: String,
    pub seeders: u32,
    pub leechers: u32,
    pub comments: u32,
    pub hash: String,
    pub trusted: bool,
    pub remake: bool,
}

impl Torrent {
    pub fn magnet(&self) -> String {
        let trackers: String = TRACKERS
            .iter()
            .flat_map(|tr| ["&tr=".into(), encode(tr)])
            .collect();
        format!(
            "magnet:?xt=urn:btih:{}&dn={}{trackers}",
            self.hash,
            encode(&self.title)
        )
    }

    /// Byte count of `size` like `120.3 MiB`, unknown units count as bytes.
    pub fn bytes(&self) -> f64 {
        let (num, unit) = self.size.split_once(' ').unwrap_or_default();
        let exp = [("KiB", 1), ("MiB", 2), ("GiB", 3), ("TiB", 4)]
            .iter()
            .find(|(name, _)| *name == unit)
            .map_or(0, |(_, exp)| *exp);
        num.parse::<f64>().unwrap_or_default() * 1024_f64.powi(exp)
    }

    pub fn link(&self) -> String {
        format!("{BASE}/download/{}.torrent", self.id)
    }
}

fn field<'a>(item: Node<'a, '_>, name: &str) -> &'a str {
    item.children()
        .find(|node| node.has_tag_name(name))
        .and_then(|node| node.text())
        .unwrap_or_default()
}

impl From<Node<'_, '_>> for Torrent {
    fn from(item: Node) -> Self {
        let get = |name: &str| field(item, name);
        let num = |name: &str| get(name).parse().unwrap_or_default();
        Self {
            id: get("guid")
                .rsplit('/')
                .next()
                .map_or(0, |id| id.parse().unwrap_or_default()),
            title: get("title").into(),
            size: get("size").into(),
            date: get("pubDate").get(5..22).unwrap_or_default().into(),
            seeders: num("seeders"),
            leechers: num("leechers"),
            comments: num("comments"),
            hash: get("infoHash").into(),
            trusted: get("trusted") == "Yes",
            remake: get("remake") == "Yes",
        }
    }
}

pub fn parse(xml: &str) -> Result<Vec<Torrent>, roxmltree::Error> {
    let doc = Document::parse(xml)?;
    Ok(doc
        .descendants()
        .filter(|node| node.has_tag_name("item"))
        .map(Torrent::from)
        .collect())
}

pub fn search(query: &Query) -> Res<Vec<Torrent>> {
    let xml = ureq::get(query.url())
        .call()?
        .into_body()
        .read_to_string()?;
    Ok(parse(&xml)?)
}

/// Raw `.torrent` file behind `link`.
pub fn torrent(link: &str) -> Res<Vec<u8>> {
    Ok(ureq::get(link).call()?.into_body().read_to_vec()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<Torrent> {
        parse(include_str!("../fixtures/rss.xml")).expect("fixture parses")
    }

    #[test]
    fn url_should_match_example_searches() {
        let cases = [
            ("", "", Filter::NoRemakes, "&f=1"),
            ("", "", Filter::Trusted, "&f=2"),
            ("480", "subsplease", Filter::None, "&f=0&q=480&u=subsplease"),
            (
                "480 slime",
                "subsplease",
                Filter::None,
                "&f=0&q=480+slime&u=subsplease",
            ),
            ("bleach 49", "", Filter::Trusted, "&f=2&q=bleach+49"),
        ];
        let urls: Vec<_> = cases
            .iter()
            .map(|&(text, user, filter, _)| {
                Query {
                    text: text.into(),
                    user: user.into(),
                    filter,
                }
                .url()
            })
            .collect();
        let expected: Vec<_> = cases
            .iter()
            .map(|(.., tail)| format!("{BASE}/?page=rss&c=1_2{tail}"))
            .collect();
        assert_eq!(urls, expected);
    }

    #[test]
    fn sort_should_toggle_seeders_and_date() {
        let mut torrents = fixture();
        let ids: Vec<_> = torrents.iter().map(|t| t.id).collect();
        Sort::Seeders.apply(&mut torrents);
        let seeded = torrents.windows(2).all(|w| w[0].seeders >= w[1].seeders);
        Sort::Size.apply(&mut torrents);
        let sized = torrents.windows(2).all(|w| w[0].bytes() >= w[1].bytes());
        Sort::Date.apply(&mut torrents);
        let restored = torrents.iter().map(|t| t.id).eq(ids);
        assert!(seeded && sized && restored);
    }

    #[test]
    fn bytes_should_scale_by_unit() {
        assert!((fixture()[0].bytes() - 120.3 * 1024.0 * 1024.0).abs() < 1.0);
    }

    #[test]
    fn next_should_wrap_around() {
        assert_eq!((next(&[1, 2, 3], 1), next(&[1, 2, 3], 3)), (2, 1));
    }

    #[test]
    fn encode_should_escape_reserved_bytes() {
        assert_eq!(encode("a&b/c [x]"), "a%26b%2Fc+%5Bx%5D");
    }

    #[test]
    fn parse_should_read_all_items() {
        assert_eq!(fixture().len(), 75);
    }

    #[test]
    fn parse_should_read_first_item_fields() {
        let first = &fixture()[0];
        assert_eq!(
            (
                first.id,
                first.seeders,
                first.leechers,
                first.size.as_str(),
                first.date.as_str()
            ),
            (2_187_689, 3, 4, "120.3 MiB", "18 Nov 2026 16:24")
        );
    }

    #[test]
    fn parse_should_read_info_hash() {
        assert_eq!(
            fixture()[0].hash,
            "a075e940824d8879298489c5a65de0e6cd06ac18"
        );
    }

    #[test]
    fn magnet_should_carry_hash_and_trackers() {
        let magnet = fixture()[0].magnet();
        assert_eq!(
            (
                magnet.starts_with(
                    "magnet:?xt=urn:btih:a075e940824d8879298489c5a65de0e6cd06ac18&dn="
                ),
                magnet.matches("&tr=").count()
            ),
            (true, 5)
        );
    }

    #[test]
    fn link_should_point_at_torrent_file() {
        assert_eq!(
            fixture()[0].link(),
            format!("{BASE}/download/2187689.torrent")
        );
    }

    #[test]
    fn parse_should_flag_trusted_and_remake() {
        let torrents = fixture();
        assert!(torrents.iter().any(|t| t.trusted) && torrents.iter().any(|t| t.remake));
    }
}
