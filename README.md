# todo

Minimal Rust CLI for personal global todos.

## Commands

```bash
todo add text
todo end ID
todo list
```

## Storage

- Active todos are stored in `~/.local/share/todo-cli/todos.txt`.
- Completed todos are stored in `~/.local/share/todo-cli/completed.txt`.
- Completed todos are also archived to `~/Documents/todo-cli/completed.txt` by default.
- Set `TODO_ARCHIVE_PATH` to change the archive file location.
- Reinstalling the binary does not remove your todo data.

## Run

```bash
cargo run -- add buy milk
cargo run -- list
cargo run -- end 1
```

## Configure Archive Path

```bash
export TODO_ARCHIVE_PATH=~/Documents/my-todos/done.txt
```

## Build

```bash
cargo build --release
```
