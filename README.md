# gantt-cli

A terminal user interface (TUI) application for visualizing and managing Gantt charts directly from your command line. Built with Rust using ratatui, this tool provides a lightweight yet powerful way to keep track of project timelines and task dependencies without leaving your terminal.

![gantt_cli](https://github.com/user-attachments/assets/8da888dc-7225-4ffb-a8e2-e09bd414bb67)

## Features

- **Interactive Gantt Chart Display**: View tasks and their durations in a clear, interactive timeline
- **Hierarchical Tasks**: Create parent tasks with subtasks for better organization
- **Dependency Tracking**: Define task dependencies with automatic schedule calculation using topological sort
- **Multiple Projects**: Manage multiple projects with easy switching between them
- **Todo List**: Built-in personal todo list with completion tracking
- **Undo/Redo**: Full state history with undo (u) and redo (Ctrl+r) support
- **Details View**: Expand tasks to view and edit detailed descriptions
- **Urgent Task Highlighting**: Toggle urgent view mode with color intensity based on task urgency
- **Push Notifications**: Send todo items to your phone via [ntfy.sh](https://ntfy.sh)
- **Keyboard-driven Interface**: Efficient vim-style navigation and interaction

## Installation

Download the binary for your platform from the [Releases](https://github.com/nickschu/gantt-cli/releases) page, make it executable, and run it.

```bash
chmod +x gantt-cli
./gantt-cli
```

## Keybindings

Press `?` in the application to see the full help screen.

### Navigation
| Key | Action |
|-----|--------|
| `j/k` or `Up/Down` | Move up/down |
| `h/l` or `Left/Right` | Move left/right (fields) |
| `Tab/Shift+Tab` | Switch focus areas |
| `g/G` | Go to top/bottom |

### Task Operations
| Key | Action |
|-----|--------|
| `a` | Add sibling task |
| `A` | Add top-level task |
| `s` | Add subtask |
| `D` | Delete task |
| `Enter` | Edit selected field |
| `> / <` | Indent/unindent task |
| `M` | Toggle details view |
| `K/J` | Move task up/down |

### Todo List
| Key | Action |
|-----|--------|
| `T` | Toggle todo list panel |
| `+ / -` | Add/remove task from todo |
| `Space` | Toggle todo complete |
| `Shift+C` | Clear completed todos |

### Calendar
| Key | Action |
|-----|--------|
| `H/L` | Move calendar left/right |
| `t` | Jump to today |
| `O` | Toggle highlight mode (today/urgent) |

### Project Management
| Key | Action |
|-----|--------|
| `N/P` | Next/previous project |
| `C` | Create new project |
| `Ctrl+d` | Delete project (press twice to confirm) |
| `Ctrl+u` | Restore deleted project |
| `Ctrl+n/Ctrl+p` | Move project order |

### General
| Key | Action |
|-----|--------|
| `Ctrl+s` | Save all projects |
| `u / Ctrl+r` | Undo / Redo |
| `Ctrl+f` | Push todo to phone (ntfy) |
| `?` | Toggle help screen |
| `q` | Quit |

## Data Storage

Project data is stored in `~/.config/gantt-cli/projects.json` by default.

## Roadmap
- [ ] Enable cloud backup and sync
- [ ] Enable collaboration

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.

## License

This project is licensed under the [MIT License](https://opensource.org/licenses/MIT).
