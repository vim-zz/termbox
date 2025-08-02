
use crossterm::{
    cursor::MoveTo,
    event::{Event, EventStream, KeyCode, KeyModifiers},
    queue,
    style::Print,
    terminal::{self, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use termbox::*;
use std::io::{Write, stdout};
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

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
fn push_content_up(out: &mut std::io::Stdout, required_lines: usize) -> anyhow::Result<()> {
    // Insert newlines to push existing content up
    for _ in 0..required_lines {
        queue!(out, Print("\n"))?;
    }
    out.flush()?;
    Ok(())
}


/// Runs the tiktok progress animation from 1 to 10 with 0.5s steps.
///
/// This function creates a single progress box that behaves like normal scrollable
/// terminal content. It prints the initial box, then updates it in place by moving
/// the cursor back to the progress line.
///
/// # Arguments
///
/// * `out` - Shared stdout handle
/// * `cols` - Terminal width in columns
/// * `rows` - Terminal height in rows
/// * `required_lines` - Number of lines used by the input frame
///
/// # Returns
///
/// Returns `Ok(())` on success or an error if the operation fails.
async fn run_tiktok_progress(
    out: Arc<Mutex<std::io::Stdout>>,
    cols: usize,
    rows: usize,
    required_lines: usize,
) -> anyhow::Result<()> {
    let scroll_region_bottom = rows - required_lines - 1;
    
    // Print initial progress box (progress = 1) to scroll region
    let initial_progress = "[█░░░░░░░░░] 1/10";
    let box_width = initial_progress.len() + 4;
    let box_left = (cols - box_width) / 2;
    
    {
        let mut out_guard = out.lock().unwrap();
        queue!(
            out_guard,
            MoveTo(0, scroll_region_bottom as u16),
            Print(format!("╭{}╮\r\n", "─".repeat(box_width - 2))),
            Print(format!("│ {} │\r\n", initial_progress)),
            Print(format!("╰{}╯\r\n", "─".repeat(box_width - 2)))
        )?;
        out_guard.flush()?;
    }
    
    sleep(Duration::from_millis(500)).await;
    
    // Update progress from 2 to 10 by moving cursor back to the progress line
    for progress in 2..=10 {
        let filled_chars = "█".repeat(progress);
        let empty_chars = "░".repeat(10 - progress);
        let progress_bar = format!("[{}{}] {}/10", filled_chars, empty_chars, progress);
        
        {
            let mut out_guard = out.lock().unwrap();
            // Move cursor up 2 lines to the progress bar line, then update it
            queue!(
                out_guard,
                MoveTo(box_left as u16, (scroll_region_bottom + 1) as u16),
                Print(format!("│ {} │", progress_bar))
            )?;
            out_guard.flush()?;
        }
        
        sleep(Duration::from_millis(500)).await;
    }
    
    Ok(())
}

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
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let out = Arc::new(Mutex::new(stdout()));
    enable_raw_mode()?;

    // ── 1. reserve the bottom lines ──────────────────────────────────
    let (cols, rows) = terminal::size()?;
    let (cols, rows) = (cols as usize, rows as usize);
    let mut state = InputState::new(cols, rows);
    
    // Push existing terminal content up to make space for the input frame
    {
        let mut out_guard = out.lock().unwrap();
        push_content_up(&mut out_guard, state.required_lines)?;
    }
    
    set_scroll_region(rows, state.required_lines)?;

    // ── 2. draw the static box once ──────────────────────────────────
    {
        let mut out_guard = out.lock().unwrap();
        draw_frame(&mut out_guard, (cols, rows), state.required_lines)?;
        draw_prompt_line(&mut out_guard, "", (cols, rows), state.required_lines)?;
    }

    // Create an async event stream
    let mut event_stream = EventStream::new();

    // ── 3. main loop ─────────────────────────────────────────────────
    loop {
        match event_stream.next().await {
            Some(Ok(Event::Key(key))) => {
                match handle_key_event(key, &mut state, out.clone()).await? {
                    KeyAction::Exit => break,
                    KeyAction::Continue => {}
                }
            }

            Some(Ok(Event::Resize(new_cols, new_rows))) => {
                handle_resize(
                    new_cols as usize,
                    new_rows as usize,
                    &mut state,
                    out.clone(),
                )?;
            }

            Some(Ok(_)) => {} // Other events
            Some(Err(e)) => eprintln!("Error reading event: {}", e),
            None => break,
        }
    }

    // ── 4. clean-up ──────────────────────────────────────────────────
    {
        let mut out_guard = out.lock().unwrap();
        let clear_line = " ".repeat(state.cols);
        // Clear all lines used by the frame
        for i in 0..=state.required_lines {
            queue!(
                out_guard,
                MoveTo(0 as u16, (state.rows - state.required_lines - 1 + i) as u16),
                Print(&clear_line)
            )?;
        }
        out_guard.flush()?;
        // give terminal its full screen back
        print!("\x1B[r");
        disable_raw_mode()?;

        // Position cursor exactly where the input cursor was (at end of current input)
        // Do this AFTER clearing scroll region to prevent cursor position restoration
        let (cursor_col, cursor_row) = calculate_cursor_position(&state.buffer, state.cols, state.rows, state.required_lines);
        queue!(out_guard, MoveTo(cursor_col as u16, cursor_row as u16))?;
        out_guard.flush()?;
    }
    Ok(())
}

/// Handle keyboard events and return the action to take
async fn handle_key_event(
    key: crossterm::event::KeyEvent,
    state: &mut InputState,
    out: Arc<Mutex<std::io::Stdout>>,
) -> anyhow::Result<KeyAction> {
    let action = state.handle_key(key.code, key.modifiers);
    
    match key.code {
        KeyCode::Enter if !key.modifiers.contains(KeyModifiers::ALT) => {
            handle_enter_key(state, out.clone()).await?;
        }
        _ => {
            update_frame_if_needed(state, out.clone())?;
        }
    }

    Ok(action)
}

/// Handle the Enter key to submit input
async fn handle_enter_key(
    state: &mut InputState,
    out: Arc<Mutex<std::io::Stdout>>,
) -> anyhow::Result<()> {
    let submitted_text = state.buffer.clone();
    
    // Clear the old frame area first
    let old_required_lines = state.required_lines;
    let new_required_lines = calculate_required_lines("", state.cols);

    // Clear the old frame area if it was larger
    if old_required_lines > new_required_lines {
        let mut out_guard = out.lock().unwrap();
        let clear_line = " ".repeat(state.cols);
        let old_frame_start = state.rows - old_required_lines;
        let new_frame_start = state.rows - new_required_lines;
        for row in old_frame_start..new_frame_start {
            queue!(out_guard, MoveTo(0, row as u16), Print(&clear_line))?;
        }
        out_guard.flush()?;
    }

    // Update the scroll region for the new frame size
    if new_required_lines != old_required_lines {
        state.required_lines = new_required_lines;
        set_scroll_region(state.rows, state.required_lines)?;
    }

    // Check for special command "tiktok"
    if submitted_text.trim() == "tiktok" {
        // Spawn the tiktok progress animation as a background task
        let out_clone = out.clone();
        let required_lines_copy = state.required_lines;
        let cols_copy = state.cols;
        let rows_copy = state.rows;
        tokio::spawn(async move {
            if let Err(e) = run_tiktok_progress(out_clone, cols_copy, rows_copy, required_lines_copy).await {
                eprintln!("Error running tiktok progress: {}", e);
            }
        });
        
        // Clear buffer and redraw frame immediately (don't wait for animation)
        state.buffer.clear();
        state.required_lines = calculate_required_lines("", state.cols);
        {
            let mut out_guard = out.lock().unwrap();
            draw_frame(&mut out_guard, (state.cols, state.rows), state.required_lines)?;
            draw_prompt_line(&mut out_guard, "", (state.cols, state.rows), state.required_lines)?;
        }
        return Ok(());
    }

    // Now print the text at the bottom of the new scroll region
    let scroll_region_bottom = state.rows - state.required_lines - 1;

    // Replace all \n with \r\n to ensure cursor returns to column 0
    let output_text = submitted_text.replace('\n', "\r\n");
    {
        let mut out_guard = out.lock().unwrap();
        queue!(
            out_guard,
            MoveTo(0, scroll_region_bottom as u16),
            Print(&output_text),
            Print("\r\n") // Final newline to scroll properly
        )?;
        out_guard.flush()?;

        // Clear buffer and draw the new frame
        state.buffer.clear();
        state.required_lines = calculate_required_lines("", state.cols);
        draw_frame(&mut out_guard, (state.cols, state.rows), state.required_lines)?;
        draw_prompt_line(&mut out_guard, "", (state.cols, state.rows), state.required_lines)?;
    }

    Ok(())
}

/// Update frame if needed based on text changes
fn update_frame_if_needed(
    state: &mut InputState,
    out: Arc<Mutex<std::io::Stdout>>,
) -> anyhow::Result<()> {
    let new_required_lines = calculate_required_lines(&state.buffer, state.cols);
    if new_required_lines != state.required_lines {
        state.required_lines = new_required_lines;
        set_scroll_region(state.rows, state.required_lines)?;
        let mut out_guard = out.lock().unwrap();
        draw_frame(&mut out_guard, (state.cols, state.rows), state.required_lines)?;
        draw_prompt_line(&mut out_guard, &state.buffer, (state.cols, state.rows), state.required_lines)?;
    } else {
        let mut out_guard = out.lock().unwrap();
        draw_prompt_line(&mut out_guard, &state.buffer, (state.cols, state.rows), state.required_lines)?;
    }
    Ok(())
}

/// Handle terminal resize event
fn handle_resize(
    new_cols: usize,
    new_rows: usize,
    state: &mut InputState,
    out: Arc<Mutex<std::io::Stdout>>,
) -> anyhow::Result<()> {
    state.handle_resize(new_cols, new_rows);
    print!("\x1B[r"); // clear any old region
    set_scroll_region(state.rows, state.required_lines)?;
    let mut out_guard = out.lock().unwrap();
    draw_frame(&mut out_guard, (state.cols, state.rows), state.required_lines)?;
    draw_prompt_line(&mut out_guard, &state.buffer, (state.cols, state.rows), state.required_lines)?;
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
