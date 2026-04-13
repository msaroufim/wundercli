use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

pub const ACTIVE_FILE: &str = "todos.txt";
pub const COMPLETED_FILE: &str = "completed.txt";
pub const ARCHIVE_ENV_VAR: &str = "TODO_ARCHIVE_PATH";
pub const END_SOUND_ENV_VAR: &str = "TODO_END_SOUND";
pub const VERBOSE_ENV_VAR: &str = "TODO_VERBOSE";
pub const DATA_DIR: &str = ".local/share/todo-cli";
pub const DEFAULT_END_SOUND_FILE: &str = "achievement-bell.wav";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Todo {
    pub id: u32,
    pub text: String,
}

pub fn data_file(name: &str) -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "Could not find HOME directory".to_string())?;
    let dir = Path::new(&home).join(DATA_DIR);
    fs::create_dir_all(&dir).map_err(io_to_string)?;
    Ok(dir.join(name))
}

pub fn archive_file() -> Result<Option<PathBuf>, String> {
    let Ok(path) = env::var(ARCHIVE_ENV_VAR) else {
        return Ok(None);
    };

    let path = path.trim();
    if path.is_empty() {
        return Err(format!("{} is set but empty", ARCHIVE_ENV_VAR));
    }

    let path = expand_home(path)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_to_string)?;
    }

    Ok(Some(path))
}

pub fn expand_home(path: &str) -> Result<PathBuf, String> {
    if path == "~" {
        let home = env::var("HOME").map_err(|_| "Could not find HOME directory".to_string())?;
        return Ok(PathBuf::from(home));
    }

    if let Some(rest) = path.strip_prefix("~/") {
        let home = env::var("HOME").map_err(|_| "Could not find HOME directory".to_string())?;
        return Ok(PathBuf::from(home).join(rest));
    }

    Ok(PathBuf::from(path))
}

pub fn env_flag(name: &str) -> bool {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            !trimmed.is_empty()
                && trimmed != "0"
                && !trimmed.eq_ignore_ascii_case("false")
                && !trimmed.eq_ignore_ascii_case("off")
        }
        Err(_) => false,
    }
}

pub fn next_active_id(active: &[Todo]) -> u32 {
    let mut used_ids: Vec<u32> = active.iter().map(|todo| todo.id).collect();
    used_ids.sort_unstable();

    let mut next_id = 1;
    for id in used_ids {
        if id < next_id {
            continue;
        }
        if id > next_id {
            break;
        }
        next_id += 1;
    }

    next_id
}

pub fn read_todos(path: &Path) -> Result<Vec<Todo>, String> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(io_to_string(err)),
    };

    let mut todos = Vec::new();

    for (line_number, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }

        let (id, text) = line.split_once('|').ok_or_else(|| {
            format!(
                "Invalid data in {} at line {}",
                path.display(),
                line_number + 1
            )
        })?;

        let id = id.parse::<u32>().map_err(|_| {
            format!(
                "Invalid ID in {} at line {}",
                path.display(),
                line_number + 1
            )
        })?;

        todos.push(Todo {
            id,
            text: text.to_string(),
        });
    }

    Ok(todos)
}

pub fn write_todos(path: &Path, todos: &[Todo]) -> Result<(), String> {
    let mut output = String::new();

    for todo in todos {
        output.push_str(&todo_line(todo));
        output.push('\n');
    }

    fs::write(path, output).map_err(io_to_string)
}

pub fn append_todo(path: &Path, todo: &Todo) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(io_to_string)?;

    file.write_all(todo_line(todo).as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .map_err(io_to_string)
}

pub fn read_all_todos() -> Result<(Vec<Todo>, Vec<Todo>), String> {
    let active_path = data_file(ACTIVE_FILE)?;
    let completed_path = data_file(COMPLETED_FILE)?;

    Ok((read_todos(&active_path)?, read_todos(&completed_path)?))
}

pub fn write_all_todos(active: &[Todo], completed: &[Todo]) -> Result<(), String> {
    let active_path = data_file(ACTIVE_FILE)?;
    let completed_path = data_file(COMPLETED_FILE)?;

    write_todos(&active_path, active)?;
    write_todos(&completed_path, completed)
}

pub fn latest_local_update() -> Result<u64, String> {
    let active_path = data_file(ACTIVE_FILE)?;
    let completed_path = data_file(COMPLETED_FILE)?;
    let active_mtime = file_modified_at(&active_path)?;
    let completed_mtime = file_modified_at(&completed_path)?;
    Ok(active_mtime.max(completed_mtime))
}

fn file_modified_at(path: &Path) -> Result<u64, String> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(io_to_string(err)),
    };

    metadata
        .modified()
        .map_err(io_to_string)?
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|err| err.to_string())
}

fn todo_line(todo: &Todo) -> String {
    format!("{}|{}", todo.id, todo.text.replace('\n', " "))
}

fn io_to_string(err: io::Error) -> String {
    err.to_string()
}

#[cfg(test)]
mod tests {
    use super::{Todo, next_active_id};

    #[test]
    fn next_active_id_starts_at_one() {
        assert_eq!(next_active_id(&[]), 1);
    }

    #[test]
    fn next_active_id_fills_smallest_gap() {
        let active = vec![
            Todo {
                id: 1,
                text: "first".to_string(),
            },
            Todo {
                id: 3,
                text: "third".to_string(),
            },
            Todo {
                id: 4,
                text: "fourth".to_string(),
            },
        ];

        assert_eq!(next_active_id(&active), 2);
    }

    #[test]
    fn next_active_id_ignores_order() {
        let active = vec![
            Todo {
                id: 5,
                text: "fifth".to_string(),
            },
            Todo {
                id: 2,
                text: "second".to_string(),
            },
            Todo {
                id: 1,
                text: "first".to_string(),
            },
        ];

        assert_eq!(next_active_id(&active), 3);
    }
}
