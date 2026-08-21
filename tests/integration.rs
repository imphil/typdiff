use typdiff::diff::diff;
use typdiff::parse::parse;
use typdiff::render::render;

/// True when the rendered output is syntactically valid Typst.
fn parses(rendered: &str) -> bool {
    !typst_syntax::parse(rendered).erroneous()
}

fn run_diff(old: &str, new: &str) -> String {
    let old_blocks: Vec<_> = parse(old)
        .into_iter()
        .filter(|b| !matches!(b, typdiff::Block::Parbreak))
        .collect();
    let new_blocks: Vec<_> = parse(new)
        .into_iter()
        .filter(|b| !matches!(b, typdiff::Block::Parbreak))
        .collect();
    let results = diff(&old_blocks, &new_blocks);
    render(&results)
}

#[test]
fn test_identical_documents() {
    let src = "= Title\n\nHello world.\n";
    let output = run_diff(src, src);
    assert!(output.contains("= Title"));
    assert!(output.contains("Hello world."));
    // The preamble defines diff-added/diff-deleted, but the body should not use them.
    assert!(!output.contains("#diff-added["));
    assert!(!output.contains("#diff-deleted["));
}

#[test]
fn test_heading_change() {
    let old = "= Introduction\n\nSome text.\n";
    let new = "= Background\n\nSome text.\n";
    let output = run_diff(old, new);
    assert!(output.contains("diff-deleted"));
    assert!(output.contains("diff-added"));
    assert!(output.contains("Some text."));
}

#[test]
fn test_paragraph_word_change() {
    let old = "= Title\n\nThis is the old text.\n";
    let new = "= Title\n\nThis is the new text.\n";
    let output = run_diff(old, new);
    // Title should be unchanged.
    assert!(output.contains("= Title"));
    // "old" should be deleted, "new" should be added.
    assert!(output.contains("#diff-deleted[old]"));
    assert!(output.contains("#diff-added[new]"));
}

#[test]
fn test_added_paragraph() {
    let old = "= Title\n";
    let new = "= Title\n\nNew paragraph here.\n";
    let output = run_diff(old, new);
    assert!(output.contains("diff-added"));
}

#[test]
fn test_deleted_paragraph() {
    let old = "= Title\n\nOld paragraph here.\n";
    let new = "= Title\n";
    let output = run_diff(old, new);
    assert!(output.contains("diff-deleted"));
}

#[test]
fn test_list_items() {
    let old = "- Apple\n- Banana\n";
    let new = "- Apple\n- Cherry\n";
    let output = run_diff(old, new);
    assert!(output.contains("Apple"));
    // Banana should be marked as deleted or modified, Cherry as added.
    assert!(output.contains("diff-deleted") || output.contains("diff-added"));
}

#[test]
fn test_empty_to_content() {
    let old = "";
    let new = "= New Document\n\nContent here.\n";
    let output = run_diff(old, new);
    assert!(output.contains("diff-added"));
}

#[test]
fn test_content_to_empty() {
    let old = "= Old Document\n\nContent here.\n";
    let new = "";
    let output = run_diff(old, new);
    assert!(output.contains("diff-deleted"));
}

#[test]
fn test_output_contains_preamble() {
    let output = run_diff("Hello\n", "World\n");
    assert!(output.contains("#let diff-added(body)"));
    assert!(output.contains("#let diff-deleted(body)"));
    assert!(output.contains("underline"));
    assert!(output.contains("strike"));
}

#[test]
fn test_multiline_paragraph_change() {
    let old = "First line and second part.\n";
    let new = "First line and third part.\n";
    let output = run_diff(old, new);
    assert!(output.contains("#diff-deleted[second]"));
    assert!(output.contains("#diff-added[third]"));
}

#[test]
fn test_inline_footnote_ref_to_funccall() {
    // When @ref changes to #footnote[...] in a long paragraph, it should produce
    // Modified with word-level diff. The paragraph must be long enough for
    // the similarity ratio to exceed 0.5.
    let old = "This is a fairly long paragraph that discusses various topics in some detail. The key finding was reported by Smith et al. in their landmark study @smith_2024. Further research is needed to confirm these results across different settings and conditions.\n\nAnother paragraph.\n";
    let new = "This is a fairly long paragraph that discusses various topics in some detail. The key finding was reported by Smith et al. in their landmark study #footnote[https://example.com/papers/smith2024.pdf accessed: 2025/01/15]. Further research is needed to confirm these results across different settings and conditions.\n\nAnother paragraph.\n";
    let output = run_diff(old, new);
    // The surrounding text should be inline with diff spans (Modified), not whole-block Delete+Add
    assert!(
        output.contains("study #diff-deleted["),
        "surrounding text should be inline with diff spans: {output}"
    );
}

#[test]
fn test_markup_reference_paragraph() {
    let old = std::fs::read_to_string("tests/fixtures/markup-ref-start-old.typ").unwrap();
    let new = std::fs::read_to_string("tests/fixtures/markup-ref-start-new.typ").unwrap();
    let output = run_diff(&old, &new);

    // the diff should remain a single paragraph with no blank line inserted.
    assert!(
        !output.contains("#[@foo]\n\n"),
        "unexpected blank line: {output}"
    );
    assert!(output.contains("#[@foo]のような"));
}

#[test]
fn test_renamed_label_is_not_diffed_inside_label_syntax() {
    let old = "= Sample\n\n<sample-widget-anchor>\n\nBody.\n";
    let new = "= Sample\n\n<sample-widget_anchor>\n\nBody.\n";
    let output = run_diff(old, new);

    assert!(output.contains("<sample-widget_anchor>"));
    assert!(!output.contains("<#diff-added"));
    assert!(!output.contains("#diff-added[_]"));
    assert!(!output.contains("#diff-deleted[-]"));
}

#[test]
fn test_unchanged_label_stays_outside_added_paragraph() {
    let old = r#"#set heading(numbering: "1.")

See #ref(<sample-anchor>, supplement: [Section]).

= Sample

<sample-anchor>
Alpha beta gamma delta epsilon zeta eta theta.
"#;
    let new = r#"#set heading(numbering: "1.")

See #ref(<sample-anchor>, supplement: [Section]).

= Sample

<sample-anchor>
Inserted paragraph before old text.

Alpha beta gamma delta changed epsilon zeta eta theta.
"#;
    let output = run_diff(old, new);

    assert!(output.contains("<sample-anchor>"));
    assert!(output.contains("#diff-added[Inserted paragraph before old text.]"));
    assert!(output.contains("Alpha beta gamma delta #diff-added[changed ]epsilon"));
    assert!(!output.contains("#diff-added[<sample-anchor>"));
    assert!(!output.contains("#diff-deleted[\\<sample-anchor>"));
}

#[test]
fn test_replaced_footnote_ref_label_is_not_escaped_on_deleted_side() {
    let old = r#"The bridge inspection memo kept its summary sentence for editors#footnote[
Earlier notes pointed reviewers to #ref(<sec-bridge-ledger>, supplement: [])
while the field log was being reconciled.
]. The closing sentence stays fixed so the paragraph can align.
"#;
    let new = r#"The bridge inspection memo kept its summary sentence for editors#footnote[
Current notes point reviewers to #ref(<sec-bridge-ledger>, supplement: [])
after the field log was reconciled.
]. The closing sentence stays fixed so the paragraph can align.
"#;
    let output = run_diff(old, new);

    assert!(output.contains("#diff-deleted[#footnote["));
    assert!(output.contains("#ref(<sec-bridge-ledger>, supplement: [])"));
    assert!(!output.contains("#ref(\\<sec-bridge-ledger>"));
}

#[test]
fn test_existing_backslash_escape_is_not_double_escaped() {
    // `\*` and `\[` are already literal. Escaping the backslash instead would
    // hand the `*`/`[` back to Typst as markup and reopen the delimiter.
    let old = "Foo bar.\n";
    let new = "Foo \\*baz* and \\[qux bar.\n";
    let output = run_diff(old, new);

    assert!(
        !output.contains("\\\\*") && !output.contains("\\\\["),
        "an existing escape must not be escaped a second time: {output}"
    );
}

#[test]
fn test_deleted_bracket_split_from_its_closer_is_escaped() {
    // Removing the "[x]" wrapper puts the deleted `[` and `]` in separate
    // spans. A bare `[` would open a nested block closed by the renderer's own
    // `]`, leaving the outer `#diff-deleted[...]` call unclosed.
    let old = "Value [x] pairs.\n";
    let new = "Value x pairs.\n";
    let output = run_diff(old, new);

    assert!(
        output.contains("#diff-deleted[\\[]"),
        "the split-off `[` should be escaped: {output}"
    );
}

#[test]
fn test_bracket_orphaned_in_unchanged_text_is_escaped() {
    // Escaping the `[` that moved into a diff span leaves its `]` behind in
    // unchanged text with nothing to pair with, which Typst rejects outright.
    let old = "[a] [b]\n";
    let new = "[ [b]\n";
    let output = run_diff(old, new);

    assert!(
        output.contains("\\[#diff-deleted["),
        "the orphaned `[` should be escaped: {output}"
    );
    assert!(parses(&output), "output must compile: {output}");
}

#[test]
fn test_content_block_split_across_spans_is_kept() {
    // Here the brackets still pair up around the diff span, so they stay a
    // content block rather than becoming literal text.
    let old = "text [note] here\n";
    let new = "text [other] here\n";
    let output = run_diff(old, new);

    assert!(
        output.contains("text [#diff-deleted[note]") && output.contains("] here"),
        "balanced brackets should be left alone: {output}"
    );
    assert!(!output.contains("\\["), "output: {output}");
}

#[test]
fn test_deleted_lone_asterisk_is_escaped() {
    // The deleted "*" ends up alone in its span, touching only the wrapper's
    // brackets, so nothing closes the emphasis it would open.
    let old = "Rate is 5 * 3 apples.\n";
    let new = "Rate is 5 x 3 apples.\n";
    let output = run_diff(old, new);

    assert!(output.contains("#diff-deleted[\\*]"), "output: {output}");
}

#[test]
fn test_added_paragraph_keeps_bold_and_italic() {
    // Emphasis that pairs up inside the span is real markup and must render as
    // such, not be flattened into literal asterisks.
    let old = "Intro.\n";
    let new = "Intro.\n\nThis is *important* and _emphasized_.\n";
    let output = run_diff(old, new);

    assert!(
        output.contains("#diff-added[This is *important* and _emphasized_.]"),
        "self-contained emphasis should survive: {output}"
    );
}

#[test]
fn test_escaped_asterisk_after_word_does_not_break_emphasis_parsing() {
    // Regression test for https://github.com/sou1118/typdiff/issues/18.
    // The `*` in "Tester*innen" is literal only because a word character sits
    // on each side. Whichever side the diff call lands on, the `*` must not be
    // left as a bare marker with nothing to pair with.
    let old = "Tester*innen testen Tests.\n";
    let new = "Tester\\*innen testen Tests.\n";
    let output = run_diff(old, new);

    assert!(
        !output.contains("]*"),
        "a bare '*' must not directly follow a diff span's closing ']': {output}"
    );
    assert!(parses(&output), "output must compile: {output}");
}

#[test]
fn test_closing_emphasis_marker_after_diff_span_is_left_alone() {
    // The `*` closes emphasis opened earlier in the paragraph; escaping it
    // would leave the opening `*` without a partner.
    let old = "*\u{65e5}\u{672c}* x\n";
    let new = "*\u{65e5}\u{672c}X* x\n";
    let output = run_diff(old, new);

    assert!(
        output.contains("]* x"),
        "a closing emphasis marker must stay a marker: {output}"
    );
}

#[test]
fn test_marker_before_diff_span_is_escaped() {
    // Mirror of the "Tester*innen" case: here the diff span replaces the word
    // character to the right of the `*`, so the `*` loses its literalness.
    let old = "Tester*innen testen.\n";
    let new = "Tester* testen.\n";
    let output = run_diff(old, new);

    assert!(
        output.contains("Tester\\*#diff-deleted["),
        "the `*` must be escaped once the word after it is deleted: {output}"
    );
}

#[test]
fn test_escape_sequence_is_not_split_across_a_diff_call() {
    // The diff boundary would otherwise fall inside `\]`, stranding the
    // backslash at the end of the unchanged text where it escapes the `#` of
    // the call that follows, printing "#diff-deleted[...]" as plain text.
    let old = "\\[x\\] [grp]\n";
    let new = "\\[x\\]\n";
    let output = run_diff(old, new);

    assert!(
        !output.contains("\\#diff-deleted"),
        "a stranded backslash must not escape the call: {output}"
    );
    assert!(output.contains("\\[x\\]#diff-deleted["), "output: {output}");
}

#[test]
fn test_indented_line_comment_does_not_split_paragraph() {
    let old = "First sentence.\n  // indented comment\nSecond sentence.\n";
    let new = "First sentence.\n  // indented comment\nSecond sentence changed.\n";
    let output = run_diff(old, new);

    assert!(output.contains("First sentence.\nSecond sentence"));
    // A whitespace-only line would be treated as a paragraph break by Typst.
    assert!(!output.contains("\n  \n"));
}
