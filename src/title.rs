/// Display title: leading `[Uploader]` tag dropped once an uploader is picked,
/// `[ABCD1234]` checksums dropped, noise between the quality and extension dropped.
pub fn short(title: &str, user: &str) -> String {
    trim_noise(&unhash(untag(title, user)))
}

fn is_hash(tag: &str) -> bool {
    tag.strip_suffix(']')
        .is_some_and(|hex| hex.len() == 8 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Drops every `[ABCD1234]` checksum tag and the double spaces it leaves.
fn unhash(title: &str) -> String {
    let kept: String = title
        .split('[')
        .enumerate()
        .map(|(at, part)| match part.split_at_checked(9) {
            _ if at == 0 => part.to_owned(),
            Some((tag, rest)) if is_hash(tag) => rest.to_owned(),
            _ => format!("[{part}"),
        })
        .collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn untag<'a>(title: &'a str, user: &str) -> &'a str {
    (!user.is_empty()).then_some(title).map_or(title, strip_tag)
}

fn strip_tag(title: &str) -> &str {
    title
        .strip_prefix('[')
        .and_then(|rest| rest.split_once(']'))
        .map_or(title, |(_, rest)| rest.trim_start())
}

fn alnum_at(bytes: &[u8], at: Option<usize>) -> bool {
    at.and_then(|at| bytes.get(at))
        .is_some_and(u8::is_ascii_alphanumeric)
}

/// First standalone `<3-4 digits>p` with a name before it, as (digits start, end after `p`).
fn quality(title: &str) -> Option<(usize, usize)> {
    let bytes = title.as_bytes();
    title.match_indices('p').find_map(|(at, _)| {
        let digits = bytes[..at]
            .iter()
            .rev()
            .take_while(|b| b.is_ascii_digit())
            .count();
        let start = at - digits;
        let bounded = !alnum_at(bytes, start.checked_sub(1)) && !alnum_at(bytes, Some(at + 1));
        let lead = title[..start].trim_end_matches(['(', '[']);
        let named = !strip_tag(lead).trim().is_empty();
        ((3..=4).contains(&digits) && bounded && named).then_some((start, at + 1))
    })
}

/// Closer for the bracket still open before the quality, if any.
fn closer(lead: &str) -> &'static str {
    let open = lead
        .rfind(['[', '('])
        .filter(|&at| Some(at) > lead.rfind([']', ')']));
    match open.map(|at| lead.as_bytes()[at]) {
        Some(b'(') => ")",
        Some(b'[') => "]",
        _ => "",
    }
}

/// `.ext` after the last dot, if it is a plain word.
fn extension(tail: &str) -> &str {
    tail.rfind('.')
        .map(|dot| &tail[dot..])
        .filter(|ext| ext[1..].starts_with(char::is_alphabetic))
        .filter(|ext| ext[1..].chars().all(char::is_alphanumeric))
        .unwrap_or_default()
}

fn trim_noise(title: &str) -> String {
    quality(title).map_or_else(
        || title.into(),
        |(start, end)| {
            let (head, tail) = title.split_at(end);
            format!("{head}{}{}", closer(&title[..start]), extension(tail))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::short;

    #[test]
    fn short_should_drop_tag_only_with_uploader() {
        let title = "[SubsPlease] Slime - 24 (480p)";
        assert_eq!(
            (short(title, "subsplease"), short(title, "")),
            ("Slime - 24 (480p)".into(), title.into())
        );
    }

    #[test]
    fn short_should_keep_quality_and_extension() {
        let cases = [
            (
                "Slime S4 - 24 (480p) [ADF6D2BF].mkv",
                "Slime S4 - 24 (480p).mkv",
            ),
            ("Slime (49-72) (1080p) [Batch]", "Slime (49-72) (1080p)"),
            ("Show (2026) - 07 (720p) [v1.5]", "Show (2026) - 07 (720p)"),
            ("Show - 07 [1080p CR WEB-DL][MultiSub]", "Show - 07 [1080p]"),
            (
                "Show S01E05 1080p CR WEB-DL AAC2.0 H.264",
                "Show S01E05 1080p",
            ),
            (
                "Show S01E05 720p WEB H.264-VARYG (Show (2026))",
                "Show S01E05 720p",
            ),
            (
                "[Shridhuu][1080p] Show - S03E09",
                "[Shridhuu][1080p] Show - S03E09",
            ),
            ("Show - 07 [10800p]", "Show - 07 [10800p]"),
            ("Show - 07 [480p][HEVC][ABCD].mkv", "Show - 07 [480p].mkv"),
            (
                "[FDE4473C] Show [Batch] [fde18d13] - 07",
                "Show [Batch] - 07",
            ),
            (
                "Show [Dub] [CR WEB-DL 720p H264] [BB37]",
                "Show [Dub] [CR WEB-DL 720p]",
            ),
        ];
        let shorts: Vec<_> = cases.iter().map(|(title, _)| short(title, "")).collect();
        let expected: Vec<_> = cases.iter().map(|(_, want)| (*want).to_string()).collect();
        assert_eq!(shorts, expected);
    }
}
