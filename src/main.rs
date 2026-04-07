use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const ACTIVE_FILE: &str = "todos.txt";
const COMPLETED_FILE: &str = "completed.txt";
const ARCHIVE_ENV_VAR: &str = "TODO_ARCHIVE_PATH";
const DATA_DIR: &str = ".local/share/todo-cli";
const ARCHIVE_DIR: &str = "todo-cli";
const ARCHIVE_FILE: &str = "completed.txt";

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
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("add") | Some("a") => {
            let text = args
                .get(2..)
                .map(|parts| parts.join(" "))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Usage: todo add|a text".to_string())?;
            add_todo(&text)
        }
        Some("end") | Some("e") => {
            let id = args
                .get(2)
                .ok_or_else(|| "Usage: todo end|e ID".to_string())?
                .parse::<u32>()
                .map_err(|_| "ID must be a positive number".to_string())?;
            end_todo(id)
        }
        Some("list") | Some("l") => list_todos(),
        _ => {
            print_help();
            Ok(())
        }
    }
}

fn add_todo(text: &str) -> Result<(), String> {
    let active_path = data_file(ACTIVE_FILE)?;
    let completed_path = data_file(COMPLETED_FILE)?;
    let mut active = read_todos(&active_path)?;
    let completed = read_todos(&completed_path)?;

    let next_id = active
        .iter()
        .chain(completed.iter())
        .map(|todo| todo.id)
        .max()
        .unwrap_or(0)
        + 1;

    let todo = Todo {
        id: next_id,
        text: text.to_string(),
    };

    active.push(todo.clone());
    write_todos(&active_path, &active)?;

    println!("Added [{0}] {1}", todo.id, todo.text);
    Ok(())
}

fn end_todo(id: u32) -> Result<(), String> {
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
    append_todo(&archive_path, &done)?;

    println!("Completed [{0}] {1}", done.id, done.text);
    println!("Saved completed todos in {}", completed_path.display());
    println!("Archived to {}", archive_path.display());
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
    println!("Active path: ~/.local/share/todo-cli/todos.txt");
    println!("Completed path: ~/.local/share/todo-cli/completed.txt");
    println!(
        "Archive path: ${} or ~/Documents/{}/{}",
        ARCHIVE_ENV_VAR, ARCHIVE_DIR, ARCHIVE_FILE
    );
}

fn data_file(name: &str) -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "Could not find HOME directory".to_string())?;
    let dir = Path::new(&home).join(DATA_DIR);
    fs::create_dir_all(&dir).map_err(io_to_string)?;
    Ok(dir.join(name))
}

fn archive_file() -> Result<PathBuf, String> {
    if let Ok(path) = env::var(ARCHIVE_ENV_VAR) {
        let path = path.trim();
        if path.is_empty() {
            return Err(format!("{} is set but empty", ARCHIVE_ENV_VAR));
        }

        let path = expand_home(path)?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_to_string)?;
        }

        return Ok(path);
    }

    let home = env::var("HOME").map_err(|_| "Could not find HOME directory".to_string())?;
    let dir = Path::new(&home).join("Documents").join(ARCHIVE_DIR);
    fs::create_dir_all(&dir).map_err(io_to_string)?;
    Ok(dir.join(ARCHIVE_FILE))
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

fn io_to_string(err: io::Error) -> String {
    err.to_string()
}
