//! A workspace of another machine shown here as a workspace of its own (hidden from the list: the panel lists it under its machine). The native mux runs `ssh -t <node>` with
//! `bhoted view <pane>` over there. This is what the script's remote_workspace did; here without starting bash and jq for a click (a click on a worktree took seconds on a Mac
//! whose security software looks at every new process).
use std::path::Path;

const US: char = '\u{1f}';

/// What runs over there, without a login shell: the daemon next to bhote (the native mux), else herdr (the script's remote_cmd).
fn remote_cmd(pane: &str) -> String {
    format!("sh -c 'for d in \"$HOME/.local/bin\" \"$HOME/.cargo/bin\" /opt/homebrew/bin /usr/local/bin /usr/bin; do if [ -x \"$d/bhote\" ]; then b=$(readlink -f \"$d/bhote\" 2>/dev/null || echo \"$d/bhote\"); x=$(dirname \"$b\")/daemon/target/release/bhoted; [ -x \"$x\" ] && exec \"$x\" view {pane}; fi; done; PATH=\"$HOME/.local/bin:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"; exec herdr agent attach {pane}'")
}

fn rows(path: &Path) -> Vec<Vec<String>> {
    crate::proc::read_cap(path).unwrap_or_default().lines().map(|l| l.split(US).map(String::from).collect()).collect()
}

fn clean(s: &str) -> String { s.chars().filter(|c| !c.is_control()).collect() }

/// Shows workspace `wid` of machine `m` here: the mount that is there is focused, else one is made. `pane` (may be empty): the pane to show, else the agent of that
/// workspace, else its first pane. `label` (may be empty): its name, else the one the panel lists. Returns false when it could not.
pub fn workspace(shared: &Path, cfg: &std::collections::HashMap<String, String>, m: &str, wid: &str, pane: &str, label: &str) -> bool {
    if !wid.chars().all(|c| c.is_ascii_alphanumeric() || ":_-".contains(c)) || wid.is_empty() { return false; }
    let node = rows(&shared.join("agents.nodes")).into_iter().find(|r| r.first().map(|x| x.as_str()) == Some(m) && r.get(1).map_or(false, |n| !n.is_empty())).map(|r| r[1].clone());
    let Some(node) = node else { return false };
    let label = if !label.is_empty() { label.to_string() } else {
        rows(&shared.join("workspaces.remote")).into_iter().find(|r| r.len() > 2 && r[0] == m && r[1] == wid).map(|r| r[2].clone()).unwrap_or_else(|| wid.to_string())
    };
    let pane = if !pane.is_empty() { pane.to_string() } else {
        rows(&shared.join("agents.meta.remote")).into_iter().find(|r| r.len() > 3 && r[0] == m && r[1].split(':').next() == Some(wid) && r[3] == "a").map(|r| r[1].clone()).unwrap_or_else(|| format!("{wid}:p1"))
    };
    if !pane.chars().all(|c| c.is_ascii_alphanumeric() || ":_-".contains(c)) { return false; }
    let name = format!("{} · {}", m, clean(&label));
    // the mount that is there
    if let Some(o) = crate::proc::run("herdr", &["workspace", "list", "--all"], 5, None).filter(|o| o.0 == 0) {
        let v: serde_json::Value = serde_json::from_slice(&o.1).unwrap_or(serde_json::Value::Null);
        if let Some(id) = v["result"]["workspaces"].as_array().and_then(|a| a.iter().find(|w| w["label"].as_str() == Some(name.as_str()))).and_then(|w| w["workspace_id"].as_str()) {
            let _ = crate::proc::run("herdr", &["workspace", "focus", id], 5, None);
            return true;
        }
    }
    let key = std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".ssh/id_bhote")).filter(|k| k.is_file());
    let mut ssh = cfg.get("REMOTE_SSH").filter(|v| !v.trim().is_empty()).cloned().unwrap_or_else(|| format!("ssh -t -o ControlMaster=auto -o ControlPath=~/.ssh/cm-bhote-%C -o ControlPersist=15m -o ConnectTimeout=8{} {{node}}", key.map_or(String::new(), |k| format!(" -i {} -o IdentitiesOnly=yes", k.display()))));
    if !ssh.contains("{node}") { ssh.push_str(" {node}"); }   // (the destination comes last when the setting does not say where it goes)
    let ssh = ssh.replace("{node}", &node);
    let cmd = remote_cmd(&pane);
    let mut args: Vec<&str> = vec!["workspace", "create", "--label", &name, "--hidden", "--focus", "--"];
    args.extend(ssh.split_whitespace());
    args.push(&cmd);
    crate::proc::run("herdr", &args, 10, None).map_or(false, |o| o.0 == 0)
}

/// "+ new" under a machine: a workspace is made over there (its home folder, not shown there) and shown here.
pub fn new_workspace(shared: &Path, cfg: &std::collections::HashMap<String, String>, m: &str) -> bool {
    let Some(o) = crate::proc::run("herdr", &["--machine", m, "workspace", "create", "--no-focus"], 20, None).filter(|o| o.0 == 0) else { return false };
    let v: serde_json::Value = serde_json::from_slice(&o.1).unwrap_or(serde_json::Value::Null);
    let (w, p, l) = (v["result"]["workspace"]["workspace_id"].as_str().unwrap_or(""), v["result"]["pane"]["pane_id"].as_str().unwrap_or(""), v["result"]["workspace"]["label"].as_str().unwrap_or(""));
    !w.is_empty() && workspace(shared, cfg, m, w, p, l)
}
