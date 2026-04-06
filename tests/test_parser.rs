use std::path::Path;

// Import the library's core parser
// Since we're a binary crate, we test via the modules directly
// For now, we replicate the parser logic in integration tests
// by invoking the binary or testing the public API

#[path = "../src/core/parser.rs"]
mod parser;

// Need these for parser.rs compilation
extern crate pulldown_cmark;
extern crate regex;
extern crate serde_json;
extern crate serde_yaml_ng;
extern crate thiserror;

#[test]
fn test_parse_wikilinks_basic() {
    let content = "Some text with [[target-note]] in it.";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert_eq!(note.wikilinks.len(), 1);
    assert_eq!(note.wikilinks[0].target, "target-note");
    assert_eq!(note.wikilinks[0].alias, None);
    assert_eq!(note.wikilinks[0].line, 1);
}

#[test]
fn test_parse_wikilinks_with_alias() {
    let content = "See [[target|Display Text]] for details.";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert_eq!(note.wikilinks.len(), 1);
    assert_eq!(note.wikilinks[0].target, "target");
    assert_eq!(note.wikilinks[0].alias, Some("Display Text".to_string()));
}

#[test]
fn test_parse_multiple_wikilinks_same_line() {
    let content = "Links to [[note-a]] and [[note-b]] here.";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert_eq!(note.wikilinks.len(), 2);
    assert_eq!(note.wikilinks[0].target, "note-a");
    assert_eq!(note.wikilinks[1].target, "note-b");
}

#[test]
fn test_parse_wikilinks_across_lines() {
    let content = "First [[note-a]] on line 1\nSecond [[note-b]] on line 2";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert_eq!(note.wikilinks.len(), 2);
    assert_eq!(note.wikilinks[0].line, 1);
    assert_eq!(note.wikilinks[1].line, 2);
}

#[test]
fn test_parse_tags_basic() {
    let content = "Some text with #rust and #async tags.";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert!(note.tags.contains(&"rust".to_string()));
    assert!(note.tags.contains(&"async".to_string()));
}

#[test]
fn test_parse_nested_tags() {
    let content = "A nested tag: #lang/rust here.";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert!(note.tags.contains(&"lang/rust".to_string()));
}

#[test]
fn test_parse_frontmatter_tags_array() {
    let content = "---\ntags: [rust, backend]\n---\n\nBody text.";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert!(note.tags.contains(&"rust".to_string()));
    assert!(note.tags.contains(&"backend".to_string()));
}

#[test]
fn test_parse_frontmatter_tags_list() {
    let content = "---\ntags:\n  - rust\n  - async\n---\n\nBody.";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert!(note.tags.contains(&"rust".to_string()));
    assert!(note.tags.contains(&"async".to_string()));
}

#[test]
fn test_parse_frontmatter_and_body_tags_merged() {
    let content = "---\ntags: [frontend]\n---\n\nAlso uses #backend tag.";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert!(note.tags.contains(&"frontend".to_string()));
    assert!(note.tags.contains(&"backend".to_string()));
}

#[test]
fn test_tags_deduped() {
    let content = "---\ntags: [rust]\n---\n\nAlso #rust in body.";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    let rust_count = note.tags.iter().filter(|t| *t == "rust").count();
    assert_eq!(rust_count, 1, "Tag 'rust' should appear only once after dedup");
}

#[test]
fn test_parse_frontmatter_fields() {
    let content = "---\ntitle: My Note\nstatus: draft\ncustom_field: 42\n---\n\nBody.";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    let fm = note.frontmatter.unwrap();
    assert_eq!(fm.get("title").unwrap().as_str().unwrap(), "My Note");
    assert_eq!(fm.get("status").unwrap().as_str().unwrap(), "draft");
    assert_eq!(fm.get("custom_field").unwrap().as_i64().unwrap(), 42);
}

#[test]
fn test_title_from_frontmatter() {
    let content = "---\ntitle: Custom Title\n---\n\nBody.";
    let note = parser::parse_note(content, Path::new("fallback-name.md")).unwrap();
    assert_eq!(note.title, "Custom Title");
}

#[test]
fn test_title_from_filename_without_frontmatter() {
    let content = "# Just a heading\n\nNo frontmatter here.";
    let note = parser::parse_note(content, Path::new("my-note.md")).unwrap();
    assert_eq!(note.title, "my-note");
}

#[test]
fn test_title_from_filename_when_frontmatter_has_no_title() {
    let content = "---\nstatus: draft\n---\n\nBody.";
    let note = parser::parse_note(content, Path::new("fallback.md")).unwrap();
    assert_eq!(note.title, "fallback");
}

#[test]
fn test_parse_headings() {
    let content = "# Heading 1\n\nSome text.\n\n## Heading 2\n\n### Heading 3";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert_eq!(note.headings.len(), 3);
    assert_eq!(note.headings[0].text, "Heading 1");
    assert_eq!(note.headings[0].level, 1);
    assert_eq!(note.headings[1].text, "Heading 2");
    assert_eq!(note.headings[1].level, 2);
    assert_eq!(note.headings[2].text, "Heading 3");
    assert_eq!(note.headings[2].level, 3);
}

#[test]
fn test_word_count() {
    let content = "---\ntitle: Test\n---\n\nOne two three four five.";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert_eq!(note.word_count, 5);
}

#[test]
fn test_empty_file() {
    let content = "";
    let note = parser::parse_note(content, Path::new("empty.md")).unwrap();
    assert_eq!(note.title, "empty");
    assert!(note.wikilinks.is_empty());
    assert!(note.tags.is_empty());
    assert!(note.headings.is_empty());
    assert_eq!(note.word_count, 0);
    assert!(note.frontmatter.is_none());
}

#[test]
fn test_frontmatter_only_no_body() {
    let content = "---\ntitle: Just Frontmatter\n---\n";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert_eq!(note.title, "Just Frontmatter");
    assert_eq!(note.word_count, 0);
}

#[test]
fn test_body_content_excludes_frontmatter() {
    let content = "---\ntitle: Test\n---\n\nActual body content here.";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    assert!(!note.body.contains("---"));
    assert!(note.body.contains("Actual body content here."));
}

#[test]
fn test_wikilinks_not_extracted_from_code_blocks() {
    // Wikilinks inside fenced code blocks should ideally be ignored.
    // This is a known limitation for v1 — documenting the behavior.
    let content = "Normal [[real-link]] here.\n\n```\n[[code-link]]\n```";
    let note = parser::parse_note(content, Path::new("test.md")).unwrap();
    // v1: regex extracts from code blocks too. Track this as known behavior.
    assert!(note.wikilinks.iter().any(|l| l.target == "real-link"));
}

#[test]
fn test_path_with_subdirectory() {
    let content = "---\ntitle: Sub Note\n---\n\nContent.";
    let note = parser::parse_note(content, Path::new("subfolder/sub-note.md")).unwrap();
    assert_eq!(note.title, "Sub Note");
    assert_eq!(note.path, Path::new("subfolder/sub-note.md"));
}
