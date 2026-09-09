//! Read-only directory listing under a run's targetDir (IMP-024 / IMP-025).
//!
//! This module deliberately does not `use` `super::run_record`: it never sees a
//! `RunRecord`, `RunStatus` or `StageStatus`, only a `&Path` root and a `&str`
//! relative path. A module that cannot name the state types cannot change them
//! (TRD 9.5-(1), PRD G-4/G-5).

use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum ProjectFilesError {
    // 문자열 prefix 규약 — TRD 9.10. api.ts의 헬퍼가 이 접두사로만 구분할 수 있다.
    #[error("PATH_OUTSIDE_TARGET_DIR: {0}")]
    OutsideTargetDir(String),
    #[error("PATH_NOT_FOUND: {0}")]
    NotFound(String),
    #[error("path is not a directory: {0}")]
    NotADirectory(String),
    #[error("target dir is unavailable: {0}")]
    TargetDirUnavailable(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub kind: EntryKind,
    /// Files only; directories and unreadable entries are None. Taken from the
    /// `read_dir` entry's own metadata, which does not traverse symlinks, so a link's
    /// size never leaks the size of a target outside targetDir.
    pub size: Option<u64>,
    /// Unix epoch milliseconds. None when the platform does not report it.
    pub modified_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DirListing {
    /// targetDir-relative path, always '/'-separated. The root is "".
    /// Neither an absolute path nor a Windows `\\?\` prefix ever appears here.
    pub path: String,
    pub entries: Vec<DirEntry>,
}

/// Layer 1 of the containment defence (TRD 9.3-(2) step 1): a pure, filesystem-free
/// segment check that also produces the canonical *relative* form used in
/// `DirListing.path`.
///
/// `..` is rejected outright rather than resolved. `sub/../other` would land back
/// inside the root and pass layer 2, but a contract that allows some `..` and forbids
/// others is hard to test and hard to review (TRD 9.3-(3)).
pub fn normalized_relative(sub_path: &str) -> Result<String, ProjectFilesError> {
    let outside = || ProjectFilesError::OutsideTargetDir(sub_path.to_string());
    // Unix's Path::components() does not treat '\' as a separator, so normalize first
    // and the check becomes platform-independent.
    let slashed = sub_path.replace('\\', "/");
    // A leading '/' is a root (and '//server/share' is a UNC root).
    if slashed.starts_with('/') {
        return Err(outside());
    }
    let mut segments: Vec<String> = Vec::new();
    for component in Path::new(&slashed).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(raw) => {
                let segment = raw.to_string_lossy().to_string();
                // On Windows "C:/x" parses as Prefix and is caught below; on Unix it
                // parses as a Normal segment "C:", so reject drive-ish segments here to
                // keep the behaviour identical on both.
                if segment.contains(':') {
                    return Err(outside());
                }
                segments.push(segment);
            }
            // ParentDir, RootDir, Prefix (Windows `C:` / UNC).
            _ => return Err(outside()),
        }
    }
    Ok(segments.join("/"))
}

/// Layers 1-5 of TRD 9.3-(2). Returns the canonical absolute path of the requested
/// directory. The canonical value is used only for the check and the `read_dir` call —
/// it never reaches the frontend (TRD 9.3-(5)).
pub async fn resolve_within(root: &Path, sub_path: &str) -> Result<PathBuf, ProjectFilesError> {
    let relative = normalized_relative(sub_path)?;

    let canonical_root = tokio::fs::canonicalize(root)
        .await
        .map_err(|_| ProjectFilesError::TargetDirUnavailable(root.to_string_lossy().to_string()))?;
    let joined = canonical_root.join(&relative);
    let canonical = tokio::fs::canonicalize(&joined)
        .await
        .map_err(|_| ProjectFilesError::NotFound(relative.clone()))?;

    // Layer 2 (TRD 9.3-(2) step 4): both sides are canonical, so Windows' `\\?\` prefix
    // matches on both and a symlink that points out of the root is now visible.
    // Path::starts_with compares components, not bytes (TRD 9.3-(4)).
    if !canonical.starts_with(&canonical_root) {
        return Err(ProjectFilesError::OutsideTargetDir(relative));
    }

    let metadata = tokio::fs::metadata(&canonical)
        .await
        .map_err(|_| ProjectFilesError::NotFound(relative.clone()))?;
    if !metadata.is_dir() {
        return Err(ProjectFilesError::NotADirectory(relative));
    }

    Ok(canonical)
}

/// Lists exactly one directory level. Deliberately non-recursive (TRD 9.2-(5)):
/// recursion invites symlink loops, it would make GAP-F2's blast radius unbounded, and
/// a flat list plus a breadcrumb needs no tree library.
pub async fn list_dir(root: &Path, sub_path: &str) -> Result<DirListing, ProjectFilesError> {
    let relative = normalized_relative(sub_path)?;
    let dir = resolve_within(root, sub_path).await?;

    let mut read_dir = tokio::fs::read_dir(&dir).await?;
    let mut entries: Vec<DirEntry> = Vec::new();
    while let Some(entry) = read_dir.next_entry().await? {
        let name = entry.file_name().to_string_lossy().to_string();
        // `file_type()` on a directory entry does not follow the link, so a symlink is
        // reported as itself (PRD G-1(b) support).
        let file_type = entry.file_type().await?;
        let kind = if file_type.is_symlink() {
            EntryKind::Symlink
        } else if file_type.is_dir() {
            EntryKind::Directory
        } else if file_type.is_file() {
            EntryKind::File
        } else {
            EntryKind::Other
        };
        let metadata = entry.metadata().await.ok();
        let size = match (kind, &metadata) {
            (EntryKind::File, Some(m)) => Some(m.len()),
            _ => None,
        };
        let modified_ms = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64);
        entries.push(DirEntry { name, kind, size, modified_ms });
    }

    entries.sort_by(|a, b| {
        (a.kind != EntryKind::Directory, a.name.as_str())
            .cmp(&(b.kind != EntryKind::Directory, b.name.as_str()))
    });

    Ok(DirListing { path: relative, entries })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_outside(result: Result<PathBuf, ProjectFilesError>, input: &str) {
        match result {
            Err(ProjectFilesError::OutsideTargetDir(_)) => {}
            other => panic!("expected OutsideTargetDir for {input:?}, got {other:?}"),
        }
    }

    /// U-G1a — PRD G-1(a). The segment check runs before any filesystem access, so a
    /// root that does not exist still yields a deterministic OutsideTargetDir.
    #[tokio::test]
    async fn rejects_parent_dir_escape() {
        let root = Path::new("/nonexistent-root-for-unit-test");
        for input in ["../secret.txt", "sub/../../secret.txt", "..", "sub\\..\\..\\secret.txt"] {
            assert_outside(resolve_within(root, input).await, input);
        }
    }

    /// U-G1c — PRD G-1(c). Absolute paths and UNC roots are rejected on every platform,
    /// not just the one whose separator they use.
    #[tokio::test]
    async fn rejects_absolute_path_injection() {
        let root = Path::new("/nonexistent-root-for-unit-test");
        for input in ["C:\\Windows\\win.ini", "/etc/passwd", "\\\\server\\share", "C:/Windows/win.ini"] {
            assert_outside(resolve_within(root, input).await, input);
        }
    }

    /// U-G2 — PRD G-2 / TRD 9.3-(4). A string `starts_with` would let `proj-evil` pass
    /// as a child of `proj`. `Path::starts_with` compares whole components, so it does
    /// not. This test runs on every platform and is the standing guard for the symlink
    /// defence when U-G1b has to skip (see its note).
    #[tokio::test]
    async fn canonical_prefix_comparison_is_component_wise() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path().join("proj");
        let evil = tmp.path().join("proj-evil");
        std::fs::create_dir(&proj).unwrap();
        std::fs::create_dir(&evil).unwrap();

        let canonical_root = tokio::fs::canonicalize(&proj).await.unwrap();
        let canonical_evil = tokio::fs::canonicalize(&evil).await.unwrap();

        assert!(!canonical_evil.starts_with(&canonical_root));
        // And the string form is exactly the trap this guards against.
        assert!(canonical_evil.to_string_lossy().starts_with(&*canonical_root.to_string_lossy()));
    }

    /// A root with: two dirs, two files, and the manifest dir the app always writes.
    fn listing_fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("src")).unwrap();
        std::fs::create_dir(tmp.path().join("assets")).unwrap();
        std::fs::create_dir(tmp.path().join(".claude-pipeline-wizard")).unwrap();
        std::fs::write(tmp.path().join("main.rs"), b"fn main() {}").unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), b"x").unwrap();
        std::fs::write(tmp.path().join("src").join("lib.rs"), b"pub fn a() {}").unwrap();
        tmp
    }

    fn names(listing: &DirListing) -> Vec<&str> {
        listing.entries.iter().map(|e| e.name.as_str()).collect()
    }

    /// U-G-s1 — the sort contract: directories first, then everything else, each group
    /// ordered by name.
    #[tokio::test]
    async fn lists_directories_first_then_files_each_sorted_by_name() {
        let tmp = listing_fixture();
        let listing = list_dir(tmp.path(), "").await.unwrap();
        assert_eq!(
            names(&listing),
            [".claude-pipeline-wizard", "assets", "src", "Cargo.toml", "main.rs"]
        );
    }

    /// U-G-s2
    #[tokio::test]
    async fn reports_size_and_mtime_for_files_and_none_for_directories() {
        let tmp = listing_fixture();
        let listing = list_dir(tmp.path(), "").await.unwrap();
        let file = listing.entries.iter().find(|e| e.name == "main.rs").unwrap();
        let dir = listing.entries.iter().find(|e| e.name == "src").unwrap();
        assert_eq!(file.kind, EntryKind::File);
        assert_eq!(file.size, Some(12));
        assert!(file.modified_ms.unwrap() > 0);
        assert_eq!(dir.kind, EntryKind::Directory);
        assert_eq!(dir.size, None);
    }

    /// U-G-s3 — TRD 9.8: the backend filters nothing. Hiding `.claude-pipeline-wizard/`
    /// is a render-layer decision, so all three GAP-F5 options stay open.
    #[tokio::test]
    async fn lists_the_manifest_directory_too() {
        let tmp = listing_fixture();
        let listing = list_dir(tmp.path(), "").await.unwrap();
        assert!(names(&listing).contains(&".claude-pipeline-wizard"));
    }

    /// U-G-s4
    #[tokio::test]
    async fn empty_sub_path_lists_the_target_dir_itself_with_path_empty_string() {
        let tmp = listing_fixture();
        let root = list_dir(tmp.path(), "").await.unwrap();
        assert_eq!(root.path, "");
        // And a nested listing reports its normalized relative path, never an absolute
        // one and never a `\\?\` prefix.
        let nested = list_dir(tmp.path(), "src").await.unwrap();
        assert_eq!(nested.path, "src");
        assert_eq!(names(&nested), ["lib.rs"]);
        let nested_backslash = list_dir(tmp.path(), ".\\src").await.unwrap();
        assert_eq!(nested_backslash.path, "src");
    }

    /// U-G-s5
    #[tokio::test]
    async fn nonexistent_sub_path_maps_to_not_found() {
        let tmp = listing_fixture();
        match list_dir(tmp.path(), "nope").await {
            Err(ProjectFilesError::NotFound(_)) => {}
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    /// U-G-s5 (second half)
    #[tokio::test]
    async fn file_sub_path_maps_to_not_a_directory() {
        let tmp = listing_fixture();
        match list_dir(tmp.path(), "main.rs").await {
            Err(ProjectFilesError::NotADirectory(_)) => {}
            other => panic!("expected NotADirectory, got {other:?}"),
        }
    }

    /// U-G-s6 — PRD G-19. These key strings are what `src/types.ts` mirrors by hand;
    /// renaming a Rust field breaks this test, which is the signal to fix the mirror.
    #[tokio::test]
    async fn dir_listing_serializes_with_the_camel_case_keys_the_ts_mirror_expects() {
        let listing = DirListing {
            path: "src".to_string(),
            entries: vec![
                DirEntry { name: "engine".into(), kind: EntryKind::Directory, size: None, modified_ms: None },
                DirEntry { name: "main.rs".into(), kind: EntryKind::File, size: Some(12), modified_ms: Some(1) },
            ],
        };
        let json = serde_json::to_string(&listing).unwrap();
        assert_eq!(
            json,
            r#"{"path":"src","entries":[{"name":"engine","kind":"directory","size":null,"modifiedMs":null},{"name":"main.rs","kind":"file","size":12,"modifiedMs":1}]}"#
        );
    }
}
