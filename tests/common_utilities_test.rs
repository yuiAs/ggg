mod common;

use common::*;

#[tokio::test]
async fn test_mock_server_setup() {
    let (_server, uri) = setup_mock_download_server().await;
    assert!(!uri.is_empty());
    assert!(uri.starts_with("http://"));
}

#[tokio::test]
async fn test_mock_file_server() {
    let content = b"test content".to_vec();
    let (_server, uri) = setup_mock_file_server("/test.txt", content.clone()).await;

    // Verify server is running
    assert!(!uri.is_empty());
}

#[test]
fn test_generate_content() {
    let content = generate_test_content(100);
    assert_eq!(content.len(), 100);
    assert_eq!(content[0], 0);
    assert_eq!(content[99], 99);
}

#[test]
fn test_create_test_task() {
    use std::path::PathBuf;

    let task = create_test_task(
        "http://example.com/file.zip".to_string(),
        PathBuf::from("/tmp"),
    );
    assert_eq!(task.url, "http://example.com/file.zip");
    assert_eq!(task.save_path, PathBuf::from("/tmp"));
}

#[test]
fn test_create_test_task_with_filename() {
    use std::path::PathBuf;

    let task = create_test_task_with_filename(
        "http://example.com/file.zip".to_string(),
        PathBuf::from("/tmp"),
        "custom.zip".to_string(),
    );
    assert_eq!(task.filename, "custom.zip");
}
