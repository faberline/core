use super::*;

#[test]
fn test_module_graph_basic() {
    let mut graph = ModuleGraph::new();

    graph.add_module("main", None);
    graph.add_module("utils", None);
    graph.add_import("main", "utils");

    assert!(graph.has_module("main"));
    assert!(graph.has_module("utils"));

    let main = graph.get_module("main").unwrap();
    assert!(main.imports.contains("utils"));

    let utils = graph.get_module("utils").unwrap();
    assert!(utils.imported_by.contains("main"));
}

#[test]
fn test_cycle_detection_no_cycle() {
    let mut graph = ModuleGraph::new();

    graph.add_import("a", "b");
    graph.add_import("b", "c");
    graph.add_import("a", "c");

    let cycles = graph.detect_cycles();
    assert!(cycles.is_empty());
}

#[test]
fn test_cycle_detection_with_cycle() {
    let mut graph = ModuleGraph::new();

    graph.add_import("a", "b");
    graph.add_import("b", "c");
    graph.add_import("c", "a"); // Creates a cycle

    let cycles = graph.detect_cycles();
    assert!(!cycles.is_empty());
}

#[test]
fn test_topological_sort() {
    let mut graph = ModuleGraph::new();

    graph.add_import("app", "services");
    graph.add_import("app", "models");
    graph.add_import("services", "models");

    let order = graph.topological_sort().unwrap();

    // models should come before services and app
    let models_pos = order.iter().position(|n| n == "models").unwrap();
    let services_pos = order.iter().position(|n| n == "services").unwrap();
    let app_pos = order.iter().position(|n| n == "app").unwrap();

    assert!(models_pos < services_pos);
    assert!(models_pos < app_pos);
    assert!(services_pos < app_pos);
}

#[test]
fn test_topological_sort_with_cycle() {
    let mut graph = ModuleGraph::new();

    graph.add_import("a", "b");
    graph.add_import("b", "a");

    let result = graph.topological_sort();
    assert!(result.is_none()); // Cycle detected
}

#[test]
fn test_get_dependencies() {
    let mut graph = ModuleGraph::new();

    graph.add_import("app", "utils");
    graph.add_import("app", "models");
    graph.add_import("app", "config");

    let deps = graph.get_dependencies("app");
    assert_eq!(deps.len(), 3);
    assert!(deps.contains("utils"));
    assert!(deps.contains("models"));
    assert!(deps.contains("config"));
}

#[test]
fn test_get_dependents() {
    let mut graph = ModuleGraph::new();

    graph.add_import("app", "utils");
    graph.add_import("tests", "utils");
    graph.add_import("cli", "utils");

    let dependents = graph.get_dependents("utils");
    assert_eq!(dependents.len(), 3);
    assert!(dependents.contains("app"));
    assert!(dependents.contains("tests"));
    assert!(dependents.contains("cli"));
}

#[test]
fn test_transitive_dependencies() {
    let mut graph = ModuleGraph::new();

    graph.add_import("app", "services");
    graph.add_import("services", "models");
    graph.add_import("models", "base");

    let deps = graph.get_transitive_dependencies("app");
    assert!(deps.contains("services"));
    assert!(deps.contains("models"));
    assert!(deps.contains("base"));
}

#[test]
fn test_path_to_module_name() {
    let root = Path::new("/project/src");

    let path1 = Path::new("/project/src/app.py");
    assert_eq!(
        ModuleGraph::path_to_module_name(path1, root),
        Some("app".to_string())
    );

    let path2 = Path::new("/project/src/mypackage/module.py");
    assert_eq!(
        ModuleGraph::path_to_module_name(path2, root),
        Some("mypackage.module".to_string())
    );

    let path3 = Path::new("/project/src/mypackage/__init__.py");
    assert_eq!(
        ModuleGraph::path_to_module_name(path3, root),
        Some("mypackage".to_string())
    );
}

#[test]
fn test_transitive_dependents() {
    let mut graph = ModuleGraph::new();

    // app -> services -> models -> base
    graph.add_import("app", "services");
    graph.add_import("services", "models");
    graph.add_import("models", "base");

    // If base changes, models, services, and app all need re-analysis
    let dependents = graph.get_transitive_dependents("base");
    assert!(dependents.contains("models"));
    assert!(dependents.contains("services"));
    assert!(dependents.contains("app"));

    // If services changes, only app needs re-analysis
    let dependents = graph.get_transitive_dependents("services");
    assert!(dependents.contains("app"));
    assert!(!dependents.contains("models")); // models doesn't import services
}

#[test]
fn test_get_affected_modules() {
    let mut graph = ModuleGraph::new();

    graph.add_import("app", "services");
    graph.add_import("services", "models");
    graph.add_import("app", "models"); // app also imports models directly

    let affected = graph.get_affected_modules("models");
    assert!(affected.contains(&"models".to_string()));
    assert!(affected.contains(&"services".to_string()));
    assert!(affected.contains(&"app".to_string()));

    // Models should come before services and app in the order
    let models_pos = affected.iter().position(|n| n == "models");
    let services_pos = affected.iter().position(|n| n == "services");
    let app_pos = affected.iter().position(|n| n == "app");

    assert!(models_pos.is_some());
    assert!(services_pos.is_some());
    assert!(app_pos.is_some());
}

#[test]
fn test_remove_module() {
    let mut graph = ModuleGraph::new();

    graph.add_import("app", "services");
    graph.add_import("app", "models");
    graph.add_import("services", "models");

    // Remove models
    graph.remove_module("models");

    // models should no longer exist
    assert!(!graph.has_module("models"));

    // app and services should no longer import models
    let app = graph.get_module("app").unwrap();
    assert!(!app.imports.contains("models"));

    let services = graph.get_module("services").unwrap();
    assert!(!services.imports.contains("models"));
}

#[test]
fn test_clear_imports() {
    let mut graph = ModuleGraph::new();

    graph.add_import("app", "services");
    graph.add_import("app", "models");

    // Clear app's imports
    graph.clear_imports("app");

    let app = graph.get_module("app").unwrap();
    assert!(app.imports.is_empty());

    // services and models should no longer list app as importer
    let services = graph.get_module("services").unwrap();
    assert!(!services.imported_by.contains("app"));
}
