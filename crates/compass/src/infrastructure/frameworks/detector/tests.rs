use super::*;

#[test]
fn test_framework_detection() {
    let detector = FrameworkDetector::new(PathBuf::from("."));
    let result = detector.detect();
    assert!(result.frameworks.is_empty()); // Empty project
}
