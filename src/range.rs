use crate::analyze_react_boundary::check::types;
use oxc::span::Span;

/// Convert a byte offset to line and column position
fn offset_to_position(source: &str, offset: u32) -> types::Position {
    let mut line = 0;
    let mut character = 0;

    for (i, ch) in source.char_indices() {
        if i >= offset as usize {
            break;
        }
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }

    types::Position { line, character }
}

/// Convert a Span to a Range
pub(crate) fn span_to_range(source: &str, span: Span) -> types::Range {
    types::Range {
        start: offset_to_position(source, span.start),
        end: offset_to_position(source, span.end),
    }
}

/// Convert a string literal Span to a Range positioned inside the string (after the opening quote)
/// This is useful for import sources where we need the position inside the quoted string
pub(crate) fn string_literal_to_range(source: &str, span: Span) -> types::Range {
    types::Range {
        start: offset_to_position(source, span.start + 1), // +1 to skip opening quote
        end: offset_to_position(source, span.end - 1),     // -1 to skip closing quote
    }
}
