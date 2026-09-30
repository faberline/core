//! The filesystem-writing CLI entry: read the spec, generate, write the files.

use crate::application::generate;
use crate::domain::{GenOptions, MANIFEST_FILE};

/// CLI entry: read spec, generate, write files. Returns a process exit code
/// (0 ok, 1 generation/write error, 2 spec read error).
pub fn run(opts: &GenOptions) -> i32 {
    let spec_json = match std::fs::read_to_string(&opts.spec_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "openapi-codegen: cannot read {}: {e}",
                opts.spec_path.display()
            );
            return 2;
        }
    };
    let output = match generate(&spec_json, opts) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("openapi-codegen: {e:#}");
            return 1;
        }
    };
    if let Err(e) = output.write_to_dir(&opts.out_dir) {
        eprintln!(
            "openapi-codegen: cannot write generated output to {}: {e}",
            opts.out_dir.display()
        );
        return 1;
    }
    for file in &output.files {
        println!("generated {}", opts.out_dir.join(&file.rel_path).display());
    }
    if output.target.is_some() {
        println!("generated {}", opts.out_dir.join(MANIFEST_FILE).display());
    }
    0
}
