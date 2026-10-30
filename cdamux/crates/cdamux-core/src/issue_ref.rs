//! Parse Kanban issue numbers from branch names / orch worktree paths.

/// `feat/72-slug`, `fix/88-foo`, bare `72-slug` → `Some(72)`.
pub fn parse_issue_number(branch_or_segment: &str) -> Option<u32> {
    let s = branch_or_segment.trim();
    if s.is_empty() || s == "—" {
        return None;
    }
    let after_slash = s.rsplit('/').next().unwrap_or(s);
    let digits: String = after_slash
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

/// Prefer branch tip; else last path segment under `.orch/worktrees/`.
pub fn issue_from_branch_or_path(branch: &str, cwd: &str) -> Option<u32> {
    if let Some(n) = parse_issue_number(branch) {
        return Some(n);
    }
    let path = cwd.trim();
    if path.is_empty() {
        return None;
    }
    if let Some(idx) = path.find("/.orch/worktrees/") {
        let rest = &path[idx + "/.orch/worktrees/".len()..];
        let seg = rest.split('/').next().unwrap_or(rest);
        return parse_issue_number(seg);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_conventional_branches() {
        assert_eq!(parse_issue_number("feat/72-issue-72"), Some(72));
        assert_eq!(parse_issue_number("fix/88-orch-empty"), Some(88));
        assert_eq!(parse_issue_number("72-only"), Some(72));
        assert_eq!(parse_issue_number("main"), None);
        assert_eq!(parse_issue_number("—"), None);
    }

    #[test]
    fn from_worktree_path() {
        assert_eq!(
            issue_from_branch_or_path(
                "—",
                "/hdd/dev/repo/.orch/worktrees/41-issue-41"
            ),
            Some(41)
        );
        assert_eq!(
            issue_from_branch_or_path("feat/9-x", "/tmp/other"),
            Some(9)
        );
    }
}
