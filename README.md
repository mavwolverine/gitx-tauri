# GitX-Tauri (Beta)

A modern, cross-platform Git repository viewer inspired by [GitX](https://github.com/gitx/gitx), built with Tauri, Rust, and React.

> **Note**: This project is in beta. Features are actively being developed and some functionality may be incomplete.

## Why GitX-Tauri?

GitX is a beloved macOS Git GUI that's no longer actively maintained and has compatibility issues with recent macOS versions. GitX-Tauri aims to preserve the familiar GitX UI/UX while bringing it to modern platforms with cross-platform support.

## Features

- 🌳 **Visual Git Graph** - Colored branch visualization with merge tracking
- 📊 **Commit History** - Browse up to 10,000 commits with filtering (All/Local/Current branch)
- 🔍 **File Diffs** - Unified diff viewer with line numbers and change highlighting
- 🌿 **Branch Management** - View and switch between local and remote branches
- 🏷️ **Tag Support** - Navigate to tagged commits
- 📝 **Stage View** - Review uncommitted changes
- 🎨 **Dark Theme** - GitX-inspired dark interface
- 🖥️ **Cross-Platform** - Works on macOS, Linux, and Windows

## Screenshots

_Coming soon_

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (v18 or later)
- [pnpm](https://pnpm.io/) (recommended) or npm

### Building from Source

```bash
# Clone the repository
git clone https://github.com/mavwolverine/gitx-tauri.git
cd gitx-tauri

# Install dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Build for production
pnpm tauri build
```

## Usage

1. Launch GitX-Tauri
2. Select a Git repository from your filesystem
3. Browse commits, view diffs, and navigate branches

### Cloning Repositories

GitX-Tauri supports cloning repositories via SSH. To use this feature, ensure your SSH agent is running and your key is loaded:

```bash
# Start SSH agent (if not already running)
eval "$(ssh-agent -s)"

# Add your SSH key
ssh-add ~/.ssh/id_rsa  # or your specific key file
```

### Keyboard Shortcuts

_Coming soon_

## Technology Stack

- **Frontend**: React + TypeScript + Vite
- **Backend**: Rust + Tauri
- **Git Operations**: libgit2 (via git2-rs)

## Roadmap

- [ ] Syntax highlighting in diffs
- [ ] Commit creation and amending
- [ ] Branch creation and deletion
- [ ] Push/pull operations
- [ ] Stash management
- [ ] Cherry-pick and rebase support
- [ ] Search functionality
- [ ] Keyboard shortcuts
- [ ] Custom themes
- [ ] Performance optimizations for large repositories

## Contributing

Contributions are welcome! This project is in beta and actively being developed.

### Development Setup

```bash
# Install dependencies
pnpm install

# Run linting
pnpm run lint

# Format code
pnpm run format

# Run Rust checks
cd src-tauri
cargo fmt
cargo clippy
```

## License

MIT License - see [LICENSE](LICENSE) for details

## Acknowledgments

- Inspired by [GitX](https://github.com/gitx/gitx) by Pieter de Bie
- Built with [Tauri](https://tauri.app/)
- Git operations powered by [libgit2](https://libgit2.org/)

## Repository Mirrors

- GitHub: https://github.com/mavwolverine/gitx-tauri
- Codeberg: https://codeberg.org/mavwolverine/gitx-tauri
