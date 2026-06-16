use std::path::Path;
use anyhow::anyhow;

pub fn git_commit(
    repo_dir: &Path,
    file_path: &Path,
    message: &str,
    author: &str,
) -> anyhow::Result<String> {
    // Get relative path from repo_dir
    let relative_path = file_path
        .strip_prefix(repo_dir)
        .unwrap_or(file_path);

    // Stage the file
    let status = std::process::Command::new("git")
        .args(&["add", relative_path.to_str().unwrap()])
        .current_dir(repo_dir)
        .output()?;

    if !status.status.success() {
        return Err(anyhow!("git add failed"));
    }

    // Commit
    let status = std::process::Command::new("git")
        .args(&["commit", "-m", message, "--author", &format!("{} <{}>", author, author)])
        .current_dir(repo_dir)
        .output()?;

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        return Err(anyhow!("git commit failed: {}", stderr));
    }

    get_current_head(repo_dir)
}

pub fn git_rename(
    repo_dir: &Path,
    old_file_path: &Path,
    new_file_path: &Path,
    message: &str,
    author: &str,
) -> anyhow::Result<String> {
    let old_relative_path = old_file_path
        .strip_prefix(repo_dir)
        .unwrap_or(old_file_path);
    let new_relative_path = new_file_path
        .strip_prefix(repo_dir)
        .unwrap_or(new_file_path);

    let status = std::process::Command::new("git")
        .args(&["mv", old_relative_path.to_str().unwrap(), new_relative_path.to_str().unwrap()])
        .current_dir(repo_dir)
        .output()?;

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        return Err(anyhow!("git mv failed: {}", stderr));
    }

    git_commit_staged(repo_dir, message, author)
}

fn git_commit_staged(
    repo_dir: &Path,
    message: &str,
    author: &str,
) -> anyhow::Result<String> {
    let status = std::process::Command::new("git")
        .args(&["commit", "-m", message, "--author", &format!("{} <{}>", author, author)])
        .current_dir(repo_dir)
        .output()?;

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        return Err(anyhow!("git commit failed: {}", stderr));
    }

    get_current_head(repo_dir)
}

pub fn get_current_head(repo_dir: &Path) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .current_dir(repo_dir)
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("git rev-parse HEAD failed"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn file_changed_since_head(
    repo_dir: &Path,
    file_path: &Path,
    expected_head: &str,
) -> anyhow::Result<bool> {
    let current_head = get_current_head(repo_dir)?;

    if current_head != expected_head {
        return Ok(true); // HEAD has moved
    }

    // Check if file has uncommitted changes
    let output = std::process::Command::new("git")
        .args(&["status", "--porcelain", file_path.to_str().unwrap()])
        .current_dir(repo_dir)
        .output()?;

    let status = String::from_utf8_lossy(&output.stdout);
    Ok(!status.trim().is_empty())
}

pub fn get_git_log(
    repo_dir: &Path,
    file_path: &Path,
    limit: usize,
) -> anyhow::Result<Vec<(String, String, String)>> {
    let relative = file_path.strip_prefix(repo_dir).unwrap_or(file_path);
    let output = std::process::Command::new("git")
        .args(&[
            "log",
            &format!("--max-count={}", limit),
            "--format=%H|%an|%s",
            "--",
            relative.to_str().unwrap(),
        ])
        .current_dir(repo_dir)
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new()); // File has no history yet
    }

    let lines = String::from_utf8_lossy(&output.stdout);
    let commits = lines
        .lines()
        .map(|line| {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() >= 3 {
                (
                    parts[0].to_string(),
                    parts[1].to_string(),
                    parts[2].to_string(),
                )
            } else {
                (String::new(), String::new(), String::new())
            }
        })
        .collect();

    Ok(commits)
}

pub fn get_diff_to_current(
    repo_dir: &Path,
    file_path: &Path,
    commit_hash: &str,
) -> anyhow::Result<String> {
    let relative = file_path.strip_prefix(repo_dir).unwrap_or(file_path);
    let output = std::process::Command::new("git")
        .args(&[
            "diff",
            commit_hash,
            "HEAD",
            "--",
            relative.to_str().unwrap(),
        ])
        .current_dir(repo_dir)
        .output()?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn get_file_at_commit(
    repo_dir: &Path,
    file_path: &Path,
    commit_hash: &str,
) -> anyhow::Result<String> {
    let relative = file_path.strip_prefix(repo_dir).unwrap_or(file_path);
    let output = std::process::Command::new("git")
        .args(&["show", &format!("{}:{}", commit_hash, relative.to_str().unwrap())])
        .current_dir(repo_dir)
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("git show failed for commit {}", commit_hash));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_repo() -> std::path::PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let tempdir = std::env::temp_dir().join(format!("wiki_git_test_{}", nanos));
        let _ = std::fs::remove_dir_all(&tempdir);
        std::fs::create_dir_all(&tempdir).ok();

        std::process::Command::new("git")
            .args(&["init"])
            .current_dir(&tempdir)
            .output()
            .ok();

        std::process::Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(&tempdir)
            .output()
            .ok();

        std::process::Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(&tempdir)
            .output()
            .ok();

        tempdir
    }

    #[test]
    fn test_git_commit() {
        let repo = setup_test_repo();
        let test_file = repo.join("test.md");
        std::fs::write(&test_file, "# Test\nContent").ok();

        let result = git_commit(&repo, &test_file, "test message", "testuser").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_get_current_head() {
        let repo = setup_test_repo();
        let test_file = repo.join("test.md");
        std::fs::write(&test_file, "# Test").ok();
        git_commit(&repo, &test_file, "init", "user1").ok();

        let head = get_current_head(&repo).unwrap();
        assert!(!head.is_empty());
        assert!(head.len() > 10);
    }

    #[test]
    fn test_file_changed_since_head() {
        let repo = setup_test_repo();
        let test_file = repo.join("test.md");
        std::fs::write(&test_file, "v1").ok();
        git_commit(&repo, &test_file, "commit1", "user1").ok();

        let head1 = get_current_head(&repo).unwrap();
        assert!(!file_changed_since_head(&repo, &test_file, &head1).unwrap());

        std::fs::write(&test_file, "v2").ok();
        assert!(file_changed_since_head(&repo, &test_file, &head1).unwrap());
    }
}
