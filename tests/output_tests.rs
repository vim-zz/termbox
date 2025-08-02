use termbox::{InputState, draw_prompt_line_to_buffer, capture_terminal_drawing};
use crossterm::event::{KeyCode, KeyModifiers};

#[test]
fn test_exact_terminal_output_simple() {
    let mut state = InputState::new(20, 10);
    
    // Add "hi" 
    state.handle_key(KeyCode::Char('h'), KeyModifiers::NONE);
    state.handle_key(KeyCode::Char('i'), KeyModifiers::NONE);
    
    let drawing = capture_terminal_drawing(&state, |buffer| {
        draw_prompt_line_to_buffer(buffer, &state.buffer, (state.cols, state.rows), state.required_lines)
    }).unwrap();
    
    // For a 20x10 terminal with 3-line frame starting at row 8:
    // - Clear lines: \x1B[8;1H + 20 spaces, \x1B[9;1H + 20 spaces, \x1B[10;1H + 20 spaces
    // - Top border: \x1B[8;1H╭──────────────────╮
    // - Bottom border: \x1B[10;1H╰──────────────────╯  
    // - Content: \x1B[9;1H│ > hi             │
    // - Cursor: \x1B[9;7H (after "│ > hi")
    
    assert!(drawing.contains("\x1B[8;1H╭──────────────────╮"));
    assert!(drawing.contains("\x1B[9;1H│ > hi             │"));
    assert!(drawing.contains("\x1B[10;1H╰──────────────────╯"));
    assert!(drawing.contains("\x1B[9;7H")); // Cursor position
}

#[test]
fn test_exact_terminal_output_multiline() {
    let mut state = InputState::new(16, 8);
    
    // Add "A\nB"
    state.handle_key(KeyCode::Char('A'), KeyModifiers::NONE);
    state.handle_key(KeyCode::Enter, KeyModifiers::ALT);
    state.handle_key(KeyCode::Char('B'), KeyModifiers::NONE);
    
    let drawing = capture_terminal_drawing(&state, |buffer| {
        draw_prompt_line_to_buffer(buffer, &state.buffer, (state.cols, state.rows), state.required_lines)
    }).unwrap();
    
    // For a 16x8 terminal with 4-line frame starting at row 5:
    // Frame goes from row 5 to row 8
    assert!(drawing.contains("\x1B[5;1H╭──────────────╮"));  // Top border at row 5
    assert!(drawing.contains("\x1B[6;1H│ > A          │"));  // First line at row 6
    assert!(drawing.contains("\x1B[7;1H│   B          │"));  // Second line at row 7
    assert!(drawing.contains("\x1B[8;1H╰──────────────╯"));  // Bottom border at row 8
    assert!(drawing.contains("\x1B[7;6H"));                  // Cursor after "B" at row 7, col 6
}

#[test]
fn test_exact_terminal_output_wrapped() {
    let mut state = InputState::new(12, 8);
    
    // Add text that will wrap: "hello world"
    for ch in "hello world".chars() {
        state.handle_key(KeyCode::Char(ch), KeyModifiers::NONE);
    }
    
    let drawing = capture_terminal_drawing(&state, |buffer| {
        draw_prompt_line_to_buffer(buffer, &state.buffer, (state.cols, state.rows), state.required_lines)
    }).unwrap();
    
    // With 12 cols and 5 frame chars, content width = 7
    // "hello world" (11 chars) should wrap to 2 lines: "hello w" and "orld"  
    // Frame should be 4 lines total (2 content + 2 borders), starting at row 5
    assert!(drawing.contains("\x1B[5;1H╭──────────╮"));   // Top border
    assert!(drawing.contains("\x1B[6;1H│ > hello w│"));  // First wrapped line (no space padding)
    assert!(drawing.contains("\x1B[7;1H│   orld   │"));  // Second wrapped line
    assert!(drawing.contains("\x1B[8;1H╰──────────╯"));   // Bottom border
    assert!(drawing.contains("\x1B[7;9H"));               // Cursor after "orld"
}

#[test]
fn test_exact_terminal_clear_operations() {
    let state = InputState::new(15, 6);
    
    let drawing = capture_terminal_drawing(&state, |buffer| {
        draw_prompt_line_to_buffer(buffer, "", (state.cols, state.rows), state.required_lines)
    }).unwrap();
    
    // Should clear the frame area first (rows 4, 5, 6 for a 6-row terminal with 3-line frame)
    // Each clear should be 15 spaces
    let clear_pattern = " ".repeat(15);
    assert!(drawing.contains(&format!("\x1B[4;1H{}", clear_pattern)));
    assert!(drawing.contains(&format!("\x1B[5;1H{}", clear_pattern)));
    assert!(drawing.contains(&format!("\x1B[6;1H{}", clear_pattern)));
}

#[test]
fn test_exact_cursor_positioning_sequences() {
    let mut state = InputState::new(25, 12);
    
    // Test various cursor positions
    state.handle_key(KeyCode::Char('x'), KeyModifiers::NONE);
    
    let drawing = capture_terminal_drawing(&state, |buffer| {
        draw_prompt_line_to_buffer(buffer, &state.buffer, (state.cols, state.rows), state.required_lines)
    }).unwrap();
    
    // Should end with cursor positioned after 'x'
    // In a 25-col terminal, frame starts at row 10 (12 - 3 + 1)
    // Content is at row 11, cursor should be at column 6 (│ > x = 5 chars + 1)
    assert!(drawing.ends_with("\x1B[11;6H"));
}

#[test]
fn test_frame_border_characters_in_output() {
    let state = InputState::new(10, 5);
    
    let drawing = capture_terminal_drawing(&state, |buffer| {
        draw_prompt_line_to_buffer(buffer, "test", (state.cols, state.rows), state.required_lines)
    }).unwrap();
    
    // Should contain Unicode box drawing characters
    assert!(drawing.contains("╭"));  // Top-left corner
    assert!(drawing.contains("╮"));  // Top-right corner  
    assert!(drawing.contains("╰"));  // Bottom-left corner
    assert!(drawing.contains("╯"));  // Bottom-right corner
    assert!(drawing.contains("│"));  // Vertical bars
    assert!(drawing.contains("─"));  // Horizontal bars
}

#[test]
fn test_terminal_output_with_unicode_content() {
    let mut state = InputState::new(20, 8);
    
    // Add Unicode text
    for ch in "Hi 🌍".chars() {
        state.handle_key(KeyCode::Char(ch), KeyModifiers::NONE);
    }
    
    let drawing = capture_terminal_drawing(&state, |buffer| {
        draw_prompt_line_to_buffer(buffer, &state.buffer, (state.cols, state.rows), state.required_lines)
    }).unwrap();
    
    // Should properly encode Unicode in the terminal output
    assert!(drawing.contains("│ > Hi 🌍        │"));  // Actual padding from debug output 
    assert!(drawing.contains("\x1B[6;1H╭──────────────────╮"));
    assert!(drawing.contains("\x1B[8;1H╰──────────────────╯"));
}