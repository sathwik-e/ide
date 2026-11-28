//! Editor commands developers expect: toggle comment, move / duplicate /
//! delete lines, select next occurrence. Each applies as one undoable edit.

use std::ops::Range;

use gpui_kit::component::input::EditorState;
use gpui_kit::{Context, Window};

/// Line comment token for a language, or a block pair for markup.
fn comment_tokens(language: &str) -> (&'static str, &'static str) {
    match language {
        "python" | "bash" | "ruby" | "yaml" | "toml" | "make" | "dockerfile" | "r" | "perl" | "pl"
        | "elixir" | "nim" | "cmake" | "graphql" | "ini" | "conf" | "fish" | "zsh" | "sh"
        | "ps1" | "jl" | "julia" | "coffee" | "tcl" | "properties" => ("# ", ""),
        "sql" | "lua" | "hs" | "haskell" | "elm" | "ada" | "vhdl" => ("-- ", ""),
        "lisp" | "clojure" | "clj" | "scheme" | "el" | "asm" | "ini_semicolon" => ("; ", ""),
        "tex" | "latex" | "erlang" | "erl" | "matlab" | "m_matlab" => ("% ", ""),
        "html" | "xml" | "markdown" | "md" | "svg" | "vue" | "svelte" | "astro" | "xhtml" => ("<!-- ", " -->"),
        "css" => ("/* ", " */"),
        "vb" | "vbs" => ("' ", ""),
        "bat" | "cmd" => ("REM ", ""),
        _ => ("// ", ""),
    }
}

/// Byte range of the whole lines touched by `sel` (without the final newline).
fn line_span(text: &str, sel: &Range<usize>) -> Range<usize> {
    let start = text[..sel.start.min(text.len())].rfind('\n').map_or(0, |i| i + 1);
    let mut end_anchor = sel.end.min(text.len());
    // A selection ending at column 0 doesn't include that line.
    if sel.end > sel.start && end_anchor > start && text.as_bytes().get(end_anchor - 1) == Some(&b'\n') {
        end_anchor -= 1;
    }
    let end = text[end_anchor..].find('\n').map_or(text.len(), |i| end_anchor + i);
    start..end
}

fn apply(editor: &mut EditorState, edits: Vec<(Range<usize>, String)>, select: Option<Range<usize>>, window: &mut Window, cx: &mut Context<EditorState>) {
    if edits.is_empty() {
        return;
    }
    editor.replace_text_in_ranges(&edits, window, cx);
    if let Some(range) = select {
        let len = editor.value().len();
        editor.set_selected_range(range.start.min(len)..range.end.min(len), cx);
    }
}

pub fn toggle_comment(editor: &mut EditorState, language: &str, window: &mut Window, cx: &mut Context<EditorState>) {
    let text = editor.value().to_string();
    let sel = editor.selected_range();
    let span = line_span(&text, &sel);
    let (open, close) = comment_tokens(language);
    let open_trim = open.trim_end();
    let close_trim = close.trim_start();
    let lines: Vec<(usize, &str)> = {
        let mut out = Vec::new();
        let mut offset = span.start;
        for line in text[span.clone()].split('\n') {
            out.push((offset, line));
            offset += line.len() + 1;
        }
        out
    };
    let non_empty: Vec<&(usize, &str)> = lines.iter().filter(|(_, l)| !l.trim().is_empty()).collect();
    if non_empty.is_empty() {
        return;
    }
    let all_commented = non_empty.iter().all(|(_, l)| {
        let t = l.trim_start();
        t.starts_with(open_trim) && (close_trim.is_empty() || t.trim_end().ends_with(close_trim))
    });
    let indent = non_empty
        .iter()
        .map(|(_, l)| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    let mut edits = Vec::new();
    for (start, line) in non_empty {
        if all_commented {
            let lead = line.len() - line.trim_start().len();
            let body = &line[lead..];
            let token_len = if body.starts_with(open) { open.len() } else { open_trim.len() };
            edits.push((start + lead..start + lead + token_len, String::new()));
            if !close_trim.is_empty() {
                let trimmed_end = line.trim_end();
                let close_len = if trimmed_end.ends_with(close) { close.len() } else { close_trim.len() };
                let end = start + trimmed_end.len();
                edits.push((end - close_len..end, String::new()));
            }
        } else {
            edits.push((start + indent..start + indent, open.to_string()));
            if !close.is_empty() {
                let end = start + line.trim_end().len();
                edits.push((end..end, close.to_string()));
            }
        }
    }
    edits.sort_by_key(|(r, _)| r.start);
    apply(editor, edits, None, window, cx);
}

pub fn move_lines(editor: &mut EditorState, up: bool, window: &mut Window, cx: &mut Context<EditorState>) {
    let text = editor.value().to_string();
    let sel = editor.selected_range();
    let span = line_span(&text, &sel);
    let block = text[span.clone()].to_string();
    if up {
        if span.start == 0 {
            return;
        }
        let prev_start = text[..span.start - 1].rfind('\n').map_or(0, |i| i + 1);
        let prev = &text[prev_start..span.start - 1];
        let new = format!("{block}\n{prev}");
        let shift = span.start - prev_start;
        apply(
            editor,
            vec![(prev_start..span.end, new)],
            Some(sel.start - shift..sel.end - shift),
            window,
            cx,
        );
    } else {
        if span.end >= text.len() {
            return;
        }
        let next_end = text[span.end + 1..].find('\n').map_or(text.len(), |i| span.end + 1 + i);
        let next = &text[span.end + 1..next_end];
        let new = format!("{next}\n{block}");
        let shift = next.len() + 1;
        apply(
            editor,
            vec![(span.start..next_end, new)],
            Some(sel.start + shift..sel.end + shift),
            window,
            cx,
        );
    }
}

pub fn duplicate_lines(editor: &mut EditorState, down: bool, window: &mut Window, cx: &mut Context<EditorState>) {
    let text = editor.value().to_string();
    let sel = editor.selected_range();
    let span = line_span(&text, &sel);
    let block = text[span.clone()].to_string();
    let shift = if down { block.len() + 1 } else { 0 };
    apply(
        editor,
        vec![(span.end..span.end, format!("\n{block}"))],
        Some(sel.start + shift..sel.end + shift),
        window,
        cx,
    );
}

pub fn delete_lines(editor: &mut EditorState, window: &mut Window, cx: &mut Context<EditorState>) {
    let text = editor.value().to_string();
    let sel = editor.selected_range();
    let span = line_span(&text, &sel);
    let (start, end) = if span.end < text.len() {
        (span.start, span.end + 1)
    } else if span.start > 0 {
        (span.start - 1, span.end)
    } else {
        (span.start, span.end)
    };
    apply(editor, vec![(start..end, String::new())], Some(start..start), window, cx);
}

fn word_at(text: &str, offset: usize) -> Option<Range<usize>> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let offset = offset.min(text.len());
    let start = text[..offset]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map_or(offset, |(i, _)| i);
    let end = text[offset..]
        .char_indices()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map_or(offset, |(i, c)| offset + i + c.len_utf8());
    (end > start).then_some(start..end)
}

pub fn select_next_occurrence(editor: &mut EditorState, cx: &mut Context<EditorState>) {
    let text = editor.value().to_string();
    let sel = editor.selected_range();
    if sel.is_empty() {
        if let Some(word) = word_at(&text, sel.start) {
            editor.set_selected_range(word, cx);
        }
        return;
    }
    let needle = &text[sel.clone()];
    let found = text[sel.end..]
        .find(needle)
        .map(|i| sel.end + i)
        .or_else(|| text[..sel.start].find(needle));
    if let Some(start) = found {
        editor.add_selection(start..start + needle.len(), cx);
    }
}

/// Where `offset` lands after applying `edits` (sorted, non-overlapping,
/// in pre-edit coordinates). Text inserted exactly at the offset goes after
/// it, so a final newline added at the cursor doesn't push the cursor down.
pub fn shift_offset(offset: usize, edits: &[(Range<usize>, String)]) -> usize {
    let mut delta: isize = 0;
    for (range, new) in edits {
        if range.start >= offset {
            break;
        }
        if range.end <= offset {
            delta += new.len() as isize - range.len() as isize;
        } else {
            // The cursor was inside removed text: put it where that text began.
            return (range.start as isize + delta) as usize;
        }
    }
    (offset as isize + delta).max(0) as usize
}

/// Minimal edits that trim trailing whitespace and ensure a final newline.
pub fn tidy_edits(text: &str, trim: bool, final_newline: bool) -> Vec<(Range<usize>, String)> {
    let mut edits = Vec::new();
    if trim {
        let mut offset = 0;
        for line in text.split('\n') {
            let kept = line.trim_end_matches([' ', '\t']).len();
            if kept < line.len() {
                edits.push((offset + kept..offset + line.len(), String::new()));
            }
            offset += line.len() + 1;
        }
    }
    if final_newline && !text.is_empty() && !text.ends_with('\n') {
        edits.push((text.len()..text.len(), "\n".to_string()));
    }
    edits
}

/// Text after removing trailing whitespace and ensuring one final newline.
#[cfg(test)]
pub fn tidy(text: &str, trim: bool, final_newline: bool) -> String {
    let mut out: String = if trim {
        text.split('\n').map(|l| l.trim_end_matches([' ', '\t', '\r'])).collect::<Vec<_>>().join("\n")
    } else {
        text.to_string()
    };
    if final_newline && !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_span_covers_selection() {
        let t = "one\ntwo\nthree";
        assert_eq!(&t[line_span(t, &(5..5))], "two");
        assert_eq!(&t[line_span(t, &(1..6))], "one\ntwo");
        // Selection ending at the start of a line excludes it.
        assert_eq!(&t[line_span(t, &(0..4))], "one");
    }

    #[test]
    fn tidy_trims_and_adds_newline() {
        assert_eq!(tidy("a  \nb\t", true, true), "a\nb\n");
        assert_eq!(tidy("a  ", false, true), "a  \n");
        assert_eq!(tidy("", true, true), "");
    }

    #[test]
    fn cursor_stays_put_through_cleanup() {
        // Typing at the end: adding the final newline must not move the cursor.
        let text = "print(1)";
        let edits = tidy_edits(text, true, true);
        assert_eq!(shift_offset(text.len(), &edits), text.len());
        // Trailing spaces earlier in the file shift the cursor back.
        let text = "a  \nbc";
        let edits = tidy_edits(text, true, false);
        assert_eq!(shift_offset(6, &edits), 4); // after "bc"
        // Cursor inside trimmed whitespace lands at the end of the kept text.
        assert_eq!(shift_offset(2, &edits), 1);
    }

    #[test]
    fn word_detection() {
        assert_eq!(word_at("let foo_bar = 1", 6), Some(4..11));
        assert_eq!(word_at("a + b", 2), None);
    }
}
