use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ParsedNote {
    pub path: PathBuf,
    pub title: String,
    pub frontmatter: Option<serde_json::Value>,
    pub wikilinks: Vec<WikiLink>,
    pub tags: Vec<String>,
    pub headings: Vec<Heading>,
    pub word_count: usize,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct WikiLink {
    pub target: String,
    pub alias: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Heading {
    pub text: String,
    pub level: u8,
    pub line: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("Failed to parse frontmatter: {0}")]
    Frontmatter(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Parse a Markdown note from its content string and file path.
/// The path is used only for metadata (title derivation) — no filesystem access.
pub fn parse_note(content: &str, path: &Path) -> Result<ParsedNote, ParserError> {
    let (frontmatter, body) = extract_frontmatter(content)?;
    let title = derive_title(&frontmatter, path);
    let wikilinks = extract_wikilinks(&body);
    let mut tags = extract_tags(&body);
    if let Some(ref fm) = frontmatter {
        extract_frontmatter_tags(fm, &mut tags);
    }
    tags.sort();
    tags.dedup();
    let headings = extract_headings(&body);
    let word_count = count_words(&body);

    Ok(ParsedNote {
        path: path.to_path_buf(),
        title,
        frontmatter,
        wikilinks,
        tags,
        headings,
        word_count,
        body,
    })
}

/// Split content into YAML frontmatter and body.
/// Frontmatter is delimited by --- at the start of the file.
fn extract_frontmatter(content: &str) -> Result<(Option<serde_json::Value>, String), ParserError> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Ok((None, content.to_string()));
    }

    // Find the closing ---
    let after_open = &trimmed[3..];
    let close_pos = after_open.find("\n---");
    match close_pos {
        None => Ok((None, content.to_string())),
        Some(pos) => {
            let yaml_str = &after_open[..pos].trim();
            let body_start = 3 + pos + 4; // "---" + yaml + "\n---"
            let body = trimmed[body_start..].trim_start_matches('\n').to_string();

            if yaml_str.is_empty() {
                return Ok((None, body));
            }

            // Parse YAML to serde_yaml_ng::Value, then convert to serde_json::Value
            let yaml_value: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml_str)
                .map_err(|e| ParserError::Frontmatter(e.to_string()))?;
            let json_value = yaml_to_json(yaml_value);

            Ok((Some(json_value), body))
        }
    }
}

fn yaml_to_json(yaml: serde_yaml_ng::Value) -> serde_json::Value {
    match yaml {
        serde_yaml_ng::Value::Null => serde_json::Value::Null,
        serde_yaml_ng::Value::Bool(b) => serde_json::Value::Bool(b),
        serde_yaml_ng::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::json!(f)
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml_ng::Value::String(s) => serde_json::Value::String(s),
        serde_yaml_ng::Value::Sequence(seq) => {
            serde_json::Value::Array(seq.into_iter().map(yaml_to_json).collect())
        }
        serde_yaml_ng::Value::Mapping(map) => {
            let obj = map
                .into_iter()
                .map(|(k, v)| {
                    let key = match k {
                        serde_yaml_ng::Value::String(s) => s,
                        other => format!("{other:?}"),
                    };
                    (key, yaml_to_json(v))
                })
                .collect();
            serde_json::Value::Object(obj)
        }
        serde_yaml_ng::Value::Tagged(tagged) => yaml_to_json(tagged.value),
    }
}

fn derive_title(frontmatter: &Option<serde_json::Value>, path: &Path) -> String {
    if let Some(fm) = frontmatter
        && let Some(title) = fm.get("title").and_then(|v| v.as_str())
        && !title.trim().is_empty()
    {
        return title.trim().to_string();
    }
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string()
}

fn extract_wikilinks(body: &str) -> Vec<WikiLink> {
    let re = regex::Regex::new(r"\[\[([^\]]+)\]\]").expect("valid regex");
    let mut links = Vec::new();

    for (line_idx, line) in body.lines().enumerate() {
        for cap in re.captures_iter(line) {
            let inner = &cap[1];
            let (target, alias) = if let Some(pos) = inner.find('|') {
                (inner[..pos].trim().to_string(), Some(inner[pos + 1..].trim().to_string()))
            } else {
                (inner.trim().to_string(), None)
            };
            links.push(WikiLink {
                target,
                alias,
                line: line_idx + 1,
            });
        }
    }

    links
}

fn extract_tags(body: &str) -> Vec<String> {
    let re = regex::Regex::new(r"(?:^|[\s,;(])#([\w][\w/\-]*)").expect("valid regex");
    let mut tags = Vec::new();

    for line in body.lines() {
        for cap in re.captures_iter(line) {
            tags.push(cap[1].to_string());
        }
    }

    tags
}

fn extract_frontmatter_tags(fm: &serde_json::Value, tags: &mut Vec<String>) {
    if let Some(fm_tags) = fm.get("tags") {
        match fm_tags {
            serde_json::Value::Array(arr) => {
                for t in arr {
                    if let Some(s) = t.as_str() {
                        tags.push(s.to_string());
                    }
                }
            }
            serde_json::Value::String(s) => {
                for t in s.split(',') {
                    let trimmed = t.trim().trim_start_matches('#');
                    if !trimmed.is_empty() {
                        tags.push(trimmed.to_string());
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_headings(body: &str) -> Vec<Heading> {
    use pulldown_cmark::{Event, Options, Parser, Tag, HeadingLevel};

    let parser = Parser::new_ext(body, Options::all());
    let mut headings = Vec::new();
    let mut current_heading_level: Option<u8> = None;
    let mut current_text = String::new();
    let mut last_offset = 0;

    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current_heading_level = Some(match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                });
                current_text.clear();
                last_offset = range.start;
            }
            Event::Text(text) if current_heading_level.is_some() => {
                current_text.push_str(&text);
            }
            Event::End(pulldown_cmark::TagEnd::Heading(_)) => {
                if let Some(level) = current_heading_level.take() {
                    let line = body[..last_offset].lines().count() + 1;
                    headings.push(Heading {
                        text: current_text.clone(),
                        level,
                        line,
                    });
                }
            }
            _ => {}
        }
    }

    headings
}

fn count_words(body: &str) -> usize {
    body.split_whitespace().count()
}
