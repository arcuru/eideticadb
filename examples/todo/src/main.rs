mod model;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use eideticadb::backend::InMemoryBackend;
use eideticadb::basedb::{BaseDB, Tree};
use model::{Todo, TodoList};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the database file to use
    #[arg(short, long, default_value = "todo_db.json")]
    database_path: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a new task to the todo list
    Add {
        /// The title of the task
        #[arg(required = true)]
        title: String,
    },
    /// Mark a task as complete
    Complete {
        /// The ID of the task to mark as complete
        #[arg(required = true)]
        id: String,
    },
    /// List all tasks
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load or create the database
    let db = load_or_create_db(&cli.database_path)?;

    // Load or create the todo tree
    let todo_tree = load_or_create_todo_tree(&db)?;

    // Handle the command
    match &cli.command {
        Commands::Add { title } => {
            add_todo(&todo_tree, title.clone())?;
            println!("Task added: {}", title);
        }
        Commands::Complete { id } => {
            complete_todo(&todo_tree, id)?;
            println!("Task completed: {}", id);
        }
        Commands::List => {
            list_todos(&todo_tree)?;
        }
    }

    // Save the database
    save_db(&db, &cli.database_path)?;

    Ok(())
}

fn load_or_create_db(path: &PathBuf) -> Result<BaseDB> {
    if path.exists() {
        let backend = InMemoryBackend::load_from_file(path)?;
        Ok(BaseDB::new(Box::new(backend)))
    } else {
        let backend = InMemoryBackend::new();
        Ok(BaseDB::new(Box::new(backend)))
    }
}

fn save_db(db: &BaseDB, path: &PathBuf) -> Result<()> {
    let backend = db.backend();
    let backend_guard = backend.lock().unwrap();

    // Cast the backend to InMemoryBackend to access save_to_file
    let in_memory_backend = backend_guard
        .as_any()
        .downcast_ref::<InMemoryBackend>()
        .ok_or(anyhow!("Failed to downcast backend to InMemoryBackend"))?;

    in_memory_backend.save_to_file(path)?;
    Ok(())
}

fn load_or_create_todo_tree(db: &BaseDB) -> Result<Tree> {
    let tree_name = "todo";

    // Try to find the tree by name among all trees
    let all_trees = db.all_trees()?;
    for tree in all_trees {
        if let Ok(name) = tree.get_name() {
            if name == tree_name {
                return Ok(tree);
            }
        }
    }

    // If not found, create a new one
    let mut settings = eideticadb::data::KVOverWrite::new();
    settings.set("name".to_string(), tree_name.to_string());

    // Create the tree with initial empty todo list
    let tree = db.new_tree(settings)?;
    let todo_list = TodoList::new();
    create_initial_entry(&tree, todo_list)?;

    Ok(tree)
}

fn create_initial_entry(tree: &Tree, todo_list: TodoList) -> Result<()> {
    // Serialize the todo list to raw data
    let todo_data = serde_json::to_string(&todo_list)?;

    // Create a new entry with this data in the "todos" subtree
    let mut entry = eideticadb::entry::Entry::new_top_level(serde_json::to_string(
        &eideticadb::data::KVOverWrite::new(),
    )?);
    entry.add_subtree("todos".to_string(), todo_data)?;

    // Insert the entry into the tree
    tree.insert(entry)?;

    Ok(())
}

fn get_todo_list(tree: &Tree) -> Result<TodoList> {
    // Get the todo list from the "todos" subtree
    Ok(tree.get_subtree_data::<TodoList>("todos")?)
}

fn add_todo(tree: &Tree, title: String) -> Result<()> {
    // Get the current todo list
    let mut todo_list = get_todo_list(tree)?;

    // Create a new todo and add it
    let todo = Todo::new(title);
    todo_list.add_todo(todo);

    // Create a new entry with the updated todo list
    let mut entry = eideticadb::entry::Entry::new(
        tree.root_id().to_string(),
        serde_json::to_string(&eideticadb::data::KVOverWrite::new())?,
    );

    // Add the updated todo list to the entry
    entry.add_subtree("todos".to_string(), serde_json::to_string(&todo_list)?)?;

    // Set parents based on current tip entries
    let tip_entries = tree.get_tip_entries()?;
    let parent_ids = tip_entries.iter().map(|e| e.id()).collect();
    entry.set_parents(parent_ids);

    // Set subtree parents
    let subtree_tip_entries = tree.get_subtree_tip_entries("todos")?;
    let subtree_parent_ids = subtree_tip_entries.iter().map(|e| e.id()).collect();
    entry.set_subtree_parents("todos", subtree_parent_ids);

    // Insert the entry
    tree.insert(entry)?;

    Ok(())
}

fn complete_todo(tree: &Tree, id: &str) -> Result<()> {
    // Get the current todo list
    let mut todo_list = get_todo_list(tree)?;

    // Find the todo and mark it as complete
    let todo = todo_list
        .get_todo_mut(id)
        .ok_or(anyhow!("Todo with ID {} not found", id))?;

    todo.complete();

    // Create a new entry with the updated todo list
    let mut entry = eideticadb::entry::Entry::new(
        tree.root_id().to_string(),
        serde_json::to_string(&eideticadb::data::KVOverWrite::new())?,
    );

    // Add the updated todo list to the entry
    entry.add_subtree("todos".to_string(), serde_json::to_string(&todo_list)?)?;

    // Set parents based on current tip entries
    let tip_entries = tree.get_tip_entries()?;
    let parent_ids = tip_entries.iter().map(|e| e.id()).collect();
    entry.set_parents(parent_ids);

    // Set subtree parents
    let subtree_tip_entries = tree.get_subtree_tip_entries("todos")?;
    let subtree_parent_ids = subtree_tip_entries.iter().map(|e| e.id()).collect();
    entry.set_subtree_parents("todos", subtree_parent_ids);

    // Insert the entry
    tree.insert(entry)?;

    Ok(())
}

fn list_todos(tree: &Tree) -> Result<()> {
    // Get the todo list
    let todo_list = get_todo_list(tree)?;

    // Get all todos and sort them by creation date
    let mut todos: Vec<_> = todo_list.get_todos();
    todos.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    // Print the todos
    if todos.is_empty() {
        println!("No tasks found.");
    } else {
        println!("Tasks:");
        for todo in todos {
            let status = if todo.completed { "✓" } else { " " };
            println!("[{}] {} (ID: {})", status, todo.title, todo.id);
        }
    }

    Ok(())
}
