/// BM25 scoring parameters.
const K1: f64 = 1.5;
const B: f64 = 0.75;

/// Search result from BM25 ranking.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub file: String,
    pub snippet: String,
    pub score: f64,
}

/// Tokenize text using dual-mode: CJK bigrams + ASCII whitespace split.
///
/// CJK characters (U+4E00..U+9FFF) are split into overlapping bigrams.
/// Non-CJK text is split on whitespace with non-word chars stripped.
pub fn tokenize(text: &str) -> Vec<String> {
    let lower = text.to_lowercase();
    let mut tokens = Vec::new();

    let mut i = 0;
    let chars: Vec<char> = lower.chars().collect();

    while i < chars.len() {
        if is_cjk(chars[i]) {
            // Collect CJK run
            let start = i;
            while i < chars.len() && is_cjk(chars[i]) {
                i += 1;
            }
            let cjk_chars = &chars[start..i];
            if cjk_chars.len() == 1 {
                tokens.push(cjk_chars[0].to_string());
            } else {
                for window in cjk_chars.windows(2) {
                    tokens.push(window.iter().collect::<String>());
                }
            }
        } else {
            // Collect non-CJK run
            let start = i;
            while i < chars.len() && !is_cjk(chars[i]) {
                i += 1;
            }
            let segment: String = chars[start..i].iter().collect();
            for word in segment.split_whitespace() {
                let clean: String = word.chars().filter(|c| c.is_alphanumeric()).collect();
                if !clean.is_empty() {
                    tokens.push(clean);
                }
            }
        }
    }

    tokens
}

fn is_cjk(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c)
}

/// Perform BM25 search over a set of documents.
///
/// `docs`: iterator of (file_name, paragraph_text) pairs
/// `query`: search query string
/// `limit`: max results to return
pub fn bm25_search(
    docs: &[(String, String)],
    query: &str,
    limit: usize,
) -> Vec<SearchResult> {
    if docs.is_empty() {
        return Vec::new();
    }

    let query_tokens = tokenize(query);
    if query_tokens.is_empty() {
        return Vec::new();
    }

    // Tokenize all documents
    let doc_tokens: Vec<Vec<String>> = docs.iter().map(|(_, text)| tokenize(text)).collect();

    let n = docs.len() as f64;
    let avg_dl: f64 = doc_tokens.iter().map(|d| d.len() as f64).sum::<f64>() / n;

    // Compute document frequency for each query token
    let mut df: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for qt in &query_tokens {
        if df.contains_key(qt.as_str()) {
            continue;
        }
        let count = doc_tokens
            .iter()
            .filter(|d| d.iter().any(|t| t == qt))
            .count();
        df.insert(qt.as_str(), count);
    }

    // Score each document
    let mut results: Vec<SearchResult> = docs
        .iter()
        .enumerate()
        .filter_map(|(i, (file, snippet))| {
            let dl = doc_tokens[i].len() as f64;
            let mut score = 0.0_f64;

            for qt in &query_tokens {
                let tf = doc_tokens[i].iter().filter(|t| *t == qt).count() as f64;
                if tf == 0.0 {
                    continue;
                }
                let df_val = *df.get(qt.as_str()).unwrap_or(&0) as f64;
                let idf = ((n - df_val + 0.5) / (df_val + 0.5) + 1.0).ln();
                score += idf * (tf * (K1 + 1.0)) / (tf + K1 * (1.0 - B + B * dl / avg_dl));
            }

            if score > 0.0 {
                Some(SearchResult {
                    file: file.clone(),
                    snippet: snippet.clone(),
                    score,
                })
            } else {
                None
            }
        })
        .collect();

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_ascii() {
        let tokens = tokenize("Hello World foo-bar");
        assert_eq!(tokens, vec!["hello", "world", "foobar"]);
    }

    #[test]
    fn tokenize_cjk_bigrams() {
        let tokens = tokenize("打开浏览器");
        assert_eq!(tokens, vec!["打开", "开浏", "浏览", "览器"]);
    }

    #[test]
    fn tokenize_single_cjk() {
        let tokens = tokenize("是");
        assert_eq!(tokens, vec!["是"]);
    }

    #[test]
    fn tokenize_mixed() {
        let tokens = tokenize("修了 plist 权限问题");
        assert_eq!(tokens, vec!["修了", "plist", "权限", "限问", "问题"]);
    }

    #[test]
    fn tokenize_empty() {
        assert!(tokenize("").is_empty());
        assert!(tokenize("   ").is_empty());
    }

    #[test]
    fn bm25_basic_search() {
        let docs = vec![
            ("a.md".into(), "如何打开 Safari 浏览器".into()),
            ("b.md".into(), "Python 配置文件处理".into()),
            ("c.md".into(), "浏览器书签管理".into()),
        ];

        let results = bm25_search(&docs, "浏览器", 5);
        assert!(!results.is_empty());
        // The docs containing "浏览器" should rank highest
        assert!(results[0].snippet.contains("浏览") || results[0].snippet.contains("览器"));
    }

    #[test]
    fn bm25_empty_query() {
        let docs = vec![("a.md".into(), "hello".into())];
        let results = bm25_search(&docs, "", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn bm25_no_match() {
        let docs = vec![("a.md".into(), "hello world".into())];
        let results = bm25_search(&docs, "你好", 5);
        assert!(results.is_empty());
    }

    #[test]
    fn bm25_respects_limit() {
        let docs: Vec<(String, String)> = (0..20)
            .map(|i| (format!("{i}.md"), format!("document about topic {i}")))
            .collect();
        let results = bm25_search(&docs, "document topic", 3);
        assert!(results.len() <= 3);
    }
}
