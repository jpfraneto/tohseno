use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn git_output(repository: &Path, arguments: &[&str]) -> Option<Vec<u8>> {
    Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| output.stdout)
}

fn git_text(repository: &Path, arguments: &[&str]) -> Option<String> {
    git_output(repository, arguments)
        .and_then(|output| String::from_utf8(output).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn emit_git_path_watch(repository: &Path, git_path: &str) {
    let Some(path) = git_text(repository, &["rev-parse", "--git-path", git_path]) else {
        return;
    };
    let path = PathBuf::from(path);
    let path = if path.is_absolute() {
        path
    } else {
        repository.join(path)
    };
    println!("cargo:rerun-if-changed={}", path.display());
}

fn emit_repository_input_watches(repository: &Path) {
    let Some(paths) = git_output(
        repository,
        &[
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    ) else {
        return;
    };
    for path in paths
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let Ok(path) = std::str::from_utf8(path) else {
            continue;
        };
        if path.contains('\n') || path.contains('\r') {
            continue;
        }
        println!("cargo:rerun-if-changed={}", repository.join(path).display());
    }
}

fn main() {
    let repository = std::path::Path::new("..");
    println!("cargo:rerun-if-env-changed=TOHSENO_RELEASE_SOURCE_STATE");
    for git_path in ["HEAD", "index", "packed-refs", "logs/HEAD"] {
        emit_git_path_watch(repository, git_path);
    }
    if let Some(reference) = git_text(repository, &["symbolic-ref", "--quiet", "HEAD"]) {
        emit_git_path_watch(repository, &reference);
    }
    emit_repository_input_watches(repository);

    let commit = git_text(repository, &["rev-parse", "--verify", "HEAD"])
        .filter(|value| {
            value.len() == 40
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .unwrap_or_else(|| "0".repeat(40));
    println!("cargo:rustc-env=TOHSENO_SOURCE_COMMIT={commit}");

    let dirty = git_output(
        repository,
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ],
    )
    .is_none_or(|status| !status.is_empty());
    println!(
        "cargo:rustc-env=TOHSENO_SOURCE_DIRTY={}",
        if dirty { "1" } else { "0" }
    );
}
