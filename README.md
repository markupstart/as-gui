# AS-GUI 📦

A modern, terminal-based GUI for browsing and installing Void Linux packages using `xbps`. Built with Rust and featuring fuzzy search, interactive installation, and a clean TUI interface.

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white)
![Void Linux](https://img.shields.io/badge/Void%20Linux-478061?style=for-the-badge)
![License](https://img.shields.io/badge/license-MIT-blue.svg?style=for-the-badge)

## ✨ Features

- 🔍 **Fuzzy Search**: Fast, intelligent package searching across names and descriptions
- 📋 **Package Browser**: Browse all available packages from Void repositories
- 💾 **Interactive Installation**: Install packages with real-time feedback
- ✨ **Clean TUI**: Modern terminal user interface with intuitive navigation
- ⚡ **Performance**: Async operations for smooth user experience
- 🎯 **Package Details**: View comprehensive package information before installing

## 🎬 Demo

```
┌─ Search (type to filter packages) ─────────────────────────────────┐
│ firefox                                                            │
└────────────────────────────────────────────────────────────────────┘
┌─ Packages (3 of 15000) ────────────────────────────────────────────┐
│ > firefox 131.0.3-1 - Standalone web browser from mozilla.org    │
│   firefox-developer-edition 132.0b9-1 - Developer Edition        │
│   firefox-i18n-af 131.0.3-1 - Afrikaans language pack           │
└────────────────────────────────────────────────────────────────────┘
┌─ Package Details ──────────────────────────────────────────────────┐
│ Package: firefox                                                   │
│ Version: 131.0.3-1                                                 │
│ Repository: extra                                                  │
│ Description: Standalone web browser from mozilla.org              │
└────────────────────────────────────────────────────────────────────┘
┌────────────────────────────────────────────────────────────────────┐
│ ↑/↓ or j/k: navigate | Enter: install | q: quit | Type to search  │
└────────────────────────────────────────────────────────────────────┘
```

## 🚀 Installation

### Prerequisites

- **Void Linux**
- **Rust** (1.70+ recommended)
- **xbps** package manager (`xbps-query`, `xbps-install`)
- **sudo** privileges for package installation

### Build from Source

1. **Clone the repository:**
   ```bash
   git clone <repository-url>
   cd as-gui
   ```

2. **Build the application:**
   ```bash
   cargo build --release
   ```

3. **Run the application:**
   ```bash
   ./target/release/as-gui
   ```

### Install System-wide (Optional)

```bash
# Copy to a directory in your PATH
sudo cp target/release/as-gui /usr/local/bin/
```

## 🎮 Usage

### Starting the Application

```bash
./target/release/as-gui
```

The application will automatically load all available packages from your configured repositories.

### Navigation & Controls

| Key | Action |
|-----|--------|
| `↑` / `k` | Navigate up |
| `↓` / `j` | Navigate down |
| `Enter` | Install selected package |
| `Type` | Search packages (fuzzy search) |
| `Backspace` | Remove last search character |
| `Esc` | Clear error messages |
| `q` | Quit application |

### Package Search

- **Type any characters** to start searching
- Search works across **package names** and **descriptions**
- **Fuzzy matching** - you don't need exact spelling
- **Multi-word search** - all words must match (in any order)

#### Search Examples:
- `fire` → finds Firefox, Firewall tools, etc.
- `web browser` → finds web browsers
- `python dev` → finds Python development tools

### Package Installation

1. **Navigate** to your desired package
2. **Press Enter** to start installation
3. **Enter sudo password** when prompted
4. **Wait for installation** to complete
5. **Press Enter** to return to the package browser

The application will temporarily exit GUI mode during installation to provide full terminal access for `xbps-install`.

## 🏗️ Architecture

### Technology Stack

- **Language**: Rust 🦀
- **TUI Framework**: [Ratatui](https://github.com/ratatui-org/ratatui)
- **Terminal Handling**: [Crossterm](https://github.com/crossterm-rs/crossterm)
- **Async Runtime**: [Tokio](https://tokio.rs/)
- **Fuzzy Search**: [fuzzy-matcher](https://github.com/lotabout/fuzzy-matcher)

### Key Components

- **App State Management**: Centralized state with browsing, loading, and error states
- **Package Loading**: Async xbps repository parsing
- **Search Engine**: Multi-word fuzzy matching with relevance scoring
- **Terminal Management**: Robust state transitions between GUI and terminal modes
- **Installation Handler**: Interactive package installation with error handling

## 🛠️ Development

### Project Structure

```
as-gui/
├── src/
│   └── main.rs          # Main application logic
├── Cargo.toml           # Dependencies and metadata
├── Cargo.lock           # Dependency lock file
├── test_gui_fix.sh      # Test script for GUI issues
└── README.md            # This file
```

### Building for Development

```bash
# Debug build (faster compilation)
cargo build

# Release build (optimized)
cargo build --release

# Run with cargo
cargo run
```

### Testing

```bash
# Run the GUI fix test
./test_gui_fix.sh

# Check code quality
cargo clippy

# Format code
cargo fmt
```

## 🐛 Troubleshooting

### Common Issues

**GUI disappears after installation:**
- This was a known issue that has been **fixed** in the current version
- The fix includes improved terminal state restoration with proper buffer clearing

**Application won't start:**
- Ensure you're running on **Void Linux**
- Check that `xbps-query` is available: `which xbps-query`
- Verify Rust installation: `rustc --version`

**Permission denied during installation:**
- Make sure you have `sudo` privileges
- Package installation requires root access via `sudo xbps-install`

**Slow package loading:**
- Large repository indexes may take a moment to load
- The application loads all packages on startup for optimal search performance

### Performance Tips

- **Search efficiently**: Use specific terms to narrow results quickly
- **Terminal size**: Larger terminals provide better viewing experience
- **Network**: Ensure good internet connection for package downloads

## 🤝 Contributing

Contributions are welcome! Here are some ways to help:

1. **Report bugs** by opening issues
2. **Suggest features** for enhancement
3. **Submit pull requests** with improvements
4. **Improve documentation**

### Development Setup

```bash
# Fork and clone the repository
git clone <your-fork-url>
cd as-gui

# Create a feature branch
git checkout -b feature/amazing-feature

# Make changes and test
cargo test
./test_gui_fix.sh

# Commit and push
git commit -m "Add amazing feature"
git push origin feature/amazing-feature
```

## 📋 Roadmap

- [ ] **Package Groups**: Browse packages by category
- [ ] **Installation History**: Track installed packages
- [ ] **Package Information**: Show more detailed package metadata
- [ ] **Configuration**: User preferences and settings
- [ ] **Package Removal**: Uninstall packages through the GUI
- [ ] **Repository Filtering**: Filter results by repository source
- [ ] **Themes**: Customizable color schemes

## 📄 License

This project is licensed under the **MIT License** - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- **Void Linux** community for the excellent package management system
- **Ratatui** team for the amazing TUI framework
- **Rust** community for the powerful language and ecosystem

---

**Made with ❤️ for the Void Linux community**

*"Void + xbps" - Now with a GUI!*
# as-gui
