# Termbox

A terminal-based input box application written in Rust that provides a fixed multi-line
input interface at the bottom of the terminal while allowing the rest of the terminal
to scroll normally.

## Features

- Fixed input box at the bottom of the terminal
- Multi-line text input with word wrapping
- Dynamic frame sizing based on content
- Terminal scroll region management
- Special commands with animated feedback
- Proper terminal cleanup on exit

## Building and Running

```bash
# Build the project
cargo build

# Run the application
cargo run

# Run in release mode
cargo build --release
cargo run --release
```

## Usage

### Input Controls

- **Enter**: Submit the current input and display it in the scrollable area
- **Alt+Enter** or **Ctrl+J**: Insert a newline for multi-line input
- **Backspace**: Delete the last character
- **Esc**, **Ctrl+C**, or **Ctrl+D**: Exit the application

### Commands

The application supports special commands that trigger animations:

- **tiktok**: Displays an animated progress bar that counts from 1/10 to 10/10

## Architecture

The application consists of several modules:

- `main.rs`: Main event loop and terminal setup
- `lib.rs`: Core data structures and utility functions
- `ui.rs`: Frame drawing and terminal UI functions
- `commands/`: Command handling system
  - `commands.rs`: Command dispatcher and enum-based command system
  - `tiktok.rs`: TikTok progress bar command implementation

## Technical Details

The application uses:

- **crossterm** for cross-platform terminal manipulation
- **tokio** for async runtime and concurrent animations
- **ANSI escape sequences** for scroll region management
- **Unicode box drawing characters** for frame borders

The input box uses DECSTBM (Set Top and Bottom Margins) escape sequences to create a fixed area
at the bottom of the terminal while allowing the rest of the content to scroll independently.
