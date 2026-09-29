use std::path::PathBuf;

use crate::domain::cross_file::inferencer::DeepTypeInferencer;

impl DeepTypeInferencer {
    /// Resolve import path using virtual environment
    ///
    /// Checks if a module exists in the virtual environment's site-packages.
    /// This is useful for resolving imports to third-party packages.
    pub fn resolve_import_path(&self, module: &str) -> Option<PathBuf> {
        if let Some(venv_path) = &self.venv_path {
            // Try lib/pythonX.Y/site-packages (Unix)
            let site_packages_patterns = vec![
                venv_path.join("lib/python3.12/site-packages"),
                venv_path.join("lib/python3.11/site-packages"),
                venv_path.join("lib/python3.10/site-packages"),
                venv_path.join("lib/python3.9/site-packages"),
                // Windows
                venv_path.join("Lib/site-packages"),
            ];

            for site_packages in site_packages_patterns {
                if site_packages.exists() {
                    // Try as module file: module.py
                    let module_file =
                        site_packages.join(format!("{}.py", module.replace(".", "/")));
                    if module_file.exists() {
                        return Some(module_file);
                    }

                    // Try as package: module/__init__.py
                    let package_dir = site_packages.join(module.replace(".", "/"));
                    let init_file = package_dir.join("__init__.py");
                    if init_file.exists() {
                        return Some(init_file);
                    }
                }
            }
        }

        None
    }
}
