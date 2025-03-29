use eideticadb::backend::InMemoryBackend;
use eideticadb::basedb::BaseDB;
use eideticadb::entry::Entry;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

fn main() -> io::Result<()> {
    println!("Welcome to EideticaDB REPL");
    print_help();

    // Create a new in-memory backend
    let backend = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);

    // Store trees by name
    let mut trees = HashMap::new();

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut input = String::new();

    loop {
        print!("> ");
        stdout.flush()?;

        input.clear();
        stdin.lock().read_line(&mut input)?;

        let args: Vec<&str> = input.split_whitespace().collect();

        if args.is_empty() {
            continue;
        }

        match args[0] {
            "help" => {
                print_help();
            }
            "exit" => {
                println!("Exiting EideticaDB REPL");
                break;
            }
            "create-tree" => {
                if args.len() < 3 {
                    println!("Usage: create-tree <name> <settings>");
                    continue;
                }

                let name = args[1];
                let settings = args[2..].join(" ");

                match db.new_tree(settings) {
                    Ok(tree) => {
                        println!("Created tree '{}' with root ID: {}", name, tree.root_id());
                        trees.insert(name.to_string(), tree);
                    }
                    Err(e) => println!("Error creating tree: {:?}", e),
                }
            }
            "list-trees" => {
                if trees.is_empty() {
                    println!("No trees created yet");
                } else {
                    println!("Trees:");
                    for (name, tree) in &trees {
                        println!("  {} (root: {})", name, tree.root_id());
                    }
                }
            }
            "get-root" => {
                if args.len() < 2 {
                    println!("Usage: get-root <tree-name>");
                    continue;
                }

                let name = args[1];

                if let Some(tree) = trees.get(name) {
                    println!("Root ID for tree '{}': {}", name, tree.root_id());
                } else {
                    println!("Tree '{}' not found", name);
                }
            }
            "get-entry" => {
                if args.len() < 2 {
                    println!("Usage: get-entry <entry-id>");
                    continue;
                }

                let id = args[1];
                let mut found = false;

                for (name, tree) in &trees {
                    if tree.root_id() == id {
                        match tree.get_root() {
                            Ok(entry) => {
                                println!("Entry found in tree '{}':", name);
                                print_entry(&entry);
                                found = true;
                                break;
                            }
                            Err(e) => {
                                println!("Error retrieving entry: {:?}", e);
                                found = true;
                                break;
                            }
                        }
                    }
                }

                if !found {
                    println!("Entry with ID '{}' not found", id);
                }
            }
            _ => println!(
                "Unknown command: {}. Type 'help' for available commands.",
                args[0]
            ),
        }
    }

    Ok(())
}

fn print_help() {
    println!("Available commands:");
    println!("  help                  - Show this help message");
    println!(
        "  create-tree <name> <settings> - Create a new tree with the given name and settings"
    );
    println!("  list-trees            - List all created trees");
    println!("  get-root <tree-name>  - Get the root ID of a tree");
    println!("  get-entry <entry-id>  - Get details of an entry by ID");
    println!("  exit                  - Exit the REPL");
}

fn print_entry(entry: &Entry) {
    println!("  ID: {}", entry.id());
    println!("  Root: {}", entry.root());
    println!("  Op: {:?}", entry.op());
    println!("  Data:");
    for (key, value) in entry.data() {
        println!("    {}: {}", key, value);
    }
    println!("  Parents: {:?}", entry.parents());
    println!("  Timestamp: {}", entry.timestamp());
}
