use super::*;
use std::collections::HashSet;
use std::fs;
use tempfile::TempDir;

fn write_py(dir: &Path, name: &str, content: &str) -> PathBuf {
    let p = dir.join(name);
    if let Some(par) = p.parent() {
        fs::create_dir_all(par).unwrap();
    }
    fs::write(&p, content).unwrap();
    p
}

#[test]
fn test_build_chain_a_b_c() {
    let tmp = TempDir::new().unwrap();
    let r = tmp.path();
    let a = write_py(r, "a.py", "from .b import x");
    let b = write_py(r, "b.py", "from .c import y");
    let c = write_py(r, "c.py", "x = 1");
    let files = vec![
        (a.clone(), fs::read_to_string(&a).unwrap()),
        (b.clone(), fs::read_to_string(&b).unwrap()),
        (c.clone(), fs::read_to_string(&c).unwrap()),
    ];
    let g = ImportGraph::build(&files, r);
    assert_eq!(g.file_count(), 3);
    assert_eq!(g.edge_count(), 2);
    assert_eq!(g.dependencies(&a)[0].resolved, Some(b.clone()));
    assert_eq!(g.dependencies(&b)[0].resolved, Some(c.clone()));
    assert!(g.dependencies(&c).is_empty());
}

#[test]
fn test_circular_dependency() {
    let tmp = TempDir::new().unwrap();
    let r = tmp.path();
    let a = write_py(r, "a.py", "from .b import x");
    let b = write_py(r, "b.py", "from .c import y");
    let c = write_py(r, "c.py", "from .a import z");
    let files = vec![
        (a.clone(), fs::read_to_string(&a).unwrap()),
        (b.clone(), fs::read_to_string(&b).unwrap()),
        (c.clone(), fs::read_to_string(&c).unwrap()),
    ];
    let g = ImportGraph::build(&files, r);
    let cycles = g.find_circular_dependencies();
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0].len(), 3);
    let set: HashSet<_> = cycles[0].iter().collect();
    assert!(set.contains(&a) && set.contains(&b) && set.contains(&c));
}

#[test]
fn test_unused_file_detection() {
    let tmp = TempDir::new().unwrap();
    let r = tmp.path();
    let entry = write_py(r, "main.py", "from .a import x");
    let a = write_py(r, "a.py", "from .b import y");
    let _b = write_py(r, "b.py", "z = 1");
    let orphan = write_py(r, "orphan.py", "lonely = True");
    let files = vec![
        (entry.clone(), fs::read_to_string(&entry).unwrap()),
        (a.clone(), fs::read_to_string(&a).unwrap()),
        (_b.clone(), fs::read_to_string(&_b).unwrap()),
        (orphan.clone(), fs::read_to_string(&orphan).unwrap()),
    ];
    let g = ImportGraph::build(&files, r);
    assert!(g.entry_points().contains(&entry));
    let unused = g.find_unused_files();
    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0], orphan);
}

#[test]
fn test_entry_point_detection() {
    let tmp = TempDir::new().unwrap();
    let r = tmp.path();
    let mp = write_py(r, "main.py", "");
    let it = write_py(r, "index.ts", "");
    let lr = write_py(r, "lib.rs", "");
    let ut = write_py(r, "utils.py", "");
    let files = vec![
        (mp.clone(), String::new()),
        (it.clone(), String::new()),
        (lr.clone(), String::new()),
        (ut.clone(), String::new()),
    ];
    let g = ImportGraph::build(&files, r);
    let ep: HashSet<_> = g.entry_points().iter().collect();
    assert!(ep.contains(&mp) && ep.contains(&it) && ep.contains(&lr));
    assert!(!ep.contains(&ut));
}

#[test]
fn test_incremental_add_remove() {
    let tmp = TempDir::new().unwrap();
    let r = tmp.path();
    let a = write_py(r, "a.py", "x = 1");
    let b = write_py(r, "b.py", "from .a import x");
    let mut g = ImportGraph::new();
    g.add_file(a.clone(), &fs::read_to_string(&a).unwrap(), r);
    assert_eq!(g.file_count(), 1);
    g.add_file(b.clone(), &fs::read_to_string(&b).unwrap(), r);
    assert_eq!(g.file_count(), 2);
    assert_eq!(g.edge_count(), 1);
    assert_eq!(g.dependencies(&b)[0].resolved, Some(a.clone()));
    assert_eq!(g.dependents(&a), vec![b.clone()]);
    g.remove_file(&a);
    assert_eq!(g.file_count(), 1);
    assert!(g.dependencies(&a).is_empty());
}
