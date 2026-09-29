use super::*;
use crate::domain::checker_config::argus_config::PythonEnvConfig;
use std::fs;

#[test]
fn test_venv_type_display() {
    assert_eq!(VenvType::Venv.to_string(), "venv");
    assert_eq!(VenvType::Poetry.to_string(), "poetry");
    assert_eq!(VenvType::Pipenv.to_string(), "pipenv");
    assert_eq!(VenvType::Conda.to_string(), "conda");
    assert_eq!(VenvType::Unknown.to_string(), "unknown");
}

#[test]
fn test_is_venv_directory_with_pyvenv_cfg() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_venv_cfg");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Not a venv initially
    assert!(!is_venv_directory(&temp_dir));

    // Add pyvenv.cfg
    fs::write(temp_dir.join("pyvenv.cfg"), "version = 3.11.0").unwrap();
    assert!(is_venv_directory(&temp_dir));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_is_venv_directory_with_structure() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_venv_struct");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Not a venv initially
    assert!(!is_venv_directory(&temp_dir));

    // Add venv structure (Unix-style)
    fs::create_dir_all(temp_dir.join("bin")).unwrap();
    fs::create_dir_all(temp_dir.join("lib/python3.11/site-packages")).unwrap();
    fs::write(temp_dir.join("bin/python"), "").unwrap();

    assert!(is_venv_directory(&temp_dir));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_find_site_packages() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_site_packages");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create Unix-style structure
    let site_packages_path = temp_dir.join("lib/python3.11/site-packages");
    fs::create_dir_all(&site_packages_path).unwrap();

    let found = find_site_packages(&temp_dir, None);
    assert!(found.is_some());
    assert!(found.unwrap().ends_with("site-packages"));

    // Test with specific version
    let found_specific = find_site_packages(&temp_dir, Some("3.11"));
    assert!(found_specific.is_some());

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_find_site_packages_windows_style() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_site_packages_win");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create Windows-style structure
    let site_packages_path = temp_dir.join("Lib/site-packages");
    fs::create_dir_all(&site_packages_path).unwrap();

    let found = find_site_packages(&temp_dir, None);
    assert!(found.is_some());
    assert!(found.unwrap().ends_with("site-packages"));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_detect_all_venvs_common_names() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_detect_venvs");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create .venv directory with structure
    let venv_path = temp_dir.join(".venv");
    fs::create_dir_all(venv_path.join("bin")).unwrap();
    fs::create_dir_all(venv_path.join("lib")).unwrap();
    fs::write(venv_path.join("pyvenv.cfg"), "version = 3.11.0").unwrap();

    let envs = detect_all_venvs(&temp_dir);
    assert!(!envs.is_empty());
    assert!(envs.iter().any(|e| e.path.ends_with(".venv")));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_detect_python_environment_with_config() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_env_config");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a venv
    let venv_path = temp_dir.join("custom_venv");
    fs::create_dir_all(venv_path.join("lib/python3.10/site-packages")).unwrap();
    fs::write(venv_path.join("pyvenv.cfg"), "version = 3.10.0").unwrap();

    // Create config with explicit venv_path
    let config = ArgusConfig {
        python_version: Some("3.10".to_string()),
        python: PythonEnvConfig {
            venv_path: Some(PathBuf::from("custom_venv")),
            ..Default::default()
        },
        ..Default::default()
    };

    let info = detect_with_config(&temp_dir, &config);
    assert!(info.active_venv.is_some());
    assert!(info
        .active_venv
        .as_ref()
        .unwrap()
        .path
        .ends_with("custom_venv"));
    assert_eq!(info.python_version, Some("3.10".to_string()));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_detect_python_environment_with_search_paths() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_search_paths");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create source directories
    fs::create_dir_all(temp_dir.join("src")).unwrap();
    fs::create_dir_all(temp_dir.join("lib")).unwrap();

    let config = ArgusConfig {
        python: PythonEnvConfig {
            search_paths: vec![PathBuf::from("src"), PathBuf::from("lib")],
            ..Default::default()
        },
        ..Default::default()
    };

    let info = detect_with_config(&temp_dir, &config);

    // Should include the configured search paths
    assert!(info.search_paths.iter().any(|p| p.ends_with("src")));
    assert!(info.search_paths.iter().any(|p| p.ends_with("lib")));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_get_venv_python_version_from_pyvenv_cfg() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_py_version");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    fs::write(
        temp_dir.join("pyvenv.cfg"),
        "home = /usr/bin\nversion = 3.11.4\n",
    )
    .unwrap();

    let version = get_venv_python_version(&temp_dir);
    assert_eq!(version, Some("3.11".to_string()));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_detect_poetry_project() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_poetry");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create poetry.lock (marker for poetry project)
    fs::write(temp_dir.join("poetry.lock"), "[[package]]").unwrap();

    // Create in-project venv
    let venv_path = temp_dir.join(".venv");
    fs::create_dir_all(venv_path.join("lib")).unwrap();
    fs::write(venv_path.join("pyvenv.cfg"), "version = 3.11.0").unwrap();

    let envs = detect_all_venvs(&temp_dir);

    // Should find the venv (Poetry type detected when poetry.lock exists)
    assert!(!envs.is_empty());

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_ignore_site_packages() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_ignore_site");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a venv
    let venv_path = temp_dir.join(".venv");
    let site_packages = venv_path.join("lib/python3.11/site-packages");
    fs::create_dir_all(&site_packages).unwrap();
    fs::write(venv_path.join("pyvenv.cfg"), "version = 3.11.0").unwrap();

    // Config with ignore_site_packages = true
    let config = ArgusConfig {
        python: PythonEnvConfig {
            ignore_site_packages: true,
            ..Default::default()
        },
        ..Default::default()
    };

    let info = detect_with_config(&temp_dir, &config);

    // Should NOT include site-packages in search paths
    assert!(!info
        .search_paths
        .iter()
        .any(|p| p.ends_with("site-packages")));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

/// Acceptance Criteria: WHEN PYTHONPATH is set THEN include in search paths
/// Spec: python-env.md#acceptance-criteria
#[test]
fn test_pythonpath_in_search_paths() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_pythonpath");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create the directory that PYTHONPATH will point to
    let extra_lib = temp_dir.join("extra_lib");
    fs::create_dir_all(&extra_lib).unwrap();

    // We need to temporarily set PYTHONPATH for this test
    // Note: This test manipulates env vars which could affect other tests if run in parallel
    let original_pythonpath = env::var("PYTHONPATH").ok();
    env::set_var("PYTHONPATH", extra_lib.to_string_lossy().to_string());

    let config = ArgusConfig::default();
    let info = detect_with_config(&temp_dir, &config);

    // PYTHONPATH should be included in search paths
    assert!(
        info.search_paths.iter().any(|p| p == &extra_lib),
        "PYTHONPATH '{}' should be in search_paths: {:?}",
        extra_lib.display(),
        info.search_paths
    );

    // Restore original PYTHONPATH
    match original_pythonpath {
        Some(val) => env::set_var("PYTHONPATH", val),
        None => env::remove_var("PYTHONPATH"),
    }

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

/// Acceptance Criteria: Configuration priority test
/// Spec: python-env.md#r1-configuration-priority
#[test]
fn test_config_priority_explicit_over_auto() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_config_priority");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create two venvs: one that will be auto-detected, one configured explicitly
    let auto_venv = temp_dir.join(".venv");
    fs::create_dir_all(auto_venv.join("lib/python3.11/site-packages")).unwrap();
    fs::write(auto_venv.join("pyvenv.cfg"), "version = 3.11.0").unwrap();

    let explicit_venv = temp_dir.join("my_custom_env");
    fs::create_dir_all(explicit_venv.join("lib/python3.10/site-packages")).unwrap();
    fs::write(explicit_venv.join("pyvenv.cfg"), "version = 3.10.0").unwrap();

    // Explicit config should take priority over auto-detected .venv
    let config = ArgusConfig {
        python: PythonEnvConfig {
            venv_path: Some(PathBuf::from("my_custom_env")),
            ..Default::default()
        },
        ..Default::default()
    };

    let info = detect_with_config(&temp_dir, &config);

    // Active venv should be the explicitly configured one
    assert!(info.active_venv.is_some());
    assert!(
        info.active_venv
            .as_ref()
            .unwrap()
            .path
            .ends_with("my_custom_env"),
        "Expected my_custom_env, got: {:?}",
        info.active_venv.as_ref().unwrap().path
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

/// Test detecting Pipenv managed environment
#[test]
fn test_detect_pipenv_project() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_pipenv");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create Pipfile (marker for pipenv project)
    fs::write(temp_dir.join("Pipfile"), "[packages]").unwrap();

    // Create in-project venv style
    let venv_path = temp_dir.join(".venv");
    fs::create_dir_all(venv_path.join("lib")).unwrap();
    fs::write(venv_path.join("pyvenv.cfg"), "version = 3.11.0").unwrap();

    let envs = detect_all_venvs(&temp_dir);

    // Should find at least one environment
    assert!(!envs.is_empty());

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

/// Test that site_packages is included in active venv info
#[test]
fn test_active_venv_has_site_packages() {
    let temp_dir = env::temp_dir().join("cclab_lens_test_active_site_pkgs");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    // Create a venv with site-packages
    let venv_path = temp_dir.join(".venv");
    let site_packages = venv_path.join("lib/python3.11/site-packages");
    fs::create_dir_all(&site_packages).unwrap();
    fs::write(venv_path.join("pyvenv.cfg"), "version = 3.11.0").unwrap();

    // Create a dummy package in site-packages
    fs::create_dir_all(site_packages.join("requests")).unwrap();
    fs::write(site_packages.join("requests/__init__.py"), "").unwrap();

    let config = ArgusConfig::default();
    let info = detect_with_config(&temp_dir, &config);

    // Should have active venv with site-packages
    assert!(info.active_venv.is_some());
    let active = info.active_venv.as_ref().unwrap();
    assert!(active.site_packages.is_some());
    assert!(active
        .site_packages
        .as_ref()
        .unwrap()
        .ends_with("site-packages"));

    // Site-packages should be in search paths
    assert!(
        info.search_paths
            .iter()
            .any(|p| p.ends_with("site-packages")),
        "site-packages should be in search_paths: {:?}",
        info.search_paths
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}
