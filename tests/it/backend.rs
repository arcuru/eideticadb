use eideticadb::backend::{Backend, InMemoryBackend};
use eideticadb::entry::Entry;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_in_memory_backend_basic_operations() {
    let mut backend = InMemoryBackend::new();

    // Create and insert a test entry
    let data = "test_data".to_string();
    let entry = Entry::new_top_level(data);
    let id = entry.id();

    // Put the entry
    let put_result = backend.put(entry);
    assert!(put_result.is_ok());

    // Get the entry back
    let get_result = backend.get(&id);
    assert!(get_result.is_ok());
    let retrieved_entry = get_result.unwrap();
    assert_eq!(retrieved_entry.id(), id);

    // Check all_roots
    let roots_result = backend.all_roots();
    assert!(roots_result.is_ok());
    let roots = roots_result.unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0], id);
}

#[test]
fn test_in_memory_backend_tree_operations() {
    let mut backend = InMemoryBackend::new();

    // Create a root entry
    let root_data = "root_data".to_string();
    let root_entry = Entry::new_top_level(root_data);
    let root_id = root_entry.id();
    backend.put(root_entry).unwrap();

    // Create child entries
    let child1_data = "child1_data".to_string();
    let mut child1_entry = Entry::new(root_id.clone(), child1_data);
    child1_entry.set_parents(vec![root_id.clone()]);
    let child1_id = child1_entry.id();
    backend.put(child1_entry).unwrap();

    let child2_data = "child2_data".to_string();
    let mut child2_entry = Entry::new(root_id.clone(), child2_data);
    child2_entry.set_parents(vec![child1_id.clone()]);
    let child2_id = child2_entry.id();
    backend.put(child2_entry).unwrap();

    // Test get_tips
    let tips_result = backend.get_tips(&root_id);
    assert!(tips_result.is_ok());
    let tips = tips_result.unwrap();
    assert_eq!(tips.len(), 1);
    assert_eq!(tips[0], child2_id);

    // Test get_tree
    let tree_result = backend.get_tree(&root_id);
    assert!(tree_result.is_ok());
    let tree = tree_result.unwrap();
    assert_eq!(tree.len(), 3); // root + 2 children

    // Verify tree contains all three entries
    let tree_ids: Vec<String> = tree.iter().map(|e| e.id()).collect();
    assert!(tree_ids.contains(&root_id));
    assert!(tree_ids.contains(&child1_id));
    assert!(tree_ids.contains(&child2_id));
}

#[test]
fn test_in_memory_backend_subtree_operations() {
    let mut backend = InMemoryBackend::new();

    // Create a root entry with a subtree
    let root_data = "root_data".to_string();
    let mut root_entry = Entry::new_top_level(root_data);
    let subtree_name = "subtree1";
    root_entry
        .add_subtree(subtree_name.to_string(), "subtree_data".to_string())
        .unwrap();
    let root_id = root_entry.id();
    backend.put(root_entry).unwrap();

    // Create child entry with subtree
    let child_data = "child_data".to_string();
    let mut child_entry = Entry::new(root_id.clone(), child_data);
    child_entry.set_parents(vec![root_id.clone()]);
    child_entry
        .add_subtree(subtree_name.to_string(), "child_subtree_data".to_string())
        .unwrap();
    child_entry.set_subtree_parents(subtree_name, vec![root_id.clone()]);
    let child_id = child_entry.id();
    backend.put(child_entry).unwrap();

    // Test get_subtree_tips
    let subtree_tips_result = backend.get_subtree_tips(&root_id, subtree_name);
    assert!(subtree_tips_result.is_ok());
    let subtree_tips = subtree_tips_result.unwrap();
    assert_eq!(subtree_tips.len(), 1);
    assert_eq!(subtree_tips[0], child_id);

    // Test get_subtree
    let subtree_result = backend.get_subtree(&root_id, subtree_name);
    assert!(subtree_result.is_ok());
    let subtree = subtree_result.unwrap();
    assert_eq!(subtree.len(), 2); // root + child
}

#[test]
fn test_in_memory_backend_save_and_load() {
    // Create a temporary file path
    let temp_dir = env!("CARGO_MANIFEST_DIR");
    let file_path = PathBuf::from(temp_dir).join("test_backend_save.json");

    // Setup: Create a backend with some data
    {
        let mut backend = InMemoryBackend::new();
        let entry = Entry::new_top_level("test_data".to_string());
        backend.put(entry).unwrap();

        // Save to file
        let save_result = backend.save_to_file(&file_path);
        assert!(save_result.is_ok());
    }

    // Verify file exists
    assert!(file_path.exists());

    // Load from file
    let load_result = InMemoryBackend::load_from_file(&file_path);
    assert!(load_result.is_ok());
    let loaded_backend = load_result.unwrap();

    // Verify data was loaded correctly
    let roots = loaded_backend.all_roots().unwrap();
    assert_eq!(roots.len(), 1);

    // Cleanup
    fs::remove_file(file_path).unwrap();
}

#[test]
fn test_in_memory_backend_error_handling() {
    let backend = InMemoryBackend::new();

    // Test retrieving a non-existent entry
    let non_existent_id = "non_existent_id".to_string();
    let get_result = backend.get(&non_existent_id);
    assert!(get_result.is_err());

    // For some backend implementations like InMemoryBackend, get_tips might return
    // an empty vector instead of an error when the tree doesn't exist
    // Let's verify it returns either an error or an empty vector
    // FIXME: Code smell, backends should be consistent.
    let tips_result = backend.get_tips(&non_existent_id);
    if let Ok(tips) = tips_result {
        // If it returns Ok, it should be an empty vector
        assert!(tips.is_empty());
    } else {
        // If it returns an error, that's also acceptable
        assert!(tips_result.is_err());
    }

    // Similarly, get_subtree might return an empty vector for non-existent trees
    let subtree_result = backend.get_subtree(&non_existent_id, "non_existent_subtree");
    if let Ok(entries) = subtree_result {
        assert!(entries.is_empty());
    } else {
        assert!(subtree_result.is_err());
    }

    // Similar to get_tips, get_subtree_tips might return an empty vector for non-existent trees
    let subtree_tips_result = backend.get_subtree_tips(&non_existent_id, "non_existent_subtree");
    if let Ok(tips) = subtree_tips_result {
        assert!(tips.is_empty());
    } else {
        assert!(subtree_tips_result.is_err());
    }
}

#[test]
fn test_in_memory_backend_complex_tree_structure() {
    let mut backend = InMemoryBackend::new();

    // Create a root entry
    let root_data = "root_data".to_string();
    let root_entry = Entry::new_top_level(root_data);
    let root_id = root_entry.id();
    backend.put(root_entry).unwrap();

    // Create a diamond pattern: root -> A, B -> C
    // First level children
    let a_data = "a_data".to_string();
    let mut a_entry = Entry::new(root_id.clone(), a_data);
    a_entry.set_parents(vec![root_id.clone()]);
    let a_id = a_entry.id();
    backend.put(a_entry).unwrap();

    let b_data = "b_data".to_string();
    let mut b_entry = Entry::new(root_id.clone(), b_data);
    b_entry.set_parents(vec![root_id.clone()]);
    let b_id = b_entry.id();
    backend.put(b_entry).unwrap();

    // Second level: one child with two parents
    let c_data = "c_data".to_string();
    let mut c_entry = Entry::new(root_id.clone(), c_data);
    c_entry.set_parents(vec![a_id.clone(), b_id.clone()]);
    let c_id = c_entry.id();
    backend.put(c_entry).unwrap();

    // Test get_tips - should only return C since it has no children
    let tips_result = backend.get_tips(&root_id);
    assert!(tips_result.is_ok());
    let tips = tips_result.unwrap();
    assert_eq!(tips.len(), 1);
    assert_eq!(tips[0], c_id);

    // Test get_tree - should return all 4 entries in topological order
    let tree_result = backend.get_tree(&root_id);
    assert!(tree_result.is_ok());
    let tree = tree_result.unwrap();
    assert_eq!(tree.len(), 4);

    // The root should be first in topological order
    assert_eq!(tree[0].id(), root_id);

    // C should be last as it depends on both A and B
    assert_eq!(tree[3].id(), c_id);

    // Verify A and B are in between (order between them could vary)
    // FIXME: This will be consistent once the API is defined. Update this test once the total ordering is fully implemented.
    let middle_ids: Vec<String> = vec![tree[1].id(), tree[2].id()];
    assert!(middle_ids.contains(&a_id));
    assert!(middle_ids.contains(&b_id));

    // Now test a new entry: add D which has C as a parent
    let d_data = "d_data".to_string();
    let mut d_entry = Entry::new(root_id.clone(), d_data);
    d_entry.set_parents(vec![c_id.clone()]);
    let d_id = d_entry.id();
    backend.put(d_entry).unwrap();

    // Tips should now be D
    let tips_result = backend.get_tips(&root_id);
    assert!(tips_result.is_ok());
    let tips = tips_result.unwrap();
    assert_eq!(tips.len(), 1);
    assert_eq!(tips[0], d_id);
}
