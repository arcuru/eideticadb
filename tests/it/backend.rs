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
        .set_subtree_data(subtree_name.to_string(), "subtree_data".to_string())
        .unwrap();
    let root_id = root_entry.id();
    backend.put(root_entry).unwrap();

    // Create child entry with subtree
    let child_data = "child_data".to_string();
    let mut child_entry = Entry::new(root_id.clone(), child_data);
    child_entry.set_parents(vec![root_id.clone()]);
    child_entry
        .set_subtree_data(subtree_name.to_string(), "child_subtree_data".to_string())
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
    let final_tips = backend.get_tips(&root_id).unwrap();
    assert_eq!(final_tips.len(), 1);
    assert_eq!(final_tips[0], d_id);
}

#[test]
fn test_backend_get_tree_from_tips() {
    let mut backend = InMemoryBackend::new();
    let root_id = "tree_root".to_string();

    // Create entries: root -> e1 -> e2a, e2b
    let entry_root = Entry::new(root_id.clone(), "root_data".to_string());
    let root_entry_id = entry_root.id();
    backend.put(entry_root).unwrap();

    let mut e1 = Entry::new(root_id.clone(), "e1_data".to_string());
    e1.set_parents(vec![root_entry_id.clone()]);
    let e1_id = e1.id();
    backend.put(e1).unwrap();

    let mut e2a = Entry::new(root_id.clone(), "e2a_data".to_string());
    e2a.set_parents(vec![e1_id.clone()]);
    let e2a_id = e2a.id();
    backend.put(e2a).unwrap();

    let mut e2b = Entry::new(root_id.clone(), "e2b_data".to_string());
    e2b.set_parents(vec![e1_id.clone()]);
    let e2b_id = e2b.id();
    backend.put(e2b).unwrap();

    // --- Test with single tip e2a ---
    let tree_e2a = backend
        .get_tree_from_tips(&root_id, &[e2a_id.clone()])
        .expect("Failed to get tree from tip e2a");
    assert_eq!(tree_e2a.len(), 3, "Tree from e2a should have root, e1, e2a");
    let ids_e2a: Vec<_> = tree_e2a.iter().map(|e| e.id()).collect();
    assert!(ids_e2a.contains(&root_entry_id));
    assert!(ids_e2a.contains(&e1_id));
    assert!(ids_e2a.contains(&e2a_id));
    assert!(!ids_e2a.contains(&e2b_id)); // Should not contain e2b

    // Verify topological order (root -> e1 -> e2a)
    assert_eq!(tree_e2a[0].id(), root_entry_id);
    assert_eq!(tree_e2a[1].id(), e1_id);
    assert_eq!(tree_e2a[2].id(), e2a_id);

    // --- Test with both tips e2a and e2b ---
    let tree_both = backend
        .get_tree_from_tips(&root_id, &[e2a_id.clone(), e2b_id.clone()])
        .expect("Failed to get tree from tips e2a, e2b");
    assert_eq!(
        tree_both.len(),
        4,
        "Tree from both tips should have all 4 entries"
    );
    let ids_both: Vec<_> = tree_both.iter().map(|e| e.id()).collect();
    assert!(ids_both.contains(&root_entry_id));
    assert!(ids_both.contains(&e1_id));
    assert!(ids_both.contains(&e2a_id));
    assert!(ids_both.contains(&e2b_id));

    // Verify topological order (root -> e1 -> {e2a, e2b})
    assert_eq!(tree_both[0].id(), root_entry_id);
    assert_eq!(tree_both[1].id(), e1_id);
    // Order of e2a and e2b might vary, check they are last two
    let last_two: Vec<_> = vec![tree_both[2].id(), tree_both[3].id()];
    assert!(last_two.contains(&e2a_id));
    assert!(last_two.contains(&e2b_id));

    // --- Test with non-existent tip ---
    let tree_bad_tip = backend
        .get_tree_from_tips(&root_id, &["bad_tip_id".to_string()])
        .expect("Failed to get tree with non-existent tip");
    assert!(
        tree_bad_tip.is_empty(),
        "Getting tree from non-existent tip should return empty vector"
    );

    // --- Test with non-existent tree root ---
    let bad_root_string = "bad_root".to_string();
    let tree_bad_root = backend
        .get_tree_from_tips(&bad_root_string, &[e1_id.clone()])
        .expect("Failed to get tree with non-existent root");
    assert!(
        tree_bad_root.is_empty(),
        "Getting tree from non-existent root should return empty vector"
    );
}

#[test]
fn test_backend_get_subtree_from_tips() {
    let mut backend = InMemoryBackend::new();
    let root_id_string = "tree_root".to_string();
    let subtree_name_string = "my_subtree".to_string();

    // Create entries: root -> e1 -> e2a, e2b
    // root: has subtree
    // e1: no subtree
    // e2a: has subtree
    // e2b: has subtree

    let mut entry_root = Entry::new(root_id_string.clone(), "root_data".to_string());
    entry_root
        .set_subtree_data(subtree_name_string.clone(), "root_sub_data".to_string())
        .unwrap();
    let root_entry_id = entry_root.id();
    backend.put(entry_root).unwrap();

    let mut e1 = Entry::new(root_id_string.clone(), "e1_data".to_string()); // No subtree
    e1.set_parents(vec![root_entry_id.clone()]);
    let e1_id = e1.id();
    backend.put(e1).unwrap();

    let mut e2a = Entry::new(root_id_string.clone(), "e2a_data".to_string());
    e2a.set_parents(vec![e1_id.clone()]);
    e2a.set_subtree_data(subtree_name_string.clone(), "e2a_sub_data".to_string())
        .unwrap();
    e2a.set_subtree_parents(&subtree_name_string, vec![root_entry_id.clone()]);
    let e2a_id = e2a.id();
    backend.put(e2a).unwrap();

    let mut e2b = Entry::new(root_id_string.clone(), "e2b_data".to_string());
    e2b.set_parents(vec![e1_id.clone()]);
    e2b.set_subtree_data(subtree_name_string.clone(), "e2b_sub_data".to_string())
        .unwrap();
    e2b.set_subtree_parents(&subtree_name_string, vec![root_entry_id.clone()]);
    let e2b_id = e2b.id();
    backend.put(e2b).unwrap();

    // --- Test with single tip e2a ---
    let subtree_e2a = backend
        .get_subtree_from_tips(&root_id_string, &subtree_name_string, &[e2a_id.clone()])
        .expect("Failed to get subtree from tip e2a");
    // Should contain root and e2a (which have the subtree), but not e1 (no subtree) or e2b (not in history of tip e2a)
    assert_eq!(
        subtree_e2a.len(),
        2,
        "Subtree from e2a should have root, e2a"
    );
    let ids_e2a: Vec<_> = subtree_e2a.iter().map(|e| e.id()).collect();
    assert!(ids_e2a.contains(&root_entry_id));
    assert!(!ids_e2a.contains(&e1_id)); // e1 doesn't have the subtree
    assert!(ids_e2a.contains(&e2a_id));
    assert!(!ids_e2a.contains(&e2b_id)); // e2b is not an ancestor of e2a

    // Verify topological order (root -> e2a)
    assert_eq!(subtree_e2a[0].id(), root_entry_id);
    assert_eq!(subtree_e2a[1].id(), e2a_id);

    // --- Test with both tips e2a and e2b ---
    let subtree_both = backend
        .get_subtree_from_tips(
            &root_id_string,
            &subtree_name_string,
            &[e2a_id.clone(), e2b_id.clone()],
        )
        .expect("Failed to get subtree from tips e2a, e2b");
    // Should contain root, e2a, e2b (all have the subtree)
    assert_eq!(
        subtree_both.len(),
        3,
        "Subtree from both tips should have root, e2a, e2b"
    );
    let ids_both: Vec<_> = subtree_both.iter().map(|e| e.id()).collect();
    assert!(ids_both.contains(&root_entry_id));
    assert!(!ids_both.contains(&e1_id));
    assert!(ids_both.contains(&e2a_id));
    assert!(ids_both.contains(&e2b_id));

    // Verify topological order (root -> {e2a, e2b})
    assert_eq!(subtree_both[0].id(), root_entry_id);
    let last_two: Vec<_> = vec![subtree_both[1].id(), subtree_both[2].id()];
    assert!(last_two.contains(&e2a_id));
    assert!(last_two.contains(&e2b_id));

    // --- Test with non-existent subtree name ---
    let bad_name_string = "bad_name".to_string();
    let subtree_bad_name =
        backend.get_subtree_from_tips(&root_id_string, &bad_name_string, &[e2a_id.clone()]);
    assert!(
        subtree_bad_name.is_ok(),
        "Getting subtree with bad name should be ok..."
    );
    assert!(
        subtree_bad_name.unwrap().is_empty(),
        "...but return empty list"
    );
    // --- Test with non-existent tip ---
    let subtree_bad_tip = backend
        .get_subtree_from_tips(
            &root_id_string,
            &subtree_name_string,
            &["bad_tip_id".to_string()],
        )
        .expect("Failed to get subtree with non-existent tip");
    assert!(
        subtree_bad_tip.is_empty(),
        "Getting subtree from non-existent tip should return empty list"
    );

    // --- Test with non-existent tree root ---
    let bad_root_string_2 = "bad_root".to_string();
    let subtree_bad_root = backend
        .get_subtree_from_tips(&bad_root_string_2, &subtree_name_string, &[e1_id.clone()])
        .expect("Failed to get subtree with non-existent root");
    assert!(
        subtree_bad_root.is_empty(),
        "Getting subtree from non-existent root should return empty list"
    );
}
