use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::{cursor::MoveTo, queue, style::Print};
use std::io::Write;

const LEFT_FRAME_CHARS: usize = const_str::to_char_array!("│ > ").len();
const RIGHT_FRAME_CHARS: usize = const_str::to_char_array!("│").len();

/// The number of characters used for frame borders and prompt prefix
/// Format: "│ > " (4 chars) + "│" (1 char) = 5 chars total
pub const FRAME_CHARS: usize = LEFT_FRAME_CHARS + RIGHT_FRAME_CHARS;

/// Result of handling a keyboard event
#[derive(Debug, PartialEq)]
pub enum KeyAction {
    Continue,
    Exit,
}

/// State of the input application
#[derive(Debug, Clone)]
pub struct InputState {
    pub buffer: String,
    pub cols: usize,
    pub rows: usize,
    pub required_lines: usize,
}

impl InputState {
    pub fn new(cols: usize, rows: usize) -> Self {
        let required_lines = calculate_required_lines("", cols);
        Self {
            buffer: String::new(),
            cols,
            rows,
            required_lines,
        }
    }

    pub fn handle_key(&mut self, key_code: KeyCode, modifiers: KeyModifiers) -> KeyAction {
        match key_code {
            KeyCode::Esc => KeyAction::Exit,

            KeyCode::Char('c') | KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                KeyAction::Exit
            }

            KeyCode::Enter if modifiers.contains(KeyModifiers::ALT) => {
                self.buffer.push('\n');
                self.update_required_lines();
                KeyAction::Continue
            }

            KeyCode::Char('j') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.buffer.push('\n');
                self.update_required_lines();
                KeyAction::Continue
            }

            KeyCode::Enter => {
                // Submit is handled separately in the main loop
                KeyAction::Continue
            }

            KeyCode::Backspace => {
                self.buffer.pop();
                self.update_required_lines();
                KeyAction::Continue
            }

            KeyCode::Char(c) => {
                self.buffer.push(c);
                self.update_required_lines();
                KeyAction::Continue
            }

            _ => KeyAction::Continue,
        }
    }

    pub fn handle_resize(&mut self, new_cols: usize, new_rows: usize) {
        self.cols = new_cols;
        self.rows = new_rows;
        self.update_required_lines();
    }

    fn update_required_lines(&mut self) {
        self.required_lines = calculate_required_lines(&self.buffer, self.cols);
    }

    pub fn get_submitted_text(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            None
        } else {
            let result = self.buffer.clone();
            self.buffer.clear();
            self.update_required_lines();
            Some(result)
        }
    }
}

/// Calculates the number of terminal lines required to display the input box.
///
/// This function determines how many lines are needed for the complete input box,
/// including the top border, text content (which may wrap across multiple lines),
/// and bottom border. The minimum is always 3 lines (empty input with borders).
///
/// # Arguments
///
/// * `text` - The current input text to measure
/// * `cols` - The terminal width in columns
///
/// # Returns
///
/// The total number of lines needed for the input box frame and content.
pub fn calculate_required_lines(text: &str, cols: usize) -> usize {
    if text.is_empty() {
        return 3; // minimum: top border, input line, bottom border
    }
    let inner_width = cols - FRAME_CHARS;

    // Split text by newlines and calculate wrapped lines for each segment
    let mut total_lines = 0;
    for line in text.split('\n') {
        if line.is_empty() {
            total_lines += 1; // Empty lines still take up space
        } else {
            let wrapped_lines = (line.len() + inner_width - 1) / inner_width;
            total_lines += wrapped_lines.max(1);
        }
    }

    total_lines + 2 // add top and bottom borders
}

/// Calculates the exact cursor position for the current text input.
///
/// This function determines where the cursor should be positioned based on the
/// current text length, accounting for text wrapping within the input box.
/// The position is calculated relative to the input box boundaries.
///
/// # Arguments
///
/// * `text` - The current input text
/// * `cols` - The terminal width in columns
/// * `rows` - The terminal height in rows
/// * `required_lines` - The number of lines the input box occupies
///
/// # Returns
///
/// A tuple `(column, row)` representing the cursor position in terminal coordinates.
pub fn calculate_cursor_position(
    text: &str,
    cols: usize,
    rows: usize,
    required_lines: usize,
) -> (usize, usize) {
    let inner_width = cols - FRAME_CHARS;

    // Split text into display lines, same as draw_prompt_line
    let mut lines = Vec::new();

    for text_line in text.split('\n') {
        if text_line.is_empty() {
            lines.push(""); // Empty lines from newlines
        } else {
            // Handle wrapping for this line segment
            let mut current_pos = 0;
            while current_pos < text_line.len() {
                let end_pos = (current_pos + inner_width).min(text_line.len());
                lines.push(&text_line[current_pos..end_pos]);
                current_pos = end_pos;
            }
        }
    }

    // Cursor is at the end of the last line
    let last_line = lines.last().unwrap_or(&"");
    let cursor_row = rows - required_lines + 1 + lines.len() - 1;
    let cursor_col = 4 + last_line.len(); // "│ > " = 4 chars + length of last line

    (cursor_col, cursor_row)
}

/// Renders the input prompt and text content as strings for testing
pub fn render_text_lines(text: &str, cols: usize) -> Vec<String> {
    let content_width = cols - FRAME_CHARS;
    let mut lines = Vec::new();

    // Split text into lines, handling both newlines and wrapping
    let mut display_lines = Vec::new();

    for text_line in text.split('\n') {
        if text_line.is_empty() {
            display_lines.push(""); // Empty lines from newlines
        } else {
            // Handle wrapping for this line segment
            let mut current_pos = 0;
            while current_pos < text_line.len() {
                let end_pos = (current_pos + content_width).min(text_line.len());
                display_lines.push(&text_line[current_pos..end_pos]);
                current_pos = end_pos;
            }
        }
    }

    // Create the visual representation
    let horiz = "─".repeat(cols - 2);
    lines.push(format!("╭{}╮", horiz)); // Top border

    // Add content lines
    for (i, line) in display_lines.iter().enumerate() {
        let prefix = if i == 0 { "> " } else { "  " }; // prompt on first line only
        let padding = " ".repeat(content_width.saturating_sub(line.len()));
        lines.push(format!("│ {}{}{}│", prefix, line, padding));
    }

    lines.push(format!("╰{}╯", horiz)); // Bottom border
    lines
}

/// Draws the border frame around the input box to a buffer for testing
pub fn draw_frame_to_buffer<W: Write>(
    out: &mut W,
    (cols, rows): (usize, usize),
    required_lines: usize,
) -> anyhow::Result<()> {
    let horiz = "─".repeat(cols - 2);
    let clear_line = " ".repeat(cols);
    let frame_start = rows - required_lines;

    // Clear only lines that won't interfere with scroll region content
    let scroll_region_bottom = rows - required_lines - 1;
    for i in 1..=2 {
        let clear_row = frame_start - i;
        if clear_row > scroll_region_bottom {
            queue!(out, MoveTo(0, clear_row as u16), Print(&clear_line))?;
        }
    }

    queue!(
        out,
        // draw top border
        MoveTo(0, frame_start as u16),
        Print(format!("╭{}╮", horiz))
    )?;

    // draw middle lines (input area) - only clear and draw the borders, not the content
    for i in 1..required_lines - 1 {
        queue!(
            out,
            MoveTo(0, (frame_start + i) as u16),
            Print("│"),
            MoveTo((cols - 1) as u16, (frame_start + i) as u16),
            Print("│")
        )?;
    }

    // draw bottom border
    queue!(
        out,
        MoveTo(0, (rows - 1) as u16),
        Print(format!("╰{}╯", horiz))
    )?;

    out.flush()?;
    Ok(())
}

/// Draws the input prompt and text content within the frame to a buffer for testing
pub fn draw_prompt_line_to_buffer<W: Write>(
    out: &mut W,
    buf: &str,
    (cols, rows): (usize, usize),
    required_lines: usize,
) -> anyhow::Result<()> {
    let content_width = cols - FRAME_CHARS; // "│ > " + content + "│"
    let frame_start = rows - required_lines;
    let clear_line = " ".repeat(cols);

    // Clear and redraw the entire frame area to ensure no artifacts
    for row in frame_start..rows {
        queue!(out, MoveTo(0, row as u16), Print(&clear_line))?;
    }

    // Draw frame borders
    let horiz = "─".repeat(cols - 2);
    queue!(
        out,
        MoveTo(0, frame_start as u16),
        Print(format!("╭{}╮", horiz)),
        MoveTo(0, (rows - 1) as u16),
        Print(format!("╰{}╯", horiz))
    )?;

    // Split text into lines, handling both newlines and wrapping
    let mut lines = Vec::new();

    for text_line in buf.split('\n') {
        if text_line.is_empty() {
            lines.push(""); // Empty lines from newlines
        } else {
            // Handle wrapping for this line segment
            let mut current_pos = 0;
            while current_pos < text_line.len() {
                let end_pos = (current_pos + content_width).min(text_line.len());
                lines.push(&text_line[current_pos..end_pos]);
                current_pos = end_pos;
            }
        }
    }

    // Draw each line with content
    for (i, line) in lines.iter().enumerate() {
        let row = frame_start + 1 + i;
        let prefix = if i == 0 { "> " } else { "  " }; // prompt on first line only
        let padding = " ".repeat(content_width.saturating_sub(line.len()));

        queue!(
            out,
            MoveTo(0, row as u16),
            Print(format!("│ {}{}{}│", prefix, line, padding))
        )?;
    }

    // Position cursor at the end of the text
    let (cursor_col, cursor_row) = calculate_cursor_position(buf, cols, rows, required_lines);
    queue!(out, MoveTo(cursor_col as u16, cursor_row as u16))?;

    out.flush()?;
    Ok(())
}

/// Captures terminal drawing operations as a string for testing
pub fn capture_terminal_drawing(
    _state: &InputState,
    draw_fn: impl FnOnce(&mut std::io::Cursor<Vec<u8>>) -> anyhow::Result<()>
) -> anyhow::Result<String> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    draw_fn(&mut buffer)?;
    let bytes = buffer.into_inner();
    Ok(String::from_utf8_lossy(&bytes).to_string())
}
