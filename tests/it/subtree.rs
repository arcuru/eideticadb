use eideticadb::backend::InMemoryBackend;
use eideticadb::basedb::BaseDB;
use eideticadb::data::KVOverWrite;
use eideticadb::subtree::KVStore;

// Helper function to set up a tree for testing
fn setup_tree() -> eideticadb::Tree {
    let backend = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);
    let settings = KVOverWrite::new();
    db.new_tree(settings)
        .expect("Failed to create tree for setup")
}

#[test]
fn test_kvstore_set_and_get_via_op() {
    let tree = setup_tree();
    let op = tree.new_operation().expect("Failed to start operation");

    {
        let kv_store = op
            .get_subtree::<KVStore>("my_kv")
            .expect("Failed to get KVStore");

        // Set initial values
        kv_store.set("key1", "value1").expect("Failed to set key1");
        kv_store.set("key2", "value2").expect("Failed to set key2");

        // Get staged values within the same operation
        assert_eq!(
            kv_store.get("key1").expect("Failed get staged key1"),
            "value1"
        );
        assert_eq!(
            kv_store.get("key2").expect("Failed get staged key2"),
            "value2"
        );

        // Overwrite a value
        kv_store
            .set("key1", "value1_updated")
            .expect("Failed to overwrite key1");
        assert_eq!(
            kv_store
                .get("key1")
                .expect("Failed get staged overwritten key1"),
            "value1_updated"
        );

        // Get non-existent key
        match kv_store.get("non_existent") {
            Err(eideticadb::Error::NotFound) => (), // Expected
            other => panic!("Expected NotFound, got {:?}", other),
        }
    }

    // Commit the operation
    op.commit().expect("Failed to commit operation");

    // Verify final state with a viewer
    let viewer = tree
        .get_subtree_viewer::<KVStore>("my_kv")
        .expect("Failed to get viewer");
    assert_eq!(
        viewer.get("key1").expect("Viewer: Failed get key1"),
        "value1_updated"
    );
    assert_eq!(
        viewer.get("key2").expect("Viewer: Failed get key2"),
        "value2"
    );
    match viewer.get("non_existent") {
        Err(eideticadb::Error::NotFound) => (), // Expected
        other => panic!("Viewer: Expected NotFound, got {:?}", other),
    }
}

#[test]
fn test_kvstore_get_all_via_viewer() {
    let tree = setup_tree();

    // Op 1: Set initial data
    let op1 = tree.new_operation().expect("Op1: Failed start");
    {
        let kv_store = op1
            .get_subtree::<KVStore>("my_kv")
            .expect("Op1: Failed get");
        kv_store.set("key_a", "val_a").expect("Op1: Failed set a");
        kv_store.set("key_b", "val_b").expect("Op1: Failed set b");
    }
    op1.commit().expect("Op1: Failed commit");

    // Op 2: Update one, add another
    let op2 = tree.new_operation().expect("Op2: Failed start");
    {
        let kv_store = op2
            .get_subtree::<KVStore>("my_kv")
            .expect("Op2: Failed get");
        kv_store
            .set("key_b", "val_b_updated")
            .expect("Op2: Failed update b");
        kv_store.set("key_c", "val_c").expect("Op2: Failed set c");
    }
    op2.commit().expect("Op2: Failed commit");

    // Verify get_all using a viewer
    let viewer = tree
        .get_subtree_viewer::<KVStore>("my_kv")
        .expect("Failed to get viewer");
    let all_data_crdt = viewer.get_all().expect("Failed to get all data");
    let all_data_map = all_data_crdt.as_hashmap();

    assert_eq!(all_data_map.len(), 3);
    assert_eq!(all_data_map.get("key_a"), Some(&"val_a".to_string()));
    assert_eq!(
        all_data_map.get("key_b"),
        Some(&"val_b_updated".to_string())
    );
    assert_eq!(all_data_map.get("key_c"), Some(&"val_c".to_string()));
}

#[test]
fn test_kvstore_get_all_empty() {
    let tree = setup_tree();

    // Get viewer for a non-existent subtree
    let viewer = tree
        .get_subtree_viewer::<KVStore>("empty_kv")
        .expect("Failed to get viewer for empty");
    let all_data_crdt = viewer.get_all().expect("Failed to get all data from empty");
    let all_data_map = all_data_crdt.as_hashmap();

    assert!(all_data_map.is_empty());
}
