use eideticadb::backend::Backend;
use eideticadb::backend::InMemoryBackend;
use eideticadb::basedb::BaseDB;
use eideticadb::data::KVOverWrite;
use eideticadb::subtree::KVStore;

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
    let backend: Box<dyn Backend> = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);

    let retrieved_backend = db.backend();
    assert!(retrieved_backend.lock().unwrap().all_roots().is_ok());
}

#[test]
fn test_create_tree_with_initial_settings() {
    let backend: Box<dyn Backend> = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);

    let mut settings = KVOverWrite::new();
    settings.set("name".to_string(), "My Settings Tree".to_string());
    settings.set("version".to_string(), "1.0".to_string());

    let tree = db.new_tree(settings).expect("Failed to create tree");

    let settings_viewer = tree
        .get_subtree_viewer::<KVStore>("settings")
        .expect("Failed to get settings viewer");

    assert_eq!(
        settings_viewer
            .get("name")
            .expect("Failed to get name setting"),
        "My Settings Tree"
    );
    assert_eq!(
        settings_viewer
            .get("version")
            .expect("Failed to get version setting"),
        "1.0"
    );

    assert_eq!(
        tree.get_name().expect("Failed to get tree name"),
        "My Settings Tree"
    );
}

#[test]
fn test_basic_subtree_modification() {
    let backend: Box<dyn Backend> = Box::new(InMemoryBackend::new());
    let db = BaseDB::new(backend);
    let settings = KVOverWrite::new();
    let tree = db.new_tree(settings).expect("Failed to create tree");

    let op = tree.new_operation().expect("Failed to start operation");
    {
        let data_store = op
            .get_subtree::<KVStore>("user_data")
            .expect("Failed to get data subtree");

        data_store
            .set("user_id", "alice")
            .expect("Failed to set user_id");
        data_store
            .set("email", "alice@example.com")
            .expect("Failed to set email");
    }

    let commit_result = op.commit();
    assert!(
        commit_result.is_ok(),
        "Commit failed: {:?}",
        commit_result.err()
    );
    let new_tip_id = commit_result.unwrap();
    assert_ne!(
        new_tip_id,
        *tree.root_id(),
        "Commit should create a new tip"
    );

    let data_viewer = tree
        .get_subtree_viewer::<KVStore>("user_data")
        .expect("Failed to get data viewer after commit");

    assert_eq!(
        data_viewer
            .get("user_id")
            .expect("Failed to get user_id after commit"),
        "alice"
    );
    assert_eq!(
        data_viewer
            .get("email")
            .expect("Failed to get email after commit"),
        "alice@example.com"
    );

    match data_viewer.get("non_existent_key") {
        Err(eideticadb::Error::NotFound) => (),
        other => panic!("Expected NotFound error, got {:?}", other),
    }
}
