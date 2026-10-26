//! Per-project markdown notes path resolution (local files only).

use crate::NotesConfig;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteRef {
    pub project_path: PathBuf,
    pub note_path: PathBuf,
}

/// Resolve the markdown file path for a project working tree.
pub fn resolve_note_path(notes: &NotesConfig, project_path: &Path) -> PathBuf {
    if let Some(root) = notes.central_root.as_ref() {
        let stem = central_filename(project_path);
        root.join(stem)
    } else {
        project_path.join(&notes.relative_path)
    }
}

fn central_filename(project_path: &Path) -> String {
    let canonical = project_path
        .canonicalize()
        .unwrap_or_else(|_| project_path.to_path_buf());
    let slug = canonical
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().replace(' ', "-")),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("_");
    format!("{slug}.md")
}

/// Build note refs for each discovered project path.
pub fn note_refs_for_projects(notes: &NotesConfig, project_paths: &[PathBuf]) -> Vec<NoteRef> {
    project_paths
        .iter()
        .map(|p| NoteRef {
            project_path: p.clone(),
            note_path: resolve_note_path(notes, p),
        })
        .collect()
}

/// Create parent dirs and an empty note file when missing.
pub fn ensure_note_file(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !path.exists() {
        std::fs::write(path, "")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn per_project_relative_path() {
        let notes = NotesConfig::default();
        let p = PathBuf::from("/tmp/my-repo");
        assert_eq!(
            resolve_note_path(&notes, &p),
            PathBuf::from("/tmp/my-repo/.cdamux/notes.md")
        );
    }

    #[test]
    fn central_root_uses_slug() {
        let tmp = tempdir().unwrap();
        let repo = tmp.path().join("my-repo");
        fs::create_dir(&repo).unwrap();
        let notes = NotesConfig {
            central_root: Some(tmp.path().join("central")),
            relative_path: PathBuf::from(".cdamux/notes.md"),
            editor: String::new(),
        };
        let resolved = resolve_note_path(&notes, &repo);
        let central = notes.central_root.as_ref().unwrap();
        assert_eq!(resolved.parent().map(Path::to_path_buf), Some(central.clone()));
        assert!(resolved.to_string_lossy().ends_with(".md"));
    }

    #[test]
    fn ensure_note_creates_file() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("sub").join("notes.md");
        ensure_note_file(&path).unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn custom_relative_path() {
        let notes = NotesConfig {
            central_root: None,
            relative_path: PathBuf::from("NOTES.md"),
            editor: String::new(),
        };
        let p = PathBuf::from("/work");
        assert_eq!(resolve_note_path(&notes, &p), PathBuf::from("/work/NOTES.md"));
    }
}

