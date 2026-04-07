use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

const ACTIVE_FILE: &str = "todos.txt";
const COMPLETED_FILE: &str = "completed.txt";
const ARCHIVE_ENV_VAR: &str = "TODO_ARCHIVE_PATH";
const END_SOUND_ENV_VAR: &str = "TODO_END_SOUND";
const VERBOSE_ENV_VAR: &str = "TODO_VERBOSE";
const DATA_DIR: &str = ".local/share/todo-cli";
const DEFAULT_END_SOUND_FILE: &str = "achievement-bell.wav";
const BUNDLED_END_SOUND: &[u8] = include_bytes!("../assets/achievement-bell.wav");

#[derive(Clone, Debug)]
struct Todo {
    id: u32,
    text: String,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let verbose = env::args().any(|arg| arg == "--verbose" || arg == "-v")
        || env_flag(VERBOSE_ENV_VAR);
    let args: Vec<String> = env::args()
        .filter(|arg| arg != "--verbose" && arg != "-v")
        .collect();

    match args.get(1).map(String::as_str) {
        Some("add") | Some("a") => {
            let text = args
                .get(2..)
                .map(|parts| parts.join(" "))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Usage: todo add|a text".to_string())?;
            add_todo(&text, verbose)
        }
        Some("end") | Some("e") => {
            let id = args
                .get(2)
                .ok_or_else(|| "Usage: todo end|e ID".to_string())?
                .parse::<u32>()
                .map_err(|_| "ID must be a positive number".to_string())?;
            end_todo(id, verbose)
        }
        Some("list") | Some("l") => list_todos(),
        _ => {
            print_help();
            Ok(())
        }
    }
}

fn add_todo(text: &str, verbose: bool) -> Result<(), String> {
    let active_path = data_file(ACTIVE_FILE)?;
    let mut active = read_todos(&active_path)?;

    let todo = Todo {
        id: next_active_id(&active),
        text: text.to_string(),
    };

    active.push(todo.clone());
    write_todos(&active_path, &active)?;

    if verbose {
        println!("Added [{0}] {1}", todo.id, todo.text);
    }
    Ok(())
}

fn next_active_id(active: &[Todo]) -> u32 {
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

fn end_todo(id: u32, verbose: bool) -> Result<(), String> {
    let active_path = data_file(ACTIVE_FILE)?;
    let completed_path = data_file(COMPLETED_FILE)?;
    let archive_path = archive_file()?;

    let mut active = read_todos(&active_path)?;
    let index = active
        .iter()
        .position(|todo| todo.id == id)
        .ok_or_else(|| format!("No active todo with ID {id}"))?;

    let done = active.remove(index);
    write_todos(&active_path, &active)?;

    append_todo(&completed_path, &done)?;
    if let Some(path) = archive_path.as_ref() {
        append_todo(path, &done)?;
    }

    play_end_sound();

    if verbose {
        println!("Completed [{0}] {1}", done.id, done.text);
        println!("Completed list: {}", completed_path.display());
        if let Some(path) = archive_path.as_ref() {
            println!("Archive copy: {}", path.display());
        }
    }
    Ok(())
}

fn list_todos() -> Result<(), String> {
    let active_path = data_file(ACTIVE_FILE)?;
    let active = read_todos(&active_path)?;

    if active.is_empty() {
        println!("No active todos.");
        return Ok(());
    }

    for todo in active {
        println!("[{}] {}", todo.id, todo.text);
    }

    Ok(())
}

fn print_help() {
    println!("todo add text");
    println!("todo a text");
    println!("todo end ID");
    println!("todo e ID");
    println!("todo list");
    println!("todo l");
    println!("todo --verbose add text");
    println!("todo --verbose end ID");
    println!("Active path: ~/.local/share/todo-cli/todos.txt");
    println!("Completed path: ~/.local/share/todo-cli/completed.txt");
    println!("Archive path: optional via ${}", ARCHIVE_ENV_VAR);
    println!(
        "Verbose mode: --verbose or ${}",
        VERBOSE_ENV_VAR
    );
    println!(
        "End sound: ${} (set to 'off' to disable, default bundled {})",
        END_SOUND_ENV_VAR, DEFAULT_END_SOUND_FILE
    );
}

fn data_file(name: &str) -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "Could not find HOME directory".to_string())?;
    let dir = Path::new(&home).join(DATA_DIR);
    fs::create_dir_all(&dir).map_err(io_to_string)?;
    Ok(dir.join(name))
}

fn archive_file() -> Result<Option<PathBuf>, String> {
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

fn expand_home(path: &str) -> Result<PathBuf, String> {
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

fn env_flag(name: &str) -> bool {
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

fn read_todos(path: &Path) -> Result<Vec<Todo>, String> {
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

fn write_todos(path: &Path, todos: &[Todo]) -> Result<(), String> {
    let mut output = String::new();

    for todo in todos {
        output.push_str(&todo_line(todo));
        output.push('\n');
    }

    fs::write(path, output).map_err(io_to_string)
}

fn append_todo(path: &Path, todo: &Todo) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(io_to_string)?;

    file.write_all(todo_line(todo).as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .map_err(io_to_string)
}

fn todo_line(todo: &Todo) -> String {
    format!("{}|{}", todo.id, todo.text.replace('\n', " "))
}

fn play_end_sound() {
    let sound_path = match end_sound_path() {
        Some(path) if path.is_file() => path,
        _ => return,
    };

    let escaped_path = sound_path.to_string_lossy().replace('\'', r"'\''");
    let command = format!("nohup afplay '{}' >/dev/null 2>&1 </dev/null &", escaped_path);

    if let Err(err) = Command::new("sh").arg("-c").arg(command).spawn() {
        eprintln!("Warning: could not play end sound: {err}");
        return;
    }

    thread::sleep(Duration::from_millis(150));
}

fn end_sound_path() -> Option<PathBuf> {
    match env::var(END_SOUND_ENV_VAR) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("off") {
                return None;
            }

            expand_home(trimmed).ok()
        }
        Err(_) => ensure_default_end_sound().ok(),
    }
}

fn ensure_default_end_sound() -> Result<PathBuf, String> {
    let path = data_file(DEFAULT_END_SOUND_FILE)?;

    if !path.is_file() {
        fs::write(&path, BUNDLED_END_SOUND).map_err(io_to_string)?;
    }

    Ok(path)
}

fn io_to_string(err: io::Error) -> String {
    err.to_string()
}

#[cfg(test)]
mod tests {
    use super::{next_active_id, Todo};

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
