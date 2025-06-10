# `lazyactions`

**A Terminal User Interface for GitHub Actions.**

`lazyactions` provides a clean, auto-refreshing TUI to monitor recent GitHub Action runs for your current Git repository. Inspired by [lazydocker](https://github.com/jesseduffield/lazydocker), it's crafted in Rust to offer a responsive and efficient experience directly in your terminal.

While `lazyactions` is designed for quick oversight of your action runs, for more extensive management and interaction with GitHub Actions, you might find [GAMA](https://github.com/termkit/gama) to be a valuable, feature-rich alternative.

## Prerequisites

To get `lazyactions` up and running, you'll need:

1.  **GitHub CLI (`gh`):** `lazyactions` utilizes the official GitHub command-line tool to fetch action data.
2.  **Cargo Package Manager:** As a Rust application, `lazyactions` requires Cargo for installation.

## Installation

Installing `lazyactions` is straightforward via Cargo:

```bash
cargo install lazyactions
```

## Usage

Simply run lazyactions inside a git repo, with GH CLI authenticated.

## How It Works

`lazyactions` leverages the [Ratatui](https://ratatui.rs) library to build its interactive terminal interface. The application's structure follows an [event-driven template](https://github.com/ratatui/templates/tree/main/event-driven), a common and robust pattern for TUI applications, ensuring responsiveness and maintainability.

## License

Copyright (c) Ben <ben.farrington@nisien.ai>

This project is licensed under the MIT License. For full details, please refer to the [LICENSE](./LICENSE) file.

---