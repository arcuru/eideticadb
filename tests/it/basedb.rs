use eideticadb::backend::InMemoryBackend;
use eideticadb::basedb::BaseDB;
use eideticadb::data::KVOverWrite;
use eideticadb::entry::Entry;

#[test]
fn test_new_db_and_tree() {
    let backend = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);
    let settings = KVOverWrite::new();
    let tree_result = db.new_tree(settings);
    assert!(tree_result.is_ok());
}

#[test]
fn test_load_tree() {
    let backend = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);
    let settings = KVOverWrite::new();
    let tree = db.new_tree(settings).expect("Failed to create tree");
    let root_id = tree.root_id().clone();

    // Drop the original tree instance
    drop(tree);

    // Create a new DB instance with the same backend (or reuse db)
    let loaded_tree_result = db.load_tree(&root_id);
    assert!(loaded_tree_result.is_ok());
    let loaded_tree = loaded_tree_result.unwrap();
    assert_eq!(loaded_tree.root_id(), &root_id);
}

#[test]
fn test_all_trees() {
    let backend = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);

    let settings1 = KVOverWrite::new();
    let tree1 = db.new_tree(settings1).expect("Failed to create tree 1");
    let root_id1 = tree1.root_id().clone();

    let mut settings2 = KVOverWrite::new();
    settings2.set("name".to_string(), "Tree2".to_string());
    let tree2 = db.new_tree(settings2).expect("Failed to create tree 2");
    let root_id2 = tree2.root_id().clone();

    let trees = db.all_trees().expect("Failed to get all trees");
    assert_eq!(trees.len(), 2);

    let found_ids: Vec<String> = trees.iter().map(|t| t.root_id().clone()).collect();
    assert!(found_ids.contains(&root_id1));
    assert!(found_ids.contains(&root_id2));
}

#[test]
fn test_get_backend() {
    let backend = Box::new(InMemoryBackend::new());
    // It might be useful to get the backend's ID or some state later
    // let backend_ptr = backend.as_ref() as *const _;
    let db = BaseDB::new(backend);

    let retrieved_backend = db.backend();
    // How to assert this is the same backend?
    // We could try adding a method to the backend trait or specific implementations
    // For now, just check it's not None implicitly by using it.
    assert!(retrieved_backend.lock().unwrap().all_roots().is_ok());
}

#[test]
fn test_trees_with_custom_settings() {
    let backend = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);

    // Create a tree with custom settings
    let mut settings1 = KVOverWrite::new();
    settings1.set("name".to_string(), "Tree1".to_string());
    settings1.set("description".to_string(), "First test tree".to_string());
    settings1.set("version".to_string(), "1.0".to_string());

    let tree1 = db
        .new_tree(settings1)
        .expect("Failed to create tree with custom settings");

    // Verify the settings were saved correctly
    let loaded_settings = tree1.get_settings().expect("Failed to get settings");
    assert_eq!(loaded_settings.get("name"), Some(&"Tree1".to_string()));
    assert_eq!(
        loaded_settings.get("description"),
        Some(&"First test tree".to_string())
    );
    assert_eq!(loaded_settings.get("version"), Some(&"1.0".to_string()));
    assert_eq!(loaded_settings.get("name").unwrap(), "Tree1");

    // Test loading the tree by ID
    let root_id = tree1.root_id().clone();
    let loaded_tree = db.load_tree(&root_id).expect("Failed to load tree");

    // Use the same approach as above
    let loaded_settings = loaded_tree
        .get_settings()
        .expect("Failed to get settings from loaded tree");
    assert_eq!(loaded_settings.get("name").unwrap(), "Tree1");
}

#[test]
fn test_tree_subtree_operations() {
    let backend = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);

    // Create a tree with default settings
    let settings = KVOverWrite::new();
    let tree = db.new_tree(settings).expect("Failed to create tree");

    // Create an entry with subtrees
    let mut entry = Entry::new_top_level("main data".to_string());
    entry
        .add_subtree(
            "users".to_string(),
            r#"{"data": {"user1": {"name": "Alice"}}}"#.to_string(),
        )
        .expect("Failed to add users subtree");
    entry
        .add_subtree(
            "posts".to_string(),
            r#"{"data": {"post1": {"title": "Hello"}}}"#.to_string(),
        )
        .expect("Failed to add posts subtree");

    // Insert the entry into the tree
    let id = tree.insert(entry).expect("Failed to insert entry");

    // Get subtree tips
    let user_tips = tree
        .get_subtree_tips("users")
        .expect("Failed to get users subtree tips");
    let post_tips = tree
        .get_subtree_tips("posts")
        .expect("Failed to get posts subtree tips");

    assert_eq!(user_tips.len(), 1);
    assert_eq!(post_tips.len(), 1);
    assert_eq!(user_tips[0], id);
    assert_eq!(post_tips[0], id);

    // Get subtree entries
    let user_entries = tree
        .get_subtree_tip_entries("users")
        .expect("Failed to get users subtree entries");
    let post_entries = tree
        .get_subtree_tip_entries("posts")
        .expect("Failed to get posts subtree entries");

    assert_eq!(user_entries.len(), 1);
    assert_eq!(post_entries.len(), 1);

    // Add a new entry with updated data for one subtree
    let mut new_entry = Entry::new(tree.root_id().clone(), "updated main data".to_string());
    new_entry
        .add_subtree(
            "users".to_string(),
            r#"{"data": {"user1": {"name": "Alice"}, "user2": {"name": "Bob"}}}"#.to_string(),
        )
        .expect("Failed to add updated users subtree");

    // Insert the new entry
    let new_id = tree.insert(new_entry).expect("Failed to insert new entry");

    // Verify users subtree tip has been updated
    let updated_user_tips = tree
        .get_subtree_tips("users")
        .expect("Failed to get updated users subtree tips");
    assert_eq!(updated_user_tips.len(), 1);
    assert_eq!(updated_user_tips[0], new_id);

    // But posts subtree tip should remain the same
    let unchanged_post_tips = tree
        .get_subtree_tips("posts")
        .expect("Failed to get unchanged posts subtree tips");
    assert_eq!(unchanged_post_tips.len(), 1);
    assert_eq!(unchanged_post_tips[0], id); // Still pointing to the original entry
}
