use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use todo::{
    ACTIVE_FILE, ARCHIVE_ENV_VAR, COMPLETED_FILE, DEFAULT_END_SOUND_FILE, END_SOUND_ENV_VAR, Todo,
    VERBOSE_ENV_VAR, append_todo, archive_file, data_file, env_flag, expand_home, next_active_id,
    read_todos, write_todos,
};

mod sync;

const BUNDLED_END_SOUND: &[u8] = include_bytes!("../assets/achievement-bell.wav");

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let verbose =
        env::args().any(|arg| arg == "--verbose" || arg == "-v") || env_flag(VERBOSE_ENV_VAR);
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
        Some("sync") => sync::run_sync_command(args.get(2..).unwrap_or(&[]), verbose),
        Some("setup") => sync::run_setup_command(),
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
    println!("todo sync");
    println!("todo setup");
    println!("todo --verbose add text");
    println!("todo --verbose end ID");
    println!("Active path: ~/.local/share/todo-cli/todos.txt");
    println!("Completed path: ~/.local/share/todo-cli/completed.txt");
    println!("Archive path: optional via ${}", ARCHIVE_ENV_VAR);
    println!("Cloud sync: optional via `todo sync` after `todo setup` or env config");
    println!("Verbose mode: --verbose or ${}", VERBOSE_ENV_VAR);
    println!(
        "End sound: ${} (set to 'off' to disable, default bundled {})",
        END_SOUND_ENV_VAR, DEFAULT_END_SOUND_FILE
    );
    if sync::sync_configured() {
        sync::print_sync_help();
    }
}

fn play_end_sound() {
    let sound_path = match end_sound_path() {
        Some(path) if path.is_file() => path,
        _ => return,
    };

    let escaped_path = sound_path.to_string_lossy().replace('\'', r"'\''");
    let command = format!(
        "nohup afplay '{}' >/dev/null 2>&1 </dev/null &",
        escaped_path
    );

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
        fs::write(&path, BUNDLED_END_SOUND).map_err(|err| err.to_string())?;
    }

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::ensure_default_end_sound;

    #[test]
    fn bundled_sound_is_materialized() {
        assert!(ensure_default_end_sound().is_ok());
    }
}
