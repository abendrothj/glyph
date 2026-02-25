# Glyph

> A zero-latency, bare-metal 2D whiteboard. Native Vim bindings for the keyboard, standard UI tools for the mouse. Built in Rust, powered by the Bevy engine.

Glyph is an interactive security auditing dashboard and anti-Electron canvas built for developers who think spatially but refuse to leave the home row. It bridges the gap between the messy freedom of a whiteboard and the structured, lightning-fast input of a terminal editor.

By bypassing the DOM and utilizing Bevy's Entity Component System (ECS) alongside an $O(1)$ Spatial Hash Grid, Glyph handles thousands of nodes with zero input lag. Visualizing the exact path a payload takes is half the battle when building security vulnerability tools like doppel or adversarial attribution engines. Glyph traces data flow through Tree-sitter ASTs and highlights the exact path across your canvas.

## ⚡ Features

* **Vim-Native Navigation:** Use `hjkl` for micro-adjustments and `f` (Easymotion) to instantly jump focus across the canvas without touching your mouse.
* **Speed of Thought Graphing:** All home-row: `n` new node, `a` add edge + node, `yy` duplicate, `ce` connect to existing (easymotion), `ge` edge labels, `dd` delete. `Ctrl+[` for Esc, `Ctrl+h` for backspace. `hjkl` movement accelerates when held. Double-click empty space to create a node.
* **Standard Mouse Fallback:** Fully supports middle-click panning, scroll zooming, and standard click-and-drag for times when you just want to use it like Miro.
* **Infinite Scaling:** A dynamic spatial index ensures off-screen nodes are culled, keeping framerates pinned at 120+ FPS even with 10,000+ entities.
* **Privacy-First (WIP):** No cloud databases. State is serialized and hydrated strictly to/from local `.glyph` files on your machine.

## 📖 User Guide

See **[USER_GUIDE.md](USER_GUIDE.md)** for a complete reference of keybindings, modes, and features.

## 🚀 Getting Started

Ensure you have the Rust toolchain installed, then clone and run:

```bash
git clone https://github.com/abendrothj/glyph.git
cd glyph
cargo run --release
```

(Note: Running in --release mode is highly recommended for optimal Bevy rendering performance.)

## 🗺️ The Roadmap

- [x] Phase 1 & 2: Core ECS, State Machine (VimNormal, VimInsert, Standard), and Vim Spatial Grammar.
- [x] Phase 3: Graphics Pipeline (Gizmo edge rendering, dynamic text syncing, state highlighting).
- [x] Phase 4: Mouse & Camera Controls (Screen-to-world math, panning, zooming, drag-and-drop).
- [x] Phase 5: Infinite Scaling (O(1) Spatial Hash Grid, Viewport Culling).
- [x] Phase 6: Serialization (Local offline .glyph save/load state hydration).
- [x] Phase 7: Immediate-Mode UI (Command Palette, visual toolbars via bevy_egui).
- [x] Phase 8: AST Crawler (Crawl a codebase to auto-generate a navigable spatial call-graph).
- [ ] Phase 9: Data Flow Graph & Threat Mapping (Define Source and Sink, use `:trace flow` to visualize data paths in bright red).
- [ ] Phase 10: WebAssembly compilation for frictionless browser demos.

## 🤝 Contributing

Contributions, issues, and feature requests are welcome!

License: MIT / Apache 2.0