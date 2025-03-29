use eideticadb::backend::InMemoryBackend;
use eideticadb::basedb::BaseDB;

#[test]
fn test_create_basedb() {
    // Create a new in-memory backend
    let backend = Box::new(InMemoryBackend::new());

    // Create a new BaseDB
    let _db = BaseDB::new(backend);
}

#[test]
fn test_tree_creation() {
    // Create a new in-memory backend
    let backend = Box::new(InMemoryBackend::new());

    // Create a new BaseDB
    let db = BaseDB::new(backend);

    // Create a new tree with some settings
    let settings = r#"{"name": "test_tree", "description": "A test tree"}"#.to_string();
    let tree = db
        .new_tree(settings.clone())
        .expect("Failed to create tree");

    // Verify that tree was created and has a valid root ID
    let root_id = tree.root_id();
    assert!(!root_id.is_empty(), "Root ID should not be empty");
}

#[test]
fn test_tree_root_data() {
    // Create a new in-memory backend
    let backend = Box::new(InMemoryBackend::new());

    // Create a new BaseDB
    let db = BaseDB::new(backend);

    // Create a new tree with some settings
    let settings = r#"{"name": "test_tree", "description": "A test tree"}"#.to_string();
    let tree = db
        .new_tree(settings.clone())
        .expect("Failed to create tree");

    // Retrieve the root entry
    let root_entry = tree.get_root().expect("Failed to get root entry");

    // Verify that the root entry contains the expected settings
    let data = root_entry.data();
    assert_eq!(
        data.get("settings"),
        Some(&settings),
        "Root entry should contain the settings"
    );

    // Verify that the root entry has the correct Op type
    use eideticadb::entry::Op;
    assert!(
        matches!(root_entry.op(), Op::Root),
        "Root entry should have Op::Root type"
    );

    // Verify that the root entry has no parents
    assert!(
        root_entry.parents().is_empty(),
        "Root entry should have no parents"
    );
}

#[test]
fn test_multiple_trees() {
    // Create a new in-memory backend
    let backend = Box::new(InMemoryBackend::new());

    // Create a new BaseDB
    let db = BaseDB::new(backend);

    // Create multiple trees with different settings
    let settings1 = r#"{"name": "tree1"}"#.to_string();
    let settings2 = r#"{"name": "tree2"}"#.to_string();

    let tree1 = db
        .new_tree(settings1.clone())
        .expect("Failed to create tree1");
    let tree2 = db
        .new_tree(settings2.clone())
        .expect("Failed to create tree2");

    // Verify that trees have different root IDs
    assert_ne!(
        tree1.root_id(),
        tree2.root_id(),
        "Trees should have different root IDs"
    );

    // Verify that each tree has the correct settings
    let root1 = tree1.get_root().expect("Failed to get root for tree1");
    let root2 = tree2.get_root().expect("Failed to get root for tree2");

    assert_eq!(
        root1.data().get("settings"),
        Some(&settings1),
        "Tree1 should have correct settings"
    );
    assert_eq!(
        root2.data().get("settings"),
        Some(&settings2),
        "Tree2 should have correct settings"
    );
}
