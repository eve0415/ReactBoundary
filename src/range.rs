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
        start: offset_to_position(source, span.start + 1), // +1 to skip the opening quote
        end: offset_to_position(source, span.end - 1),     // -1 to skip the closing quote
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc::span::Span;

    #[test]
    fn test_offset_to_position_single_line() {
        let source = "const x = 10;";
        let position = offset_to_position(source, 6); // Points to 'x'

        assert_eq!(position.line, 0);
        assert_eq!(position.character, 6);
    }

    #[test]
    fn test_offset_to_position_multi_line() {
        let source = "const x = 10;\nconst y = 20;";
        let position = offset_to_position(source, 20); // Points to 'y' on the second line

        assert_eq!(position.line, 1);
        assert_eq!(position.character, 6);
    }

    #[test]
    fn test_offset_to_position_start_of_line() {
        let source = "line1\nline2";
        let position = offset_to_position(source, 6); // Points to 'l' in 'line2'

        assert_eq!(position.line, 1);
        assert_eq!(position.character, 0);
    }

    #[test]
    fn test_offset_to_position_with_unicode() {
        let source = "const emoji = '😀';";
        // The emoji is multiple bytes, but character count should still work
        let position = offset_to_position(source, 14);

        assert_eq!(position.line, 0);
        // Character position should be after "const emoji = "
        assert_eq!(position.character, 14);
    }

    #[test]
    fn test_span_to_range_single_line() {
        let source = "const MyComponent = () => {};";
        let span = Span::new(6, 17); // "MyComponent"

        let range = span_to_range(source, span);

        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 6);
        assert_eq!(range.end.line, 0);
        assert_eq!(range.end.character, 17);
    }

    #[test]
    fn test_span_to_range_multi_line() {
        let source = "const MyComponent = () => {\n  return <div />;\n};";
        let span = Span::new(6, 17); // "MyComponent"

        let range = span_to_range(source, span);

        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 6);
        assert_eq!(range.end.line, 0);
        assert_eq!(range.end.character, 17);
    }

    #[test]
    fn test_string_literal_to_range_double_quotes() {
        let source = r#"import X from "./client";"#;
        // Span includes quotes: "./client" at positions 14-24
        let span = Span::new(14, 24);

        let range = string_literal_to_range(source, span);

        // Should skip opening quote at 14, start at 15 (the dot)
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 15);
        // Should skip closing quote at 23
        assert_eq!(range.end.line, 0);
        assert_eq!(range.end.character, 23);
    }

    #[test]
    fn test_string_literal_to_range_single_quotes() {
        let source = "import X from './client';";
        // Span includes quotes: './client' at positions 14-24
        let span = Span::new(14, 24);

        let range = string_literal_to_range(source, span);

        // Should position inside the string
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 15);
        assert_eq!(range.end.line, 0);
        assert_eq!(range.end.character, 23);
    }

    #[test]
    fn test_string_literal_to_range_multi_line() {
        let source = "const code = `\n  ./path\n`;";
        // Multi-line template literal
        let span = Span::new(13, 24); // `\n  ./path\n`

        let range = string_literal_to_range(source, span);

        // Should skip opening backtick
        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 14);
    }
}
