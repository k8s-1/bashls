use lsp_types::{Position, Range};

#[must_use]
pub const fn is_position_in_range(pos: Position, range: Range) -> bool {
    let before_start = pos.line < range.start.line
        || (pos.line == range.start.line && pos.character < range.start.character);
    let after_end = pos.line > range.end.line
        || (pos.line == range.end.line && pos.character > range.end.character);
    !before_start && !after_end
}
