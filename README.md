# Glyph

> A zero-latency, bare-metal 2D whiteboard with native Vim bindings. Built in Rust on the Bevy engine.

Glyph is an interactive canvas built for developers who think spatially but refuse to leave the home row. It bridges the messy freedom of a whiteboard with the structured, lightning-fast input of a terminal editor.

By utilizing Bevy's Entity Component System alongside an O(1) Spatial Hash Grid, Glyph handles thousands of nodes with zero input lag. Visualize code architecture by crawling your codebase through Tree-sitter ASTs, or sketch freeform diagrams — all without touching your mouse.

## ⚡ Features

- **Vim-Native Navigation** — `hjkl` movement, `f` easymotion jump, `i` insert, `n` new node, `dd` delete. All home-row.
- **Speed of Thought Graphing** — `a` add edge + node, `yy` duplicate, `ce` connect existing, `ge` edge labels. No reaching.
- **Standard Mouse Fallback** — Middle-click pan, scroll zoom, click-and-drag. Works like Miro when you want it to.
- **Fuzzy Finder (`/`)** — Search all nodes by text, jump camera to the match. Like Telescope for your canvas.
- **Shell Piping (`!`)** — Select a node, press `!`, type a command. Node text is piped to stdin, stdout becomes a new connected node.
- **AST Crawler (`:crawl`)** — Auto-generate spatial call-graphs from Rust, Python, and TypeScript codebases.
- **Dotfile Config (`~/.glyphrc`)** — Customize background and node colors via TOML.
- **Stdin Piping** — `cat file.glyph | glyph` to load from stdin.
- **Headless Export** — `glyph --headless --export out.png` for CI/automation screenshots.
- **Infinite Scaling** — Dynamic spatial index ensures off-screen nodes are culled. 120+ FPS with 10,000+ entities.
- **Privacy-First** — No cloud. State is serialized to local `.glyph` files.

## 📖 User Guide

See **[USER_GUIDE.md](USER_GUIDE.md)** for a complete reference of keybindings, modes, and features.

## 🚀 Getting Started

```bash
git clone https://github.com/abendrothj/glyph.git
cd glyph
cargo run --release
```

> Running in `--release` mode is highly recommended for optimal Bevy rendering performance.

### Configuration

Create `~/.glyphrc` (TOML):
```toml
background_color = "#1e1e2e"
node_color = "#313244"
```

### CLI Options

```bash
glyph                                    # Normal interactive mode
cat session.glyph | glyph               # Load from stdin (JSON)
glyph --headless --export screenshot.png # Headless screenshot
```

## 🏗️ Architecture

```
src/
├── core/       → ECS components, state machine, resources, config, helpers
├── input/      → Vim mode systems, mouse selection, easymotion, camera
├── ui/         → egui overlays: command palette, fuzzy finder, shell
├── render/     → Edge/node drawing, force-directed layout, cluster blobs
├── io/         → File save/load, stdin piping, headless export
└── crawler/    → Tree-sitter AST parsing (Rust, Python, TypeScript)
```

## 🗺️ Roadmap

- [x] Core ECS, State Machine, Vim Spatial Grammar
- [x] Graphics Pipeline (Gizmo edges, dynamic text, state highlighting)
- [x] Mouse & Camera Controls (screen-to-world math, panning, zooming)
- [x] Infinite Scaling (O(1) Spatial Hash Grid, viewport culling)
- [x] Serialization (local `.glyph` save/load)
- [x] Immediate-Mode UI (command palette, toolbars via bevy_egui)
- [x] AST Crawler (auto-generate navigable spatial call-graphs)
- [x] Unix Power Tools (dotfile config, stdin, headless, fuzzy finder, shell pipe)
- [ ] Undo/Redo (`u` / `Ctrl+R`)
- [ ] Multi-select & Bulk Operations
- [ ] Node Auto-resize
- [ ] Minimap / Overview Panel

## 🤝 Contributing

Contributions, issues, and feature requests are welcome!

License: MIT / Apache 2.0