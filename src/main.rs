use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    prelude::*,
    widgets::{block::*, *},
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, stdout};
use std::panic;
use std::path::Path;
use unicode_width::UnicodeWidthStr;

// --- DATA STRUCTURES ---

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    id: u32,
    name: String,
    assigned_to: String,
    duration: i64,
    progress: u8,
    dependencies: Vec<u32>,
    manual_start_date: Option<NaiveDate>,
    details: Option<String>,
    parent_id: Option<u32>,
    #[serde(skip)]
    start_date: Option<NaiveDate>,
    #[serde(skip)]
    end_date: Option<NaiveDate>,
}

#[derive(Clone, Serialize, Deserialize)]
struct ProjectData {
    project_name: String,
    project_start_date: NaiveDate,
    project_end_date: Option<NaiveDate>,
    week_to_show: u32,
    tasks: Vec<Task>,
}

#[derive(Clone, Serialize, Deserialize)]
struct AllProjectsData {
    projects: Vec<ProjectData>,
    active_project_index: usize,
    #[serde(default)]
    todo_list: Vec<String>,
}

#[derive(Clone)]
struct ProjectState {
    project_data: ProjectData,
}

// --- APPLICATION STATE ---

#[derive(PartialEq, Eq, Clone, Copy)]
enum InputMode {
    Normal,
    Editing,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum TaskField {
    Name,
    AssignedTo,
    StartDate,
    Duration,
    Progress,
    Dependencies,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum ProjectField {
    Name,
    StartDate,
    EndDate,
    WeekToShow,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum FocusArea {
    Project(ProjectField),
    Tasks,
    TodoList,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum HighlightMode {
    Today,
    Urgent,
}

struct App {
    all_projects: AllProjectsData,
    current_project_index: usize,
    today: NaiveDate,
    table_state: TableState,
    todo_list_state: ListState,
    input_mode: InputMode,
    focus_area: FocusArea,
    selected_task_field: TaskField,
    input_buffer: String,
    next_task_id: u32,
    should_quit: bool,
    status_message: String,
    gantt_area_width: u16,
    history: Vec<ProjectState>,
    redo_history: Vec<ProjectState>,
    current_file_path: String, // Always "projects.json"
    details_view_open: bool,
    todo_list_open: bool,
    details_buffer: String,
    highlight_mode: HighlightMode,
}

impl App {
    fn new() -> Self {
        let mut app = App {
            all_projects: AllProjectsData {
                projects: vec![],
                active_project_index: 0,
                todo_list: vec![],
            },
            current_project_index: 0,
            today: Local::now().date_naive(),
            table_state: TableState::default(),
            todo_list_state: ListState::default(),
            input_mode: InputMode::Normal,
            focus_area: FocusArea::Tasks,
            selected_task_field: TaskField::Name,
            input_buffer: String::new(),
            next_task_id: 1,
            should_quit: false,
            status_message: "Welcome! Press 'q' to quit.".to_string(),
            gantt_area_width: 0,
            history: vec![],
            redo_history: vec![],
            current_file_path: "projects.json".to_string(),
            details_view_open: false,
            todo_list_open: false,
            details_buffer: String::new(),
            highlight_mode: HighlightMode::Today,
        };

        let load_result = app.load_all_projects();
        if load_result.is_err() {
            let msg = format!("Failed to load projects from {}. Starting with a new default project.", app.current_file_path);
            app.status_message = msg;
            app.add_default_project();
        } else {
            let msg = format!("Projects loaded successfully from {}.", app.current_file_path);
            app.status_message = msg;
        }
        
        if !app.get_current_project().tasks.is_empty() {
            app.table_state.select(Some(0));
            app.focus_area = FocusArea::Tasks;
        } else {
            app.focus_area = FocusArea::Project(ProjectField::Name);
        }

        app.recalculate_schedule();
        app
    }

    fn add_default_project(&mut self) {
        let mut default_project = ProjectData {
            project_name: "New Project".to_string(),
            project_start_date: NaiveDate::from_ymd_opt(2024, 8, 1).unwrap(),
            project_end_date: None,
            week_to_show: 0,
            tasks: vec![],
        };
        default_project.tasks.push(Task { id: 0, name: "Requirement Gathering".into(), assigned_to: "Alice".into(), duration: 5, progress: 100, dependencies: vec![], manual_start_date: None, details: None, parent_id: None, start_date: None, end_date: None });
        default_project.tasks.push(Task { id: 0, name: "UI/UX Design".into(), assigned_to: "Bob".into(), duration: 7, progress: 50, dependencies: vec![1], manual_start_date: None, details: None, parent_id: None, start_date: None, end_date: None });
        
        self.all_projects.projects.push(default_project);
        self.current_project_index = self.all_projects.projects.len() - 1;
        self.history.clear();
        self.redo_history.clear();
    }

    fn add_new_project(&mut self) {
        self.save_all_projects().unwrap_or_else(|_| self.status_message = "Failed to save current project before creating new one.".into());
        let new_project_name = format!("New Project {}", self.all_projects.projects.len() + 1);
        let new_project = ProjectData {
            project_name: new_project_name.clone(),
            project_start_date: Local::now().date_naive(),
            project_end_date: None,
            week_to_show: 0,
            tasks: vec![],
        };
        self.all_projects.projects.push(new_project);
        self.current_project_index = self.all_projects.projects.len() - 1;
        self.history.clear();
        self.redo_history.clear();
        self.recalculate_schedule();
        self.table_state.select(None); // Deselect any task
        self.focus_area = FocusArea::Project(ProjectField::Name); // Focus on new project name
        self.status_message = format!("New project '{}' created.", new_project_name);
    }

    fn get_current_project(&self) -> &ProjectData {
        &self.all_projects.projects[self.current_project_index]
    }

    fn get_current_project_mut(&mut self) -> &mut ProjectData {
        &mut self.all_projects.projects[self.current_project_index]
    }

    fn add_task(&mut self, mut task: Task) -> usize {
        self.save_state_for_undo();
        let next_id = self.next_task_id;
        task.id = next_id;

        let selected_index = self.table_state.selected(); // Get selected_index before mutable borrow
        let current_project = self.get_current_project_mut();
        let new_task_index;

        if let Some(idx) = selected_index {
            current_project.tasks.insert(idx + 1, task);
            new_task_index = idx + 1;
        } else {
            current_project.tasks.push(task);
            new_task_index = current_project.tasks.len() - 1;
        }
        // next_task_id is updated in recalculate_schedule
        self.remap_ids_and_dependencies();
        new_task_index
    }

    fn delete_selected_task(&mut self) {
        if let FocusArea::Tasks = self.focus_area {
            if let Some(selected_index) = self.table_state.selected() {
                self.save_state_for_undo();
                let mut new_selected_index = None;
                let mut new_focus_area = self.focus_area;

                { 
                    let current_project = self.get_current_project_mut();
                    if selected_index < current_project.tasks.len() {
                        current_project.tasks.remove(selected_index);
                        if selected_index > 0 && current_project.tasks.len() > 0 && selected_index >= current_project.tasks.len() {
                            new_selected_index = Some(current_project.tasks.len() - 1);
                        } else if current_project.tasks.is_empty() {
                            new_selected_index = None;
                            new_focus_area = FocusArea::Project(ProjectField::WeekToShow);
                        } else if selected_index < current_project.tasks.len() {
                            new_selected_index = Some(selected_index);
                        } else if current_project.tasks.len() > 0 {
                            new_selected_index = Some(current_project.tasks.len() - 1);
                        }
                    }
                } 

                if let Some(idx) = new_selected_index {
                    self.table_state.select(Some(idx));
                } else {
                    self.table_state.select(None);
                }
                self.focus_area = new_focus_area;
                self.remap_ids_and_dependencies();
            }
        }
    }

    fn move_task_up(&mut self) {
        if let Some(selected_index) = self.table_state.selected() {
            if selected_index == 0 { return; }

            let my_family_range = self.get_contiguous_family_range(selected_index);
            
            if my_family_range.start == 0 { return; }
            
            let target_index_before = my_family_range.start - 1;
            let target_family_range = self.get_contiguous_family_range(target_index_before);

            self.save_state_for_undo();
            
            let tasks_to_move: Vec<_> = self.get_current_project_mut().tasks.drain(my_family_range.clone()).collect();
            
            let new_insert_index = target_family_range.start;
            self.get_current_project_mut().tasks.splice(new_insert_index..new_insert_index, tasks_to_move);
            
            self.table_state.select(Some(new_insert_index));
            self.remap_ids_and_dependencies();
        }
    }

    fn move_task_down(&mut self) {
        if let Some(selected_index) = self.table_state.selected() {
            let tasks_len = self.get_current_project().tasks.len();
            let my_family_range = self.get_contiguous_family_range(selected_index);

            if my_family_range.end >= tasks_len { return; }

            let target_index = my_family_range.end;
            let target_family_range = self.get_contiguous_family_range(target_index);

            self.save_state_for_undo();

            let tasks_to_move: Vec<_> = self.get_current_project_mut().tasks.drain(my_family_range.clone()).collect();
            
            let new_insert_index = target_family_range.end - my_family_range.len();

            self.get_current_project_mut().tasks.splice(new_insert_index..new_insert_index, tasks_to_move);
            
            self.table_state.select(Some(new_insert_index));
            self.remap_ids_and_dependencies();
        }
    }

    fn remap_ids_and_dependencies(&mut self) {
        let current_project = self.get_current_project_mut();
        let id_map: HashMap<u32, u32> = current_project.tasks
            .iter()
            .enumerate()
            .map(|(i, task)| (task.id, (i + 1) as u32))
            .collect();

        let mut new_tasks = Vec::new();
        for (i, old_task) in current_project.tasks.iter().enumerate() {
                    let mut new_task = old_task.clone();
                    new_task.id = (i + 1) as u32;
                    
                    new_task.parent_id = old_task.parent_id
                        .and_then(|old_parent_id| id_map.get(&old_parent_id).cloned());
            
                    new_task.dependencies = old_task.dependencies
                        .iter()
                        .filter_map(|old_dep_id| id_map.get(old_dep_id).cloned())
                        .collect();
                        
                    new_tasks.push(new_task);        }

        current_project.tasks = new_tasks;
        self.recalculate_schedule();
    }

    fn recalculate_schedule(&mut self) {
        let next_id = self.get_current_project().tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
        self.next_task_id = next_id;
        let current_project = self.get_current_project_mut();
        let task_map: HashMap<u32, Task> = current_project.tasks.iter().map(|t| (t.id, t.clone())).collect();
        let mut calculated_tasks: HashMap<u32, Task> = HashMap::new();
        let mut tasks_to_process: Vec<u32> = current_project.tasks.iter().map(|t| t.id).collect();
        
        let mut iterations = 0;
        while !tasks_to_process.is_empty() && iterations < 100 {
            tasks_to_process.retain(|task_id| {
                let task = task_map.get(task_id).unwrap();
                let deps_calculated = task.dependencies.iter().all(|dep_id| calculated_tasks.contains_key(dep_id) || !task_map.contains_key(dep_id));

                if deps_calculated {
                    let mut updated_task = task.clone();
                    if !task.dependencies.is_empty() {
                        let max_dep_end_date = task.dependencies.iter()
                            .filter_map(|dep_id| calculated_tasks.get(dep_id))
                            .filter_map(|dep| dep.end_date)
                            .max();
                        updated_task.start_date = Some(max_dep_end_date.map_or(current_project.project_start_date, |d| d + Duration::days(1)));
                    } else if let Some(manual_date) = task.manual_start_date {
                        updated_task.start_date = Some(manual_date);
                    } else {
                        updated_task.start_date = Some(current_project.project_start_date);
                    }
                    updated_task.end_date = updated_task.start_date.map(|d| d + Duration::days(updated_task.duration.max(1) - 1));
                    calculated_tasks.insert(*task_id, updated_task);
                    false
                } else { true }
            });
            iterations += 1;
        }

        for task in &mut current_project.tasks {
            if let Some(calculated) = calculated_tasks.get(&task.id) {
                task.start_date = calculated.start_date;
                task.end_date = calculated.end_date;
            } else {
                task.start_date = None;
                task.end_date = None;
            }
        }

        // --- Auto-adjust parent tasks ---
        let mut changed = true;
        while changed {
            changed = false;
            let tasks_clone = current_project.tasks.clone();
            let parent_ids: Vec<_> = tasks_clone.iter().filter_map(|t| t.parent_id).collect();

            for parent_task in &mut current_project.tasks {
                if parent_ids.contains(&parent_task.id) { // Only process tasks that are actual parents
                    let children: Vec<_> = tasks_clone.iter()
                        .filter(|t| t.parent_id == Some(parent_task.id))
                        .collect();
                    
                    let min_start = children.iter().filter_map(|t| t.start_date).min();
                    let max_end = children.iter().filter_map(|t| t.end_date).max();

                    let old_start = parent_task.start_date;
                    let old_end = parent_task.end_date;

                    if let Some(start) = min_start {
                        parent_task.start_date = Some(start);
                    }
                    if let Some(end) = max_end {
                        parent_task.end_date = Some(end);
                        if let Some(start) = parent_task.start_date {
                            parent_task.duration = (end - start).num_days() + 1;
                        }
                    }

                    if old_start != parent_task.start_date || old_end != parent_task.end_date {
                        changed = true;
                    }
                }
            }
        }
    }

    fn save_all_projects(&mut self) -> io::Result<()> {
        self.all_projects.active_project_index = self.current_project_index;
        let json_data = serde_json::to_string_pretty(&self.all_projects)?;
        fs::write(&self.current_file_path, json_data)?;
        self.status_message = format!("All projects saved successfully to {}!", self.current_file_path);
        Ok(())
    }

    fn load_all_projects(&mut self) -> io::Result<()> {
        let path = Path::new(&self.current_file_path);
        if path.exists() {
            let json_data = fs::read_to_string(path)?;
            let all_projects: AllProjectsData = serde_json::from_str(&json_data)?;
            self.all_projects = all_projects;
            self.current_project_index = self.all_projects.active_project_index;
            self.history.clear();
            self.redo_history.clear();
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::NotFound, "File not found"))
        }
    }

    fn save_state_for_undo(&mut self) {
        self.history.push(ProjectState {
            project_data: self.get_current_project().clone(),
        });
        self.redo_history.clear();
    }

    fn undo(&mut self) {
        if let Some(previous_state) = self.history.pop() {
            self.redo_history.push(ProjectState {
                project_data: self.get_current_project().clone(),
            });
            *self.get_current_project_mut() = previous_state.project_data;
            self.recalculate_schedule();
            self.status_message = "Undo successful.".to_string();
        } else {
            self.status_message = "Nothing to undo.".to_string();
        }
    }

    fn redo(&mut self) {
        if let Some(next_state) = self.redo_history.pop() {
            self.history.push(ProjectState {
                project_data: self.get_current_project().clone(),
            });
            *self.get_current_project_mut() = next_state.project_data;
            self.recalculate_schedule();
            self.status_message = "Redo successful.".to_string();
        } else {
            self.status_message = "Nothing to redo.".to_string();
        }
    }

    fn toggle_todo_list(&mut self) {
        self.todo_list_open = !self.todo_list_open;
        if self.todo_list_open {
            self.focus_area = FocusArea::TodoList;
            if self.todo_list_state.selected().is_none() && !self.all_projects.todo_list.is_empty() {
                self.todo_list_state.select(Some(0));
            }
        } else {
            self.focus_area = FocusArea::Tasks;
        }
    }

    fn add_selected_task_to_todo(&mut self) {
        if let Some(idx) = self.table_state.selected() {
            let task_name = self.get_current_project().tasks[idx].name.clone();
            if self.all_projects.todo_list.contains(&task_name) {
                self.status_message = format!("Task '{}' is already in the todo list.", task_name);
            } else {
                self.all_projects.todo_list.push(task_name.clone());
                self.status_message = format!("Task '{}' added to todo list.", task_name);
            }
        }
    }

    fn remove_selected_todo_item(&mut self) {
        if let Some(idx) = self.todo_list_state.selected() {
            if idx < self.all_projects.todo_list.len() {
                let removed = self.all_projects.todo_list.remove(idx);
                if self.all_projects.todo_list.is_empty() {
                    self.todo_list_state.select(None);
                } else if idx >= self.all_projects.todo_list.len() {
                    self.todo_list_state.select(Some(self.all_projects.todo_list.len() - 1));
                }
                self.status_message = format!("Item '{}' removed from todo list.", removed);
            }
        }
    }

    fn move_todo_item_up(&mut self) {
        if let Some(idx) = self.todo_list_state.selected() {
            if idx > 0 {
                self.all_projects.todo_list.swap(idx, idx - 1);
                self.todo_list_state.select(Some(idx - 1));
            }
        }
    }

    fn move_todo_item_down(&mut self) {
        if let Some(idx) = self.todo_list_state.selected() {
            if idx < self.all_projects.todo_list.len() - 1 {
                self.all_projects.todo_list.swap(idx, idx + 1);
                self.todo_list_state.select(Some(idx + 1));
            }
        }
    }

    fn next_project(&mut self) {
        if self.all_projects.projects.len() > 1 {
            self.save_all_projects().unwrap_or_else(|_| self.status_message = "Failed to save current project before switching.".into());
            self.current_project_index = (self.current_project_index + 1) % self.all_projects.projects.len();
            self.status_message = format!("Switched to project: {}", self.get_current_project().project_name);
            self.recalculate_schedule();
            self.table_state.select(Some(0));
        } else {
            self.status_message = "No other projects to switch to.".to_string();
        }
    }

    fn previous_project(&mut self) {
        if self.all_projects.projects.len() > 1 {
            self.save_all_projects().unwrap_or_else(|_| self.status_message = "Failed to save current project before switching.".into());
            self.current_project_index = (self.current_project_index + self.all_projects.projects.len() - 1) % self.all_projects.projects.len();
            self.status_message = format!("Switched to project: {}", self.get_current_project().project_name);
            self.recalculate_schedule();
            self.table_state.select(Some(0));
        } else {
            self.status_message = "No other projects to switch to.".to_string();
        }
    }

    fn get_task_level(&self, task: &Task) -> u32 {
        let mut level = 0;
        let mut current_parent_id = task.parent_id;
        let tasks = &self.get_current_project().tasks;

        while let Some(parent_id) = current_parent_id {
            level += 1;
            if let Some(parent_task) = tasks.iter().find(|t| t.id == parent_id) {
                current_parent_id = parent_task.parent_id;
            } else {
                break; // Parent task not found, break the loop
            }
        }
        level
    }

    fn indent_task(&mut self) {
        if let Some(selected_index) = self.table_state.selected() {
            if selected_index > 0 {
                let tasks = &self.get_current_project().tasks;
                let task_id = tasks[selected_index].id;
                let new_parent_id = tasks[selected_index - 1].id;

                // Prevent making a task its own parent
                if task_id == new_parent_id {
                    return;
                }

                // Prevent creating circular dependencies (simplified check)
                let mut current_parent_id = Some(new_parent_id);
                while let Some(parent_id) = current_parent_id {
                    if parent_id == task_id {
                        return; // Circular dependency detected
                    }
                    let parent_task = tasks.iter().find(|t| t.id == parent_id);
                    if let Some(parent) = parent_task {
                        current_parent_id = parent.parent_id;
                    } else {
                        break;
                    }
                }
                
                self.save_state_for_undo();
                let tasks_mut = &mut self.get_current_project_mut().tasks;
                tasks_mut[selected_index].parent_id = Some(new_parent_id);
                self.recalculate_schedule();
            }
        }
    }

    fn unindent_task(&mut self) {
        if let Some(selected_index) = self.table_state.selected() {
            // Get immutable data first
            let current_project = self.get_current_project();
            if selected_index >= current_project.tasks.len() {
                return; // Ensure selected_index is valid
            }
            let selected_task = &current_project.tasks[selected_index];

            if let Some(parent_id) = selected_task.parent_id {
                let parent_task_parent_id = current_project.tasks
                    .iter()
                    .find(|t| t.id == parent_id)
                    .and_then(|t| t.parent_id);
                
                // Now perform mutable operations
                self.save_state_for_undo();
                let tasks_mut = &mut self.get_current_project_mut().tasks;
                tasks_mut[selected_index].parent_id = parent_task_parent_id;
                self.recalculate_schedule();
            }
        }
    }

    fn add_new_top_level_task(&mut self) {
        self.save_state_for_undo();
        let new_task = Task {
            id: 0, // Will be remapped
            name: "New Task".into(),
            assigned_to: "Unassigned".into(),
            duration: 1,
            progress: 0,
            dependencies: vec![],
            manual_start_date: None,
            details: None,
            parent_id: None, // Top-level
            start_date: None,
            end_date: None,
        };

        let current_project = self.get_current_project_mut();
        current_project.tasks.push(new_task); // Add to the end of the list
        let new_task_index = current_project.tasks.len() - 1;

        self.remap_ids_and_dependencies();
        self.table_state.select(Some(new_task_index));
        self.focus_area = FocusArea::Tasks;
        self.selected_task_field = TaskField::Name;
        self.input_mode = InputMode::Editing;
        self.status_message = "Added new top-level task.".to_string();
        // load_buffer_for_editing(self); // Handled by handle_normal_mode context
    }

    fn add_new_sibling_task(&mut self) {
        self.save_state_for_undo();
        
        let (parent_id_for_new_task, insert_index, parent_start_date) = if let Some(selected_index) = self.table_state.selected() {
            let selected_task = &self.get_current_project().tasks[selected_index];
            let parent_start = if let Some(p_id) = selected_task.parent_id {
                self.get_current_project().tasks.iter().find(|t| t.id == p_id).and_then(|t| t.start_date)
            } else {
                None // No parent, so no default start date from parent
            };
            (selected_task.parent_id, selected_index + 1, parent_start)
        } else {
            // If nothing is selected, add a top-level task at the end
            (None, self.get_current_project().tasks.len(), None)
        };

        let new_task = Task {
            id: 0, // Will be remapped
            name: "New Task".into(),
            assigned_to: "Unassigned".into(),
            duration: 1,
            progress: 0,
            dependencies: vec![],
            manual_start_date: parent_start_date,
            details: None,
            parent_id: parent_id_for_new_task,
            start_date: None,
            end_date: None,
        };

        let current_project = self.get_current_project_mut();
        current_project.tasks.insert(insert_index, new_task);
        
        self.remap_ids_and_dependencies();
        self.table_state.select(Some(insert_index));
        self.focus_area = FocusArea::Tasks;
        self.selected_task_field = TaskField::Name;
        self.input_mode = InputMode::Editing;
        self.status_message = "Added new sibling task.".to_string();
    }

    fn generate_task_display_ids(&self) -> HashMap<u32, String> {
        let tasks = &self.get_current_project().tasks;
        let mut task_display_ids: HashMap<u32, String> = HashMap::new();

        for (i, task) in tasks.iter().enumerate() {
            if let Some(parent_id) = task.parent_id {
                if let Some(parent_display_id) = task_display_ids.get(&parent_id) {
                    // Find how many siblings with a smaller index this task has.
                    let siblings_before = tasks.iter().take(i)
                        .filter(|t| t.parent_id == Some(parent_id))
                        .count();
                    
                    let letter = (('a' as u8) + siblings_before as u8) as char;
                    let display_id = format!("{}.{}", parent_display_id, letter);
                    task_display_ids.insert(task.id, display_id);
                } else {
                    // Parent appears after child in the list or is an orphan
                    task_display_ids.insert(task.id, "?".to_string());
                }
            } else {
                // Top-level task.
                let top_level_before = tasks.iter().take(i)
                    .filter(|t| t.parent_id.is_none())
                    .count();
                task_display_ids.insert(task.id, (top_level_before + 1).to_string());
            }
        }
        task_display_ids
    }

    fn get_contiguous_family_range(&self, start_index: usize) -> std::ops::Range<usize> {
        let tasks = &self.get_current_project().tasks;
        if start_index >= tasks.len() {
            return start_index..start_index;
        }

        let start_level = self.get_task_level(&tasks[start_index]);
        let mut end_index = start_index + 1;

        while let Some(next_task) = tasks.get(end_index) {
            let next_level = self.get_task_level(next_task);
            if next_level > start_level {
                end_index += 1;
            } else {
                break;
            }
        }
        start_index..end_index
    }
}

// --- MAIN ---
fn main() -> io::Result<()> {
    setup_terminal()?;
    let mut app = App::new();
    run_app(&mut app)?;
    restore_terminal()?;
    Ok(())
}

fn run_app(app: &mut App) -> io::Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    while !app.should_quit {
        terminal.draw(|f| ui(f, app))?;
        handle_events(app)?;
    }
    Ok(())
}

// --- EVENT HANDLING ---
fn handle_events(app: &mut App) -> io::Result<()> {
    if event::poll(std::time::Duration::from_millis(50))? {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match app.input_mode {
                    InputMode::Normal => handle_normal_mode(app, key),
                    InputMode::Editing => handle_editing_mode(app, key),
                }
            }
        }
    }
    Ok(())
}

fn handle_normal_mode(app: &mut App, key: KeyEvent) {
    if key.modifiers == KeyModifiers::CONTROL {
        match key.code {
            KeyCode::Char('s') => { app.save_all_projects().unwrap_or_else(|_| app.status_message = "Failed to save projects.".into()); },
            KeyCode::Char('r') => app.redo(),
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('g') => go_to_top(app),
        KeyCode::Char('G') => go_to_bottom(app),
        KeyCode::Char('K') => {
            if app.focus_area == FocusArea::TodoList {
                app.move_todo_item_up();
            } else {
                app.move_task_up();
            }
        }
        KeyCode::Char('J') => {
            if app.focus_area == FocusArea::TodoList {
                app.move_todo_item_down();
            } else {
                app.move_task_down();
            }
        }
        KeyCode::Char('j') | KeyCode::Down => navigate_down(app),
        KeyCode::Char('k') | KeyCode::Up => navigate_up(app),
        KeyCode::Char('h') | KeyCode::Left => select_previous_field(app),
        KeyCode::Char('l') | KeyCode::Right => select_next_field(app),
        KeyCode::Char('a') => {
            app.add_new_sibling_task();
            load_buffer_for_editing(app);
        },
        KeyCode::Tab => app.indent_task(),
        KeyCode::BackTab => app.unindent_task(),
        KeyCode::Char('s') => {
            if let Some(selected_index) = app.table_state.selected() {
                let parent_task = &app.get_current_project().tasks[selected_index];
                let parent_id = parent_task.id;
                let parent_start_date = parent_task.start_date; // Get parent's start date

                let new_task_index = app.add_task(Task { 
                    id: 0, 
                    name: "New Sub-task".into(), 
                    assigned_to: "Unassigned".into(), 
                    duration: 1, 
                    progress: 0, 
                    dependencies: vec![], 
                    manual_start_date: parent_start_date, 
                    details: None, 
                    parent_id: Some(parent_id), 
                    start_date: None, 
                    end_date: None 
                });
                app.table_state.select(Some(new_task_index));
                app.focus_area = FocusArea::Tasks;
                app.selected_task_field = TaskField::Name;
                app.input_mode = InputMode::Editing;
                load_buffer_for_editing(app);
            }
        },
        KeyCode::Char('A') => {
            app.add_new_top_level_task();
            load_buffer_for_editing(app);
        }
        KeyCode::Char('D') => app.delete_selected_task(),
        KeyCode::Char('u') => app.undo(),
        KeyCode::Char('t') => {
            let today_date = app.today; // Capture app.today before mutable borrow
            let current_project = app.get_current_project_mut();
            if today_date < current_project.project_start_date {
                current_project.week_to_show = 0;
            } else {
                let days_from_start = (today_date - current_project.project_start_date).num_days();
                current_project.week_to_show = (days_from_start / 7) as u32;
            }
            app.status_message = format!("Jumped to the week of today's date.");
        }
        KeyCode::Char('T') => app.toggle_todo_list(),
        KeyCode::Char('+') => app.add_selected_task_to_todo(),
        KeyCode::Char('-') => {
            if app.focus_area == FocusArea::TodoList {
                app.remove_selected_todo_item();
            }
        }
        KeyCode::Char('N') => app.next_project(),
        KeyCode::Char('P') => app.previous_project(),
        KeyCode::Char('C') => app.add_new_project(),
        KeyCode::Char('M') => {
            if let Some(selected_index) = app.table_state.selected() {
                app.details_view_open = !app.details_view_open;
                if app.details_view_open {
                    let task = &app.get_current_project().tasks[selected_index];
                    app.details_buffer = task.details.clone().unwrap_or_default();
                    app.input_mode = InputMode::Editing;
                } else {
                    let buffer = app.details_buffer.clone();
                    let task = &mut app.get_current_project_mut().tasks[selected_index];
                    task.details = if buffer.is_empty() { None } else { Some(buffer) };
                    app.input_mode = InputMode::Normal;
                }
            }
        },
        KeyCode::Char('O') => {
            app.highlight_mode = match app.highlight_mode {
                HighlightMode::Today => HighlightMode::Urgent,
                HighlightMode::Urgent => HighlightMode::Today,
            };
        },
        KeyCode::Enter => {
            match app.focus_area {
                FocusArea::Project(_) => {
                    app.input_mode = InputMode::Editing;
                    load_buffer_for_editing(app);
                }
                FocusArea::Tasks => {
                    if let Some(selected_index) = app.table_state.selected() {
                        let current_project = app.get_current_project();
                        let is_editable = match app.selected_task_field {
                            TaskField::StartDate => current_project.tasks[selected_index].dependencies.is_empty(),
                            _ => true,
                        };
                        if is_editable {
                            app.input_mode = InputMode::Editing;
                            load_buffer_for_editing(app);
                        } else {
                            app.status_message = "Cannot edit Start Date when Dependencies are set.".to_string();
                        }
                    }
                }
                FocusArea::TodoList => {}
            }
        }
        _ => {}
    }
}

fn handle_editing_mode(app: &mut App, key: KeyEvent) {
    if app.details_view_open {
        match key.code {
            KeyCode::Enter => {
                if let Some(selected_index) = app.table_state.selected() {
                    let buffer = app.details_buffer.clone();
                    let task = &mut app.get_current_project_mut().tasks[selected_index];
                    task.details = if buffer.is_empty() { None } else { Some(buffer) };
                }
                app.details_view_open = false;
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Esc => {
                app.details_view_open = false;
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT => {
                app.details_buffer.push(c);
            }
            KeyCode::Backspace => {
                app.details_buffer.pop();
            }
            KeyCode::Char('w') if key.modifiers == KeyModifiers::CONTROL => {
                let buffer = &mut app.details_buffer;
                let last_word_start = buffer.trim_end().rfind(' ').map_or(0, |i| i + 1);
                buffer.truncate(last_word_start);
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('w') if key.modifiers == KeyModifiers::CONTROL => {
            let buffer = &mut app.input_buffer;
            let last_word_start = buffer.trim_end().rfind(' ').map_or(0, |i| i + 1);
            buffer.truncate(last_word_start);
        }
        KeyCode::Enter => {
            app.save_state_for_undo();
            save_buffer_to_task(app);
            app.input_mode = InputMode::Normal;
            app.input_buffer.clear();
            app.recalculate_schedule();
        }
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.input_buffer.clear();
        }
        KeyCode::Char(c) if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT => {
            app.input_buffer.push(c);
        }
        KeyCode::Backspace => { app.input_buffer.pop(); }
        _ => {}
    }
}

// --- STATE HELPERS ---
fn navigate_up(app: &mut App) {
    match app.focus_area {
        FocusArea::Project(ProjectField::WeekToShow) => app.focus_area = FocusArea::Project(ProjectField::EndDate),
        FocusArea::Project(ProjectField::EndDate) => app.focus_area = FocusArea::Project(ProjectField::StartDate),
        FocusArea::Project(ProjectField::StartDate) => app.focus_area = FocusArea::Project(ProjectField::Name),
        FocusArea::Project(ProjectField::Name) => {}
        FocusArea::Tasks => {
            if let Some(selected) = app.table_state.selected() {
                if selected == 0 {
                    app.table_state.select(None);
                    app.focus_area = FocusArea::Project(ProjectField::WeekToShow);
                } else {
                    app.table_state.select(Some(selected - 1));
                }
            }
        }
        FocusArea::TodoList => {
            if let Some(selected) = app.todo_list_state.selected() {
                if selected > 0 {
                    app.todo_list_state.select(Some(selected - 1));
                }
            }
        }
    }
}

fn navigate_down(app: &mut App) {
    match app.focus_area {
        FocusArea::Project(ProjectField::Name) => app.focus_area = FocusArea::Project(ProjectField::StartDate),
        FocusArea::Project(ProjectField::StartDate) => app.focus_area = FocusArea::Project(ProjectField::EndDate),
        FocusArea::Project(ProjectField::EndDate) => app.focus_area = FocusArea::Project(ProjectField::WeekToShow),
        FocusArea::Project(ProjectField::WeekToShow) => {
            if !app.get_current_project().tasks.is_empty() {
                app.focus_area = FocusArea::Tasks;
                app.table_state.select(Some(0));
            }
        }
        FocusArea::Tasks => {
            if let Some(selected) = app.table_state.selected() {
                if selected < app.get_current_project().tasks.len() - 1 {
                    app.table_state.select(Some(selected + 1));
                }
            }
        }
        FocusArea::TodoList => {
            if let Some(selected) = app.todo_list_state.selected() {
                if selected < app.all_projects.todo_list.len() - 1 {
                    app.todo_list_state.select(Some(selected + 1));
                }
            }
        }
    }
}

fn select_next_field(app: &mut App) {
    if let FocusArea::Tasks = app.focus_area {
        app.selected_task_field = match app.selected_task_field {
            TaskField::Name => TaskField::AssignedTo,
            TaskField::AssignedTo => TaskField::StartDate,
            TaskField::StartDate => TaskField::Duration,
            TaskField::Duration => TaskField::Progress,
            TaskField::Progress => TaskField::Dependencies,
            TaskField::Dependencies => TaskField::Name,
        };
    }
}

fn select_previous_field(app: &mut App) {
    if let FocusArea::Tasks = app.focus_area {
        app.selected_task_field = match app.selected_task_field {
            TaskField::Name => TaskField::Dependencies,
            TaskField::AssignedTo => TaskField::Name,
            TaskField::StartDate => TaskField::AssignedTo,
            TaskField::Duration => TaskField::StartDate,
            TaskField::Progress => TaskField::Duration,
            TaskField::Dependencies => TaskField::Progress,
        };
    }
}

fn go_to_top(app: &mut App) {
    if !app.get_current_project().tasks.is_empty() {
        app.table_state.select(Some(0));
        app.focus_area = FocusArea::Tasks;
    }
}

fn go_to_bottom(app: &mut App) {
    if !app.get_current_project().tasks.is_empty() {
        let last_index = app.get_current_project().tasks.len() - 1;
        app.table_state.select(Some(last_index));
        app.focus_area = FocusArea::Tasks;
    }
}

fn load_buffer_for_editing(app: &mut App) {
    let current_project = app.get_current_project();
    match app.focus_area {
        FocusArea::Project(ProjectField::Name) => app.input_buffer = current_project.project_name.clone(),
        FocusArea::Project(ProjectField::StartDate) => app.input_buffer = current_project.project_start_date.format("%m/%d/%Y").to_string(),
        FocusArea::Project(ProjectField::EndDate) => app.input_buffer = current_project.project_end_date.map_or_else(|| "".to_string(), |d| d.format("%m/%d/%Y").to_string()),
        FocusArea::Project(ProjectField::WeekToShow) => app.input_buffer = current_project.week_to_show.to_string(),
        FocusArea::Tasks => {
            if let Some(index) = app.table_state.selected() {
                let task = &current_project.tasks[index];
                app.input_buffer = match app.selected_task_field {
                    TaskField::Name => task.name.clone(),
                    TaskField::AssignedTo => task.assigned_to.clone(),
                    TaskField::Duration => task.duration.to_string(),
                    TaskField::Progress => task.progress.to_string(),
                    TaskField::Dependencies => {
                        let display_ids = app.generate_task_display_ids();
                        task.dependencies.iter()
                            .map(|dep_id| display_ids.get(dep_id).cloned().unwrap_or_else(|| "?".to_string()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    },
                    TaskField::StartDate => task.manual_start_date.map_or("".to_string(), |d| d.format("%m/%d/%Y").to_string()),
                };
            }
        }
        FocusArea::TodoList => {}
    }
}

fn save_buffer_to_task(app: &mut App) {
    let focus_area = app.focus_area;
    let selected_task_field = app.selected_task_field;
    let input_buffer_owned = app.input_buffer.clone(); // Clone input_buffer
    let selected_table_index = app.table_state.selected(); // Get selected_index before mutable borrow

    match focus_area {
        FocusArea::Project(field) => {
            let current_project = app.get_current_project_mut();
            match field {
                ProjectField::Name => current_project.project_name = input_buffer_owned.clone(),
                ProjectField::StartDate => {
                    if input_buffer_owned.to_lowercase() == "today" {
                        current_project.project_start_date = Local::now().date_naive();
                    }
                    else if let Ok(date) = NaiveDate::parse_from_str(&input_buffer_owned, "%m/%d/%Y") {
                        current_project.project_start_date = date;
                    } else {
                        app.status_message = "Invalid date format. Please use mm/dd/yyyy or 'today'.".to_string();
                    }
                }
                ProjectField::EndDate => {
                    if input_buffer_owned.is_empty() {
                        current_project.project_end_date = None;
                    } else if input_buffer_owned.to_lowercase() == "today" {
                        current_project.project_end_date = Some(Local::now().date_naive());
                    } else if let Ok(date) = NaiveDate::parse_from_str(&input_buffer_owned, "%m/%d/%Y") {
                        current_project.project_end_date = Some(date);
                    } else {
                        app.status_message = "Invalid date format. Please use mm/dd/yyyy or 'today'.".to_string();
                    }
                }
                ProjectField::WeekToShow => {
                    if let Ok(week) = input_buffer_owned.parse() {
                        current_project.week_to_show = week;
                    } else {
                        app.status_message = "Invalid number for week.".to_string();
                    }
                }
            }
        }
        FocusArea::Tasks => {
            if let Some(index) = selected_table_index {
                if selected_task_field == TaskField::Dependencies {
                    let display_ids = app.generate_task_display_ids();
                    let reverse_id_map: HashMap<String, u32> = display_ids.iter().map(|(id, display)| (display.clone(), *id)).collect();
                    let tasks_clone = app.get_current_project().tasks.clone(); // Use a clone for validation

                    let new_deps: Vec<u32> = input_buffer_owned.split(',')
                        .filter_map(|s| {
                            let trimmed = s.trim();
                            if let Some(id) = reverse_id_map.get(trimmed) {
                                return Some(*id);
                            }
                            if let Ok(id) = trimmed.parse::<u32>() {
                                if tasks_clone.iter().any(|t| t.id == id) {
                                    return Some(id);
                                }
                            }
                            None
                        })
                        .collect();
                    
                    let task = &mut app.get_current_project_mut().tasks[index];
                    task.dependencies = new_deps;
                    if !task.dependencies.is_empty() {
                        task.manual_start_date = None;
                    }
                } else {
                    let task = &mut app.get_current_project_mut().tasks[index];
                    match selected_task_field {
                        TaskField::Name => task.name = input_buffer_owned.clone(),
                        TaskField::AssignedTo => task.assigned_to = input_buffer_owned.clone(),
                        TaskField::Duration => {
                            let mut duration = task.duration;
                            let trimmed = input_buffer_owned.trim();
                            if trimmed.ends_with('w') {
                                if let Ok(val) = trimmed[..trimmed.len()-1].parse::<i64>() {
                                    duration = val * 7;
                                }
                            } else if trimmed.ends_with('m') {
                                if let Ok(val) = trimmed[..trimmed.len()-1].parse::<i64>() {
                                    duration = val * 30;
                                }
                            } else if trimmed.ends_with('y') {
                                if let Ok(val) = trimmed[..trimmed.len()-1].parse::<i64>() {
                                    duration = val * 365;
                                }
                            } else if let Ok(val) = trimmed.parse::<i64>() {
                                duration = val;
                            }
                            task.duration = duration;
                        },
                        TaskField::Progress => task.progress = input_buffer_owned.parse().unwrap_or(task.progress).min(100),
                        TaskField::StartDate => {
                            if input_buffer_owned.is_empty() {
                                task.manual_start_date = None;
                            } else if input_buffer_owned.to_lowercase() == "today" {
                                task.manual_start_date = Some(Local::now().date_naive());
                                task.dependencies.clear();
                                app.status_message = "Dependencies cleared for task with manual start date.".to_string();
                            } else if let Ok(date) = NaiveDate::parse_from_str(&input_buffer_owned, "%m/%d/%Y") {
                                task.manual_start_date = Some(date);
                                task.dependencies.clear();
                                app.status_message = "Dependencies cleared for task with manual start date.".to_string();
                            } else {
                                app.status_message = "Invalid date format. Please use mm/dd/yyyy or 'today'.".to_string();
                            }
                        }
                        _ => {} // Dependencies case is handled above
                    }
                }
            }
        }
        FocusArea::TodoList => {}
    }
}

// --- UI RENDERING ---
fn calculate_column_widths(app: &App) -> [u16; 7] {
    const PADDING: u16 = 2;
    let current_project = app.get_current_project();
    let display_ids = app.generate_task_display_ids(); // Generate IDs here too

    let id_col_width = current_project.tasks.iter()
        .map(|t| {
            let id_str = display_ids.get(&t.id).cloned().unwrap_or_default();
            UnicodeWidthStr::width(id_str.as_str())
        })
        .max().unwrap_or(0).max(UnicodeWidthStr::width("ID")) as u16 + PADDING;

    let name_col_width = current_project.tasks.iter()
        .map(|t| UnicodeWidthStr::width(t.name.as_str()))
        .max().unwrap_or(0).max(UnicodeWidthStr::width("Name")) as u16 + 12 + PADDING;

    let assigned_col_width = current_project.tasks.iter()
        .map(|t| UnicodeWidthStr::width(t.assigned_to.as_str()))
        .max().unwrap_or(0).max(UnicodeWidthStr::width("Assigned")) as u16 + PADDING;

    let start_col_width = UnicodeWidthStr::width("mm/dd/yyyy") as u16 + PADDING;
    let dur_col_width = UnicodeWidthStr::width("Dur").max(4) as u16 + PADDING;
    let prog_col_width = UnicodeWidthStr::width("Prog%").max(4) as u16 + PADDING;
    
    let deps_col_width = current_project.tasks.iter()
        .map(|t| {
            if t.dependencies.is_empty() { 0 }
            else {
                t.dependencies.iter().map(|d| UnicodeWidthStr::width(d.to_string().as_str())).sum::<usize>() 
                + (t.dependencies.len() - 1) * 2
            }
        })
        .max().unwrap_or(0).max(UnicodeWidthStr::width("Deps")) as u16 + PADDING;

    [id_col_width, name_col_width, assigned_col_width, start_col_width, dur_col_width, prog_col_width, deps_col_width]
}

// --- UI RENDERING ---
fn ui(frame: &mut Frame, app: &mut App) {
    let main_layout = if app.details_view_open {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(5), // Details view height
                Constraint::Length(3), // Footer height
            ])
            .split(frame.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(frame.area())
    };

    let content_area = main_layout[0];
    let footer_area = if app.details_view_open { main_layout[2] } else { main_layout[1] };
    let details_area = if app.details_view_open { Some(main_layout[1]) } else { None };


    let total_width = content_area.width;
    let min_right_width = (total_width as f32 * 0.3) as u16;

    let column_widths = calculate_column_widths(app);
    let ideal_left_width: u16 = column_widths.iter().sum();

    let mut left_width = ideal_left_width;
    if total_width.saturating_sub(left_width) < min_right_width {
        left_width = total_width.saturating_sub(min_right_width);
    }

    let mut constraints = vec![Constraint::Length(left_width), Constraint::Min(0)];
    if app.todo_list_open {
        constraints.push(Constraint::Length(30));
    }

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(content_area);

    let table_area = main_chunks[0];
    render_task_table(frame, table_area, app, &column_widths);
    render_gantt_chart(frame, main_chunks[1], app);

    if app.todo_list_open {
        render_todo_list(frame, main_chunks[2], app);
    }

    if let Some(details_area) = details_area {
        render_details_view(frame, details_area, app);
    }

    render_footer(frame, footer_area, app);

    if let InputMode::Editing = app.input_mode {
        if app.details_view_open {
            if let Some(details_area) = details_area {
                 frame.set_cursor_position((
                    details_area.x + 1 + (app.details_buffer.len() as u16 % (details_area.width - 2)),
                    details_area.y + 1 + (app.details_buffer.len() as u16 / (details_area.width - 2)),
                ));
            }
        } else {
            match app.focus_area {
                FocusArea::Project(field) => {
                    let y_offset = match field {
                        ProjectField::Name => 1,
                        ProjectField::StartDate | ProjectField::EndDate => 2,
                        ProjectField::WeekToShow => 3,
                    };
                    let x_offset = match field {
                        ProjectField::Name => "Project: ".len(),
                        ProjectField::StartDate => "Start Date: ".len(),
                        ProjectField::EndDate => "Start Date: YYYY/MM/DD | End Date: ".len(),
                        ProjectField::WeekToShow => "Week to Show: ".len(),
                    };
                    frame.set_cursor_position(
                        (table_area.x + 1 + (x_offset + app.input_buffer.len()) as u16,
                        table_area.y + y_offset),
                    );
                }
                FocusArea::Tasks => {
                    if let Some(selected_row_index) = app.table_state.selected() {
                        let block = Block::default().borders(Borders::ALL);
                        let inner_area = block.inner(table_area);
                        let layout = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Length(1),
                                Constraint::Length(1),
                                Constraint::Length(1),
                                Constraint::Length(1),
                                Constraint::Min(0),
                            ])
                            .split(inner_area);
                        let tasks_area = layout[4];

                        let col_constraints: Vec<Constraint> = column_widths.iter().map(|w| Constraint::Length(*w)).collect();
                        let col_layout = Layout::default().direction(Direction::Horizontal).constraints(col_constraints).split(tasks_area);

                        let selected_col_index = app.selected_task_field as usize + 1;
                        let selected_col_rect = col_layout[selected_col_index];

                        let indent_len = if app.selected_task_field == TaskField::Name {
                            let task = &app.get_current_project().tasks[selected_row_index];
                            let level = app.get_task_level(task);
                            (level * 2) as u16 // 2 spaces per level
                        } else {
                            0
                        };

                        // The content being rendered in an active cell is `> ` + indent + buffer
                        let prefix_len = "> ".len() as u16;
                        
                        // Account for column spacing, which is 1 char per column
                        let spacing_offset = selected_col_index as u16;

                        let cursor_x = selected_col_rect.x
                                     + spacing_offset
                                     + prefix_len
                                     + indent_len
                                     + UnicodeWidthStr::width(app.input_buffer.as_str()) as u16;

                        let cursor_y = tasks_area.y + selected_row_index as u16;
                        frame.set_cursor_position((cursor_x, cursor_y));
                    }
                }
                FocusArea::TodoList => {}
            }
        }
    }
}

fn render_todo_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Todo List")
        .border_style(if app.focus_area == FocusArea::TodoList {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });
    
    let items: Vec<ListItem> = app.all_projects.todo_list.iter()
        .map(|item| ListItem::new(format!("• {}", item)))
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::Blue).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");

    frame.render_stateful_widget(list, area, &mut app.todo_list_state);
}

fn render_task_table(frame: &mut Frame, area: Rect, app: &App, column_widths: &[u16; 7]) {
    let current_project = app.get_current_project();
    let block = Block::default().borders(Borders::ALL).title(format!("Project Details & Tasks - {}", current_project.project_name));
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Project Name
            Constraint::Length(1), // Project Start Date
            Constraint::Length(1), // Week to Show
            Constraint::Length(1), // Header
            Constraint::Min(0),    // Tasks
        ])
        .split(inner_area);

    let name_style = if app.focus_area == FocusArea::Project(ProjectField::Name) { Style::default().bg(Color::Blue) } else { Style::default() };
    let start_date_style = if app.focus_area == FocusArea::Project(ProjectField::StartDate) { Style::default().bg(Color::Blue) } else { Style::default() };
    let end_date_style = if app.focus_area == FocusArea::Project(ProjectField::EndDate) { Style::default().bg(Color::Blue) } else { Style::default() };
    let week_style = if app.focus_area == FocusArea::Project(ProjectField::WeekToShow) { Style::default().bg(Color::Blue) } else { Style::default() };
    
    let name_text = if app.focus_area == FocusArea::Project(ProjectField::Name) && app.input_mode == InputMode::Editing { &app.input_buffer } else { &current_project.project_name };
    let start_date_text = if app.focus_area == FocusArea::Project(ProjectField::StartDate) && app.input_mode == InputMode::Editing { app.input_buffer.clone() } else { current_project.project_start_date.format("%m/%d/%Y").to_string() };
    
    let end_date_text = if app.focus_area == FocusArea::Project(ProjectField::EndDate) && app.input_mode == InputMode::Editing {
        app.input_buffer.clone()
    } else {
        current_project.project_end_date.map_or_else(|| "-".to_string(), |d| d.format("%m/%d/%Y").to_string())
    };

    let week_text = if app.focus_area == FocusArea::Project(ProjectField::WeekToShow) && app.input_mode == InputMode::Editing { app.input_buffer.clone() } else { current_project.week_to_show.to_string() };

    frame.render_widget(Paragraph::new(format!("Project: {} ({}/{})", name_text, app.current_project_index + 1, app.all_projects.projects.len())).style(name_style), layout[0]);
    frame.render_widget(Paragraph::new(Line::from(vec![
        Span::styled(format!("Start Date: {}", start_date_text), start_date_style),
        Span::raw(" | "),
        Span::styled(format!("End Date: {}", end_date_text), end_date_style),
    ])), layout[1]);
    frame.render_widget(Paragraph::new(format!("Week to Show: {}", week_text)).style(week_style), layout[2]);

    let header_area = layout[3];
    let tasks_area = layout[4];

    let constraints = [
        Constraint::Length(column_widths[0]),
        Constraint::Length(column_widths[1]),
        Constraint::Length(column_widths[2]),
        Constraint::Length(column_widths[3]),
        Constraint::Length(column_widths[4]),
        Constraint::Length(column_widths[5]),
        Constraint::Length(column_widths[6]),
    ];

    let header_cells = ["ID", "Name", "Assigned", "Start", "Dur", "Prog%", "Deps"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header_row = Row::new(header_cells).style(Style::default().bg(Color::LightBlue)).height(1);
    let header_table = Table::new(vec![header_row], constraints.clone());
    frame.render_widget(header_table, header_area);

    let display_ids = app.generate_task_display_ids();

    let rows = current_project.tasks.iter().enumerate().map(|(i, task)| {
        let is_selected_row = app.table_state.selected() == Some(i);
        let level = app.get_task_level(task);
        let indent = "  ".repeat(level as usize);

        let is_today_task = task.start_date.map_or(false, |start| {
            task.end_date.map_or(false, |end| app.today >= start && app.today <= end)
        });

        let is_urgent = if let (Some(start), Some(end)) = (task.start_date, task.end_date) {
            if app.today >= start && app.today <= end {
                let days_from_start = (app.today - start).num_days() + 1; // Add 1 to include the start day
                let total_duration = (end - start).num_days() + 1;
                if total_duration > 0 {
                    let expected_progress = (days_from_start as f32 / total_duration as f32 * 100.0) as u8;
                    task.progress < expected_progress
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        let is_overdue = if let Some(end) = task.end_date {
            app.today > end && task.progress < 100
        } else {
            false
        };

        let row_style = if is_overdue {
            Style::default().fg(Color::Red)
        } else { match app.highlight_mode {
            HighlightMode::Today => {
                if is_today_task {
                    Style::default().fg(Color::Rgb(173, 216, 230))
                } else {
                    Style::default()
                }
            }
            HighlightMode::Urgent => {
                if is_urgent {
                    Style::default().fg(Color::Rgb(255, 165, 0)) // Orange for urgent
                } else {
                    Style::default()
                }
            }
        }};

        let deps_str = task.dependencies.iter()
            .map(|dep_id| display_ids.get(dep_id).cloned().unwrap_or_else(|| "?".to_string()))
            .collect::<Vec<_>>()
            .join(", ");
        
        let display_id_str = display_ids.get(&task.id).cloned().unwrap_or_else(|| task.id.to_string());
        let id_cell = if task.details.is_some() {
            Cell::from(format!(" {}*", display_id_str))
        } else {
            Cell::from(format!(" {}", display_id_str))
        };

        let name_display = format!("{}{}", indent, task.name);

        let cells_data = vec![
            (TaskField::Name, name_display),
            (TaskField::AssignedTo, task.assigned_to.clone()),
            (TaskField::StartDate, task.start_date.map_or_else(|| "-".to_string(), |d| d.format("%m/%d/%Y").to_string())),
            (TaskField::Duration, task.duration.to_string()),
            (TaskField::Progress, task.progress.to_string()),
            (TaskField::Dependencies, deps_str),
        ];

        let mut other_cells: Vec<Cell> = cells_data.iter().map(|(field, data)| {
            let is_active_cell = is_selected_row && app.selected_task_field == *field;
            let style = if is_active_cell {
                match app.input_mode {
                    InputMode::Editing => Style::default().fg(Color::White).bg(Color::Magenta),
                    InputMode::Normal => Style::default().bg(Color::Blue),
                }
            } else { Style::default() };

            let content_text = if is_active_cell {
                let text = if app.input_mode == InputMode::Editing {
                    if *field == TaskField::Name {
                        format!("{}{}", indent, &app.input_buffer)
                    } else {
                        app.input_buffer.clone()
                    }
                } else {
                    data.clone()
                };
                format!("> {}", text)
            } else {
                format!(" {}", data)
            };
            
            Cell::from(content_text).style(style)
        }).collect();
        
        let mut all_cells = vec![id_cell];
        all_cells.append(&mut other_cells);

        Row::new(all_cells).style(row_style)
    });

    let table = Table::new(rows, constraints)
        .row_highlight_style(Style::default().bg(Color::Rgb(50, 50, 50)).add_modifier(Modifier::BOLD));

    frame.render_stateful_widget(table, tasks_area, &mut app.table_state.clone());
}

fn render_details_view(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().title("Task Details").borders(Borders::ALL);
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let text = app.details_buffer.clone();
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, inner_area);
}

fn render_gantt_chart(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default().title("Gantt Chart Timeline").borders(Borders::ALL);
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    let chart_layout = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(3), Constraint::Min(0)]).split(inner_area);
    let header_area = chart_layout[0];
    let content_area = chart_layout[1];
    
    app.gantt_area_width = content_area.width;
    let current_project = app.get_current_project();
    let min_date = current_project.project_start_date + Duration::weeks(current_project.week_to_show as i64);
    
    const DAY_WIDTH: u16 = 3;
    let date_range_days = (app.gantt_area_width / DAY_WIDTH) as i64;

    let mut month_spans = vec![];
    let mut day_spans = vec![];
    let mut weekday_spans = vec![];
    let mut last_month = 0;

    for day in 0..=date_range_days {
        let current_date = min_date + Duration::days(day);
        let is_today = current_date == app.today;
        let is_deadline_day = app.get_current_project().project_end_date == Some(current_date);

        let mut day_style = Style::default();
        if is_today {
            day_style = day_style.fg(Color::Black).bg(Color::Cyan);
        } else if is_deadline_day {
            day_style = day_style.bg(Color::DarkGray).fg(Color::Red);
        }

        let weekday_char = match current_date.weekday() {
            Weekday::Mon => "M",
            Weekday::Tue => "T",
            Weekday::Wed => "W",
            Weekday::Thu => "T",
            Weekday::Fri => "F",
            Weekday::Sat => "S",
            Weekday::Sun => "S",
        };

        day_spans.push(Span::styled(format!("{:>2} ", current_date.day()), day_style));
        weekday_spans.push(Span::styled(format!("{:>2} ", weekday_char), day_style));

        if current_date.month() != last_month {
            last_month = current_date.month();
            month_spans.push(Span::styled(format!("{:<3}", current_date.format("%b")), Style::default()));
        } else {
            month_spans.push(Span::raw(" ".repeat(DAY_WIDTH as usize)));
        }
    }
    
    let header_layout = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)]).split(header_area);
    frame.render_widget(Paragraph::new(Line::from(month_spans)).scroll((0, 0)), header_layout[0]);
    frame.render_widget(Paragraph::new(Line::from(day_spans)).scroll((0, 0)), header_layout[1]);
    frame.render_widget(Paragraph::new(Line::from(weekday_spans)).scroll((0, 0)), header_layout[2]);

    let parent_ids: HashSet<u32> = current_project.tasks.iter()
        .filter_map(|t| t.parent_id)
        .collect();

    let mut lines = vec![Line::from(""); 1]; // 1 for header alignment

    for (i, task) in current_project.tasks.iter().enumerate() {
        let is_parent = parent_ids.contains(&task.id);
        let row_style = if app.focus_area == FocusArea::Tasks && app.table_state.selected() == Some(i) { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) };
        let mut bar_spans = vec![];
        if let (Some(start), Some(end)) = (task.start_date, task.end_date) {
            let progress_duration = (task.duration as f32 * (task.progress as f32 / 100.0)).round() as i64;
            let progress_end = if progress_duration > 0 {
                start + Duration::days(progress_duration - 1)
            } else {
                start - Duration::days(1)
            };

            for day in 0..=date_range_days {
                let current_date = min_date + Duration::days(day);
                let is_today = current_date == app.today;
                let is_deadline_day = app.get_current_project().project_end_date == Some(current_date);
                let is_task_day = current_date >= start && current_date <= end;
                
                let content = if is_task_day {
                    if is_parent {
                        if current_date == start {
                            "[=="
                        } else if current_date == end {
                            "==]"
                        } else {
                            "==="
                        }
                    } else {
                        let is_progress_day = current_date <= progress_end;
                        if is_today {
                            if is_progress_day { "|░░" } else { "|██" } // Today marker + 2 progress filled/empty
                        } else {
                            if is_progress_day { "░░░" } else { "███" } // 3 progress filled/empty
                        }
                    }
                } else {
                    if is_today { "|  " } else { "   " } // Today marker + 2 spaces / 3 spaces
                };

                let mut style = if is_today { row_style.fg(Color::Cyan) } else { row_style };
                if is_deadline_day {
                    style = style.fg(Color::Red);
                }
                bar_spans.push(Span::styled(content, style));
            }
        }
        lines.push(Line::from(bar_spans).style(row_style));
    }

    frame.render_widget(Paragraph::new(lines), content_area);
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let help_text = match app.input_mode {
        InputMode::Normal => "Nav (j/k/h/l) | A/a/s (Add) | Tab/S-Tab (Indent) | D(el) | (t)oday | (u)ndo | (Ctrl-r)edo | (M)ore | (T)odo | +/- (Todo) | (Ctrl-s)ave | (C)/(N)/(P) Project | (q)uit",
        InputMode::Editing => "Editing... (Enter) save | (Esc) cancel | (Ctrl-w) del word",
    };
    
    let layout = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);
    frame.render_widget(Paragraph::new(app.status_message.clone()).alignment(Alignment::Left), layout[0]);
    frame.render_widget(Paragraph::new(help_text).alignment(Alignment::Right).wrap(Wrap { trim: true }), layout[1]);
}

// --- TERMINAL SETUP & RESTORATION ---
fn setup_terminal() -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));
    Ok(())
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
