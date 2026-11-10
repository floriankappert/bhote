//! The names of the agents: a name from Middle-earth instead of w2:a1 (the same list as RING_NAMES in the bhote script and RING in the herdr-app mod; a test keeps
//! them equal). The place of an agent's record among all agent records (by the time it was made, then its normalised title; deleted ones count too) decides
//! the name, as `ring_load` in the script does. The records are read again every 5 seconds.
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const RING_NAMES: [&str; 60] = [
    "Frodo", "Sam", "Merry", "Pippin", "Bilbo", "Gandalf", "Aragorn", "Legolas", "Gimli", "Boromir",
    "Elrond", "Arwen", "Gollum", "Sauron", "Saruman", "Theoden", "Eowyn", "Eomer", "Faramir", "Haldir",
    "Thorin", "Balin", "Dwalin", "Fili", "Kili", "Bofur", "Bombur", "Dori", "Nori", "Ori",
    "Oin", "Gloin", "Smaug", "Beorn", "Bard", "Tauriel", "Shelob", "Grima", "Isildur", "Elendil",
    "Rosie", "Tom", "Gwaihir", "Elladan", "Elrohir", "Azog", "Bolg", "Imrahil", "Hama", "Gamling",
    "Rohan", "Gondor", "Mordor", "Moria", "Lorien", "Erebor", "Bree", "Shire", "Dale", "Rumil",
];

/// lower case, letters and digits of ASCII only: "Omarchy/IMS (wt1)" -> "omarchyimswt1"
fn norm(s: &str) -> String { s.chars().filter_map(|c| { let l = c.to_ascii_lowercase(); if l.is_ascii_alphanumeric() { Some(l) } else { None } }).collect() }

fn name_at(i: usize) -> String {
    let r = i / RING_NAMES.len();
    format!("{}{}", RING_NAMES[i % RING_NAMES.len()], if r > 0 { (r + 1).to_string() } else { String::new() })
}

fn build(topics: &Path) -> HashMap<String, String> {
    let mut recs: Vec<(u64, String)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(topics) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if !(n.starts_with("an-") && n.ends_with(".topic")) { continue; }
            let txt = std::fs::read_to_string(e.path()).unwrap_or_default();
            let (mut title, mut created) = (String::new(), 0u64);
            for l in txt.lines() {
                if let Some(v) = l.strip_prefix("title=") { if title.is_empty() { title = v.to_string(); } }
                else if let Some(v) = l.strip_prefix("created=") { created = v.trim().parse().unwrap_or(0); }
            }
            if !title.is_empty() { recs.push((created, norm(&title))); }
        }
    }
    recs.sort();
    let mut map = HashMap::new();
    for (i, (_, k)) in recs.iter().enumerate() { map.entry(k.clone()).or_insert_with(|| name_at(i)); }   // (the first of equal keys has the lowest rank)
    map
}

thread_local! { static CACHE: RefCell<Option<(Instant, PathBuf, HashMap<String, String>)>> = RefCell::new(None); }

/// the name of the agent `agent` on the machine `machine` (as bhote calls it: Mac, Omarchy), None while it has no record
pub fn name(topics: &Path, machine: &str, agent: &str) -> Option<String> {
    let key = norm(&format!("{}/{}", machine, agent));
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.as_ref().map_or(true, |(t, p, _)| t.elapsed() > Duration::from_secs(5) || p != topics) { *c = Some((Instant::now(), topics.to_path_buf(), build(topics))); }
        c.as_ref().and_then(|(_, _, m)| m.get(&key).cloned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn places_and_counts() {
        assert_eq!(name_at(0), "Frodo"); assert_eq!(name_at(59), "Rumil"); assert_eq!(name_at(60), "Frodo2"); assert_eq!(name_at(121), "Sam3"); assert_eq!(name_at(125), "Gandalf3");
        assert_eq!(norm("Omarchy/IMS (wt1)"), "omarchyimswt1"); assert_eq!(norm("Mac/IMS-wt1"), "macimswt1");
    }
    #[test]
    fn rank_not_number() {
        let d = std::env::temp_dir().join(format!("bhote-ring-{}", std::process::id())); let _ = std::fs::create_dir_all(&d);
        let mk = |id: &str, title: &str, created: u64| std::fs::write(d.join(format!("an-{}.topic", id)), format!("id=an-{}\nkind=agentnum\ntitle={}\nnum=1\ncreated={}\ndeleted=0\n", id, title, created)).unwrap();
        mk("a", "Mac/IMS (main)", 20); mk("b", "Omarchy/bhote", 10); mk("c", "Mac/old", 15); mk("d", "Omarchy/IMS (wt1)", 30);
        let m = build(&d);
        assert_eq!(m.get("omarchybhote").map(|s| s.as_str()), Some("Frodo")); assert_eq!(m.get("macold").map(|s| s.as_str()), Some("Sam"));
        assert_eq!(m.get("macimsmain").map(|s| s.as_str()), Some("Merry")); assert_eq!(m.get("omarchyimswt1").map(|s| s.as_str()), Some("Pippin"));
        let _ = std::fs::remove_dir_all(&d);
    }
}
