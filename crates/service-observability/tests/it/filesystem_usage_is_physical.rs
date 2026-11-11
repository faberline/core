// HANDWRITE-BEGIN gap="missing-generator:e2e:filesystem-usage-physical" tracker="#2947" reason="End-to-end evidence that filesystem usage reads physical volume storage and tracks writes."
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use service_observability::filesystem_usage;

struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    fn new(prefix: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id();
        let dir_name = format!("service_obs_fs_{prefix}_{pid}_{nanos}_{count}");
        let path = std::env::temp_dir().join(dir_name);
        fs::create_dir_all(&path).expect("create scratch directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

static PHYSICAL_WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn lock_physical_write() -> std::sync::MutexGuard<'static, ()> {
    PHYSICAL_WRITE_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn write_16_mib(dir: &Path) {
    let file_path = dir.join("16mib_payload.bin");
    let mut file = fs::File::create(&file_path).expect("create test payload file");
    let chunk = vec![0xAB_u8; 1024 * 1024]; // 1 MiB chunk
    for _ in 0..16 {
        file.write_all(&chunk).expect("write chunk");
    }
    file.sync_all().expect("sync file to disk");
}

const ATTEMPTS: usize = 3;

/// Calls `measure` up to `attempts` times and returns the first value within
/// `min..=max`. When no attempt is in range, returns every value seen.
fn first_in_range(
    attempts: usize,
    mut measure: impl FnMut() -> u64,
    min: u64,
    max: u64,
) -> Result<u64, Vec<u64>> {
    let mut seen = Vec::new();
    for _ in 0..attempts {
        let value = measure();
        if (min..=max).contains(&value) {
            return Ok(value);
        }
        seen.push(value);
    }
    Err(seen)
}

#[test]
fn total_bytes_agrees_with_an_independent_df_reading() {
    let scratch = ScratchDir::new("df_agreement");
    let sample =
        filesystem_usage(scratch.path()).expect("filesystem_usage should succeed on scratch dir");

    let output = Command::new("df")
        .args(["-k", "-P"])
        .arg(scratch.path())
        .output()
        .expect("df -k -P should execute");
    assert!(
        output.status.success(),
        "df exited with failure: {:?}",
        output.status
    );

    let stdout = String::from_utf8(output.stdout).expect("df output is UTF-8");
    let lines = stdout.lines().collect::<Vec<_>>();
    assert!(
        lines.len() >= 2,
        "df output should have at least header and data line: {stdout}"
    );
    let fields = lines[1].split_whitespace().collect::<Vec<_>>();
    assert!(
        fields.len() >= 2,
        "df data line should have at least 2 fields: {}",
        lines[1]
    );
    let df_total_kib: u64 = fields[1].parse().expect("df 1024-blocks field is numeric");
    let expected_total_bytes = df_total_kib * 1024;

    assert_eq!(
        sample.total_bytes, expected_total_bytes,
        "total_bytes ({}) did not match df reading ({})",
        sample.total_bytes, expected_total_bytes
    );
}

#[test]
fn available_bytes_tracks_a_physical_write() {
    let _guard = lock_physical_write();
    let scratch = ScratchDir::new("physical_write");
    const EIGHT_MIB: u64 = 8 * 1024 * 1024;
    const SIXTY_FOUR_MIB: u64 = 64 * 1024 * 1024;

    let result = first_in_range(
        ATTEMPTS,
        || {
            let before = filesystem_usage(scratch.path()).expect("initial sample");
            write_16_mib(scratch.path());
            let after = filesystem_usage(scratch.path()).expect("post-write sample");
            let _ = fs::remove_file(scratch.path().join("16mib_payload.bin"));
            before.available_bytes.saturating_sub(after.available_bytes)
        },
        EIGHT_MIB,
        SIXTY_FOUR_MIB,
    );
    if let Err(seen) = result {
        panic!(
            "available_bytes_tracks_a_physical_write: drops {seen:?} were not in 8 MiB..=64 MiB"
        );
    }
}

#[test]
fn used_bytes_tracks_a_physical_write() {
    let _guard = lock_physical_write();
    let scratch = ScratchDir::new("used_physical_write");
    const EIGHT_MIB: u64 = 8 * 1024 * 1024;
    const SIXTY_FOUR_MIB: u64 = 64 * 1024 * 1024;

    let result = first_in_range(
        ATTEMPTS,
        || {
            let before = filesystem_usage(scratch.path()).expect("initial sample");
            write_16_mib(scratch.path());
            let after = filesystem_usage(scratch.path()).expect("post-write sample");
            let _ = fs::remove_file(scratch.path().join("16mib_payload.bin"));
            after.used_bytes.saturating_sub(before.used_bytes)
        },
        EIGHT_MIB,
        SIXTY_FOUR_MIB,
    );
    if let Err(seen) = result {
        panic!("used_bytes_tracks_a_physical_write: rises {seen:?} were not in 8 MiB..=64 MiB");
    }
}

#[test]
fn first_in_range_returns_first_value_in_range() {
    let values = std::cell::RefCell::new(vec![0_u64, 9 * 1024 * 1024].into_iter());
    let calls = std::cell::Cell::new(0_usize);
    let result = first_in_range(
        ATTEMPTS,
        || {
            calls.set(calls.get() + 1);
            values
                .borrow_mut()
                .next()
                .expect("measure called too often")
        },
        8 * 1024 * 1024,
        64 * 1024 * 1024,
    );
    assert_eq!(result, Ok(9 * 1024 * 1024));
    assert_eq!(calls.get(), 2);
}

#[test]
fn first_in_range_never_passes_on_zero_change() {
    let result = first_in_range(ATTEMPTS, || 0, 8 * 1024 * 1024, 64 * 1024 * 1024);
    assert_eq!(result, Err(vec![0, 0, 0]));
}

#[test]
fn missing_path_is_an_error_not_a_zeroed_sample() {
    let scratch = ScratchDir::new("missing_path");
    let missing = scratch.path().join("never_created_path");
    let result = filesystem_usage(&missing);
    assert!(
        result.is_err(),
        "expected Err for non-existent path, got Ok: {:?}",
        result
    );
}
// HANDWRITE-END
