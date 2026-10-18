//! Editor command layer. Every document mutation — UI gesture, MCP tool call —
//! is a named tool applied through [`apply`], recorded as an undoable [`Tx`]
//! of JSON-pointer edits on the project root.

pub mod assets;
pub mod catalog;
pub mod fx;
pub mod keys;
pub mod layers;
pub mod machines;
pub mod presets;
pub mod scene;
pub mod shapes;
pub mod text;
pub mod themes;
pub mod util;

use serde_json::{Map, Value};

use crate::project;

#[derive(Debug, Clone, PartialEq)]
pub struct OpError(pub String);

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for OpError {}

pub type OpResult<T = Value> = Result<T, OpError>;

pub fn err<T>(msg: impl Into<String>) -> OpResult<T> {
    Err(OpError(msg.into()))
}

// ------------------------------------------------------------- JSON pointers

pub fn escape(seg: &str) -> String {
    seg.replace('~', "~0").replace('/', "~1")
}

fn parts(ptr: &str) -> Vec<String> {
    if ptr.is_empty() {
        return vec![];
    }
    ptr.trim_start_matches('/').split('/').map(|s| s.replace("~1", "/").replace("~0", "~")).collect()
}

fn child_mut<'a>(v: &'a mut Value, key: &str, create: bool) -> Option<&'a mut Value> {
    match v {
        Value::Object(o) => {
            if create && !o.contains_key(key) {
                o.insert(key.to_string(), Value::Object(Map::new()));
            }
            o.get_mut(key)
        }
        Value::Array(a) => key.parse::<usize>().ok().and_then(move |i| a.get_mut(i)),
        _ => None,
    }
}

/// Navigate to the parent of `ptr`, creating missing intermediate objects.
fn parent_mut<'a>(root: &'a mut Value, ptr: &str) -> OpResult<(&'a mut Value, String)> {
    let mut ps = parts(ptr);
    let last = ps.pop().ok_or_else(|| OpError("cannot replace the document root".into()))?;
    let mut cur = root;
    for p in &ps {
        cur = child_mut(cur, p, true).ok_or_else(|| OpError(format!("path not found: {ptr}")))?;
    }
    Ok((cur, last))
}

fn put(root: &mut Value, ptr: &str, v: Option<Value>) -> OpResult<Option<Value>> {
    let (parent, key) = parent_mut(root, ptr)?;
    match parent {
        Value::Object(o) => Ok(match v {
            Some(v) => o.insert(key, v),
            None => o.shift_remove(&key),
        }),
        Value::Array(a) => {
            let i: usize = key.parse().map_err(|_| OpError(format!("bad array index in {ptr}")))?;
            if i >= a.len() {
                return err(format!("index out of range: {ptr}"));
            }
            match v {
                Some(v) => Ok(Some(std::mem::replace(&mut a[i], v))),
                None => err("use remove() for array elements"),
            }
        }
        _ => err(format!("not a container: {ptr}")),
    }
}

fn array_mut<'a>(root: &'a mut Value, ptr: &str) -> OpResult<&'a mut Vec<Value>> {
    let mut cur = root;
    for p in parts(ptr) {
        cur = child_mut(cur, &p, true).ok_or_else(|| OpError(format!("path not found: {ptr}")))?;
    }
    if cur.is_object() && cur.as_object().map(Map::is_empty).unwrap_or(false) {
        *cur = Value::Array(vec![]);
    }
    if cur.is_null() {
        *cur = Value::Array(vec![]);
    }
    cur.as_array_mut().ok_or_else(|| OpError(format!("not an array: {ptr}")))
}

// ----------------------------------------------------------------- Tx

#[derive(Debug, Clone)]
enum Edit {
    Put { ptr: String, old: Option<Value>, new: Option<Value> },
    Insert { ptr: String, idx: usize, value: Value },
    Remove { ptr: String, idx: usize, value: Value },
}

/// An undoable group of edits.
#[derive(Debug, Clone, Default)]
pub struct Tx {
    pub label: String,
    edits: Vec<Edit>,
}

impl Tx {
    pub fn new(label: &str) -> Self {
        Tx { label: label.into(), edits: vec![] }
    }

    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Set (or create) the value at `ptr`. Missing intermediate objects are
    /// created as part of the same undoable edit.
    pub fn set(&mut self, root: &mut Value, ptr: &str, v: Value) -> OpResult<()> {
        let ps = parts(ptr);
        let mut cur: &Value = root;
        let mut missing_at = None;
        for (k, p) in ps.iter().enumerate().take(ps.len().saturating_sub(1)) {
            let next = match cur {
                Value::Object(o) => o.get(p),
                Value::Array(a) => p.parse::<usize>().ok().and_then(|i| a.get(i)),
                _ => None,
            };
            match next {
                Some(n) => cur = n,
                None => {
                    missing_at = Some(k);
                    break;
                }
            }
        }
        let (ptr, v) = match missing_at {
            Some(k) => {
                let mut nested = v;
                for p in ps[k + 1..].iter().rev() {
                    let mut m = Map::new();
                    m.insert(p.clone(), nested);
                    nested = Value::Object(m);
                }
                (ps[..=k].iter().map(|p| format!("/{}", escape(p))).collect::<String>(), nested)
            }
            None => (ptr.to_string(), v),
        };
        let old = put(root, &ptr, Some(v.clone()))?;
        self.edits.push(Edit::Put { ptr, old, new: Some(v) });
        Ok(())
    }

    /// Delete an object key at `ptr` (no-op if missing).
    pub fn delete(&mut self, root: &mut Value, ptr: &str) -> OpResult<()> {
        let old = put(root, ptr, None)?;
        if old.is_some() {
            self.edits.push(Edit::Put { ptr: ptr.into(), old, new: None });
        }
        Ok(())
    }

    /// Insert into the array at `ptr` (created if missing); `idx` is clamped.
    pub fn insert(&mut self, root: &mut Value, ptr: &str, idx: usize, v: Value) -> OpResult<usize> {
        let a = array_mut(root, ptr)?;
        let idx = idx.min(a.len());
        a.insert(idx, v.clone());
        self.edits.push(Edit::Insert { ptr: ptr.into(), idx, value: v });
        Ok(idx)
    }

    pub fn remove(&mut self, root: &mut Value, ptr: &str, idx: usize) -> OpResult<Value> {
        let a = array_mut(root, ptr)?;
        if idx >= a.len() {
            return err(format!("index {idx} out of range for {ptr}"));
        }
        let v = a.remove(idx);
        self.edits.push(Edit::Remove { ptr: ptr.into(), idx, value: v.clone() });
        Ok(v)
    }

    pub fn undo(&self, root: &mut Value) {
        for e in self.edits.iter().rev() {
            let _ = match e {
                Edit::Put { ptr, old, .. } => put(root, ptr, old.clone()).map(|_| ()),
                Edit::Insert { ptr, idx, .. } => array_mut(root, ptr).map(|a| {
                    if *idx < a.len() {
                        a.remove(*idx);
                    }
                }),
                Edit::Remove { ptr, idx, value } => array_mut(root, ptr).map(|a| a.insert((*idx).min(a.len()), value.clone())),
            };
        }
    }

    pub fn redo(&self, root: &mut Value) {
        for e in &self.edits {
            let _ = match e {
                Edit::Put { ptr, new, .. } => put(root, ptr, new.clone()).map(|_| ()),
                Edit::Insert { ptr, idx, value } => array_mut(root, ptr).map(|a| a.insert((*idx).min(a.len()), value.clone())),
                Edit::Remove { ptr, idx, .. } => array_mut(root, ptr).map(|a| {
                    if *idx < a.len() {
                        a.remove(*idx);
                    }
                }),
            };
        }
    }
}

// ------------------------------------------------------------- History

pub struct History {
    undo: Vec<Tx>,
    redo: Vec<Tx>,
    cap: usize,
    /// When batching, pushes merge into this transaction.
    batch: Option<Tx>,
}

impl Default for History {
    fn default() -> Self {
        History::with_cap(200)
    }
}

impl History {
    pub fn with_cap(cap: usize) -> Self {
        History { undo: vec![], redo: vec![], cap: cap.max(1), batch: None }
    }
    /// Start merging subsequent transactions into one undo step.
    pub fn begin_batch(&mut self, label: &str) {
        if self.batch.is_none() {
            self.batch = Some(Tx::new(label));
        }
    }
    /// Finish the current batch (no-op if empty).
    pub fn end_batch(&mut self) {
        if let Some(b) = self.batch.take() {
            self.push(b);
        }
    }
    pub fn push(&mut self, tx: Tx) {
        if tx.is_empty() {
            return;
        }
        if let Some(b) = self.batch.as_mut() {
            b.edits.extend(tx.edits);
            return;
        }
        self.undo.push(tx);
        if self.undo.len() > self.cap {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// Undo the last transaction; returns its label.
    pub fn undo(&mut self, root: &mut Value) -> Option<String> {
        self.end_batch();
        let tx = self.undo.pop()?;
        tx.undo(root);
        let l = tx.label.clone();
        self.redo.push(tx);
        Some(l)
    }
    pub fn redo(&mut self, root: &mut Value) -> Option<String> {
        let tx = self.redo.pop()?;
        tx.redo(root);
        let l = tx.label.clone();
        self.undo.push(tx);
        Some(l)
    }
    pub fn clear(&mut self) {
        self.batch = None;
        self.undo.clear();
        self.redo.clear();
    }
}

// ------------------------------------------------------------- Cx

/// Context handed to every tool: the project root plus the transaction being built.
pub struct Cx<'a> {
    pub root: &'a mut Value,
    pub tx: Tx,
}

/// A resolved layer address.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerRef {
    /// Pointer to the `layers` array containing the layer.
    pub layers_ptr: String,
    pub index: usize,
    /// Stable id: `<comp>:<ind>`, comp = `root` or a precomp asset id.
    pub id: String,
}

impl LayerRef {
    pub fn ptr(&self) -> String {
        format!("{}/{}", self.layers_ptr, self.index)
    }
}

impl<'a> Cx<'a> {
    pub fn new(root: &'a mut Value, label: &str) -> Self {
        Cx { root, tx: Tx::new(label) }
    }

    pub fn anim_ptr(&self) -> String {
        format!("/animations/{}", escape(&project::active_id(self.root)))
    }

    pub fn anim(&self) -> &Value {
        project::active_anim(self.root)
    }

    pub fn get(&self, ptr: &str) -> Option<&Value> {
        self.root.pointer(ptr)
    }

    pub fn set(&mut self, ptr: &str, v: Value) -> OpResult<()> {
        self.tx.set(self.root, ptr, v)
    }

    pub fn delete(&mut self, ptr: &str) -> OpResult<()> {
        self.tx.delete(self.root, ptr)
    }

    pub fn insert(&mut self, ptr: &str, idx: usize, v: Value) -> OpResult<usize> {
        self.tx.insert(self.root, ptr, idx, v)
    }

    pub fn remove(&mut self, ptr: &str, idx: usize) -> OpResult<Value> {
        self.tx.remove(self.root, ptr, idx)
    }

    /// Pointer of the `layers` array of a composition (`root` or asset id).
    pub fn comp_layers_ptr(&self, comp: &str) -> OpResult<String> {
        let a = self.anim_ptr();
        if comp == "root" || comp.is_empty() {
            return Ok(format!("{a}/layers"));
        }
        let assets = self.anim().get("assets").and_then(Value::as_array).cloned().unwrap_or_default();
        match assets.iter().position(|x| x.get("id").and_then(Value::as_str) == Some(comp) && x.get("layers").is_some()) {
            Some(i) => Ok(format!("{a}/assets/{i}/layers")),
            None => err(format!("no scene/precomp with id '{comp}'")),
        }
    }

    /// All compositions: ("root", layers ptr) plus each precomp asset.
    pub fn comps(&self) -> Vec<(String, String)> {
        let a = self.anim_ptr();
        let mut out = vec![("root".to_string(), format!("{a}/layers"))];
        if let Some(assets) = self.anim().get("assets").and_then(Value::as_array) {
            for (i, x) in assets.iter().enumerate() {
                if let (Some(id), Some(_)) = (x.get("id").and_then(Value::as_str), x.get("layers")) {
                    out.push((id.to_string(), format!("{a}/assets/{i}/layers")));
                }
            }
        }
        out
    }

    /// Resolve a layer by id (`comp:ind`) or by unique name.
    pub fn layer(&self, key: &str) -> OpResult<LayerRef> {
        if let Some((comp, ind)) = key.rsplit_once(':') {
            if let Ok(ind) = ind.parse::<i64>() {
                let lp = self.comp_layers_ptr(comp)?;
                let layers = self.get(&lp).and_then(Value::as_array).cloned().unwrap_or_default();
                if let Some(i) = layers.iter().position(|l| l.get("ind").and_then(Value::as_i64) == Some(ind)) {
                    return Ok(LayerRef { layers_ptr: lp, index: i, id: format!("{comp}:{ind}") });
                }
                return err(format!("layer '{key}' not found"));
            }
        }
        let mut hits = vec![];
        for (comp, lp) in self.comps() {
            if let Some(layers) = self.get(&lp).and_then(Value::as_array) {
                for (i, l) in layers.iter().enumerate() {
                    if l.get("nm").and_then(Value::as_str) == Some(key) {
                        let ind = l.get("ind").and_then(Value::as_i64).unwrap_or(i as i64);
                        hits.push(LayerRef { layers_ptr: lp.clone(), index: i, id: format!("{comp}:{ind}") });
                    }
                }
            }
        }
        match hits.len() {
            1 => Ok(hits.remove(0)),
            0 => err(format!("layer '{key}' not found (use an id like 'root:1' from read_scene)")),
            _ => err(format!("layer name '{key}' is ambiguous; use its id")),
        }
    }

    pub fn layer_value(&self, r: &LayerRef) -> Value {
        self.get(&r.ptr()).cloned().unwrap_or(Value::Null)
    }

    /// Next free `ind` in a layers array.
    pub fn next_ind(&self, layers_ptr: &str) -> i64 {
        self.get(layers_ptr)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|l| l.get("ind").and_then(Value::as_i64)).max().unwrap_or(0) + 1)
            .unwrap_or(1)
    }

    pub fn comp_of(r: &LayerRef) -> &str {
        r.id.rsplit_once(':').map(|x| x.0).unwrap_or("root")
    }
}

// ------------------------------------------------------------- args

pub fn arg_str<'v>(args: &'v Value, k: &str) -> OpResult<&'v str> {
    args.get(k).and_then(Value::as_str).ok_or_else(|| OpError(format!("missing string argument '{k}'")))
}

pub fn opt_str<'v>(args: &'v Value, k: &str) -> Option<&'v str> {
    args.get(k).and_then(Value::as_str)
}

pub fn arg_f64(args: &Value, k: &str) -> OpResult<f64> {
    args.get(k).and_then(Value::as_f64).ok_or_else(|| OpError(format!("missing number argument '{k}'")))
}

pub fn opt_f64(args: &Value, k: &str) -> Option<f64> {
    args.get(k).and_then(Value::as_f64)
}

pub fn opt_bool(args: &Value, k: &str) -> Option<bool> {
    args.get(k).and_then(Value::as_bool)
}

pub fn opt_vec(args: &Value, k: &str) -> Option<Vec<f64>> {
    match args.get(k)? {
        Value::Array(a) => a.iter().map(Value::as_f64).collect(),
        Value::Number(n) => n.as_f64().map(|x| vec![x]),
        _ => None,
    }
}

// ------------------------------------------------------------- dispatch

/// Apply a tool atomically: on error every partial edit is rolled back and
/// nothing is recorded in `hist`.
pub fn apply(root: &mut Value, hist: &mut History, tool: &str, args: &Value) -> OpResult {
    let Some(def) = catalog::find(tool) else {
        return err(format!("unknown tool '{tool}'"));
    };
    let empty = Value::Object(Map::new());
    let args = if args.is_null() { &empty } else { args };
    let mut cx = Cx::new(root, tool);
    match (def.handler)(&mut cx, args) {
        Ok(v) => {
            let tx = std::mem::take(&mut cx.tx);
            hist.push(tx);
            Ok(v)
        }
        Err(e) => {
            cx.tx.undo(cx.root);
            Err(e)
        }
    }
}
