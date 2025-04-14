use eideticadb::entry::Entry;

#[test]
fn test_entry_creation() {
    let root = "test_root".to_string();
    let data = "test_data".to_string();
    let entry = Entry::new(root.clone(), data.clone());

    assert_eq!(entry.root(), &root);
    assert!(!entry.is_root()); // Regular entries are not root entries
    assert!(!entry.is_toplevel_root()); // Should be false as it's not a top-level entry
    assert_eq!(entry.get_settings().unwrap(), data); // Use get_settings() to access the main data
    assert!(entry.parents().unwrap().is_empty()); // New entry has no parents
}

#[test]
fn test_entry_toplevel_creation() {
    let data = "test_data".to_string();
    let entry = Entry::new_top_level(data.clone());

    assert!(entry.is_toplevel_root());
    assert!(entry.is_root());
    assert_eq!(entry.get_settings().unwrap(), data); // Use get_settings() to access the main data
    assert!(entry.in_subtree("root")); // Top-level entries have a "root" subtree
}

#[test]
fn test_entry_add_subtree() {
    let root = "test_root".to_string();
    let data = "test_data".to_string();
    let mut entry = Entry::new(root, data);

    let subtree_name = "subtree1";
    let subtree_data = "subtree_data".to_string();
    let result = entry.add_subtree(subtree_name.to_string(), subtree_data.clone());
    assert!(result.is_ok());

    // Verify subtree was added
    let subtrees = entry.subtrees().unwrap();
    assert_eq!(subtrees.len(), 1);
    assert_eq!(subtrees[0], subtree_name);

    // Verify subtree data
    let fetched_data = entry.data(subtree_name).unwrap();
    assert_eq!(fetched_data, &subtree_data);

    // Check subtree parents
    let subtree_parents = entry.subtree_parents(subtree_name).unwrap();
    assert!(subtree_parents.is_empty()); // New subtree has no parents initially
}

#[test]
fn test_entry_parents() {
    let root = "test_root".to_string();
    let data = "test_data".to_string();
    let mut entry = Entry::new(root, data);

    // Set parents for the main tree
    let parent1 = "parent1".to_string();
    let parent2 = "parent2".to_string();
    let parents = vec![parent1.clone(), parent2.clone()];
    entry.set_parents(parents.clone());

    // Verify parents were set
    let fetched_parents = entry.parents().unwrap();
    assert_eq!(fetched_parents, parents);

    // Test subtree parents
    let subtree_name = "subtree1";
    let subtree_data = "subtree_data".to_string();
    entry
        .add_subtree(subtree_name.to_string(), subtree_data)
        .unwrap();

    let subtree_parent = "subtree_parent".to_string();
    entry.set_subtree_parents(subtree_name, vec![subtree_parent.clone()]);

    let fetched_subtree_parents = entry.subtree_parents(subtree_name).unwrap();
    assert_eq!(fetched_subtree_parents, vec![subtree_parent]);
}

#[test]
fn test_entry_id() {
    let root = "test_root".to_string();
    let data = "test_data".to_string();
    let entry = Entry::new(root.clone(), data.clone());

    let id = entry.id();
    assert!(!id.is_empty());

    // Create identical entry - should have same ID (content-addressable)
    let identical_entry = Entry::new(root, data);
    assert_eq!(identical_entry.id(), id);

    // Create different entry - should have different ID
    let different_entry = Entry::new("different_root".to_string(), "different_data".to_string());
    assert_ne!(different_entry.id(), id);
}

#[test]
fn test_in_tree_and_subtree() {
    let root = "test_root".to_string();
    let data = "test_data".to_string();
    let mut entry = Entry::new(root.clone(), data);

    assert!(entry.in_tree(&root));
    assert!(!entry.in_tree("other_tree"));

    let subtree_name = "subtree1";
    entry
        .add_subtree(subtree_name.to_string(), "subtree_data".to_string())
        .unwrap();

    assert!(entry.in_subtree(subtree_name));
    assert!(!entry.in_subtree("non_existent_subtree"));
}

#[test]
fn test_entry_with_multiple_subtrees() {
    let root = "test_root".to_string();
    let main_data = "main_data".to_string();

    // Create an entry with multiple subtrees
    let mut entry = Entry::new(root, main_data);

    // Add several subtrees
    let subtrees = [
        ("users", "user_data"),
        ("posts", "post_data"),
        ("comments", "comment_data"),
        ("ratings", "rating_data"),
    ];

    for (name, data) in subtrees.iter() {
        entry
            .add_subtree(name.to_string(), data.to_string())
            .unwrap_or_else(|_| panic!("Failed to add subtree {}", name));
    }

    // Verify all subtrees were added
    let subtree_names = entry.subtrees().unwrap();
    assert_eq!(subtree_names.len(), 4);

    // Verify each subtree has the right data
    for (name, data) in subtrees.iter() {
        assert!(entry.in_subtree(name));
        assert_eq!(entry.data(name).unwrap(), &data.to_string());
    }

    // Try to access a non-existent subtree
    let non_existent = entry.data("non_existent");
    assert!(non_existent.is_err());

    // Add parents to each subtree
    for (name, _) in subtrees.iter() {
        let parent_id = format!("parent_for_{}", name);
        entry.set_subtree_parents(name, vec![parent_id.clone()]);

        // Verify parents were set correctly
        let parents = entry.subtree_parents(name).unwrap();
        assert_eq!(parents.len(), 1);
        assert_eq!(parents[0], parent_id);
    }
}

#[test]
fn test_entry_id_determinism() {
    // Test that entries with the same data but created differently have the same ID

    // First entry
    let mut entry1 = Entry::new("test_root".to_string(), "main_data".to_string());
    // Parents order should not matter
    entry1.set_parents(vec!["parent1".to_string(), "parent2".to_string()]);
    entry1
        .add_subtree("subtree1".to_string(), "data1".to_string())
        .unwrap();
    entry1
        .add_subtree("subtree2".to_string(), "data2".to_string())
        .unwrap();
    entry1.set_subtree_parents("subtree1", vec!["sub_parent1".to_string()]);

    // Second entry with same content but adding subtrees and parents in different order
    let mut entry2 = Entry::new("test_root".to_string(), "main_data".to_string());
    // Order of adding subtrees should not matter
    entry2
        .add_subtree("subtree2".to_string(), "data2".to_string())
        .unwrap();
    entry2
        .add_subtree("subtree1".to_string(), "data1".to_string())
        .unwrap();
    // Order of parents should not matter
    // Now using different order to test that the order of parents does not matter
    entry2.set_parents(vec!["parent2".to_string(), "parent1".to_string()]);
    entry2.set_subtree_parents("subtree1", vec!["sub_parent1".to_string()]);

    // IDs should be the same
    assert_eq!(entry1.id(), entry2.id());

    // Now modify entry2 in a subtle way
    let mut entry3 = entry2.clone();
    entry3.set_subtree_parents("subtree1", vec!["different_parent".to_string()]);

    // IDs should now be different
    assert_ne!(entry1.id(), entry3.id());
}
