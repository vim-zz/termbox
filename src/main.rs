use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyModifiers},
    queue,
    style::Print,
    terminal::{self, disable_raw_mode, enable_raw_mode},
};
use std::{
    io::{Write, stdout},
    time::Duration,
};
const LEFT_FRAME_CHARS: usize = const_str::to_char_array!("│ > ").len();
const RIGHT_FRAME_CHARS: usize = const_str::to_char_array!("│").len();

/// The number of characters used for frame borders and prompt prefix
/// Format: "│ > " (4 chars) + "│" (1 char) = 5 chars total
const FRAME_CHARS: usize = LEFT_FRAME_CHARS + RIGHT_FRAME_CHARS;

/// Main entry point for the terminal input box application.
///
/// Sets up a terminal-based input interface with the following features:
/// - Raw mode terminal input handling
/// - Dynamic frame sizing based on input text length
/// - Scroll region management to keep the input box at the bottom
/// - Multi-line text input with word wrapping
/// - Proper cleanup on exit (Ctrl+C, Ctrl+D, or Esc)
///
/// The application maintains a fixed input box at the bottom of the terminal
/// while allowing the rest of the terminal to scroll normally.
///
/// # Returns
///
/// Returns `Ok(())` on successful completion or an error if terminal operations fail.
fn main() -> anyhow::Result<()> {
    let mut out = stdout();
    enable_raw_mode()?;

    // ── 1. reserve the bottom lines ──────────────────────────────────
    let (cols, rows) = terminal::size()?;
    let (mut cols, mut rows) = (cols as usize, rows as usize);
    let mut required_lines = calculate_required_lines("", cols);
    set_scroll_region(rows, required_lines)?;

    // ── 2. draw the static box once ──────────────────────────────────
    draw_frame(&mut out, (cols, rows), required_lines)?;
    draw_prompt_line(&mut out, "", (cols, rows), required_lines)?;

    let mut buf = String::new();

    // ── 3. main loop ─────────────────────────────────────────────────
    loop {
        // poll() lets us handle resize events smoothly
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                // keyboard -------------------------------------------
                Event::Key(key) => match key.code {
                    KeyCode::Esc => break, // quit
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break, // quit
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => break, // quit
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                        buf.push('\n');
                        let new_required_lines = calculate_required_lines(&buf, cols);
                        if new_required_lines != required_lines {
                            required_lines = new_required_lines;
                            set_scroll_region(rows, required_lines)?;
                            draw_frame(&mut out, (cols, rows), required_lines)?;
                        }
                        draw_prompt_line(&mut out, &buf, (cols, rows), required_lines)?;
                    }
                    // Alternative: Use Ctrl+J for newline (common fallback) so user can use Shift+Enter!
                    KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        buf.push('\n');
                        let new_required_lines = calculate_required_lines(&buf, cols);
                        if new_required_lines != required_lines {
                            required_lines = new_required_lines;
                            set_scroll_region(rows, required_lines)?;
                            draw_frame(&mut out, (cols, rows), required_lines)?;
                        }
                        draw_prompt_line(&mut out, &buf, (cols, rows), required_lines)?;
                    }
                    KeyCode::Enter => {
                        // Clear the old frame area first
                        let old_required_lines = required_lines;
                        let new_required_lines = calculate_required_lines("", cols);

                        // Clear the old frame area if it was larger
                        if old_required_lines > new_required_lines {
                            let clear_line = " ".repeat(cols);
                            let old_frame_start = rows - old_required_lines;
                            let new_frame_start = rows - new_required_lines;
                            // Clear the lines that were part of the old frame but not the new one
                            for row in old_frame_start..new_frame_start {
                                queue!(out, MoveTo(0, row as u16), Print(&clear_line))?;
                            }
                            out.flush()?;
                        }

                        // Update the scroll region for the new frame size
                        if new_required_lines != old_required_lines {
                            required_lines = new_required_lines;
                            set_scroll_region(rows, required_lines)?;
                        }

                        // Now print the text at the bottom of the new scroll region
                        let scroll_region_bottom = rows - required_lines - 1; // Last line of scroll region (0-based)
                        queue!(
                            out,
                            MoveTo(0, scroll_region_bottom as u16),
                            Print(&buf),
                            Print("\r\n") // Carriage return + line feed to scroll properly
                        )?;
                        out.flush()?;

                        // Clear buffer and draw the new frame
                        buf.clear();
                        draw_frame(&mut out, (cols, rows), required_lines)?;
                        draw_prompt_line(&mut out, "", (cols, rows), required_lines)?;
                    }
                    KeyCode::Backspace => {
                        buf.pop();
                        let new_required_lines = calculate_required_lines(&buf, cols);
                        if new_required_lines != required_lines {
                            required_lines = new_required_lines;
                            set_scroll_region(rows, required_lines)?;
                            draw_frame(&mut out, (cols, rows), required_lines)?;
                        }
                        draw_prompt_line(&mut out, &buf, (cols, rows), required_lines)?;
                    }
                    KeyCode::Char(c) => {
                        buf.push(c);
                        let new_required_lines = calculate_required_lines(&buf, cols);
                        if new_required_lines != required_lines {
                            required_lines = new_required_lines;
                            set_scroll_region(rows, required_lines)?;
                            draw_frame(&mut out, (cols, rows), required_lines)?;
                        }
                        draw_prompt_line(&mut out, &buf, (cols, rows), required_lines)?;
                    }
                    _ => {}
                },

                // window resized -------------------------------------
                Event::Resize(new_cols, new_rows) => {
                    cols = new_cols as usize;
                    rows = new_rows as usize;
                    required_lines = calculate_required_lines(&buf, cols);
                    // reset scroll region then set a new one
                    print!("\x1B[r"); // clear any old region
                    set_scroll_region(rows, required_lines)?;
                    draw_frame(&mut out, (cols, rows), required_lines)?;
                    draw_prompt_line(&mut out, &buf, (cols, rows), required_lines)?;
                }
                _ => {}
            }
        }
    }

    // ── 4. clean-up ──────────────────────────────────────────────────
    let clear_line = " ".repeat(cols);
    // Clear all lines used by the frame
    for i in 0..=required_lines {
        queue!(
            out,
            MoveTo(0 as u16, (rows - required_lines - 1 + i) as u16),
            Print(&clear_line)
        )?;
    }
    out.flush()?;
    // give terminal its full screen back
    print!("\x1B[r");
    disable_raw_mode()?;

    // Position cursor exactly where the input cursor was (at end of current input)
    // Do this AFTER clearing scroll region to prevent cursor position restoration
    let (cursor_col, cursor_row) = calculate_cursor_position(&buf, cols, rows, required_lines);
    queue!(out, MoveTo(cursor_col as u16, cursor_row as u16))?;
    out.flush()?;
    Ok(())
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
fn calculate_required_lines(text: &str, cols: usize) -> usize {
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
fn calculate_cursor_position(
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
    let last_line = lines.last().unwrap();
    let cursor_row = rows - required_lines + 1 + lines.len() - 1;
    let cursor_col = 4 + last_line.len(); // "│ > " = 4 chars + length of last line

    (cursor_col, cursor_row)
}

/// Sets up a terminal scroll region to keep the input box fixed at the bottom.
///
/// Uses the DECSTBM (DEC Set Top and Bottom Margins) escape sequence to create
/// a scrolling region that excludes the bottom lines where the input box is drawn.
/// This allows the rest of the terminal content to scroll normally while keeping
/// the input interface stationary.
///
/// # Arguments
///
/// * `rows` - The total terminal height in rows
/// * `required_lines` - The number of lines to reserve at the bottom for the input box
///
/// # Returns
///
/// Returns `Ok(())` on success or an error if the operation fails.
fn set_scroll_region(rows: usize, required_lines: usize) -> anyhow::Result<()> {
    let scroll_bottom = rows - required_lines; // keep bottom lines fixed for frame
    // DECSTBM is 1-based & inclusive:  ESC[{top};{bottom}r
    print!("\x1B[1;{}r", scroll_bottom);
    Ok(())
}

// draw the variable-height frame (top border, input lines, bottom border)
/// Draws the border frame around the input box.
///
/// Creates a box using Unicode drawing characters (╭─╮│╰─╯) that surrounds
/// the input area. The frame is drawn at the bottom of the terminal and
/// adjusts its height based on the content requirements.
///
/// # Arguments
///
/// * `out` - Mutable reference to stdout for writing output
/// * `(cols, rows)` - Terminal dimensions as a tuple (width, height)
/// * `required_lines` - The number of lines the complete input box needs
///
/// # Returns
///
/// Returns `Ok(())` on successful drawing or an error if output operations fail.
// draw the variable-height frame (top border, input lines, bottom border)
fn draw_frame(
    out: &mut std::io::Stdout,
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

// redraw the multi-line editable prompt
/// Draws the input prompt and text content within the frame.
///
/// This function renders the complete input interface including:
/// - The prompt symbol (">") on the first line
/// - Multi-line text content with proper wrapping
/// - Proper spacing and alignment within the frame borders
/// - Cursor positioning at the end of the input text
///
/// The function handles text that spans multiple lines by wrapping at the
/// available width and continues with proper indentation on subsequent lines.
///
/// # Arguments
///
/// * `out` - Mutable reference to stdout for writing output
/// * `buf` - The current input text buffer
/// * `(cols, rows)` - Terminal dimensions as a tuple (width, height)
/// * `required_lines` - The number of lines the input box occupies
///
/// # Returns
///
/// Returns `Ok(())` on successful rendering or an error if output operations fail.
// redraw the multi-line editable prompt
fn draw_prompt_line(
    out: &mut std::io::Stdout,
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
