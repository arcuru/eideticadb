# EideticaDB Todo App Demo

This is a simple CLI todo list application that demonstrates how to use the EideticaDB library. It provides basic functionality to manage a list of tasks.

## Features

- Add new tasks
- Mark tasks as complete
- List all tasks
- Data persistence using EideticaDB

## Usage

Build the application:

```bash
cargo build
```

### Commands

#### Add a task

```bash
cargo run -- add "Buy groceries"
```

#### Mark a task as complete

```bash
cargo run -- complete <task-id>
```

The task ID is displayed when listing tasks.

#### List all tasks

```bash
cargo run -- list
```

### Database

By default, the application saves its database to `todo_db.json` in the current directory. You can specify a different file with the `-d` or `--database-path` option:

```bash
cargo run -- -d my_todos.json list
```

## How it Works

This application demonstrates several core EideticaDB concepts:

1. Using the `BaseDB` to manage trees of data
2. Creating and accessing trees
3. Creating and managing entries with subtrees
4. Implementing the `CRDT` trait for custom data types
5. Using parent references to maintain the Merkle DAG structure
6. Serializing/deserializing data to/from entries

The todo items are stored in a tree called "todo" with a subtree called "todos". The `TodoList` struct implements the `CRDT` trait, allowing it to be merged deterministically in a distributed environment.
