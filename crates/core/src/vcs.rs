//! Local version control: a content-addressed store of project snapshots with
//! commits, branches, restore and a semantic diff summary. Hosts persist the
//! whole repository with [`Repo::to_bytes`] (deflate-compressed JSON).

use std::collections::BTreeMap;
use std::io::{Read, Write};

use base64::Engine as _;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq)]
pub struct Commit {
    pub id: String,
    pub tree: String,
    pub parents: Vec<String>,
    pub message: String,
    pub author: String,
    pub ts: f64,
    pub branch: String,
}

impl Commit {
    pub fn to_json(&self) -> Value {
        json!({"id": self.id, "tree": self.tree, "parents": self.parents, "message": self.message, "author": self.author, "ts": self.ts, "branch": self.branch})
    }
    fn from_json(v: &Value) -> Option<Commit> {
        Some(Commit {
            id: v.get("id")?.as_str()?.into(),
            tree: v.get("tree")?.as_str()?.into(),
            parents: v.get("parents")?.as_array()?.iter().filter_map(|p| p.as_str().map(str::to_string)).collect(),
            message: v.get("message").and_then(Value::as_str).unwrap_or("").into(),
            author: v.get("author").and_then(Value::as_str).unwrap_or("").into(),
            ts: v.get("ts").and_then(Value::as_f64).unwrap_or(0.0),
            branch: v.get("branch").and_then(Value::as_str).unwrap_or("main").into(),
        })
    }
}

pub struct Repo {
    objects: BTreeMap<String, Vec<u8>>,
    commits: BTreeMap<String, Commit>,
    branches: BTreeMap<String, String>,
    head: String,
}

impl Default for Repo {
    fn default() -> Self {
        Self::new()
    }
}

fn sha(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

impl Repo {
    pub fn new() -> Repo {
        let mut branches = BTreeMap::new();
        branches.insert("main".to_string(), String::new());
        Repo { objects: BTreeMap::new(), commits: BTreeMap::new(), branches, head: "main".into() }
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    pub fn current_branch(&self) -> &str {
        &self.head
    }

    pub fn head_commit(&self) -> Option<&Commit> {
        self.branches.get(&self.head).and_then(|id| self.commits.get(id))
    }

    /// Commit project bytes on the current branch. Refuses when nothing changed.
    pub fn commit(&mut self, bytes: &[u8], message: &str, author: &str, ts: f64) -> Result<String, String> {
        let tree = sha(bytes);
        let parent = self.branches.get(&self.head).cloned().unwrap_or_default();
        if let Some(p) = self.commits.get(&parent) {
            if p.tree == tree {
                return Err("no changes since the last version".into());
            }
        }
        self.objects.entry(tree.clone()).or_insert_with(|| bytes.to_vec());
        let parents: Vec<String> = if parent.is_empty() { vec![] } else { vec![parent] };
        let id = sha(format!("{tree}\n{}\n{message}\n{author}\n{ts}\n{}", parents.join(","), self.head).as_bytes())[..16].to_string();
        let c = Commit { id: id.clone(), tree, parents, message: message.to_string(), author: author.to_string(), ts, branch: self.head.clone() };
        self.commits.insert(id.clone(), c);
        self.branches.insert(self.head.clone(), id.clone());
        Ok(id)
    }

    /// History of the current branch, newest first (first-parent chain).
    pub fn log(&self) -> Vec<Commit> {
        let mut out = vec![];
        let mut cur = self.branches.get(&self.head).cloned().unwrap_or_default();
        while let Some(c) = self.commits.get(&cur) {
            out.push(c.clone());
            cur = c.parents.first().cloned().unwrap_or_default();
            if out.len() > 100_000 {
                break;
            }
        }
        out
    }

    /// Project bytes of a commit.
    pub fn checkout(&self, id: &str) -> Result<Vec<u8>, String> {
        let c = self.commits.get(id).ok_or_else(|| format!("no version '{id}'"))?;
        self.objects.get(&c.tree).cloned().ok_or_else(|| "version data missing".into())
    }

    pub fn branches(&self) -> Vec<(String, String)> {
        self.branches.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    /// Create a branch at `at` (default: current head) and switch to it.
    pub fn create_branch(&mut self, name: &str, at: Option<&str>) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() || self.branches.contains_key(name) {
            return Err(format!("branch '{name}' already exists or is invalid"));
        }
        let at = match at {
            Some(id) if self.commits.contains_key(id) => id.to_string(),
            Some(id) => return Err(format!("no version '{id}'")),
            None => self.branches.get(&self.head).cloned().unwrap_or_default(),
        };
        self.branches.insert(name.to_string(), at);
        self.head = name.to_string();
        Ok(())
    }

    /// Switch branch; returns the branch head's project bytes (None for an empty branch).
    pub fn switch(&mut self, name: &str) -> Result<Option<Vec<u8>>, String> {
        let head = self.branches.get(name).cloned().ok_or_else(|| format!("no branch '{name}'"))?;
        self.head = name.to_string();
        if head.is_empty() {
            return Ok(None);
        }
        self.checkout(&head).map(Some)
    }

    /// Semantic summary of what changed between two versions.
    pub fn diff(&self, a: &str, b: &str) -> Result<Value, String> {
        let pa: Value = serde_json::from_slice(&self.checkout(a)?).map_err(|e| e.to_string())?;
        let pb: Value = serde_json::from_slice(&self.checkout(b)?).map_err(|e| e.to_string())?;
        Ok(diff_projects(&pa, &pb))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let objects: Map<String, Value> = self
            .objects
            .iter()
            .map(|(k, v)| {
                let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                let _ = enc.write_all(v);
                (k.clone(), json!(base64::engine::general_purpose::STANDARD.encode(enc.finish().unwrap_or_default())))
            })
            .collect();
        json!({
            "format": "lumetrope-vcs", "version": 1, "head": self.head,
            "branches": self.branches, "commits": self.commits.values().map(Commit::to_json).collect::<Vec<_>>(),
            "objects": objects
        })
        .to_string()
        .into_bytes()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Repo, String> {
        let v: Value = serde_json::from_slice(bytes).map_err(|e| format!("invalid history file: {e}"))?;
        if v.get("format").and_then(Value::as_str) != Some("lumetrope-vcs") {
            return Err("not a Lumetrope history file".into());
        }
        let mut objects = BTreeMap::new();
        for (k, data) in v.get("objects").and_then(Value::as_object).cloned().unwrap_or_default() {
            let raw = base64::engine::general_purpose::STANDARD.decode(data.as_str().unwrap_or("")).map_err(|e| e.to_string())?;
            let mut out = vec![];
            flate2::read::DeflateDecoder::new(raw.as_slice()).read_to_end(&mut out).map_err(|e| e.to_string())?;
            objects.insert(k, out);
        }
        let commits = v.get("commits").and_then(Value::as_array).cloned().unwrap_or_default().iter().filter_map(Commit::from_json).map(|c| (c.id.clone(), c)).collect();
        let branches: BTreeMap<String, String> = v.get("branches").and_then(Value::as_object).cloned().unwrap_or_default().into_iter().map(|(k, v)| (k, v.as_str().unwrap_or("").to_string())).collect();
        let head = v.get("head").and_then(Value::as_str).filter(|h| branches.contains_key(*h)).unwrap_or("main").to_string();
        let mut r = Repo { objects, commits, branches, head };
        r.branches.entry("main".into()).or_default();
        Ok(r)
    }
}

fn layer_map(p: &Value) -> Vec<(String, String, Value)> {
    let active = crate::project::active_id(p);
    let anim = &p["animations"][active.as_str()];
    let mut out = vec![];
    let mut push = |comp: &str, layers: &Value| {
        for l in layers.as_array().cloned().unwrap_or_default() {
            let key = format!("{comp}:{}", l.get("ind").map(|x| x.to_string()).unwrap_or_else(|| l.get("nm").map(|n| n.to_string()).unwrap_or_default()));
            let name = l.get("nm").and_then(Value::as_str).unwrap_or("").to_string();
            out.push((key, name, l));
        }
    };
    push("root", &anim["layers"]);
    for a in anim.get("assets").and_then(Value::as_array).cloned().unwrap_or_default() {
        if let (Some(id), Some(l)) = (a.get("id").and_then(Value::as_str), a.get("layers")) {
            push(id, l);
        }
    }
    out
}

fn keyed_changes(a: &Value, b: &Value) -> Vec<String> {
    let (oa, ob) = (a.as_object().cloned().unwrap_or_default(), b.as_object().cloned().unwrap_or_default());
    let mut out: Vec<String> = ob.iter().filter(|(k, v)| oa.get(*k) != Some(*v)).map(|(k, _)| k.clone()).collect();
    out.extend(oa.keys().filter(|k| !ob.contains_key(*k)).cloned());
    out
}

/// What changed from project `a` to project `b`.
pub fn diff_projects(a: &Value, b: &Value) -> Value {
    let (la, lb) = (layer_map(a), layer_map(b));
    let added: Vec<&str> = lb.iter().filter(|x| !la.iter().any(|y| y.0 == x.0)).map(|x| x.1.as_str()).collect();
    let removed: Vec<&str> = la.iter().filter(|x| !lb.iter().any(|y| y.0 == x.0)).map(|x| x.1.as_str()).collect();
    let changed: Vec<&str> = lb.iter().filter(|x| la.iter().any(|y| y.0 == x.0 && y.2 != x.2)).map(|x| x.1.as_str()).collect();
    let (aa, ab) = (&a["animations"][crate::project::active_id(a).as_str()], &b["animations"][crate::project::active_id(b).as_str()]);
    let settings: Vec<&str> = ["w", "h", "fr", "ip", "op", "nm"].into_iter().filter(|k| aa.get(*k) != ab.get(*k)).collect();
    json!({
        "layers_added": added, "layers_removed": removed, "layers_changed": changed, "settings_changed": settings,
        "state_machines_changed": keyed_changes(&a["stateMachines"], &b["stateMachines"]),
        "themes_changed": keyed_changes(&a["themes"], &b["themes"]),
        "segments_changed": aa.get("markers") != ab.get("markers"),
    })
}
