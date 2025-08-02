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

The application is a single-file Rust program (`src/main.rs`) that implements a terminal UI with the following key components:

1. **Terminal Setup**: Uses crossterm to enable raw mode and handle terminal events
2. **Scroll Region Management**: Implements DECSTBM escape sequences to create a fixed input area at the bottom while allowing the rest of the terminal to scroll
3. **Dynamic Frame Calculation**: Automatically adjusts the input box height based on text content and terminal width
4. **Multi-line Input**: Supports Alt+Enter or Ctrl+J for inserting newlines within the input
5. **Proper Cleanup**: Ensures terminal state is restored on exit

Key functions:
- `main()`: Event loop handling keyboard input and terminal resize events
- `calculate_required_lines()`: Determines frame height based on text content
- `set_scroll_region()`: Configures terminal scroll boundaries
- `draw_frame()`: Renders the box borders using Unicode characters
- `draw_prompt_line()`: Renders the input text with word wrapping

## Input Controls

- **Enter**: Submit the current input and clear the buffer
- **Alt+Enter** or **Ctrl+J**: Insert a newline for multi-line input
- **Backspace**: Delete the last character
- **Esc**, **Ctrl+C**, or **Ctrl+D**: Exit the application