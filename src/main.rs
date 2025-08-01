
use std::{
    io::{stdout, Write},
    time::Duration,
};
use crossterm::{
    cursor::MoveTo,
    event::{self, Event, KeyCode, KeyModifiers},
    style::Print,
    terminal::{self, disable_raw_mode, enable_raw_mode},
    queue,
};

fn main() -> anyhow::Result<()> {
    let mut out = stdout();
    enable_raw_mode()?;

    // ── 1. reserve the bottom 3 lines ────────────────────────────────
    let mut cols_rows = terminal::size()?;              // (cols, rows)
    set_scroll_region(cols_rows.1)?;

    // ── 2. draw the static box once ──────────────────────────────────
    draw_frame(&mut out, cols_rows)?;
    draw_prompt_line(&mut out, "", cols_rows)?;

    let mut buf = String::new();

    // ── 3. main loop ─────────────────────────────────────────────────
    loop {
        // poll() lets us handle resize events smoothly
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                // keyboard -------------------------------------------
                Event::Key(key) => match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break, // quit
                    KeyCode::Esc => break,           // quit
                    KeyCode::Enter => {
                        // Move cursor to the bottom of the scroll region and print
                        let scroll_region_bottom = cols_rows.1 - 4; // Last line of scroll region (0-indexed)
                        queue!(
                            out,
                            MoveTo(0, scroll_region_bottom),
                            Print(&buf),
                            Print("\r\n") // Carriage return + line feed to scroll properly
                        )?;
                        out.flush()?;
                        buf.clear();
                        draw_prompt_line(&mut out, "", cols_rows)?;
                    }
                    KeyCode::Backspace => {
                        buf.pop();
                        draw_prompt_line(&mut out, &buf, cols_rows)?;
                    }
                    KeyCode::Char(c) => {
                        buf.push(c);
                        draw_prompt_line(&mut out, &buf, cols_rows)?;
                    }
                    _ => {}
                },

                // window resized -------------------------------------
                Event::Resize(new_cols, new_rows) => {
                    cols_rows = (new_cols, new_rows);
                    // reset scroll region then set a new one
                    print!("\x1B[r");          // clear any old region
                    set_scroll_region(cols_rows.1)?;
                    draw_frame(&mut out, cols_rows)?;
                    draw_prompt_line(&mut out, &buf, cols_rows)?;
                }
                _ => {}
            }
        }
    }

    // ── 4. clean-up ──────────────────────────────────────────────────
    print!("\x1B[r");           // give terminal its full screen back
    disable_raw_mode()?;
    
    // Position cursor exactly where the input cursor was (at end of current input)
    // Do this AFTER clearing scroll region to prevent cursor position restoration
    queue!(out, MoveTo((4 + buf.len()) as u16, cols_rows.1 - 2))?;
    out.flush()?;
    Ok(())
}

fn set_scroll_region(rows: u16) -> anyhow::Result<()> {
    let scroll_bottom = rows - 3;      // keep last 3 lines fixed
    // DECSTBM is 1-based & inclusive:  ESC[{top};{bottom}r
    print!("\x1B[1;{}r", scroll_bottom);
    Ok(())
}

// draw the 3-line frame (top border, prompt line, bottom border)
fn draw_frame(out: &mut std::io::Stdout, (cols, rows): (u16, u16)) -> anyhow::Result<()> {
    let horiz = "─".repeat((cols - 2) as usize);
    let clear_line = " ".repeat(cols as usize);

    queue!(
        out,
        // clear and draw top border
        MoveTo(0, rows - 3),
        Print(&clear_line),
        MoveTo(0, rows - 3),
        Print(format!("╭{}╮", horiz)),
        // clear and draw prompt line – empty for now
        MoveTo(0, rows - 2),
        Print(&clear_line),
        MoveTo(0, rows - 2),
        Print(format!("│ {:width$}│", "", width = (cols - 2) as usize)),
        // clear and draw bottom border
        MoveTo(0, rows - 1),
        Print(&clear_line),
        MoveTo(0, rows - 1),
        Print(format!("╰{}╯", horiz))
    )?;
    out.flush()?;
    Ok(())
}

// redraw just the editable prompt line
fn draw_prompt_line(
    out: &mut std::io::Stdout,
    buf: &str,
    (cols, rows): (u16, u16),
) -> anyhow::Result<()> {
    // truncate if user typed beyond the inner width
    let inner = (cols - 5) as usize;           // "│ > " + content + "│" = 5 chars total
    let mut slice = buf.to_string();
    if slice.len() > inner { slice = slice[slice.len() - inner..].to_string(); }
    let padding = " ".repeat(inner.saturating_sub(slice.len()));

    queue!(
        out,
        MoveTo(0, rows - 2),
        Print(format!("│ > {}{}│", slice, padding)),
        // place cursor after the visible text - account for "│ > " = 4 chars
        MoveTo((4 + slice.len()) as u16, rows - 2)
    )?;
    out.flush()?;
    Ok(())
}
