//! Address checks used to keep "local" honest: a URL is local only if its host is exactly a
//! loopback name — not a look-alike such as `localhost.evil.com` or `127.0.0.1@evil.com`.

/// The host of an http(s) URL, or `None` if the URL is malformed or carries user info.
pub fn url_host(url: &str) -> Option<String> {
    let lower = url.trim().to_ascii_lowercase();
    let rest = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') || authority.contains(char::is_whitespace) {
        return None;
    }
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let (h, after) = v6.split_once(']')?;
        (h.to_owned(), after.strip_prefix(':'))
    } else {
        match authority.split_once(':') {
            Some((h, p)) => (h.to_owned(), Some(p)),
            None => (authority.to_owned(), None),
        }
    };
    if let Some(p) = port
        && (p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) || p.len() > 5)
    {
        return None;
    }
    (!host.is_empty()).then_some(host)
}

/// True when `url` is http(s) to this Mac (127.0.0.1, ::1 or localhost) and nothing else.
pub fn is_loopback_url(url: &str) -> bool {
    matches!(
        url_host(url).as_deref(),
        Some("127.0.0.1" | "::1" | "localhost")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_loopback_counts_as_local() {
        for ok in [
            "http://127.0.0.1:11434",
            "http://localhost:8080/v1",
            "http://[::1]:1234/",
            "https://LOCALHOST/",
        ] {
            assert!(is_loopback_url(ok), "{ok}");
        }
        for bad in [
            "http://localhost.evil.com",
            "http://127.0.0.1@evil.com",
            "http://localhost:1@evil.com",
            "https://localhost:x@api.example.com/v1",
            "http://[::1]@evil.com",
            "http://127.0.0.1.nip.io",
            "ftp://127.0.0.1",
            "http://127.0.0.1:99999999",
            "http://",
        ] {
            assert!(!is_loopback_url(bad), "{bad}");
        }
        assert_eq!(
            url_host("https://api.openai.com/v1").as_deref(),
            Some("api.openai.com")
        );
    }
}
