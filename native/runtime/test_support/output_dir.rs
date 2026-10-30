// Included only by test targets. No runtime fixture directory or authority.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct OutputDir { path: PathBuf, owned_base: Option<PathBuf> }
impl OutputDir {
    pub fn new(variable: &str, label: &str) -> Self {
        if let Some(path) = std::env::var_os(variable) {
            let path = PathBuf::from(path);
            assert!(path.is_absolute(), "explicit test output must be absolute");
            std::fs::create_dir_all(&path).expect("explicit isolated test output");
            return Self { path, owned_base: None };
        }
        Self::isolated(label)
    }
    fn isolated(label: &str) -> Self {
        assert!(label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
        let base = std::env::temp_dir().canonicalize().expect("canonical test temp directory");
        loop {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let epoch = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let path = base.join(format!("opl2s-test-{label}-{}-{epoch}-{n}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self { path, owned_base: Some(base) },
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("create isolated test output: {e}"),
            }
        }
    }
    pub fn path(&self) -> &Path { &self.path }
}
impl Drop for OutputDir {
    fn drop(&mut self) {
        if let Some(base) = &self.owned_base {
            let exact = self.path.canonicalize().expect("owned test directory still exists");
            assert_eq!(&exact, &self.path);
            assert_eq!(exact.parent(), Some(base.as_path()));
            assert!(exact.file_name().unwrap().to_string_lossy().starts_with("opl2s-test-"));
            std::fs::remove_dir_all(exact).expect("remove only owned isolated test output");
        }
    }
}
#[test]
fn owned_test_outputs_are_disjoint_and_cleanup_preserves_the_other() {
    let a = OutputDir::isolated("parallel");
    let b = OutputDir::isolated("parallel");
    assert_ne!(a.path(), b.path());
    let ap = a.path().to_path_buf();
    std::fs::write(a.path().join("marker"), b"a").unwrap();
    std::fs::write(b.path().join("marker"), b"b").unwrap();
    drop(a);
    assert!(!ap.exists());
    assert_eq!(std::fs::read(b.path().join("marker")).unwrap(), b"b");
}
