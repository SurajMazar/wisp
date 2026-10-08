//! Redaction of secret-shaped strings. Applied to every log line before it is written.

use std::borrow::Cow;
use std::sync::LazyLock;

use regex::Regex;

// Patterns are compile-time constants covered by tests; a failure here is a programming error.
#[allow(clippy::expect_used)]
static PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        // Provider API keys
        r"sk-ant-[A-Za-z0-9_\-]{8,}",
        r"sk-(?:proj-)?[A-Za-z0-9_\-]{16,}",
        r"AIza[0-9A-Za-z_\-]{30,}",
        r"gh[pousr]_[A-Za-z0-9]{20,}",
        r"xox[abprs]-[A-Za-z0-9\-]{10,}",
        // Authorization headers / bearer tokens
        r"(?i)bearer\s+[A-Za-z0-9._\-~+/]{12,}=*",
        // key=value / "key": "value" for sensitive key names
        r#"(?i)("?(?:api[_-]?key|token|secret|password|authorization)"?\s*[:=]\s*"?)[^"\s,}]{4,}"#,
    ]
    .iter()
    .map(|p| Regex::new(p).expect("static redaction pattern compiles"))
    .collect()
});

pub const MASK: &str = "[REDACTED]";

/// Replace anything that looks like a credential with [`MASK`].
pub fn redact(input: &str) -> Cow<'_, str> {
    let mut out: Cow<'_, str> = Cow::Borrowed(input);
    for (i, re) in PATTERNS.iter().enumerate() {
        if re.is_match(&out) {
            let replaced = if i == PATTERNS.len() - 1 {
                re.replace_all(&out, format!("${{1}}{MASK}")).into_owned()
            } else {
                re.replace_all(&out, MASK).into_owned()
            };
            out = Cow::Owned(replaced);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_known_secret_shapes() {
        // Fixtures are split with concat! so secret scanners don't flag this file.
        let cases = [
            concat!("key sk-", "ant-api03-abcdefghijklmnop end"),
            concat!("OPENAI sk-", "proj-ABCDEFGHIJKLMNOPQRSTUV"),
            concat!("gemini AI", "zaSyA1234567890abcdefghijklmnopqrstu"),
            "Authorization: Bearer abcdefghijklmnop.qrs",
            r#"{"api_key": "hunter2hunter2"}"#,
            "password=correcthorse",
        ];
        for c in cases {
            let r = redact(c);
            assert!(r.contains(MASK), "not redacted: {c} -> {r}");
            for secret in [
                "abcdefghijklmnop",
                "ABCDEFGHIJKLMNOP",
                "1234567890abc",
                "hunter2",
                "correcthorse",
            ] {
                assert!(!r.contains(secret), "leaked {secret} in {r}");
            }
        }
    }

    #[test]
    fn leaves_ordinary_text_alone() {
        let s = "model qwen3:8b responded in 420ms";
        assert!(matches!(redact(s), Cow::Borrowed(_)));
    }
}
