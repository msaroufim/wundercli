# todo

Minimal Rust CLI for personal global todos, with a satisfying completion sound inspired by Wunderlist.

This is a terminal-first todo experience: quick capture, quick listing, and a rewarding little bell when you finish something.

## Commands

```bash
todo (a)dd text
todo (e)nd ID
todo (l)ist
todo sync
todo setup
todo --verbose (a)dd text
todo --verbose (e)nd ID
```

## Architecture

- Active todos are stored in `~/.local/share/todo-cli/todos.txt`.
- Completed todos are stored in `~/.local/share/todo-cli/completed.txt`.
- `todo` is local-only and stays fast.
- `todo sync` owns all network traffic and gist reconciliation.
- Set `TODO_ARCHIVE_PATH` if you want `todo end` to also write a second archive copy somewhere else.
- By default, `todo add` and `todo end` stay quiet. Use `todo list` to inspect your todos.
- Use `--verbose` or `TODO_VERBOSE=1` if you want `add` and `end` to print confirmations and file paths.
- `todo end` plays a bundled `Achievement bell` sound by default.
- Set `TODO_END_SOUND=off` to disable it, or set `TODO_END_SOUND` to another audio file path.
- Reinstalling the binary does not remove your todo data.

## Configure Gist Sync

Authenticate GitHub CLI once:

```bash
gh auth login
```

Then run:

```bash
todo setup
```

`todo setup` will:

- create a private gist if you do not already have one configured
- store the gist id in `~/.local/share/todo-cli/sync-config.json`
- upload your current local todos into `wundercli.json`

Optional overrides:

```bash
export TODO_GIST_ID=your_gist_id
export TODO_GIST_TOKEN=github_pat_with_gist_scope
```

`GITHUB_TOKEN` also works if you already have it set.

Run one sync pass:

```bash
todo sync
```

The gist file should look like this:

```json
{
  "version": 1,
  "updated_at": 1710000000,
  "active": [
    { "id": 1, "text": "buy milk" }
  ],
  "completed": []
}
```

Manual edits to that gist become local state the next time `todo sync` runs.

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

- Tagging `v*` publishes a macOS Apple Silicon release binary through GitHub Actions.
- The release workflow attaches a `todo-macos-aarch64.tar.gz` archive to the GitHub release.
