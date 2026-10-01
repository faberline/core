use super::*;
use std::sync::Mutex;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod commit;
mod current;
mod inherited;
mod initialize;
mod store;

#[derive(Clone, Debug)]
struct InjectedFailure {
    point: FailurePoint,
    kind: io::ErrorKind,
}

#[derive(Default)]
struct RecordingInjector {
    points: Mutex<Vec<FailurePoint>>,
    failure: Mutex<Option<InjectedFailure>>,
}

impl RecordingInjector {
    fn points(&self) -> Vec<FailurePoint> {
        self.points.lock().unwrap().clone()
    }

    fn fail_at(&self, point: FailurePoint, kind: io::ErrorKind) {
        *self.failure.lock().unwrap() = Some(InjectedFailure { point, kind });
    }

    fn clear(&self) {
        self.points.lock().unwrap().clear();
        *self.failure.lock().unwrap() = None;
    }
}

impl FailureInjector for RecordingInjector {
    fn check(&self, point: &FailurePoint) -> io::Result<()> {
        self.points.lock().unwrap().push(point.clone());
        let failure = self.failure.lock().unwrap().clone();
        if failure
            .as_ref()
            .is_some_and(|failure| failure.point == *point)
        {
            return Err(io::Error::from(failure.unwrap().kind));
        }
        Ok(())
    }
}

fn name(value: &str) -> GenerationName {
    GenerationName::parse(value).unwrap()
}

fn seed_current(root: &Path, generation: &str) {
    let generation_path = root.join(generation);
    std::fs::create_dir(&generation_path).unwrap();
    std::fs::write(generation_path.join("payload"), generation.as_bytes()).unwrap();
    std::fs::write(
        root.join(CURRENT_FILE_NAME),
        format!("generation:{generation}\n"),
    )
    .unwrap();
}

fn stage_fixture(store: &GenerationStore, generation: &str) -> StagedGeneration {
    let staged = store.begin(name(generation)).unwrap();
    std::fs::write(staged.path().join("z.txt"), b"z").unwrap();
    std::fs::write(staged.path().join("a.txt"), b"a").unwrap();
    std::fs::create_dir(staged.path().join("b")).unwrap();
    std::fs::write(staged.path().join("b/m.txt"), b"m").unwrap();
    std::fs::create_dir(staged.path().join("b/c")).unwrap();
    std::fs::write(staged.path().join("b/c/n.txt"), b"n").unwrap();
    std::fs::create_dir(staged.path().join("a-dir")).unwrap();
    staged
}

fn instrumented_fixture(
    stale_temp: bool,
) -> (
    tempfile::TempDir,
    Arc<RecordingInjector>,
    GenerationStore,
    StagedGeneration,
) {
    let directory = tempfile::tempdir().unwrap();
    seed_current(directory.path(), "old");
    if stale_temp {
        std::fs::write(directory.path().join(CURRENT_TEMP_FILE_NAME), b"stale").unwrap();
    }
    let injector = Arc::new(RecordingInjector::default());
    let store = GenerationStore::open_with_injector(directory.path(), injector.clone()).unwrap();
    let staged = stage_fixture(&store, "new");
    (directory, injector, store, staged)
}

fn current_generation(store: &GenerationStore) -> String {
    match store.read_current().unwrap() {
        CurrentTarget::Generation(generation) => generation.as_str().to_owned(),
        CurrentTarget::Empty => panic!("expected named generation"),
    }
}

fn successful_points(stale_temp: bool) -> Vec<FailurePoint> {
    let (_directory, injector, store, staged) = instrumented_fixture(stale_temp);
    store.commit(staged).unwrap();
    injector.points()
}
