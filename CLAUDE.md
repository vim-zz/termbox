# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A terminal-based input box application written in Rust that provides a fixed multi-line input interface at the bottom of the terminal while allowing the rest of the terminal to scroll normally. The application uses crossterm for terminal manipulation and implements features like dynamic frame sizing, text wrapping, and proper terminal cleanup.

## Build and Development Commands

```bash
# Build the project
cargo build

# Run the application
cargo run

# Run in release mode
cargo build --release
cargo run --release

# Run tests
cargo test

# Check code with clippy
cargo clippy

# Format code
cargo fmt

# Check formatting without applying changes
cargo fmt --check
```

## Architecture

The application is structured as a Rust library (`src/lib.rs`) with the main application (`src/main.rs`) that implements a terminal UI with the following key components:

1. **Terminal Setup**: Uses crossterm to enable raw mode and handle terminal events
2. **Scroll Region Management**: Implements DECSTBM escape sequences to create a fixed input area at the bottom while allowing the rest of the terminal to scroll
3. **Dynamic Frame Calculation**: Automatically adjusts the input box height based on text content and terminal width
4. **Multi-line Input**: Supports Alt+Enter or Ctrl+J for inserting newlines within the input
5. **Proper Cleanup**: Ensures terminal state is restored on exit

### Core Library (`src/lib.rs`)
- `InputState`: Main state management struct with key handling and resize logic
- `calculate_required_lines()`: Determines frame height based on text content and terminal width
- `calculate_cursor_position()`: Calculates exact cursor placement for text input
- `render_text_lines()`: Creates string representation for testing
- Drawing functions: `draw_frame_to_buffer()`, `draw_prompt_line_to_buffer()` for terminal output

### Main Application (`src/main.rs`) 
- Async event loop using tokio and crossterm's EventStream
- `push_content_up()`: Pushes existing terminal content up before drawing input frame
- Progress animation functionality with tiktok-style progress bars
- Terminal setup, cleanup, and scroll region management

## Input Controls

- **Enter**: Submit the current input and clear the buffer
- **Alt+Enter** or **Ctrl+J**: Insert a newline for multi-line input
- **Backspace**: Delete the last character
- **Esc**, **Ctrl+C**, or **Ctrl+D**: Exit the application

## Testing

The project includes comprehensive unit tests in the `tests/` directory:

- `tests/input_tests.rs`: Tests for input handling, key events, and state management
- `tests/output_tests.rs`: Tests for terminal output rendering and display logic

Run tests with standard Rust testing commands. Tests use the library's public API to verify input handling, text wrapping, cursor positioning, and frame calculation logic.