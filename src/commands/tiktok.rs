use crossterm::{cursor::MoveTo, queue, style::Print};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tokio::time::{Duration, sleep};

use crate::{InputState, ScrollEvent, calculate_required_lines, ui};

// The height of the TikTok animation box in terminal lines
pub const TIKTOK_ANIMATION_HEIGHT: usize = 3;

// Track active animations count
static ACTIVE_ANIMATIONS: AtomicUsize = AtomicUsize::new(0);

/// Get the count of currently active animations
pub fn get_active_animations() -> usize {
    ACTIVE_ANIMATIONS.load(Ordering::SeqCst)
}

/// Handle the TikTok command
pub async fn handle_tiktok_command(
    state: &mut InputState,
    out: Arc<Mutex<std::io::Stdout>>,
) -> anyhow::Result<()> {
    // Subscribe to scroll events using broadcast channel
    let scroll_rx = state.setup_scroll_broadcast();

    // Get the current scroll region bottom position where content appears
    let scroll_region_bottom = state.rows - state.required_lines - 1;

    // Create space for the animation box atomically
    {
        let mut out_guard = out.lock().unwrap();

        // Create space for the 3-line animation box by printing 3 newlines
        // This pushes everything up by 3 lines
        queue!(
            out_guard,
            MoveTo(0, scroll_region_bottom as u16),
            Print("\r\n\r\n\r\n")
        )?;
        out_guard.flush()?;
    }

    // Send scroll event for the 3 lines we just created space for
    if let Some(broadcast_tx) = &state.scroll_broadcast {
        let _ = broadcast_tx.send(ScrollEvent::ScrolledUp(3));
    }

    // Increment active animations counter
    ACTIVE_ANIMATIONS.fetch_add(1, Ordering::SeqCst);

    // Spawn the tiktok progress animation as a background task
    let out_clone = out.clone();
    let required_lines_copy = state.required_lines;
    let cols_copy = state.cols;
    let rows_copy = state.rows;

    // After creating 3 lines of space, the content has scrolled up by 3 lines
    // The box should be drawn at the position where it will appear after scrolling
    // So we draw it at scroll_region_bottom - 2 (to fit the 3-line box)
    let animation_box_top = scroll_region_bottom.saturating_sub(2);

    tokio::spawn(async move {
        let result = run_tiktok_progress(
            out_clone,
            cols_copy,
            rows_copy,
            required_lines_copy,
            scroll_rx,
            animation_box_top,
        )
        .await;

        // Decrement active animations counter
        ACTIVE_ANIMATIONS.fetch_sub(1, Ordering::SeqCst);

        if let Err(e) = result {
            eprintln!("Error running tiktok progress: {}", e);
        }
        // Animation is complete - receiver will be dropped automatically
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
/// * `box_top` - The row where the top of the animation box should be drawn
///
/// # Returns
///
/// Returns `Ok(())` on success or an error if the operation fails.
async fn run_tiktok_progress(
    out: Arc<Mutex<std::io::Stdout>>,
    cols: usize,
    rows: usize,
    _required_lines: usize,
    mut scroll_rx: broadcast::Receiver<ScrollEvent>,
    box_top: usize,
) -> anyhow::Result<()> {
    let box_width = cols;
    let horizontal_line = "─".repeat(box_width - 2);

    // Clear any pending scroll events that occurred before we started
    while let Ok(_) = scroll_rx.try_recv() {
        // Discard events that happened before animation started
    }

    // Draw the initial progress box at the determined position
    {
        let mut out_guard = out.lock().unwrap();

        // Initial progress (1/10)
        let filled = "█".repeat(1);
        let empty = "░".repeat(10 - 1);
        let progress_text = format!("[{}{}] 1/10", filled, empty);
        let padded_text = format!("{:<width$}", progress_text, width = box_width - 4);

        // Draw the complete box at the specified position - no additional scrolling
        queue!(
            out_guard,
            MoveTo(0, box_top as u16),
            Print(format!("╭{}╮", horizontal_line)),
            MoveTo(0, (box_top + 1) as u16),
            Print(format!("│ {} │", padded_text)),
            MoveTo(0, (box_top + 2) as u16),
            Print(format!("╰{}╯", horizontal_line))
        )?;
        out_guard.flush()?;
    }

    // Track total lines scrolled since the box was drawn
    let mut lines_scrolled_total: usize = 0;

    // Update progress from 2 to 10
    for progress in 2..=10 {
        // Sleep first to allow time for progress to be visible
        sleep(Duration::from_millis(500)).await;

        // Check for any scroll events that occurred during sleep
        loop {
            match scroll_rx.try_recv() {
                Ok(ScrollEvent::ScrolledUp(lines)) => {
                    lines_scrolled_total = lines_scrolled_total.saturating_add(lines);
                }
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Closed) => return Ok(()),
                Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                    // Handle lag by assuming we missed some scroll events
                    eprintln!("Animation lagged, skipped {} scroll events", skipped);
                    // Continue to catch up with current events
                }
            }
        }

        // Calculate current position: the box has moved up by the number of lines scrolled
        let current_box_top = box_top.saturating_sub(lines_scrolled_total);
        let progress_line_position = current_box_top + 1; // Middle line of the 3-line box

        let filled = "█".repeat(progress);
        let empty = "░".repeat(10 - progress);
        let progress_text = format!("[{}{}] {}/10", filled, empty, progress);
        let padded_text = format!("{:<width$}", progress_text, width = box_width - 4);

        {
            let mut out_guard = out.lock().unwrap();

            // Only update if the progress line is still visible on screen
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
