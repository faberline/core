use super::*;
use crate::domain::package::manager::{PackageManager, PackageManagerDetection};

#[test]
fn test_package_manager_display_name() {
    assert_eq!(PackageManager::Uv.display_name(), "uv");
    assert_eq!(PackageManager::Poetry.display_name(), "Poetry");
    assert_eq!(PackageManager::Pipenv.display_name(), "Pipenv");
    assert_eq!(PackageManager::Pip.display_name(), "pip");
}

#[test]
fn test_parse_simple_dependency() {
    let dep = PackageManagerDetector::parse_dependency_line("django>=4.0").unwrap();
    assert_eq!(dep.name, "django");
    assert_eq!(dep.version, Some(">=4.0".to_string()));
    assert!(dep.extras.is_empty());
}

#[test]
fn test_parse_dependency_with_extras() {
    let dep = PackageManagerDetector::parse_dependency_line("fastapi[all]>=0.100").unwrap();
    assert_eq!(dep.name, "fastapi");
    assert_eq!(dep.version, Some(">=0.100".to_string()));
    assert_eq!(dep.extras, vec!["all"]);
}

#[test]
fn test_parse_dependency_multiple_extras() {
    let dep = PackageManagerDetector::parse_dependency_line("package[dev,test]").unwrap();
    assert_eq!(dep.name, "package");
    assert_eq!(dep.extras, vec!["dev", "test"]);
}

#[test]
fn test_parse_dependency_no_version() {
    let dep = PackageManagerDetector::parse_dependency_line("requests").unwrap();
    assert_eq!(dep.name, "requests");
    assert_eq!(dep.version, None);
}

#[test]
fn test_framework_detection() {
    let django = Dependency::new("django".to_string());
    assert!(django.is_framework());

    let fastapi = Dependency::new("fastapi".to_string());
    assert!(fastapi.is_framework());

    let requests = Dependency::new("requests".to_string());
    assert!(!requests.is_framework());
}

#[test]
fn test_dependency_builder() {
    let dep = Dependency::new("django".to_string())
        .with_version(">=4.0".to_string())
        .with_extras(vec!["postgres".to_string()])
        .as_dev();

    assert_eq!(dep.name, "django");
    assert_eq!(dep.version, Some(">=4.0".to_string()));
    assert_eq!(dep.extras, vec!["postgres"]);
    assert!(dep.is_dev);
}

#[test]
fn test_parse_poetry_dependency() {
    let dep = PackageManagerDetector::parse_poetry_dependency("django = \"^4.0\"").unwrap();
    assert_eq!(dep.name, "django");
    assert_eq!(dep.version, Some("^4.0".to_string()));
}

#[test]
fn test_parse_poetry_dependency_skip_python() {
    let dep = PackageManagerDetector::parse_poetry_dependency("python = \"^3.10\"");
    assert!(dep.is_none());
}

#[test]
fn test_detection_unknown() {
    let detection = PackageManagerDetection::unknown();
    assert_eq!(detection.manager, PackageManager::Unknown);
    assert_eq!(detection.confidence, 0.0);
    assert!(detection.dependencies.is_empty());
}

#[test]
fn test_has_dependency() {
    let mut detection = PackageManagerDetection::unknown();
    detection
        .dependencies
        .push(Dependency::new("django".to_string()));

    assert!(detection.has_dependency("django"));
    assert!(!detection.has_dependency("flask"));
}

#[test]
fn test_framework_dependencies() {
    let mut detection = PackageManagerDetection::unknown();
    detection
        .dependencies
        .push(Dependency::new("django".to_string()));
    detection
        .dependencies
        .push(Dependency::new("requests".to_string()));
    detection
        .dependencies
        .push(Dependency::new("fastapi".to_string()));

    let frameworks = detection.framework_dependencies();
    assert_eq!(frameworks.len(), 2);
    assert!(frameworks.iter().any(|d| d.name == "django"));
    assert!(frameworks.iter().any(|d| d.name == "fastapi"));
}
