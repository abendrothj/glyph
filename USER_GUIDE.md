# Glyph User Guide

A complete reference for all keybindings, modes, and features. All primary bindings stay on the home row.

## Modes

| Mode | Description |
|------|-------------|
| **Vim Normal** | Default. Navigate, create, delete, connect. |
| **Vim Insert** | Edit node/edge text. Type to add, Esc to exit. |
| **Vim Easymotion** | Jump to visible node by typing its letter tag. |
| **Vim Command** | `:` command-line for save, open, crawl. |
| **Standard** | Mouse drag mode. Click to select and drag. |

---

## Vim Normal Mode

### Movement
| Keys | Action |
|------|--------|
| `h` `j` `k` `l` | Move selected node. Accelerates 2.5× when held. |
| `f` | Easymotion — jump to any visible node. |
| Arrow keys | Pan camera. |

### Creating
| Keys | Action |
|------|--------|
| `n` | New node at cursor (or viewport center). Enters Insert. |
| `i` | Insert mode. Creates node at cursor first if nothing selected. |
| `a` | Add edge + new node from selected. Enters Insert. |
| `yy` | Duplicate selected node with text and color. |

### Connecting
| Keys | Action |
|------|--------|
| `ce` | Connect selected → existing. Easymotion picks the target. |
| `ge` | Edit edge labels via Easymotion. |

### Deleting
| Keys | Action |
|------|--------|
| `dd` | Delete selected node and its edges. |
| `Delete` / `Backspace` | Same as `dd`. |

### Search & Shell
| Keys | Action |
|------|--------|
| `/` | **Fuzzy Finder** — search nodes by text, jump camera to match. |
| `!` | **Shell Execute** — pipe selected node text through a shell command, spawn stdout as new connected node. |

### Marks
| Keys | Action |
|------|--------|
| `m` + letter | Set a named mark at the current selected node position. |
| `'` + letter | Jump camera to a named mark. |

### Command Line
| Keys | Action |
|------|--------|
| `:` (Shift+;) | Enter command-line mode. |

---

## Vim Insert Mode

| Keys | Action |
|------|--------|
| Type | Add characters to selected node (or edge label). |
| `Backspace` / `Ctrl+h` | Delete character. Hold for repeat (0.4s delay, then 50ms). |
| `Esc` / `Ctrl+[` | Return to Normal. |

---

## Vim Easymotion

| Keys | Action |
|------|--------|
| Type letter | Jump to that node (or connect if via `ce`). |
| `Esc` / `Ctrl+[` | Cancel. |

---

## Command-Line Mode (`:`)

| Command | Action |
|---------|--------|
| `:w [path]` | Save to current file or specified path. |
| `:e <path>` | Open a `.glyph` file. |
| `:crawl <path>` | Crawl codebase, generate spatial call-graph. |
| `:crawl <path> --no-flow` | Crawl without data-flow edges. |
| `:trace flow` | Interactive threat mapping — trace data paths. |

---

## Standard Mode (Mouse)

| Action | Result |
|--------|--------|
| Click node | Select and start dragging. |
| Shift+click node | Start drawing edge. Drag to target. |
| Click empty | Deselect. |
| Double-click empty | Create node at click position. |

---

## Command Palette (`Cmd+K`)

| Action | Result |
|--------|--------|
| Search | Filter commands/edges by typing. |
| Save / Load / Open | File operations. |
| Add Node | Create at viewport center. |
| Delete Selected | Remove node and edges. |
| Clear Canvas | Remove everything. |
| Edge Labels | Edit all edge labels. |
| `Esc` / `Ctrl+[` | Close palette. |

---

## Camera

| Action | Result |
|--------|--------|
| Scroll | Zoom in/out. |
| Middle-click drag | Pan canvas. |
| Space + left-drag | Pan (no middle button needed). |
| `+` / `-` | Zoom in/out (keyboard). |
| Arrow keys | Pan (hold for continuous). |

---

## Configuration (`~/.glyphrc`)

TOML format:
```toml
background_color = "#1e1e2e"   # Catppuccin Mocha Base
node_color = "#313244"         # Catppuccin Surface0
```

Colors are hex strings. Invalid values fall back to defaults.

---

## CLI Options

```bash
glyph                                    # Interactive mode
cat session.glyph | glyph               # Load JSON from stdin
glyph --headless --export screenshot.png # Headless screenshot export
```

---

## Tips

- **Home row only:** `i` `f` `ge` `n` `a` `yy` `ce` `dd` `hjkl` — no reaching.
- **Connect flow:** Select source → `ce` → type target letter.
- **Duplicate flow:** Select → `yy` → edit the copy.
- **Pipe chain:** Select node → `!` → `wc -l` → creates word-count node connected by edge.
- **Find anything:** `/` → type partial text → Enter jumps to best match.
- **Camera prefs:** Zoom and position are saved per `.glyph` file.
