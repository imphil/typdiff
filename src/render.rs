use std::fmt::Write;

use crate::{Block, BlockKind, DiffResult, DiffSpan, SpanTag, TypstLabel};

const PREAMBLE: &str = r##"#let diff-added(body) = {
  set text(fill: rgb("#0000ff"))
  underline(body)
}
#let diff-deleted(body) = {
  set text(fill: rgb("#cc0000"))
  strike(body)
}
"##;

/// Render a list of diff results into a compilable Typst source string.
pub fn render(results: &[DiffResult]) -> String {
    let mut out = String::new();
    out.push_str(PREAMBLE);
    out.push('\n');

    for result in results {
        match result {
            DiffResult::Unchanged(block) => {
                render_block(block, &mut out);
                out.push('\n');
            }
            DiffResult::Added(block) => {
                render_added_or_deleted(block, DiffTag::Added, &mut out);
                out.push('\n');
            }
            DiffResult::Deleted(block) => {
                render_added_or_deleted(block, DiffTag::Deleted, &mut out);
                out.push('\n');
            }
            DiffResult::Modified { kind, spans } => {
                render_modified(kind, spans, &mut out);
                out.push('\n');
            }
        }
        // Always add a blank line between blocks (paragraph break).
        // Parbreaks are filtered out before diffing, so we insert them
        // unconditionally here instead.
        out.push('\n');
    }

    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DiffTag {
    Added,
    Deleted,
}

impl DiffTag {
    fn func_name(self) -> &'static str {
        match self {
            DiffTag::Added => "diff-added",
            DiffTag::Deleted => "diff-deleted",
        }
    }
}

/// Render a block unchanged.
fn render_block(block: &Block, out: &mut String) {
    match block {
        Block::Paragraph { source_text } => out.push_str(source_text),
        Block::Heading { depth, body_text } => {
            write_heading_prefix(*depth, out);
            out.push_str(body_text);
        }
        Block::ListItem { body_text } => {
            out.push_str("- ");
            out.push_str(body_text);
        }
        Block::EnumItem { number, body_text } => {
            write_enum_prefix(*number, out);
            out.push_str(body_text);
        }
        Block::TermItem { term, description } => {
            out.push_str("/ ");
            out.push_str(term);
            out.push_str(": ");
            out.push_str(description);
        }
        Block::RawBlock { content } => out.push_str(content),
        Block::Equation { content, .. } => out.push_str(content),
        Block::FuncCall { content } => out.push_str(content),
        Block::Parbreak => {} // newline added by caller
    }
}

/// Returns true if the text consists only of a Typst label `<...>`.
fn is_label_only(text: &str) -> bool {
    TypstLabel::is_only(text)
}

/// Render a block wrapped in #diff-added[...] or #diff-deleted[...].
fn render_added_or_deleted(block: &Block, tag: DiffTag, out: &mut String) {
    let escape_refs = tag == DiffTag::Deleted;
    let func = tag.func_name();

    match block {
        Block::Paragraph { source_text } => {
            // Labels must be output bare so they attach to the preceding element.
            // Deleted labels are suppressed to avoid duplicates with added labels.
            if is_label_only(source_text) {
                if tag == DiffTag::Added {
                    out.push_str(source_text.trim());
                }
                return;
            }
            write!(
                out,
                "#{}[{}]",
                func,
                escape_content(source_text, escape_refs)
            )
            .unwrap();
        }
        Block::Heading { depth, body_text } => {
            write_heading_prefix(*depth, out);
            write!(out, "#{}[{}]", func, escape_content(body_text, escape_refs)).unwrap();
        }
        Block::ListItem { body_text } => {
            out.push_str("- ");
            write!(out, "#{}[{}]", func, escape_content(body_text, escape_refs)).unwrap();
        }
        Block::EnumItem { number, body_text } => {
            write_enum_prefix(*number, out);
            write!(out, "#{}[{}]", func, escape_content(body_text, escape_refs)).unwrap();
        }
        Block::TermItem { term, description } => {
            out.push_str("/ ");
            write!(
                out,
                "#{}[{}: {}]",
                func,
                escape_content(term, escape_refs),
                escape_content(description, escape_refs)
            )
            .unwrap();
        }
        // Raw blocks and equations are content-mode; they can be wrapped.
        Block::RawBlock { content } | Block::Equation { content, .. } => {
            write!(out, "#{}[{}]", func, escape_content(content, escape_refs)).unwrap();
        }
        // FuncCall represents code-mode expressions (#import, #show, etc.).
        // Wrapping them in a content block would change their semantics,
        // so we output added code as-is and deleted code as comments.
        Block::FuncCall { content } => {
            if tag == DiffTag::Added {
                out.push_str(content);
            } else {
                for line in content.lines() {
                    write!(out, "// {line}").unwrap();
                    out.push('\n');
                }
                // Remove the trailing newline since the caller adds one.
                if out.ends_with('\n') {
                    out.pop();
                }
            }
        }
        Block::Parbreak => {} // paragraph breaks have no visual content to mark
    }
}

/// Render a modified block with per-span diff markup.
fn render_modified(kind: &BlockKind, spans: &[DiffSpan], out: &mut String) {
    match kind {
        BlockKind::Heading { depth } => write_heading_prefix(*depth, out),
        BlockKind::ListItem => out.push_str("- "),
        BlockKind::EnumItem { number } => write_enum_prefix(*number, out),
        BlockKind::TermItem => out.push_str("/ "),
        BlockKind::Paragraph => {}
        BlockKind::Atomic | BlockKind::Parbreak => {}
    }
    render_spans(spans, out);
}

/// Render diff spans with markup for deleted/inserted text.
fn render_spans(spans: &[DiffSpan], out: &mut String) {
    let stray_brackets = unbalanced_equal_brackets(spans);
    let mut prev_was_diff = false;
    for (i, span) in spans.iter().enumerate() {
        if span.text.is_empty() {
            continue;
        }
        match span.tag {
            SpanTag::Equal => {
                // After `#diff-added[...]` or `#diff-deleted[...]`, a `(` or `[`
                // would be parsed as function arguments by Typst. Insert a
                // zero-width space to break the call syntax.
                if prev_was_diff && (span.text.starts_with('(') || span.text.starts_with('[')) {
                    out.push('\u{200B}');
                }
                write_equal_text(&span.text, &stray_brackets[i], out);
                prev_was_diff = false;
            }
            SpanTag::Deleted => {
                if is_label_only(&span.text) {
                    // Nothing is emitted, so keep `prev_was_diff` tied to the
                    // last actual output.
                    continue;
                }
                write!(out, "#diff-deleted[{}]", escape_content(&span.text, true)).unwrap();
                prev_was_diff = true;
            }
            SpanTag::Inserted => {
                if is_label_only(&span.text) {
                    out.push_str(&span.text);
                    prev_was_diff = false;
                    continue;
                }
                write!(out, "#diff-added[{}]", escape_content(&span.text, false)).unwrap();
                prev_was_diff = true;
            }
        }
    }
}

/// Write the heading prefix (`= `, `== `, etc.).
fn write_heading_prefix(depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push('=');
    }
    out.push(' ');
}

/// Write the enum item prefix (`+ ` for auto, `{n}. ` for explicit).
fn write_enum_prefix(number: Option<usize>, out: &mut String) {
    match number {
        Some(n) => write!(out, "{}. ", n).unwrap(),
        None => out.push_str("+ "),
    }
}

/// Escape content so it can safely be placed inside a Typst content block `[...]`.
///
/// - Balanced `[...]` pairs are left untouched. An unbalanced `]` or `[` is
///   escaped, since either one would unbalance the caller's own brackets and
///   leave the `#diff-added[...]`/`#diff-deleted[...]` call unclosed.
/// - Emphasis markers (`*`/`_`) that would not pair up inside the block are
///   escaped; see `escape_markup`.
/// - Backslash escapes already in the source are passed through as-is, so an
///   escaped character is not escaped a second time.
/// - If the content ends with an odd number of backslashes, a trailing space is
///   appended to prevent the closing `]` from being interpreted as `\]`.
/// - When `escape_refs` is true, `@` is escaped as `\@` and `<` is escaped
///   as `\<` to suppress reference resolution and label creation (used for
///   deleted content where the referenced label may no longer exist or would
///   create duplicates). Label literals inside `#ref(<label>, ...)` are left
///   unchanged because they are code arguments rather than content labels.
fn escape_content(s: &str, escape_refs: bool) -> String {
    let result = escape_markup(s, escape_refs, false);
    // Whether the remaining `*`/`_` pair up is decided by Typst's recursive
    // markup grammar, so let Typst's own parser judge rather than reimplement
    // it. If it objects, fall back to keeping every marker that is not clearly
    // literal literal, trading in-span bold/italic for a block that compiles.
    if typst_syntax::parse(&format!("#diff-added[{result}]")).erroneous() {
        escape_markup(s, escape_refs, true)
    } else {
        result
    }
}

/// Escape `s` for use inside a content block. With `escape_markers`, every
/// `*`/`_` that Typst would not treat as literal text is escaped too.
fn escape_markup(s: &str, escape_refs: bool, escape_markers: bool) -> String {
    let mut result = String::with_capacity(s.len());
    // Byte offsets in `result` of `[` characters pushed so far that have not
    // yet been matched by a `]`. Any left over at the end get a `\` inserted
    // in front of them, escaping them retroactively.
    let mut unmatched_open_brackets: Vec<usize> = Vec::new();
    let mut chars = s.char_indices();
    while let Some((i, ch)) = chars.next() {
        match ch {
            // An escape sequence is already literal; copy it through whole so
            // the escaped character is not treated as markup below.
            '\\' => {
                result.push('\\');
                if let Some((_, escaped)) = chars.next() {
                    result.push(escaped);
                }
            }
            '[' => {
                unmatched_open_brackets.push(result.len());
                result.push(ch);
            }
            ']' => {
                if unmatched_open_brackets.pop().is_some() {
                    result.push(ch);
                } else {
                    result.push('\\');
                    result.push(']');
                }
            }
            '@' if escape_refs => {
                result.push('\\');
                result.push('@');
            }
            // Bare deleted labels are escaped so they do not create anchors.
            // In `#ref(<label>, ...)`, though, the label is code and must stay bare.
            '<' if escape_refs && !is_ref_label_arg(s, i) => {
                result.push('\\');
                result.push('<');
            }
            '*' | '_' if escape_markers && !is_word_adjacent_marker(s, i, ch) => {
                result.push('\\');
                result.push(ch);
            }
            _ => result.push(ch),
        }
    }
    // Escape any `[` that never found a matching `]` within this span, in
    // reverse order so earlier byte offsets stay valid as we insert.
    for pos in unmatched_open_brackets.into_iter().rev() {
        result.insert(pos, '\\');
    }
    // If the result ends with an odd number of backslashes, the closing `]`
    // added by the caller would be interpreted as `\]` (an escaped bracket).
    // Append a space to break the escape sequence.
    let trailing_backslashes = result.chars().rev().take_while(|&c| c == '\\').count();
    if trailing_backslashes % 2 != 0 {
        result.push(' ');
    }
    result
}

/// True when the `*`/`_` at byte offset `i` in `s` touches a word character on
/// both sides, which is Typst's rule for such a marker being literal text
/// rather than the start or end of emphasis. The ends of `s` never count, since
/// the caller wraps the text in `[...]`.
fn is_word_adjacent_marker(s: &str, i: usize, ch: char) -> bool {
    let prev_word = s[..i]
        .chars()
        .next_back()
        .is_some_and(char::is_alphanumeric);
    let next_word = s[i + ch.len_utf8()..]
        .chars()
        .next()
        .is_some_and(char::is_alphanumeric);
    prev_word && next_word
}

/// Write unchanged text, escaping the brackets at the byte offsets in
/// `brackets`.
fn write_equal_text(text: &str, brackets: &[usize], out: &mut String) {
    let mut chars = text.char_indices();
    while let Some((i, ch)) = chars.next() {
        match ch {
            '\\' => {
                out.push('\\');
                if let Some((_, escaped)) = chars.next() {
                    out.push(escaped);
                }
            }
            '[' | ']' if brackets.binary_search(&i).is_ok() => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
}

/// Byte offsets, per span, of brackets in unchanged text that nothing balances.
/// The `[...]` of a `#diff-added[...]`/`#diff-deleted[...]` call cannot pair
/// with them, so Typst would either report an unexpected closing bracket or
/// swallow the rest of the block into a content block that never closes.
fn unbalanced_equal_brackets(spans: &[DiffSpan]) -> Vec<Vec<usize>> {
    let mut stray = vec![Vec::new(); spans.len()];
    let mut open: Vec<(usize, usize)> = Vec::new();
    for (i, span) in spans.iter().enumerate() {
        if !matches!(span.tag, SpanTag::Equal) {
            continue;
        }
        let mut chars = span.text.char_indices();
        while let Some((j, ch)) = chars.next() {
            match ch {
                '\\' => {
                    chars.next();
                }
                '[' => open.push((i, j)),
                ']' => {
                    // A `]` with no `[` before it in unchanged text is stray.
                    let opener = open.pop();
                    if opener.is_none() {
                        stray[i].push(j);
                    }
                }
                _ => {}
            }
        }
    }
    for (i, j) in open {
        stray[i].push(j);
    }
    for offsets in &mut stray {
        offsets.sort_unstable();
    }
    stray
}

/// True when `label_start` points at the `<` in the first argument to `#ref(...)`.
fn is_ref_label_arg(s: &str, label_start: usize) -> bool {
    // First require a real Typst label literal. Then look left: the first
    // argument is preceded by `(`, and the call before that must be `#ref`.
    TypstLabel::end(&s[label_start..]).is_some()
        && s[..label_start]
            .trim_end()
            .strip_suffix('(')
            .is_some_and(|prefix| prefix.trim_end().ends_with("#ref"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_content_no_special() {
        assert_eq!(escape_content("hello world", false), "hello world");
    }

    #[test]
    fn test_escape_content_balanced_brackets() {
        assert_eq!(escape_content("a [b] c", false), "a [b] c");
    }

    #[test]
    fn test_escape_content_unbalanced_bracket() {
        assert_eq!(escape_content("a ] b", false), "a \\] b");
    }

    #[test]
    fn test_escape_content_unbalanced_open_bracket() {
        // A lone `[` would open a nested block that swallows the closing `]`.
        assert_eq!(escape_content("a [ b", false), "a \\[ b");
        assert_eq!(escape_content("[", false), "\\[");
        assert_eq!(escape_content("[a [b", false), "\\[a \\[b");
    }

    #[test]
    fn test_escape_content_preserves_existing_escapes() {
        // `\]` is already literal; escaping the backslash would hand the `]`
        // back to Typst and close the caller's content block early.
        assert_eq!(escape_content("a \\] b", false), "a \\] b");
        assert_eq!(escape_content("\\[x", false), "\\[x");
        assert_eq!(escape_content("\\*baz*", false), "\\*baz\\*");
    }

    #[test]
    fn test_escape_content_unpaired_marker_is_escaped() {
        // Nothing closes these, so they must not be left as emphasis markers.
        assert_eq!(escape_content("*", false), "\\*");
        assert_eq!(escape_content("_", false), "\\_");
        // Word-adjacent on one side only, so Typst reads it as a marker.
        assert_eq!(escape_content("word* ", false), "word\\* ");
        assert_eq!(escape_content(" *word", false), " \\*word");
    }

    #[test]
    fn test_escape_content_keeps_self_contained_emphasis() {
        // The markers pair up inside the block, so the styling survives.
        assert_eq!(escape_content("*bold*", false), "*bold*");
        assert_eq!(escape_content("_it_ and *bold*", false), "_it_ and *bold*");
        // Both neighbours are word characters, so this is literal text already.
        assert_eq!(escape_content("in*side", false), "in*side");
    }

    #[test]
    fn test_escape_content_unpaired_marker_forces_all_markers_literal() {
        // `*bold*` alone would pair, but the stray `*` does not, so the whole
        // span falls back to literal markers to keep the block closed.
        assert_eq!(escape_content("*bold* and *", false), "\\*bold\\* and \\*");
    }

    #[test]
    fn test_escape_content_trailing_backslash() {
        assert_eq!(escape_content("text\\", false), "text\\ ");
    }

    #[test]
    fn test_escape_content_trailing_double_backslash() {
        assert_eq!(escape_content("text\\\\", false), "text\\\\");
    }

    #[test]
    fn test_escape_content_refs_not_escaped_by_default() {
        assert_eq!(escape_content("see @ref here", false), "see @ref here");
    }

    #[test]
    fn test_escape_content_refs_escaped_when_requested() {
        assert_eq!(escape_content("see @ref here", true), "see \\@ref here");
    }

    #[test]
    fn test_escape_content_labels_escaped_when_requested() {
        assert_eq!(escape_content("text <my-label>", true), "text \\<my-label>");
    }

    #[test]
    fn test_escape_content_ref_call_label_not_escaped_when_requested() {
        assert_eq!(
            escape_content("compare #ref(<tbl-cobalt-notes>, supplement: [])", true),
            "compare #ref(<tbl-cobalt-notes>, supplement: [])"
        );
    }

    #[test]
    fn test_is_label_only() {
        assert!(is_label_only("<my-label>"));
        assert!(is_label_only("  <my-label>  "));
        assert!(!is_label_only("text <label>"));
        assert!(!is_label_only("<a> <b>"));
        assert!(!is_label_only("no label"));
    }

    #[test]
    fn test_render_preamble() {
        let output = render(&[]);
        assert!(output.contains("#let diff-added"));
        assert!(output.contains("#let diff-deleted"));
    }

    #[test]
    fn test_render_unchanged() {
        let results = vec![DiffResult::Unchanged(Block::Paragraph {
            source_text: "Hello".into(),
        })];
        let output = render(&results);
        assert!(output.contains("\nHello\n"));
        let body = output.split("\n\n").last().unwrap();
        assert!(!body.contains("#diff-added["));
        assert!(!body.contains("#diff-deleted["));
    }

    #[test]
    fn test_render_added_paragraph() {
        let results = vec![DiffResult::Added(Block::Paragraph {
            source_text: "New text".into(),
        })];
        let output = render(&results);
        assert!(output.contains("#diff-added[New text]"));
    }

    #[test]
    fn test_render_deleted_paragraph_escapes_refs() {
        let results = vec![DiffResult::Deleted(Block::Paragraph {
            source_text: "see @old-ref here".into(),
        })];
        let output = render(&results);
        assert!(output.contains("#diff-deleted[see \\@old-ref here]"));
    }

    #[test]
    fn test_render_label_only_paragraph_not_wrapped() {
        let results = vec![DiffResult::Added(Block::Paragraph {
            source_text: "<my-label>".into(),
        })];
        let output = render(&results);
        assert!(output.contains("<my-label>"));
        assert!(!output.contains("#diff-added[<my-label>]"));
    }

    #[test]
    fn test_render_modified() {
        let results = vec![DiffResult::Modified {
            kind: BlockKind::Paragraph,
            spans: vec![
                DiffSpan {
                    tag: SpanTag::Equal,
                    text: "Hello ".into(),
                },
                DiffSpan {
                    tag: SpanTag::Deleted,
                    text: "world".into(),
                },
                DiffSpan {
                    tag: SpanTag::Inserted,
                    text: "there".into(),
                },
            ],
        }];
        let output = render(&results);
        assert!(output.contains("Hello #diff-deleted[world]#diff-added[there]"));
    }

    #[test]
    fn test_render_modified_label_bare() {
        let results = vec![DiffResult::Modified {
            kind: BlockKind::Paragraph,
            spans: vec![
                DiffSpan {
                    tag: SpanTag::Deleted,
                    text: "<old-label>".into(),
                },
                DiffSpan {
                    tag: SpanTag::Inserted,
                    text: "<new-label>".into(),
                },
            ],
        }];
        let output = render(&results);
        assert!(output.contains("<new-label>"));
        assert!(!output.contains("#diff-added[<new-label>]"));
        assert!(!output.contains("#diff-deleted[\\<old-label>]"));
    }

    #[test]
    fn test_render_skipped_deleted_label_keeps_diff_call_guard() {
        let results = vec![DiffResult::Modified {
            kind: BlockKind::Paragraph,
            spans: vec![
                DiffSpan {
                    tag: SpanTag::Deleted,
                    text: "old".into(),
                },
                DiffSpan {
                    tag: SpanTag::Deleted,
                    text: "<old-label>".into(),
                },
                DiffSpan {
                    tag: SpanTag::Equal,
                    text: "(next)".into(),
                },
            ],
        }];
        let output = render(&results);
        assert!(output.contains("#diff-deleted[old]\u{200B}(next)"));
    }
}
