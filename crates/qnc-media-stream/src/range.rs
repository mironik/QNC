use std::ops::Range;

/// A single RFC byte range converted to a half-open interval. Multipart is not supported.
pub(crate) fn parse(header: Option<&str>, len: u64) -> Result<Range<u64>, ()> {
    let Some(header) = header else {
        return Ok(0..len);
    };
    let value = header.strip_prefix("bytes=").ok_or(())?;
    let (start, end) = value.split_once('-').ok_or(())?;
    fn number(s: &str) -> Result<u64, ()> {
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
            return Err(());
        }
        s.parse().map_err(|_| ())
    }
    if len == 0 {
        return Err(());
    }
    if start.is_empty() {
        let count = number(end)?;
        if count == 0 {
            return Err(());
        }
        return Ok(len.saturating_sub(count)..len);
    }
    let start = number(start)?;
    if start >= len {
        return Err(());
    }
    let end = if end.is_empty() {
        len
    } else {
        let last = number(end)?;
        if last < start {
            return Err(());
        }
        last.min(len - 1) + 1
    };
    Ok(start..end)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_open_and_suffix_ranges_are_bounded() {
        assert_eq!(parse(None, 10), Ok(0..10));
        assert_eq!(parse(Some("bytes=0-0"), 10), Ok(0..1));
        assert_eq!(parse(Some("bytes=5-"), 10), Ok(5..10));
        assert_eq!(parse(Some("bytes=3-999"), 10), Ok(3..10));
        assert_eq!(parse(Some("bytes=-3"), 10), Ok(7..10));
        assert_eq!(parse(Some("bytes=-99"), 10), Ok(0..10));
        assert_eq!(parse(None, 0), Ok(0..0));
    }
    #[test]
    fn malformed_overflow_and_multiple_ranges_are_rejected() {
        for header in [
            "bytes=10-",
            "bytes=4-3",
            "bytes=-0",
            "bytes=",
            "bytes=+1-2",
            "items=0-1",
            "bytes=0-1,3-4",
            "bytes=1--1",
            "bytes=18446744073709551616-",
        ] {
            assert!(parse(Some(header), 10).is_err(), "{header}");
        }
        assert!(parse(Some("bytes=0-"), 0).is_err());
    }
}
