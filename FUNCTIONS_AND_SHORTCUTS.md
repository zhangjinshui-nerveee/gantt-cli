# gantt-cli — User Interactions & Keyboard Shortcuts

A guide to everything you can *do* in gantt-cli and the key(s) that trigger it. This
reflects the actual key-dispatch code in `src/main.rs`, not just the README, so it's
current even where the README has drifted.

> **Stale doc note:** Both `README.md` and the in-app footer hint still mention an `M`
> key for a "toggle details view" feature. It no longer exists — pressing `M` does
> nothing. Leftover reference to a removed feature.
>
> **Removed:** The Scheduled Events / Reminders feature (`S` key) has been deleted
> from the app — it's no longer available.

---

## 1. Navigation & Focus

The UI has four focus areas you cycle through: **Project fields → Task table → Todo
list (if open) → Ntfy topic field**.

| Key | Interaction |
|---|---|
| `j` / `↓` | Move selection down |
| `k` / `↑` | Move selection up |
| `h` / `←` | Move to previous field (task table columns), or collapse out of a todo subtask |
| `l` / `→` | Move to next field, or drill into a todo item's subtasks |
| `g` | Jump to first task |
| `G` | Jump to last task |
| `Tab` | Move focus to the next area |
| `Shift+Tab` | Move focus to the previous area |

Moving up past the first task returns focus to the project fields; moving up from the
project name field does nothing further (it's the top).

## 2. Task Management

| Key | Interaction |
|---|---|
| `a` | Add a new sibling task right after the selected one (inherits its parent) and immediately opens it for naming |
| `A` | Add a new top-level task at the end of the list and open it for naming |
| `s` | Add a subtask under the selected task, defaulting its start date to the parent's |
| `D` | Delete the selected task |
| `Enter` | Edit the selected field of the selected task |
| `>` | Indent the task — makes it a child of the task directly above it |
| `<` | Unindent the task — promotes it to its parent's level |
| `Shift+K` | Move the task (and its whole subtree) up, skipping over its parent's boundary |
| `Shift+J` | Move the task (and its whole subtree) down |

While editing a field:
- **Name / Assigned To**: free text.
- **Duration**: a number of days, or a number with a `w`/`m`/`y` suffix (weeks/months/years), e.g. `2w` → 14 days.
- **Progress**: 0–100 (values above 100 are clamped).
- **Dependencies**: comma-separated task IDs (the short outline IDs shown in the table, e.g. `1a`, `2`). Setting any dependency clears the task's manual start date (dates become dependency-driven). Invalid IDs are dropped with a status-bar warning.
- **Start Date**: `mm/dd/yy` or the literal word `today`. Only editable if the task has no incomplete dependencies — otherwise you get "Cannot edit Start Date: dependencies are not all finished." Setting a manual start date clears any dependencies.
- **End Date**: `mm/dd/yy` or `today` — this recomputes the task's *duration* to match, rather than being stored directly.

Task dates are otherwise computed automatically: a task with dependencies starts the
day after its last dependency finishes; a parent task's start/end always expands to
cover the earliest/latest of its children.

## 3. Gantt / Timeline View

| Key | Interaction |
|---|---|
| `t` | Jump the visible calendar window to today |
| `H` | Scroll the timeline one day earlier |
| `L` | Scroll the timeline one day later |
| `O` | Toggle highlight mode between **Today** (highlights tasks active today) and **Urgent** (colors tasks yellow→orange the further behind schedule they are) |
| `Z` | Toggle compact timeline: 1 character per day vs. the normal 3 characters per day |

Visual cues you'll see without pressing anything: completed tasks (100%) are dimmed
gray; overdue, incomplete tasks are red; today's column and any project deadline day
are marked in the header.

## 4. Todo List

`T` toggles a todo-list popup that overlays the task view.

| Key | Interaction |
|---|---|
| `T` | Open/close the todo list |
| `j`/`k`, `↓`/`↑` | Move between todo items (or between subtasks, once drilled in) |
| `l` / `→` | Drill into a todo item's subtasks |
| `h` / `←` | Back out of subtasks to the item level |
| `a` | Add a new top-level todo item — **or**, if you're positioned inside a todo's subtasks, insert a new subtask right after the current one |
| `i` | Append a new subtask to the end of the selected todo item's subtask list |
| `Enter` | Edit the selected item's or subtask's text |
| `Space` | Toggle complete. Completing the top 3 items or a linked task jumps you to that task's Progress field for a quick update; a matching task elsewhere is also located via `sync_project_with_todo_selection`-style lookup |
| `-` | Remove the selected item (or subtask, if drilled in) |
| `Shift+C` | Clear all completed items |
| `Esc` | Close the todo list |

The first 3 undone items are shown bold ("priority"); once all three are done, the
rest un-dim. Any todo item whose text matches a task name gets a highlighted row in
the task table.

**Push to phone**: `Ctrl+f` sends the whole todo list (grouped into Priority / Future /
Done sections with emoji markers) as a push notification to `ntfy.sh`, using a
per-install topic name that's editable via the Ntfy Topic field (`Tab` to it, `Enter`
to edit).

## 5. Project Management

gantt-cli manages multiple independent projects, each with its own tasks and dates.

| Key | Interaction |
|---|---|
| `N` | Switch to the next project (skips archived ones unless "show archived" is on) |
| `P` | Switch to the previous project |
| `C` | Create a new empty project and focus its name field (Tasks focus only — inside the todo list, `C` instead clears completed todos) |
| `X` | Archive the current project, or unarchive it if already archived. Archiving auto-advances you to the next non-archived project (unless that would leave none, in which case archived projects are revealed) |
| `Ctrl+b` | Toggle whether archived projects are shown at all |
| `Ctrl+n` | Move the current project forward in the project order |
| `Ctrl+p` | Move the current project backward in the project order |
| `Ctrl+d` | Delete the current project — **press twice** to confirm (any other key cancels); blocked if it's your only project |
| `Ctrl+u` | Restore the most recently deleted project |

## 6. Column Customization

`\` opens a small popup to show/hide task-table columns (Assigned To, Start Date, End
Date, Duration, Progress, Dependencies — Name is always shown).

| Key | Interaction |
|---|---|
| `\` | Open/close the column-visibility popup |
| `j`/`k`, `↓`/`↑` | Move between columns |
| `Space` / `Enter` | Toggle the selected column's visibility |
| `Esc` / `\` | Close |

## 7. Data, Undo & Session

| Key | Interaction |
|---|---|
| `Ctrl+s` | Save all projects to disk. If another instance modified the file since your last save/load, you're warned and must press `Ctrl+s` again to overwrite |
| `u` | Undo the last change |
| `Ctrl+r` | Redo |
| `q` | Quit — if you have unsaved changes, the first press warns you and a second press discards them (or `Ctrl+s` to save first) |

Undo/redo covers task edits, todo edits, and project structure changes — it snapshots
the entire data set, not just the current project.

## 8. Help

| Key | Interaction |
|---|---|
| `?` | Open the in-app help screen (a condensed version of this guide) |
| `?` / `Esc` | Close it |
