# todo

Minimal Rust CLI for local todos.

## Commands

```bash
todo add "text"
todo end ID
todo list
```

## Storage

- Active todos are stored in `todos.txt` in the project directory.
- Completed todos are stored in `completed.txt` in the project directory.
- Completed todos are also archived to `~/Documents/todo-cli/completed.txt`.

## Run

```bash
cargo run -- add "buy milk"
cargo run -- list
cargo run -- end 1
```

## Build

```bash
cargo build --release
```
