//! bhoted prototype: the topic index of bhote (`topics.idx`), same format as `topics_index_build` in the bash script.
//!   bhoted index <topic dir> <out>      build once
//!   bhoted run   <topic dir> <out>      keep it current: rebuild when a topic file is added, removed or changed
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const US: char = '\u{1f}';
const KEYS: &str = "title status waiting_for waiting_since updated deleted description agent agent_machine agent_pane agent_session delegated transfer_to_name slack_label slack_msg agent_branch project kind deploy_wait dismissed auto_now auto review_src";

fn topic_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map_or(false, |x| x == "topic")).collect())
        .unwrap_or_default();
    v.sort(); // the shell glob sorts by name
    v
}

/// One line per file: path, then the first value of each key (like the awk: the first one counts, also when it is empty).
fn record(path: &Path, keys: &[&str]) -> String {
    let text = fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let mut seen: HashMap<&str, &str> = HashMap::new();
    for line in text.lines() {
        let (k, v) = match line.find('=') { Some(i) => (&line[..i], &line[i + 1..]), None => (line, "") };
        seen.entry(k).or_insert(v);
    }
    let mut out = path.to_string_lossy().into_owned();
    for k in keys { out.push(US); out.push_str(seen.get(k).copied().unwrap_or("")); }
    out
}

fn build(dir: &Path, out: &Path, token: &str) -> std::io::Result<()> {
    let keys: Vec<&str> = KEYS.split(' ').collect();
    let files = topic_files(dir);
    let mut body = format!("#{} {} {}\n", files.len(), dir.display(), token);
    for f in &files { body.push_str(&record(f, &keys)); body.push('\n'); }
    let tmp = out.with_extension(format!("tmp.{}", std::process::id()));
    fs::File::create(&tmp)?.write_all(body.as_bytes())?;
    fs::rename(&tmp, out)
}

fn signature(dir: &Path) -> Vec<(PathBuf, u128, u64)> {
    topic_files(dir).into_iter().map(|p| {
        let m = fs::metadata(&p).ok();
        let t = m.as_ref().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
        (p, t, m.map_or(0, |m| m.len()))
    }).collect()
}

fn token() -> String {
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("{}{}", std::process::id(), n)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 4 { eprintln!("usage: bhoted index|run <topic dir> <out>"); std::process::exit(2); }
    let (dir, out) = (Path::new(&a[2]), Path::new(&a[3]));
    match a[1].as_str() {
        "index" => build(dir, out, &token()).unwrap_or_else(|e| { eprintln!("bhoted: {e}"); std::process::exit(1) }),
        "run" => {
            let mut last = Vec::new();
            loop {
                let sig = signature(dir);
                if sig != last || !out.exists() { if build(dir, out, &token()).is_ok() { last = sig; } }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        _ => { eprintln!("usage: bhoted index|run <topic dir> <out>"); std::process::exit(2); }
    }
}
