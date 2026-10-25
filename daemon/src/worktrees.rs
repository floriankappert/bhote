//! The git worktrees of the repositories the agents of this machine work in, and who works in them: for the section `worktrees` of a panel.
//! The list comes from `git worktree list` and is kept in a file (worktrees.cache) that is made again in the background when it is older than 20 seconds,
//! so that drawing never waits for git.
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime};

const US: char = '\u{1f}';

#[derive(Clone, Debug, PartialEq)]
pub struct Wt { pub repo: String, pub branch: String, pub path: String, pub dirty: bool }

fn git(dir: &str, args: &[&str]) -> Option<String> {
    let o = Command::new("git").arg("-C").arg(dir).args(args).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
    if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).into_owned()) } else { None }
}

/// The worktrees of the repository that contains `dir`.
pub fn of_repo(dir: &str) -> Vec<Wt> {
    let Some(common) = git(dir, &["rev-parse", "--path-format=absolute", "--git-common-dir"]) else { return Vec::new() };
    let common = common.trim().to_string();
    let repo = Path::new(&common).parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let Some(list) = git(dir, &["worktree", "list", "--porcelain"]) else { return Vec::new() };
    let mut out = Vec::new();
    for block in list.split("\n\n") {
        let (mut path, mut branch, mut bare) = (String::new(), String::new(), false);
        for l in block.lines() {
            if let Some(p) = l.strip_prefix("worktree ") { path = p.to_string(); }
            else if let Some(b) = l.strip_prefix("branch ") { branch = b.trim_start_matches("refs/heads/").to_string(); }
            else if l == "bare" { bare = true; }
            else if l == "detached" && branch.is_empty() { branch = "(detached)".into(); }
        }
        if path.is_empty() || bare { continue; }
        let dirty = git(&path, &["status", "--porcelain", "-uno"]).map_or(false, |s| !s.trim().is_empty());
        out.push(Wt { repo: repo.clone(), branch, path, dirty });
    }
    out
}

fn render(v: &[Wt]) -> String { v.iter().map(|w| format!("{}{US}{}{US}{}{US}{}\n", w.repo, w.branch, w.path, w.dirty as u8)).collect() }
fn parse(t: &str) -> Vec<Wt> {
    t.lines().filter_map(|l| { let f: Vec<&str> = l.split(US).collect(); if f.len() < 4 { None } else { Some(Wt { repo: f[0].into(), branch: f[1].into(), path: f[2].into(), dirty: f[3] == "1" }) } }).collect()
}

/// The cached list; when it is old, a new one is made in the background (from the working folders of the agents, `cwds`).
pub fn cached(shared: &Path, cwds: Vec<String>) -> Vec<Wt> {
    let f = shared.join("worktrees.cache");
    let age = fs::metadata(&f).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok());
    if age.map_or(true, |a| a > Duration::from_secs(20)) {
        // the file is touched at once: one refresh at a time
        if fs::metadata(&f).is_ok() { let _ = fs::OpenOptions::new().append(true).open(&f); } else { let _ = fs::write(&f, ""); }
        let shared = shared.to_path_buf();
        std::thread::spawn(move || {
            let mut seen: Vec<String> = Vec::new(); let mut all: Vec<Wt> = Vec::new();
            for c in cwds {
                if seen.iter().any(|s| c == *s || c.starts_with(&format!("{s}/"))) { continue; }
                let wts = of_repo(&c);
                for w in &wts { seen.push(w.path.clone()); }
                if wts.is_empty() { continue; }
                for w in wts { if !all.contains(&w) { all.push(w); } }
            }
            let tmp = shared.join(format!("worktrees.cache.tmp.{}", std::process::id()));
            if fs::write(&tmp, render(&all)).is_ok() { let _ = fs::rename(&tmp, shared.join("worktrees.cache")); }
        });
    }
    fs::read_to_string(&f).map(|t| parse(&t)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worktrees_of_a_repo() {
        let t = std::env::temp_dir().join(format!("bhote-wt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&t); fs::create_dir_all(&t).unwrap();
        let sh = |args: &[&str], d: &Path| { assert!(Command::new("git").arg("-C").arg(d).args(args).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap().status.success(), "{args:?}"); };
        let main = t.join("proj"); fs::create_dir_all(&main).unwrap();
        sh(&["init", "-q", "-b", "main"], &main); fs::write(main.join("a"), "1").unwrap(); sh(&["add", "."], &main); sh(&["commit", "-qm", "x"], &main);
        sh(&["worktree", "add", "-q", "-b", "feat", t.join("proj-wt1").to_str().unwrap()], &main);
        fs::write(t.join("proj-wt1").join("a"), "2").unwrap();
        let v = of_repo(t.join("proj-wt1").to_str().unwrap());
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].repo.as_str(), v[0].branch.as_str(), v[0].dirty), ("proj", "main", false));
        assert_eq!((v[1].branch.as_str(), v[1].dirty), ("feat", true));
        assert_eq!(parse(&render(&v)), v);
        let _ = fs::remove_dir_all(&t);
    }
}
