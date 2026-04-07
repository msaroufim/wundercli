# todo

Minimal Rust CLI for personal global todos, with a satisfying completion sound inspired by Wunderlist.

This is a terminal-first todo experience: quick capture, quick listing, and a rewarding little bell when you finish something.

## Commands

```bash
todo (a)dd text
todo (e)nd ID
todo (l)ist
todo --verbose (a)dd text
todo --verbose (e)nd ID
```

## Storage

- Active todos are stored in `~/.local/share/todo-cli/todos.txt`.
- Completed todos are stored in `~/.local/share/todo-cli/completed.txt`.
- Set `TODO_ARCHIVE_PATH` if you want `todo end` to also write a second archive copy somewhere else.
- By default, `todo add` and `todo end` stay quiet. Use `todo list` to inspect your todos.
- Use `--verbose` or `TODO_VERBOSE=1` if you want `add` and `end` to print confirmations and file paths.
- `todo end` plays a bundled `Achievement bell` sound by default.
- Set `TODO_END_SOUND=off` to disable it, or set `TODO_END_SOUND` to another audio file path.
- Reinstalling the binary does not remove your todo data.

## Configure Archive Path

```bash
export TODO_ARCHIVE_PATH=~/Documents/my-todos/done.txt
```

## Configure End Sound

```bash
export TODO_END_SOUND=off
export TODO_END_SOUND=/System/Library/Sounds/Glass.aiff
```

## Verbose Mode

```bash
todo --verbose add buy milk
todo --verbose end 1
export TODO_VERBOSE=1
```

## Build

```bash
cargo build --release
```

## Releases

- Tagging `v*` publishes macOS release binaries through GitHub Actions.
- The release workflow builds `todo` for Apple Silicon and Intel macOS and attaches `.tar.gz` archives to the GitHub release.
