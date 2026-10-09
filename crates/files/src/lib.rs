//! Local knowledge (spec §68–69): what can be imported, decoding text formats, chunking with
//! locators for citations, and lexical scoring. Pure — rich formats (PDF, DOCX, images) are
//! read by the host; embeddings come from the host too.

/// Largest file accepted for import.
pub const MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
/// Most text kept from one file (characters).
pub const MAX_TEXT_CHARS: usize = 2_000_000;
pub const CHUNK_CHARS: usize = 900;
pub const CHUNK_OVERLAP: usize = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Plain text, Markdown, CSV, JSON, source code: decoded here.
    Text,
    /// PDF: read by the host page by page.
    Pdf,
    /// Word, RTF, HTML: read by the host's document readers.
    Document,
    /// Images: text recognized on device by the host.
    Image,
}

const TEXT_EXT: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "json", "jsonl", "yaml", "yml", "toml", "xml", "log",
    "ini", "conf", "rs", "swift", "ts", "tsx", "js", "jsx", "mjs", "py", "rb", "go", "java", "kt",
    "c", "h", "cpp", "hpp", "cs", "m", "mm", "php", "sh", "zsh", "bash", "sql", "css", "scss",
    "vue", "svelte", "lua", "r", "dart", "scala",
];

/// What kind of file this is, by extension; `None` = not supported (never imported).
pub fn kind(name: &str) -> Option<Kind> {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase())?;
    let ext = ext.as_str();
    if TEXT_EXT.contains(&ext) {
        Some(Kind::Text)
    } else if ext == "pdf" {
        Some(Kind::Pdf)
    } else if ["docx", "doc", "rtf", "rtfd", "html", "htm", "odt"].contains(&ext) {
        Some(Kind::Document)
    } else if [
        "png", "jpg", "jpeg", "heic", "tiff", "tif", "gif", "bmp", "webp",
    ]
    .contains(&ext)
    {
        Some(Kind::Image)
    } else {
        None
    }
}

pub fn mime(name: &str) -> &'static str {
    let ext = name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "pdf" => "application/pdf",
        "md" | "markdown" => "text/markdown",
        "csv" => "text/csv",
        "json" | "jsonl" => "application/json",
        "html" | "htm" => "text/html",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "doc" => "application/msword",
        "rtf" => "application/rtf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "heic" => "image/heic",
        "gif" => "image/gif",
        _ => "text/plain",
    }
}

/// A safe file name: no folders, no hidden-file dots, only ordinary characters, bounded.
pub fn sanitize_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " ._-()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').to_owned();
    let cleaned = if cleaned.is_empty() {
        "file".to_owned()
    } else {
        cleaned
    };
    if cleaned.chars().count() > 120 {
        let (stem, ext) = cleaned.rsplit_once('.').unwrap_or((&cleaned, ""));
        let stem: String = stem.chars().take(100).collect();
        if ext.is_empty() {
            stem
        } else {
            format!("{stem}.{ext}")
        }
    } else {
        cleaned
    }
}

/// Decode a text file: UTF-8 (lossy), BOM stripped. Rejects binary content.
pub fn decode_text(bytes: &[u8]) -> Result<String, String> {
    let sample = &bytes[..bytes.len().min(8192)];
    if sample.contains(&0) {
        return Err("This file looks binary, not text.".into());
    }
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    Ok(text.chars().take(MAX_TEXT_CHARS).collect())
}

/// A piece of a document and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub content: String,
    /// 1-based page (PDF), if known.
    pub page: Option<u32>,
    /// 1-based line range within the page or file.
    pub line_start: u32,
    pub line_end: u32,
}

/// Split one page of text into overlapping chunks along line boundaries.
pub fn chunk_page(text: &str, page: Option<u32>) -> Vec<Chunk> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = vec![];
    let mut start = 0;
    while start < lines.len() {
        let mut len = 0;
        let mut end = start;
        while end < lines.len() && (len == 0 || len + lines[end].len() < CHUNK_CHARS) {
            len += lines[end].len() + 1;
            end += 1;
        }
        let content: String = lines[start..end].join("\n");
        // Very long single lines are hard-wrapped.
        if content.chars().count() > CHUNK_CHARS * 2 {
            let chars: Vec<char> = content.chars().collect();
            let mut i = 0;
            while i < chars.len() {
                let j = (i + CHUNK_CHARS).min(chars.len());
                let piece: String = chars[i..j].iter().collect();
                if !piece.trim().is_empty() {
                    out.push(Chunk {
                        content: piece,
                        page,
                        line_start: start as u32 + 1,
                        line_end: end as u32,
                    });
                }
                if j == chars.len() {
                    break;
                }
                i = j - CHUNK_OVERLAP;
            }
        } else if !content.trim().is_empty() {
            out.push(Chunk {
                content,
                page,
                line_start: start as u32 + 1,
                line_end: end as u32,
            });
        }
        if end >= lines.len() {
            break;
        }
        // Step back a little for overlap, but always make progress.
        let mut back = 0;
        let mut back_len = 0;
        while back < end - start - 1 && back_len < CHUNK_OVERLAP {
            back += 1;
            back_len += lines[end - back].len() + 1;
        }
        start = (end - back).max(start + 1);
    }
    out
}

/// "report.pdf, page 3" / "notes.md, lines 10–24".
pub fn citation(name: &str, c: &Chunk) -> String {
    match c.page {
        Some(p) => format!("{name}, page {p}"),
        None if c.line_start == c.line_end => format!("{name}, line {}", c.line_start),
        None => format!("{name}, lines {}–{}", c.line_start, c.line_end),
    }
}

fn terms(s: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "the", "and", "for", "with", "that", "this", "are", "was", "what", "when", "where",
        "which", "how", "who", "does", "did", "can", "from", "into", "about", "there", "their",
        "they", "have", "has", "will", "would", "should", "you", "your", "our", "its", "not",
        "but",
    ];
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1 && !STOP.contains(w))
        .map(|w| w.trim_end_matches('s').to_owned())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Fraction of the query's distinct terms found in the text, weighted toward rarer, longer
/// terms (0–1). Cheap BM25-like signal that works without embeddings.
pub fn lexical(query: &str, text: &str) -> f32 {
    let mut q = terms(query);
    q.sort();
    q.dedup();
    if q.is_empty() {
        return 0.0;
    }
    let t = terms(text);
    let total: f32 = q.iter().map(|w| w.len().min(10) as f32).sum();
    let hit: f32 = q
        .iter()
        .filter(|w| t.contains(w))
        .map(|w| w.len().min(10) as f32)
        .sum();
    hit / total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_what_it_can_import() {
        assert_eq!(kind("Notes.MD"), Some(Kind::Text));
        assert_eq!(kind("main.rs"), Some(Kind::Text));
        assert_eq!(kind("report.pdf"), Some(Kind::Pdf));
        assert_eq!(kind("letter.docx"), Some(Kind::Document));
        assert_eq!(kind("scan.HEIC"), Some(Kind::Image));
        for no in ["app.exe", "Installer.dmg", "script", "archive.zip", "a.app"] {
            assert_eq!(kind(no), None, "{no}");
        }
    }

    #[test]
    fn sanitizes_names() {
        assert_eq!(sanitize_name("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_name(".hidden.txt"), "hidden.txt");
        assert_eq!(
            sanitize_name("Q3 report (final).pdf"),
            "Q3 report (final).pdf"
        );
        assert_eq!(sanitize_name("a;rm -rf $HOME.md"), "a_rm -rf _HOME.md");
        assert!(sanitize_name(&format!("{}.txt", "x".repeat(300))).ends_with(".txt"));
        assert_eq!(sanitize_name("///"), "file");
    }

    #[test]
    fn decodes_text_and_rejects_binary() {
        assert_eq!(
            decode_text("\u{feff}hello".as_bytes()).expect("text"),
            "hello"
        );
        assert!(decode_text(&[0x50, 0x4b, 0x03, 0x04, 0, 0, 1]).is_err());
    }

    #[test]
    fn chunks_with_overlap_and_locators() {
        let text: String = (1..=200)
            .map(|i| format!("Line {i} of the quarterly planning notes for the team.\n"))
            .collect();
        let chunks = chunk_page(&text, None);
        assert!(chunks.len() > 5);
        assert!(chunks.iter().all(|c| c.content.len() <= CHUNK_CHARS + 60));
        assert_eq!(chunks[0].line_start, 1);
        // Consecutive chunks overlap and cover everything.
        assert!(chunks[1].line_start <= chunks[0].line_end);
        assert_eq!(chunks.last().expect("last").line_end, 200);
        assert_eq!(
            citation("notes.md", &chunks[0]),
            format!("notes.md, lines 1–{}", chunks[0].line_end)
        );
        let one = chunk_page("x".repeat(5000).as_str(), Some(3));
        assert!(one.len() >= 5 && one.iter().all(|c| c.page == Some(3)));
        assert_eq!(citation("r.pdf", &one[0]), "r.pdf, page 3");
    }

    #[test]
    fn scores_keyword_overlap() {
        let doc = "The Q3 budget for marketing is $42,000, approved by Priya in July.";
        assert!(lexical("What is the marketing budget for Q3?", doc) > 0.6);
        assert!(lexical("How many vacation days do I get?", doc) < 0.2);
    }
}
