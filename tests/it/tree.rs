use eideticadb::backend::InMemoryBackend;
use eideticadb::basedb::BaseDB;
use eideticadb::data::KVOverWrite;
use eideticadb::entry::Entry;

#[test]
fn test_insert_into_tree() {
    let backend = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);
    let settings = KVOverWrite::new();
    let tree = db.new_tree(settings).expect("Failed to create tree");
    let root_id = tree.root_id().clone();

    let data1 = "entry_data_1".to_string();
    let entry1 = Entry::new(root_id.clone(), data1);
    // Parents will be set automatically by tree.insert

    let id1 = tree.insert(entry1).expect("Failed to insert entry 1");

    let data2 = "entry_data_2".to_string();
    let entry2 = Entry::new(root_id.clone(), data2);
    // Parents should now include id1

    let id2 = tree.insert(entry2).expect("Failed to insert entry 2");

    // Verify tips include id2
    let tips = tree.get_tips().expect("Failed to get tips");
    assert!(tips.contains(&id2));
    assert!(!tips.contains(&id1)); // id1 should no longer be a tip

    // Verify retrieval through backend directly
    let backend = tree.backend();
    let backend_guard = backend.lock().unwrap();

    let retrieved_entry1 = backend_guard.get(&id1).expect("Failed to get entry 1");
    assert_eq!(retrieved_entry1.id(), id1);

    let retrieved_entry2 = backend_guard.get(&id2).expect("Failed to get entry 2");
    assert_eq!(retrieved_entry2.id(), id2);
    assert_eq!(retrieved_entry2.parents().unwrap(), vec![id1]);
}

#[test]
fn test_get_settings() {
    let backend = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);
    let mut settings = KVOverWrite::new();
    let key = "setting_key";
    let value = "setting_value";
    settings.set(key.to_string(), value.to_string());

    let tree = db
        .new_tree(settings.clone())
        .expect("Failed to create tree");
    let retrieved_settings = tree.get_settings().expect("Failed to get settings");

    assert_eq!(retrieved_settings.get(key), Some(&value.to_string()));
}

#[test]
fn test_subtree_operations() {
    let backend = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);
    let settings = KVOverWrite::new();
    let tree = db.new_tree(settings).expect("Failed to create tree");

    // Create a new entry with two subtrees
    let mut entry = Entry::new(tree.root_id().clone(), "main_data".to_string());
    entry
        .add_subtree(
            "users".to_string(),
            r#"{"users": {"user1": {"name": "Alice"}}}"#.to_string(),
        )
        .expect("Failed to add users subtree");
    entry
        .add_subtree(
            "posts".to_string(),
            r#"{"posts": {"post1": {"title": "First Post"}}}"#.to_string(),
        )
        .expect("Failed to add posts subtree");

    // Insert the entry into the tree
    let entry_id = tree.insert(entry).expect("Failed to insert entry");

    // Get subtree tips
    let user_tips = tree
        .get_subtree_tips("users")
        .expect("Failed to get users subtree tips");
    assert_eq!(user_tips.len(), 1);
    assert_eq!(user_tips[0], entry_id);

    let post_tips = tree
        .get_subtree_tips("posts")
        .expect("Failed to get posts subtree tips");
    assert_eq!(post_tips.len(), 1);
    assert_eq!(post_tips[0], entry_id);

    // Get subtree tip entries
    let user_entries = tree
        .get_subtree_tip_entries("users")
        .expect("Failed to get users subtree entries");
    assert_eq!(user_entries.len(), 1);
    assert_eq!(user_entries[0].id(), entry_id);

    // Create another entry with just one subtree
    let mut second_entry = Entry::new(tree.root_id().clone(), "second_main_data".to_string());
    second_entry
        .add_subtree(
            "users".to_string(),
            r#"{"users": {"user2": {"name": "Bob"}}}"#.to_string(),
        )
        .expect("Failed to add users subtree to second entry");

    // Insert the second entry
    let second_id = tree
        .insert(second_entry)
        .expect("Failed to insert second entry");

    // Now users subtree should have a new tip
    let updated_user_tips = tree
        .get_subtree_tips("users")
        .expect("Failed to get updated users subtree tips");
    assert_eq!(updated_user_tips.len(), 1);
    assert_eq!(updated_user_tips[0], second_id);

    // But posts subtree tip should remain the same
    let unchanged_post_tips = tree
        .get_subtree_tips("posts")
        .expect("Failed to get unchanged posts subtree tips");
    assert_eq!(unchanged_post_tips.len(), 1);
    assert_eq!(unchanged_post_tips[0], entry_id);

    // Test get_subtree_data
    // This would require serializing/deserializing proper CRDT types
    // Just testing basic functionality here
    let backend = tree.backend();
    let backend_guard = backend.lock().unwrap();

    // Get all entries in the users subtree
    let user_subtree = backend_guard
        .get_subtree(tree.root_id(), "users")
        .expect("Failed to get users subtree");
    assert_eq!(user_subtree.len(), 2); // Both entries should be included

    // Get all entries in the posts subtree
    let post_subtree = backend_guard
        .get_subtree(tree.root_id(), "posts")
        .expect("Failed to get posts subtree");
    assert_eq!(post_subtree.len(), 1); // Only the first entry should be included
}

#[test]
fn test_complex_tree_operations() {
    let backend = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);

    // Create a tree with proper settings
    let mut settings = KVOverWrite::new();
    settings.set("name".to_string(), "ComplexTree".to_string());
    settings.set("version".to_string(), "1.0".to_string());

    let tree = db.new_tree(settings).expect("Failed to create tree");

    // Phase 1: Create a linear history of 3 entries
    let mut entries = Vec::new();

    for i in 0..3 {
        let main_data = format!("Main data for entry {}", i);
        let mut entry = Entry::new(tree.root_id().clone(), main_data);

        // Add subtrees to each entry
        entry
            .add_subtree("config".to_string(), format!(r#"{{"version": "{}.0"}}"#, i))
            .expect("Failed to add config subtree");

        // Every entry gets a different metadata subtree
        if i % 2 == 0 {
            entry
                .add_subtree("even".to_string(), format!(r#"{{"value": {}}}"#, i))
                .expect("Failed to add even subtree");
        } else {
            entry
                .add_subtree("odd".to_string(), format!(r#"{{"value": {}}}"#, i))
                .expect("Failed to add odd subtree");
        }

        // Insert the entry
        let id = tree.insert(entry).expect("Failed to insert entry");
        entries.push(id);
    }

    // Phase 2: Create a fork by adding two children to the last entry

    // Fork A
    let mut fork_a = Entry::new(tree.root_id().clone(), "Fork A data".to_string());
    fork_a
        .add_subtree("config".to_string(), r#"{"version": "fork-a"}"#.to_string())
        .expect("Failed to add config subtree to fork A");
    fork_a
        .add_subtree("fork".to_string(), r#"{"name": "fork-a"}"#.to_string())
        .expect("Failed to add fork subtree to fork A");

    // Force the fork by setting exact parents instead of using tree.insert's auto-parent feature
    fork_a.set_parents(vec![entries[2].clone()]);
    let fork_a_id = tree.insert_raw(fork_a).expect("Failed to insert fork A");

    // Fork B
    let mut fork_b = Entry::new(tree.root_id().clone(), "Fork B data".to_string());
    fork_b
        .add_subtree("config".to_string(), r#"{"version": "fork-b"}"#.to_string())
        .expect("Failed to add config subtree to fork B");
    fork_b
        .add_subtree("fork".to_string(), r#"{"name": "fork-b"}"#.to_string())
        .expect("Failed to add fork subtree to fork B");

    // Force the fork by setting exact parents
    fork_b.set_parents(vec![entries[2].clone()]);
    let fork_b_id = tree.insert_raw(fork_b).expect("Failed to insert fork B");

    // Verify there are now two tips
    let tips = tree.get_tips().expect("Failed to get tips");
    assert_eq!(tips.len(), 2);
    assert!(tips.contains(&fork_a_id));
    assert!(tips.contains(&fork_b_id));

    // Phase 3: Merge the forks with a new entry that has both as parents
    let mut merge_entry = Entry::new(tree.root_id().clone(), "Merged data".to_string());
    merge_entry
        .add_subtree("config".to_string(), r#"{"version": "merged"}"#.to_string())
        .expect("Failed to add config subtree to merge entry");
    merge_entry
        .add_subtree("fork".to_string(), r#"{"name": "merged"}"#.to_string())
        .expect("Failed to add fork subtree to merge entry");

    // Set both forks as parents
    merge_entry.set_parents(vec![fork_a_id.clone(), fork_b_id.clone()]);
    let merge_id = tree
        .insert_raw(merge_entry)
        .expect("Failed to insert merge entry");

    // Now there should be only one tip (the merge entry)
    let final_tips = tree.get_tips().expect("Failed to get final tips");
    assert_eq!(final_tips.len(), 1);
    assert_eq!(final_tips[0], merge_id);

    // Get the full tree and check its size and structure
    let backend = tree.backend();
    let backend_guard = backend.lock().unwrap();
    let full_tree = backend_guard
        .get_tree(tree.root_id())
        .expect("Failed to get full tree");

    // Should have 7 entries: root + 3 linear + 2 forks + 1 merge
    assert_eq!(full_tree.len(), 7);
}

#[test]
fn test_get_name_from_settings() {
    let backend = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);
    let mut settings = KVOverWrite::new();
    let name = "MyTestTree";
    settings.set("name".to_string(), name.to_string());

    let tree = db.new_tree(settings).expect("Failed to create tree");

    // Get the settings from the tree and check if it contains the expected name
    let retrieved_settings = tree.get_settings().expect("Failed to get settings");
    assert_eq!(retrieved_settings.get("name"), Some(&name.to_string()));
}
