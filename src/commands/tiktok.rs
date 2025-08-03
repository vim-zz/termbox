use crossterm::{cursor::MoveTo, queue, style::Print};
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};

use crate::{InputState, ScrollEvent, calculate_required_lines, ui};

/// Handle the TikTok command
pub async fn handle_tiktok_command(
    state: &mut InputState,
    out: Arc<Mutex<std::io::Stdout>>,
) -> anyhow::Result<()> {
    // Create a channel for scroll events
    let (scroll_tx, scroll_rx) = mpsc::channel::<ScrollEvent>(100);

    // Store the sender in the state for future scroll events
    state.active_scroll_sender = Some(scroll_tx.clone());

    // Spawn the tiktok progress animation as a background task
    let out_clone = out.clone();
    let required_lines_copy = state.required_lines;
    let cols_copy = state.cols;
    let rows_copy = state.rows;
    tokio::spawn(async move {
        if let Err(e) = run_tiktok_progress(
            out_clone,
            cols_copy,
            rows_copy,
            required_lines_copy,
            scroll_rx,
        )
        .await
        {
            eprintln!("Error running tiktok progress: {}", e);
        }
        // Animation is complete, clear the scroll sender
        // In a real implementation, this would need proper synchronization
    });

    // Clear buffer and redraw frame immediately (don't wait for animation)
    state.buffer.clear();
    state.required_lines = calculate_required_lines("", state.cols);
    {
        let mut out_guard = out.lock().unwrap();
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

    Ok(())
}

/// Runs the tiktok progress animation from 1 to 10 with 0.5s steps.
///
/// This function creates a progress box that tracks its position as the
/// terminal scrolls. It receives scroll events through a channel to know
/// when content has moved up, allowing it to update at the correct position.
///
/// # Arguments
///
/// * `out` - Shared stdout handle
/// * `cols` - Terminal width in columns
/// * `rows` - Terminal height in rows
/// * `required_lines` - Number of lines used by the input frame
/// * `scroll_rx` - Receiver for scroll events
///
/// # Returns
///
/// Returns `Ok(())` on success or an error if the operation fails.
async fn run_tiktok_progress(
    out: Arc<Mutex<std::io::Stdout>>,
    cols: usize,
    rows: usize,
    required_lines: usize,
    mut scroll_rx: mpsc::Receiver<ScrollEvent>,
) -> anyhow::Result<()> {
    let scroll_region_bottom = rows - required_lines - 1;

    // Print initial progress box covering full terminal width
    let box_width = cols;
    let horizontal_line = "─".repeat(box_width - 2);

    // The initial position where we print the TOP of the box
    // We need to ensure the entire 3-line box fits within the scroll region
    // Box takes lines: top, middle (progress), bottom
    let initial_box_top = scroll_region_bottom - 3; // Start 3 lines up so bottom border is at scroll_region_bottom - 1

    {
        let mut out_guard = out.lock().unwrap();
        // Initial progress (1/10)
        let filled = "█".repeat(1);
        let empty = "░".repeat(10 - 1);
        let progress_text = format!("[{}{}] 1/10", filled, empty);
        let padded_text = format!("{:<width$}", progress_text, width = box_width - 4);
        queue!(
            out_guard,
            MoveTo(0, initial_box_top as u16),
            Print(format!("╭{}╮\r\n", horizontal_line)),
            Print(format!("│ {} │\r\n", padded_text)),
            Print(format!("╰{}╯\r\n", horizontal_line))
        )?;
        out_guard.flush()?;
    }

    // Track total lines scrolled - start with 0 since we'll receive the initial scroll event
    let mut lines_scrolled_total: usize = 0;

    // Update progress from 2 to 10
    for progress in 2..=10 {
        // Sleep first to allow time for progress to be visible
        sleep(Duration::from_millis(500)).await;

        // Check for any scroll events that occurred during sleep
        while let Ok(event) = scroll_rx.try_recv() {
            match event {
                ScrollEvent::ScrolledUp(lines) => {
                    lines_scrolled_total = lines_scrolled_total.saturating_add(lines);
                }
            }
        }

        // Calculate where the progress box currently is
        // The box was printed at initial_box_top and has scrolled up by lines_scrolled_total
        let current_box_top = initial_box_top.saturating_sub(lines_scrolled_total);
        let progress_line_position = current_box_top + 1; // Middle line of the 3-line box

        let filled = "█".repeat(progress);
        let empty = "░".repeat(10 - progress);
        let progress_text = format!("[{}{}] {}/10", filled, empty, progress);
        let padded_text = format!("{:<width$}", progress_text, width = box_width - 4); // Left align with padding to full width

        {
            let mut out_guard = out.lock().unwrap();

            // Only update if the progress line is still visible on screen
            // The progress line must be:
            // - Not above the top of the screen (>= 0, which is always true for usize)
            // - Not below the bottom of the terminal (< rows)
            if progress_line_position < rows {
                queue!(
                    out_guard,
                    MoveTo(0, progress_line_position as u16),
                    Print(format!("│ {} │", padded_text))
                )?;
                out_guard.flush()?;
            }
        }
    }

    Ok(())
}
