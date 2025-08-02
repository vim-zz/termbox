use crossterm::{
    cursor::MoveTo,
    event::{Event, EventStream, KeyCode, KeyModifiers},
    queue,
    style::Print,
    terminal::{self, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use std::io::{Write, stdout};
use std::sync::{Arc, Mutex};
use termbox::{tiktok, ui, *};

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
        ui::push_content_up(&mut out_guard, state.required_lines)?;
    }

    ui::set_scroll_region(rows, state.required_lines)?;

    // ── 2. draw the static box once ──────────────────────────────────
    {
        let mut out_guard = out.lock().unwrap();
        ui::draw_frame(&mut out_guard, (cols, rows), state.required_lines)?;
        ui::draw_prompt_line(&mut out_guard, "", (cols, rows), state.required_lines)?;
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
        let (cursor_col, cursor_row) =
            calculate_cursor_position(&state.buffer, state.cols, state.rows, state.required_lines);
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
        ui::set_scroll_region(state.rows, state.required_lines)?;
    }

    // Check for special commands (like "tiktok")
    if let Some(_) = tiktok::handle_tiktok_command(&submitted_text, state, out.clone()).await? {
        return Ok(());
    }

    // Now print the text at the bottom of the new scroll region
    let scroll_region_bottom = state.rows - state.required_lines - 1;

    // Calculate how many terminal lines the output will actually take
    // This needs to account for line wrapping
    let mut total_terminal_lines = 0;
    for line in submitted_text.lines() {
        if line.is_empty() {
            total_terminal_lines += 1;
        } else {
            // Calculate how many terminal lines this logical line will take due to wrapping
            let line_length = line.len();
            let terminal_width = state.cols;
            let wrapped_lines = (line_length + terminal_width - 1) / terminal_width;
            total_terminal_lines += wrapped_lines;
        }
    }
    // If the text was empty or had no newlines, we still print at least one line
    if total_terminal_lines == 0 {
        total_terminal_lines = 1;
    }

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
        ui::draw_frame(
            &mut out_guard,
            (state.cols, state.rows),
            state.required_lines,
        )?;
        ui::draw_prompt_line(
            &mut out_guard,
            "",
            (state.cols, state.rows),
            state.required_lines,
        )?;
    }

    // Send scroll event if there's an active progress animation
    if let Some(sender) = &state.active_scroll_sender {
        // The content scrolled up by the number of terminal lines (including wrapped lines)
        let _ = sender
            .send(ScrollEvent::ScrolledUp(total_terminal_lines))
            .await;
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
        ui::set_scroll_region(state.rows, state.required_lines)?;
        let mut out_guard = out.lock().unwrap();
        ui::draw_frame(
            &mut out_guard,
            (state.cols, state.rows),
            state.required_lines,
        )?;
        ui::draw_prompt_line(
            &mut out_guard,
            &state.buffer,
            (state.cols, state.rows),
            state.required_lines,
        )?;
    } else {
        let mut out_guard = out.lock().unwrap();
        ui::draw_prompt_line(
            &mut out_guard,
            &state.buffer,
            (state.cols, state.rows),
            state.required_lines,
        )?;
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
    ui::set_scroll_region(state.rows, state.required_lines)?;
    let mut out_guard = out.lock().unwrap();
    ui::draw_frame(
        &mut out_guard,
        (state.cols, state.rows),
        state.required_lines,
    )?;
    ui::draw_prompt_line(
        &mut out_guard,
        &state.buffer,
        (state.cols, state.rows),
        state.required_lines,
    )?;
    Ok(())
}
