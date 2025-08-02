use crate::{FRAME_CHARS, calculate_cursor_position};
use crossterm::{cursor::MoveTo, queue, style::Print};
use std::io::Write;

/// Pushes existing terminal content up by inserting newlines to make space for the input frame.
///
/// This function ensures that any existing content in the terminal is scrolled up
/// by the required number of lines before the input box is drawn, preventing the
/// frame from overwriting existing content.
///
/// # Arguments
///
/// * `out` - Mutable reference to stdout for writing output
/// * `required_lines` - The number of lines to push the content up by
///
/// # Returns
///
/// Returns `Ok(())` on success or an error if the operation fails.
pub fn push_content_up(out: &mut std::io::Stdout, required_lines: usize) -> anyhow::Result<()> {
    // Insert newlines to push existing content up
    for _ in 0..required_lines {
        queue!(out, Print("\n"))?;
    }
    out.flush()?;
    Ok(())
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
pub fn set_scroll_region(rows: usize, required_lines: usize) -> anyhow::Result<()> {
    let scroll_bottom = rows - required_lines; // keep bottom lines fixed for frame
    // DECSTBM is 1-based & inclusive:  ESC[{top};{bottom}r
    print!("\x1B[1;{}r", scroll_bottom);
    Ok(())
}

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
pub fn draw_frame(
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
pub fn draw_prompt_line(
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
