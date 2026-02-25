# Glyph User Guide

A reference for all keybindings, modes, and features. All primary bindings stay on the home row.

## Modes

| Mode | Description |
|------|-------------|
| **Vim Normal** | Default mode. Navigate, create, delete, connect. |
| **Vim Insert** | Edit node text. Type to add, Esc to exit. |
| **Vim Easymotion** | Jump to any visible node by typing its letter tag. |
| **Standard** | Mouse drag mode. Click node to select and drag. |

---

## Vim Normal Mode

### Movement
| Keys | Action |
|------|--------|
| `h` `j` `k` `l` | Move selected node (left, down, up, right). Speed increases 2.5× when held. |
| `f` | Enter Easymotion — jump to any visible node. |

### Creating
| Keys | Action |
|------|--------|
| `n` | New node at cursor (or viewport center). Enters Insert. |
| `i` | Insert mode. If nothing selected, creates node at cursor first. |
| `a` | Add edge + new node (from selected). Enters Insert. |
| `yy` | Duplicate selected node. Copies text and color. Enters Insert. |

### Connecting
| Keys | Action |
|------|--------|
| `ce` | Connect selected to existing. `c` then `e` → Easymotion, pick letter to connect. |
| `ge` | Open Edge Labels (command palette). Edit labels for all edges. |

### Deleting
| Keys | Action |
|------|--------|
| `dd` | Delete selected node and its edges. |
| `Delete` / `Backspace` | Same as `dd`. |

---

## Vim Insert Mode

| Keys | Action |
|------|--------|
| Type | Add characters to selected node. |
| `Backspace` or `Ctrl+h` | Delete character. Hold for repeat (0.4s delay, then 50ms repeat). |
| `Esc` or `Ctrl+[` | Return to Normal. |

---

## Vim Easymotion Mode

| Keys | Action |
|------|--------|
| Type letter | Jump to that node (or connect if entered via `ce`). |
| `Esc` or `Ctrl+[` | Cancel without jumping. |

---

## Standard Mode (Mouse)

| Action | Result |
|--------|--------|
| Click node | Select and start dragging. |
| Shift+click node | Start drawing edge. Drag to target, release. |
| Click empty | Deselect. |
| Double-click empty | Create node at click position. |

---

## Command Palette (Cmd+K or `ge`)

| Action | Result |
|--------|--------|
| Search | Filter commands and edge labels by typing. |
| Save Workspace | Save to current file (or workspace.glyph). |
| Load workspace.glyph | Load default workspace file. |
| Open file... | File picker to open any .glyph file. |
| Open path | Type a file path (e.g. /path/to/workspace.glyph) and click Open or press Enter. |
| Add Node | Create node at viewport center. |
| Delete Selected Node | Remove selected node and edges. |
| Clear Canvas | Remove all nodes and edges. |
| Edge Labels | Edit labels for each edge (A → B: [label]). |
| :crawl [path] | Crawl a codebase to generate a Spatial Flow Graph. |
| :trace flow | Interactive Threat Mapping to trace data flow from Source to Sink. |
| Esc or Ctrl+[ | Close palette. |

---

## File Menu

| Item | Shortcut | Action |
|------|----------|--------|
| Open... | Ctrl+O / Cmd+O | Open .glyph file. |
| Save | Ctrl+S / Cmd+S | Save to current file. |
| Save As... | — | Save to new file. |
| Edit → Edge Labels... | — | Open command palette. |

---

## Camera

| Action | Result |
|--------|--------|
| Scroll | Zoom in/out. |
| Middle-click drag | Pan canvas. |
| Space + left-drag | Pan canvas (no middle button needed). |
| Arrow keys | Pan (hold for continuous). |

---

## Project Preferences

Camera zoom and position are saved per project. When you save a .glyph file, the current view (pan + zoom) is stored. Loading that file restores the view.

## Tips

- **Home row only:** `i` `f` `ge` `n` `a` `yy` `ce` `dd` `hjkl` — no reaching.
- **Connect flow:** Select source → `ce` → type target letter.
- **Duplicate flow:** Select node → `yy` → edit the copy.
- **Empty canvas:** Use `n` or `i` to create first node.
