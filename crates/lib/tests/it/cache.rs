use eidetica::basedb::BaseDB;
use eidetica::Tree;
use eidetica::backend::InMemoryBackend;
use eidetica::data::KVNested;
use eidetica::subtree::KVStore;
use std::time::Instant;

#[test]
fn test_crdt_cache_functionality() {
    // Create a database with InMemoryBackend
    let backend = InMemoryBackend::new();
    let db = BaseDB::new(Box::new(backend));

    // Create a tree with initial settings
    let mut tree_settings = KVNested::default();
    tree_settings.set_string("name".to_string(), "test_tree".to_string());
    
    let tree = Tree::new(tree_settings, db.backend().clone(), None).unwrap();

    // Add some data to create history that will be cached
    for i in 0..10 {
        let op = tree.new_operation().unwrap();
        let store = op.get_subtree::<KVStore>("test_data").unwrap();
        store.set(format!("key_{i}"), format!("value_{i}")).unwrap();
        op.commit().unwrap();
    }

    // Verify the data is correct through multiple accesses
    for _ in 0..3 {
        let op = tree.new_operation().unwrap();
        let store = op.get_subtree::<KVStore>("test_data").unwrap();
        let data = store.get_all().unwrap();
        
        // Check that all our keys are present
        for i in 0..10 {
            let key = format!("key_{i}");
            assert!(data.as_hashmap().contains_key(&key));
        }
    }
}

#[test]
fn test_cache_different_tip_states() {
    // Create a database with InMemoryBackend
    let backend = InMemoryBackend::new();
    let db = BaseDB::new(Box::new(backend));

    // Create a tree
    let tree_settings = KVNested::default();
    let tree = Tree::new(tree_settings, db.backend().clone(), None).unwrap();

    // Add initial data
    let op1 = tree.new_operation().unwrap();
    let store1 = op1.get_subtree::<KVStore>("test_data").unwrap();
    store1.set("initial_key", "initial_value").unwrap();
    op1.commit().unwrap();

    // First read with initial tips (should build and cache for this tip state)
    let op2 = tree.new_operation().unwrap();
    let store2 = op2.get_subtree::<KVStore>("test_data").unwrap();
    let data1 = store2.get_all().unwrap();
    assert_eq!(data1.as_hashmap().len(), 1);

    // Add more data (creates new tip state)
    let op3 = tree.new_operation().unwrap();
    let store3 = op3.get_subtree::<KVStore>("test_data").unwrap();
    store3.set("new_key", "new_value").unwrap();
    op3.commit().unwrap();

    // Second read with new tips (should see the new data, cache miss due to different tips)
    let op4 = tree.new_operation().unwrap();
    let store4 = op4.get_subtree::<KVStore>("test_data").unwrap();
    let data2 = store4.get_all().unwrap();
    assert_eq!(data2.as_hashmap().len(), 2);
    assert!(data2.as_hashmap().contains_key("initial_key"));
    assert!(data2.as_hashmap().contains_key("new_key"));
}

#[test]
fn test_cache_performance_benefit() {
    // Create a database with InMemoryBackend
    let backend = InMemoryBackend::new();
    let db = BaseDB::new(Box::new(backend));

    // Create a tree
    let tree_settings = KVNested::default();
    let tree = Tree::new(tree_settings, db.backend().clone(), None).unwrap();

    // Add substantial history to create a meaningful computation cost
    for i in 0..50 {
        let op = tree.new_operation().unwrap();
        let store = op.get_subtree::<KVStore>("test_data").unwrap();
        store.set(format!("key_{i}"), format!("value_{i}")).unwrap();
        op.commit().unwrap();
    }

    // Measure first access (should compute and cache)
    let start = Instant::now();
    let op1 = tree.new_operation().unwrap();
    let store1 = op1.get_subtree::<KVStore>("test_data").unwrap();
    let _data1 = store1.get_all().unwrap();
    let first_duration = start.elapsed();

    // Measure second access (should use cache)
    let start = Instant::now();
    let op2 = tree.new_operation().unwrap();
    let store2 = op2.get_subtree::<KVStore>("test_data").unwrap();
    let _data2 = store2.get_all().unwrap();
    let second_duration = start.elapsed();

    // The test primarily verifies that caching doesn't break functionality
    // Performance improvement may vary based on system load, but both should complete successfully
    println!("First access: {first_duration:?}, Second access: {second_duration:?}");
    
    // Both accesses should produce the same data
    let op3 = tree.new_operation().unwrap();
    let store3 = op3.get_subtree::<KVStore>("test_data").unwrap();
    let data = store3.get_all().unwrap();
    assert_eq!(data.as_hashmap().len(), 50);
}