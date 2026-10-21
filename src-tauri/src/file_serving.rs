//! How a local file is served, shared by the editor's preview scheme and
//! the loopback server that shows local files in browser tabs. What type a
//! file is comes from the core's `http::content_type`.

pub use sikemux_core::http::content_type;

/// A single `bytes=start-` or `bytes=start-end` range, which is all media elements ask for.
pub fn parse_range(value: &str) -> Option<(u64, Option<u64>)> {
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    let start = start.trim().parse().ok()?;
    let end = match end.trim() {
        "" => None,
        end => Some(end.parse().ok()?),
    };
    Some((start, end))
}

pub fn percent_decode(segment: &str) -> Option<String> {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = segment
                .get(at + 1..at + 3)
                .filter(|hex| hex.bytes().all(|byte| byte.is_ascii_hexdigit()))?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_ranges_parse() {
        assert_eq!(parse_range("bytes=0-"), Some((0, None)));
        assert_eq!(parse_range("bytes=10-19"), Some((10, Some(19))));
        assert_eq!(parse_range("bytes=-5"), None);
        assert_eq!(parse_range("items=0-1"), None);
    }

    #[test]
    fn percent_escapes_decode() {
        assert_eq!(percent_decode("a%20b"), Some("a b".into()));
        assert_eq!(percent_decode("%2FUsers%2Fme"), Some("/Users/me".into()));
        assert_eq!(percent_decode("%zz"), None);
        assert_eq!(percent_decode("%+f"), None);
    }
}
