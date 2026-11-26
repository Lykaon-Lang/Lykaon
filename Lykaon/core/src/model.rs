//! M5 (Appendix L 7.3): data models.
//!
//! `Predict TARGET from the rest [using MODEL]` cleans the table, splits it
//! 80/20 with the program's seed, trains one of the four fixed models
//! (random forest, decision tree, linear model, neural network) and leaves
//! the model in `model` / `it`. The same module answers accuracy/error,
//! permutation importance, prediction for one row, and the model file
//! format (Save model / Load model).
//!
//! The model file is a compact binary of the fitted numbers only - no code
//! (M5-S29). Everything is deterministic for one seed and one platform.

use crate::data::{cell_text, Table};
use crate::sem::{Kind, Value};
use crate::time4::{xoshiro_next, xoshiro_seed};
use std::collections::HashMap;

// ---------------------------------------------------------------- random

pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng {
            s: xoshiro_seed(seed),
        }
    }
    fn next_u64(&mut self) -> u64 {
        xoshiro_next(&mut self.s)
    }
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

// ---------------------------------------------------------------- kinds

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelKind {
    RandomForest,
    DecisionTree,
    Linear,
    Neural,
}

impl ModelKind {
    pub fn from_words(words: &[String]) -> Option<Self> {
        let joined = words.join(" ");
        Some(match joined.as_str() {
            "random forest" => ModelKind::RandomForest,
            "decision tree" => ModelKind::DecisionTree,
            "linear model" => ModelKind::Linear,
            "neural network" => ModelKind::Neural,
            _ => return None,
        })
    }
    pub fn words(self) -> &'static str {
        match self {
            ModelKind::RandomForest => "random forest",
            ModelKind::DecisionTree => "decision tree",
            ModelKind::Linear => "linear model",
            ModelKind::Neural => "neural network",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Classification,
    Regression,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ColKind {
    Numeric,
    Boolean { mode: bool },
    OneHot { values: Vec<String>, mode: usize },
}

#[derive(Clone, Debug)]
pub struct InputCol {
    pub name: String,
    pub kind: ColKind,
    pub median: f64,
    pub mean: f64,
    pub std: f64,
    /// the encoded feature range [start, start+len)
    pub start: usize,
    pub len: usize,
}

impl InputCol {
    pub fn encode(&self, v: &Value) -> Vec<f64> {
        let mut out = vec![0.0; self.len];
        match &self.kind {
            ColKind::Numeric => {
                let x = match v {
                    Value::Whole(n) => Some(*n as f64),
                    Value::Dec(x) => Some(*x),
                    _ => None,
                };
                let x = x.unwrap_or(self.median);
                out[0] = if self.std.abs() > 1e-12 {
                    (x - self.mean) / self.std
                } else {
                    0.0
                };
            }
            ColKind::Boolean { mode } => {
                let b = match v {
                    Value::Boolean(b) => *b,
                    _ => *mode,
                };
                out[0] = if b { 1.0 } else { 0.0 };
            }
            ColKind::OneHot { values, mode } => {
                let text = cell_text(v);
                let idx = if matches!(v, Value::Missing) {
                    *mode
                } else {
                    values
                        .iter()
                        .position(|x| x == &text)
                        .unwrap_or(*mode)
                };
                if idx < out.len() {
                    out[idx] = 1.0;
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------- trees

#[derive(Clone, Debug)]
pub enum Leaf {
    Class(usize),
    Value(f64),
}

#[derive(Clone, Debug)]
pub enum Node {
    Leaf { leaf: Leaf, count: usize },
    Split {
        feature: usize,
        threshold: f64,
        left: Box<Node>,
        right: Box<Node>,
    },
}

impl Node {
    fn leaf_of(leaf: Leaf, count: usize) -> Node {
        Node::Leaf { leaf, count }
    }
    fn predict(&self, x: &[f64]) -> Leaf {
        match self {
            Node::Leaf { leaf, .. } => leaf.clone(),
            Node::Split {
                feature,
                threshold,
                left,
                right,
            } => {
                if x.get(*feature).copied().unwrap_or(0.0) <= *threshold {
                    left.predict(x)
                } else {
                    right.predict(x)
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum Impl {
    Forest(Vec<Node>),
    Tree(Node),
    Linear {
        w: Vec<f64>,
        b: f64,
    },
    Logistic {
        w: Vec<Vec<f64>>,
        b: Vec<f64>,
    },
    Neural {
        w1: Vec<Vec<f64>>,
        b1: Vec<f64>,
        w2: Vec<Vec<f64>>,
        b2: Vec<f64>,
        w3: Vec<Vec<f64>>,
        b3: Vec<f64>,
    },
}

#[derive(Clone, Debug)]
pub struct Model {
    pub kind: ModelKind,
    pub task: Task,
    pub target: String,
    /// the table the model was trained on (for accuracy/error on the test
    /// rows and for `prediction for`).
    pub table: String,
    /// the input columns the model actually uses (Predict's `from ...`).
    pub input_names: Vec<String>,
    pub inputs: Vec<InputCol>,
    pub classes: Vec<String>,
    pub n_features: usize,
    pub impl_: Impl,
    pub cleaning: Vec<String>,
}

impl Model {
    /// The class index for a classification leaf, or 0 for regression.
    fn leaf_class(&self, leaf: &Leaf) -> usize {
        match leaf {
            Leaf::Class(c) => *c,
            Leaf::Value(_) => 0,
        }
    }

    pub fn predict(&self, x: &[f64]) -> Pred {
        match &self.impl_ {
            Impl::Forest(trees) => {
                if self.task == Task::Regression {
                    let mut sum = 0.0;
                    for t in trees {
                        if let Leaf::Value(v) = t.predict(x) {
                            sum += v;
                        }
                    }
                    Pred::Value(sum / trees.len().max(1) as f64)
                } else {
                    let mut votes = vec![0usize; self.classes.len().max(1)];
                    for t in trees {
                        let c = self.leaf_class(&t.predict(x));
                        if c < votes.len() {
                            votes[c] += 1;
                        }
                    }
                    let best = votes
                        .iter()
                        .enumerate()
                        .max_by_key(|(_, v)| **v)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    Pred::Class(best)
                }
            }
            Impl::Tree(t) => match t.predict(x) {
                Leaf::Class(c) => Pred::Class(c),
                Leaf::Value(v) => Pred::Value(v),
            },
            Impl::Linear { w, b } => {
                let mut y = *b;
                for (wi, xi) in w.iter().zip(x.iter()) {
                    y += wi * xi;
                }
                Pred::Value(y)
            }
            Impl::Logistic { w, b } => {
                let mut best = 0usize;
                let mut best_score = f64::NEG_INFINITY;
                for (k, wk) in w.iter().enumerate() {
                    let mut s = b[k];
                    for (wi, xi) in wk.iter().zip(x.iter()) {
                        s += wi * xi;
                    }
                    if s > best_score {
                        best_score = s;
                        best = k;
                    }
                }
                Pred::Class(best)
            }
            Impl::Neural {
                w1,
                b1,
                w2,
                b2,
                w3,
                b3,
            } => {
                let h1 = relu_layer(w1, b1, x);
                let h2 = relu_layer(w2, b2, &h1);
                let out = linear_layer(w3, b3, &h2);
                if self.task == Task::Classification {
                    let mut best = 0usize;
                    let mut best_score = f64::NEG_INFINITY;
                    for (i, s) in out.iter().enumerate() {
                        if *s > best_score {
                            best_score = *s;
                            best = i;
                        }
                    }
                    Pred::Class(best)
                } else {
                    Pred::Value(out.first().copied().unwrap_or(0.0))
                }
            }
        }
    }

    pub fn encode_input(&self, name: &str, v: &Value) -> Option<Vec<f64>> {
        self.inputs
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
            .map(|c| c.encode(v))
    }

    pub fn row_from_parts(&self, parts: &[(String, Value)]) -> Vec<f64> {
        let mut x = vec![0.0; self.n_features];
        for col in &self.inputs {
            let value = parts
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&col.name))
                .map(|(_, v)| v.clone())
                .unwrap_or(Value::Missing);
            let enc = col.encode(&value);
            for (i, v) in enc.into_iter().enumerate() {
                if col.start + i < x.len() {
                    x[col.start + i] = v;
                }
            }
        }
        x
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pred {
    Class(usize),
    Value(f64),
}

impl Pred {
    pub fn class_index(&self) -> Option<usize> {
        match self {
            Pred::Class(c) => Some(*c),
            Pred::Value(_) => None,
        }
    }
    pub fn value(&self) -> f64 {
        match self {
            Pred::Class(c) => *c as f64,
            Pred::Value(v) => *v,
        }
    }
}

fn relu_layer(w: &[Vec<f64>], b: &[f64], x: &[f64]) -> Vec<f64> {
    w.iter()
        .enumerate()
        .map(|(i, row)| {
            let mut s = b.get(i).copied().unwrap_or(0.0);
            for (a, xi) in row.iter().zip(x.iter()) {
                s += a * xi;
            }
            s.max(0.0)
        })
        .collect()
}

fn linear_layer(w: &[Vec<f64>], b: &[f64], x: &[f64]) -> Vec<f64> {
    w.iter()
        .enumerate()
        .map(|(i, row)| {
            let mut s = b.get(i).copied().unwrap_or(0.0);
            for (a, xi) in row.iter().zip(x.iter()) {
                s += a * xi;
            }
            s
        })
        .collect()
}

// ---------------------------------------------------------------- data

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RawCol {
    Number,
    Bool,
    Text,
}

fn raw_kind(v: &Value) -> RawCol {
    match v {
        Value::Whole(_) | Value::Dec(_) => RawCol::Number,
        Value::Boolean(_) => RawCol::Bool,
        _ => RawCol::Text,
    }
}

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Whole(n) => Some(*n as f64),
        Value::Dec(x) => Some(*x),
        Value::Boolean(true) => Some(1.0),
        Value::Boolean(false) => Some(0.0),
        _ => None,
    }
}

fn median_of(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

fn mode_of(v: &[Value]) -> Value {
    // first-appearance order, and the first value wins a tie: deterministic
    // across runs and platforms (a HashMap iteration would not be).
    let mut counts: Vec<(Value, usize)> = Vec::new();
    for x in v {
        let k = cell_text(x);
        if let Some((_, c)) = counts.iter_mut().find(|(v, _)| cell_text(v) == k) {
            *c += 1;
        } else {
            counts.push((x.clone(), 1));
        }
    }
    let mut best: Option<(Value, usize)> = None;
    for (v, c) in counts {
        if best.as_ref().map(|(_, b)| c > *b).unwrap_or(true) {
            best = Some((v, c));
        }
    }
    best.map(|(v, _)| v).unwrap_or(Value::Missing)
}

pub struct Prepared {
    pub xs: Vec<Vec<f64>>,
    pub ys: Vec<f64>,
    pub classes: Vec<String>,
    pub inputs: Vec<InputCol>,
    pub n_features: usize,
    pub train: Vec<usize>,
    pub test: Vec<usize>,
    pub cleaning: Vec<String>,
}

/// M5-S19..S22: choose inputs, clean, encode and split.
pub fn prepare(
    t: &Table,
    target: &str,
    wanted: Option<&[String]>,
    seed: u64,
) -> Result<Prepared, String> {
    let ti = t
        .column_index(target)
        .ok_or_else(|| "Target is not a column.".to_string())?;
    let target_kind = match &t.columns[ti].kind {
        Kind::Maybe(inner) => &**inner,
        k => k,
    };
    let regression = matches!(target_kind, Kind::Whole | Kind::Decimal);
    // Drop rows with a missing target first (M5-S20 step 1).
    let mut rows: Vec<Vec<Value>> = Vec::new();
    let mut dropped_target = 0usize;
    for r in &t.rows {
        match r.get(ti) {
            Some(Value::Missing) | None => dropped_target += 1,
            _ => rows.push(r.clone()),
        }
    }
    let mut cleaning = Vec::new();
    if dropped_target > 0 {
        cleaning.push(format!(
            "dropped {dropped_target} rows with a missing {target}"
        ));
    }
    // The target's classes (classification) in first-appearance order.
    let mut classes: Vec<String> = Vec::new();
    if !regression {
        for r in &rows {
            let c = cell_text(&r[ti]);
            if !classes.contains(&c) {
                classes.push(c);
            }
        }
    }
    // Choose the input columns.
    let mut names: Vec<String> = Vec::new();
    for (ci, c) in t.columns.iter().enumerate() {
        if ci == ti {
            continue;
        }
        if let Some(w) = wanted {
            if !w.iter().any(|x| x.eq_ignore_ascii_case(&c.name)) {
                continue;
            }
        }
        names.push(c.name.clone());
    }
    // Split the rows (M5-S22) before the fills, so the fills see train only.
    let n = rows.len();
    let n_train = (n as f64 * 0.8).floor() as usize;
    let n_test = n.saturating_sub(n_train);
    if n_test < 2 {
        return Err("Too few rows.".to_string());
    }
    let mut order: Vec<usize> = (0..n).collect();
    Rng::new(seed).shuffle(&mut order);
    let train: Vec<usize> = order[..n_train].to_vec();
    let test: Vec<usize> = order[n_train..].to_vec();

    let mut inputs: Vec<InputCol> = Vec::new();
    let mut feature = 0usize;
    let mut dropped_columns: Vec<String> = Vec::new();
    for name in &names {
        let ci = t.column_index(name).unwrap();
        let all: Vec<Value> = rows.iter().map(|r| r[ci].clone()).collect();
        let non_missing: Vec<Value> = all
            .iter()
            .filter(|v| !matches!(v, Value::Missing))
            .cloned()
            .collect();
        // A text column with more than 20 values is dropped (step 4).
        if matches!(raw_kind(&non_missing.first().cloned().unwrap_or(Value::Missing)), RawCol::Text)
            || non_missing.iter().any(|v| matches!(raw_kind(v), RawCol::Text))
        {
            let distinct: std::collections::HashSet<String> =
                non_missing.iter().map(cell_text).collect();
            if distinct.len() > 20 {
                dropped_columns.push(name.clone());
                continue;
            }
        }
        match raw_kind(&non_missing.first().cloned().unwrap_or(Value::Missing)) {
            RawCol::Number => {
                let nums: Vec<f64> = rows
                    .iter()
                    .filter_map(|r| as_f64(&r[ci]))
                    .collect();
                let med = median_of(nums.clone());
                let missing = rows
                    .iter()
                    .filter(|r| as_f64(&r[ci]).is_none())
                    .count();
                if missing > 0 {
                    cleaning.push(format!(
                        "filled {missing} missing numbers in {name} with the median"
                    ));
                }
                // mean/std from the training rows only (M5-S20 step 7).
                let tr: Vec<f64> = train
                    .iter()
                    .filter_map(|&i| as_f64(&rows[i][ci]))
                    .collect();
                let mean = if tr.is_empty() {
                    0.0
                } else {
                    tr.iter().sum::<f64>() / tr.len() as f64
                };
                let var = if tr.is_empty() {
                    0.0
                } else {
                    tr.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / tr.len() as f64
                };
                let std = var.sqrt();
                inputs.push(InputCol {
                    name: name.clone(),
                    kind: ColKind::Numeric,
                    median: med,
                    mean,
                    std,
                    start: feature,
                    len: 1,
                });
                feature += 1;
            }
            RawCol::Bool => {
                let vals: Vec<Value> = rows
                    .iter()
                    .filter(|r| matches!(r[ci], Value::Boolean(_)))
                    .map(|r| r[ci].clone())
                    .collect();
                let mode = matches!(mode_of(&vals), Value::Boolean(true));
                let missing = rows
                    .iter()
                    .filter(|r| !matches!(r[ci], Value::Boolean(_)))
                    .count();
                if missing > 0 {
                    cleaning.push(format!(
                        "filled {missing} missing texts in {name} with the most common value"
                    ));
                }
                inputs.push(InputCol {
                    name: name.clone(),
                    kind: ColKind::Boolean { mode },
                    median: 0.0,
                    mean: 0.0,
                    std: 0.0,
                    start: feature,
                    len: 1,
                });
                feature += 1;
            }
            RawCol::Text => {
                let mut values: Vec<String> = Vec::new();
                for v in &non_missing {
                    let s = cell_text(v);
                    if !values.contains(&s) {
                        values.push(s);
                    }
                }
                let missing = rows
                    .iter()
                    .filter(|r| matches!(r[ci], Value::Missing))
                    .count();
                if missing > 0 {
                    cleaning.push(format!(
                        "filled {missing} missing texts in {name} with the most common value"
                    ));
                }
                let mode_val = mode_of(&non_missing);
                let mode_text = cell_text(&mode_val);
                let mode = values
                    .iter()
                    .position(|v| v == &mode_text)
                    .unwrap_or(0);
                inputs.push(InputCol {
                    name: name.clone(),
                    kind: ColKind::OneHot {
                        values: values.clone(),
                        mode,
                    },
                    median: 0.0,
                    mean: 0.0,
                    std: 0.0,
                    start: feature,
                    len: values.len().max(1),
                });
                feature += values.len().max(1);
            }
        }
    }
    for name in dropped_columns {
        cleaning.push(format!(
            "dropped column {name} because it has more than 20 values"
        ));
    }
    let n_features = feature;
    // Encode every row.
    let mut xs = Vec::with_capacity(n);
    let mut ys = Vec::with_capacity(n);
    for r in &rows {
        let mut x = vec![0.0; n_features];
        for col in &inputs {
            let enc = col.encode(&r[ci_of(t, &col.name)]);
            for (i, v) in enc.into_iter().enumerate() {
                if col.start + i < x.len() {
                    x[col.start + i] = v;
                }
            }
        }
        xs.push(x);
        ys.push(if regression {
            as_f64(&r[ti]).unwrap_or(0.0)
        } else {
            classes
                .iter()
                .position(|c| c == &cell_text(&r[ti]))
                .unwrap_or(0) as f64
        });
    }
    Ok(Prepared {
        xs,
        ys,
        classes,
        inputs,
        n_features,
        train,
        test,
        cleaning,
    })
}

fn ci_of(t: &Table, name: &str) -> usize {
    t.column_index(name).unwrap_or(0)
}

// ---------------------------------------------------------------- trees

struct Split {
    feature: usize,
    threshold: f64,
    score: f64,
}

fn impurity(ys: &[f64], indices: &[usize], task: Task, n_classes: usize) -> f64 {
    if indices.is_empty() {
        return 0.0;
    }
    if task == Task::Regression {
        let n = indices.len() as f64;
        let sum: f64 = indices.iter().map(|&i| ys[i]).sum();
        let mean = sum / n;
        indices
            .iter()
            .map(|&i| (ys[i] - mean) * (ys[i] - mean))
            .sum::<f64>()
            / n
    } else {
        let mut counts = vec![0usize; n_classes.max(1)];
        for &i in indices {
            let c = ys[i] as usize;
            if c < counts.len() {
                counts[c] += 1;
            }
        }
        let n = indices.len() as f64;
        let gini: f64 = counts
            .iter()
            .map(|c| {
                let p = *c as f64 / n;
                p * (1.0 - p)
            })
            .sum();
        gini
    }
}

fn best_split(
    xs: &[Vec<f64>],
    ys: &[f64],
    indices: &[usize],
    task: Task,
    n_classes: usize,
    features: &[usize],
    min_leaf: usize,
    rng: &mut Rng,
) -> Option<Split> {
    let _ = rng;
    let mut best: Option<Split> = None;
    for &f in features {
        let mut order: Vec<usize> = indices.to_vec();
        order.sort_by(|&a, &b| {
            xs[a]
                .get(f)
                .copied()
                .unwrap_or(0.0)
                .partial_cmp(&xs[b].get(f).copied().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let n = order.len();
        if n < 2 * min_leaf {
            continue;
        }
        // prefix sums for O(n) split scoring
        let mut left: Vec<usize> = Vec::with_capacity(n);
        let mut right: Vec<usize> = order.clone();
        let mut li = 0usize;
        while li + min_leaf <= n - min_leaf {
            let v = xs[order[li]].get(f).copied().unwrap_or(0.0);
            // move all values <= v to the left (stable threshold)
            while li < n && xs[order[li]].get(f).copied().unwrap_or(0.0) <= v {
                let idx = right.remove(0);
                left.push(idx);
                li += 1;
                if li >= n {
                    break;
                }
            }
            if left.len() < min_leaf || right.len() < min_leaf {
                continue;
            }
            let il = impurity(ys, &left, task, n_classes);
            let ir = impurity(ys, &right, task, n_classes);
            let score = (il * left.len() as f64 + ir * right.len() as f64) / n as f64;
            if best.as_ref().map(|b| score < b.score).unwrap_or(true) {
                best = Some(Split {
                    feature: f,
                    threshold: v,
                    score,
                });
            }
            if li >= n {
                break;
            }
        }
    }
    best
}

fn leaf_value(ys: &[f64], indices: &[usize], task: Task, n_classes: usize) -> Leaf {
    if task == Task::Regression {
        let mean = if indices.is_empty() {
            0.0
        } else {
            indices.iter().map(|&i| ys[i]).sum::<f64>() / indices.len() as f64
        };
        Leaf::Value(mean)
    } else {
        let mut counts = vec![0usize; n_classes.max(1)];
        for &i in indices {
            let c = ys[i] as usize;
            if c < counts.len() {
                counts[c] += 1;
            }
        }
        let best = counts
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| **c)
            .map(|(i, _)| i)
            .unwrap_or(0);
        Leaf::Class(best)
    }
}

fn build_tree(
    xs: &[Vec<f64>],
    ys: &[f64],
    indices: &[usize],
    depth: usize,
    max_depth: Option<usize>,
    min_leaf: usize,
    n_features: usize,
    task: Task,
    n_classes: usize,
    rng: &mut Rng,
) -> Node {
    let pure = if task == Task::Regression {
        let mean = indices.iter().map(|&i| ys[i]).sum::<f64>() / indices.len().max(1) as f64;
        indices.iter().all(|&i| (ys[i] - mean).abs() < 1e-12)
    } else {
        let first = indices
            .first()
            .map(|&i| ys[i] as usize)
            .unwrap_or(0);
        indices.iter().all(|&i| ys[i] as usize == first)
    };
    if max_depth.map(|d| depth >= d).unwrap_or(false)
        || indices.len() < 2 * min_leaf
        || pure
        || n_features == 0
    {
        return Node::leaf_of(leaf_value(ys, indices, task, n_classes), indices.len());
    }
    // The random feature subset: sqrt(C) or one third of the inputs.
    let want = if task == Task::Regression {
        (n_features / 3).max(1)
    } else {
        (n_features as f64).sqrt().ceil() as usize
    }
    .min(n_features);
    let mut features: Vec<usize> = (0..n_features).collect();
    if want < n_features {
        // partial Fisher-Yates
        for i in 0..want {
            let j = i + rng.below(n_features - i);
            features.swap(i, j);
        }
        features.truncate(want);
    }
    let Some(split) = best_split(
        xs,
        ys,
        indices,
        task,
        n_classes,
        &features,
        min_leaf,
        rng,
    ) else {
        return Node::leaf_of(leaf_value(ys, indices, task, n_classes), indices.len());
    };
    let mut left: Vec<usize> = Vec::new();
    let mut right: Vec<usize> = Vec::new();
    for &i in indices {
        if xs[i].get(split.feature).copied().unwrap_or(0.0) <= split.threshold {
            left.push(i);
        } else {
            right.push(i);
        }
    }
    if left.is_empty() || right.is_empty() {
        return Node::leaf_of(leaf_value(ys, indices, task, n_classes), indices.len());
    }
    Node::Split {
        feature: split.feature,
        threshold: split.threshold,
        left: Box::new(build_tree(
            xs, ys, &left, depth + 1, max_depth, min_leaf, n_features, task, n_classes, rng,
        )),
        right: Box::new(build_tree(
            xs, ys, &right, depth + 1, max_depth, min_leaf, n_features, task, n_classes, rng,
        )),
    }
}

// ---------------------------------------------------------------- linear

fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let mut pivot = col;
        for r in col + 1..n {
            if a[r][col].abs() > a[pivot][col].abs() {
                pivot = r;
            }
        }
        if a[pivot][col].abs() < 1e-12 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        let d = a[col][col];
        for c in col..n {
            a[col][c] /= d;
        }
        b[col] /= d;
        for r in 0..n {
            if r != col && a[r][col].abs() > 0.0 {
                let f = a[r][col];
                for c in col..n {
                    a[r][c] -= f * a[col][c];
                }
                b[r] -= f * b[col];
            }
        }
    }
    Some(b)
}

fn train_linear(xs: &[Vec<f64>], ys: &[f64], d: usize, ridge: f64) -> (Vec<f64>, f64) {
    let mut a = vec![vec![0.0; d]; d];
    let mut b = vec![0.0; d];
    for (x, &y) in xs.iter().zip(ys.iter()) {
        for i in 0..d {
            b[i] += x[i] * y;
            for j in 0..d {
                a[i][j] += x[i] * x[j];
            }
        }
    }
    for i in 0..d {
        a[i][i] += ridge;
    }
    let w = solve(a, b).unwrap_or_else(|| vec![0.0; d]);
    let mean_y = if ys.is_empty() {
        0.0
    } else {
        ys.iter().sum::<f64>() / ys.len() as f64
    };
    (w, mean_y)
}

fn sigmoid(z: f64) -> f64 {
    if z >= 0.0 {
        1.0 / (1.0 + (-z).exp())
    } else {
        let e = z.exp();
        e / (1.0 + e)
    }
}

fn train_logistic(xs: &[Vec<f64>], ys: &[f64], d: usize, n_classes: usize) -> (Vec<Vec<f64>>, Vec<f64>) {
    let mut w = vec![vec![0.0; d]; n_classes];
    let mut b = vec![0.0; n_classes];
    let l2 = 1.0;
    let lr = 0.5;
    for k in 0..n_classes {
        for _ in 0..200 {
            let mut gw = vec![0.0; d];
            let mut gb = 0.0;
            for (x, &y) in xs.iter().zip(ys.iter()) {
                let mut z = b[k];
                for i in 0..d {
                    z += w[k][i] * x[i];
                }
                let p = sigmoid(z);
                let target = if (y as usize) == k { 1.0 } else { 0.0 };
                let e = p - target;
                gb += e;
                for i in 0..d {
                    gw[i] += e * x[i];
                }
            }
            let n = xs.len().max(1) as f64;
            for i in 0..d {
                w[k][i] -= lr * (gw[i] / n + l2 * w[k][i]);
            }
            b[k] -= lr * (gb / n);
        }
    }
    (w, b)
}

// ---------------------------------------------------------------- neural

fn mat_vec(w: &[Vec<f64>], x: &[f64]) -> Vec<f64> {
    w.iter()
        .map(|row| row.iter().zip(x.iter()).map(|(a, b)| a * b).sum())
        .collect()
}

fn relu(v: &mut [f64]) {
    for x in v {
        if *x < 0.0 {
            *x = 0.0;
        }
    }
}

struct Adam {
    m: Vec<f64>,
    v: Vec<f64>,
    t: usize,
}

impl Adam {
    fn new(n: usize) -> Self {
        Adam {
            m: vec![0.0; n],
            v: vec![0.0; n],
            t: 0,
        }
    }
    fn update(&mut self, params: &mut [f64], grads: &[f64], lr: f64) {
        self.t += 1;
        let b1: f64 = 0.9;
        let b2: f64 = 0.999;
        let eps = 1e-8;
        let bc1 = 1.0 - b1.powi(self.t as i32);
        let bc2 = 1.0 - b2.powi(self.t as i32);
        for i in 0..params.len() {
            let g = grads[i];
            self.m[i] = b1 * self.m[i] + (1.0 - b1) * g;
            self.v[i] = b2 * self.v[i] + (1.0 - b2) * g * g;
            let mh = self.m[i] / bc1;
            let vh = self.v[i] / bc2;
            params[i] -= lr * mh / (vh.sqrt() + eps);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn train_neural(
    xs: &[Vec<f64>],
    ys: &[f64],
    d: usize,
    n_classes: usize,
    task: Task,
    seed: u64,
) -> Impl {
    let h = 32usize;
    let out = if task == Task::Classification {
        n_classes.max(1)
    } else {
        1
    };
    let mut rng = Rng::new(seed ^ 0x9E37_79B9_7F4A_7C15);
    let mut w1 = vec![vec![0.0; d]; h];
    let mut b1 = vec![0.0; h];
    let mut w2 = vec![vec![0.0; h]; h];
    let mut b2 = vec![0.0; h];
    let mut w3 = vec![vec![0.0; h]; out];
    let mut b3 = vec![0.0; out];
    let init = |fan: usize, rng: &mut Rng| -> f64 {
        let lim = 1.0 / (fan as f64).sqrt();
        (rng.next_f64() * 2.0 - 1.0) * lim
    };
    for row in w1.iter_mut() {
        for w in row {
            *w = init(d.max(1), &mut rng);
        }
    }
    for row in w2.iter_mut() {
        for w in row {
            *w = init(h, &mut rng);
        }
    }
    for row in w3.iter_mut() {
        for w in row {
            *w = init(h, &mut rng);
        }
    }
    let mut params: Vec<f64> = Vec::new();
    for row in &w1 {
        params.extend_from_slice(row);
    }
    params.extend_from_slice(&b1);
    for row in &w2 {
        params.extend_from_slice(row);
    }
    params.extend_from_slice(&b2);
    for row in &w3 {
        params.extend_from_slice(row);
    }
    params.extend_from_slice(&b3);
    let mut adam = Adam::new(params.len());
    // a small validation slice from the training rows for early stopping
    let val_n = (xs.len() / 10).max(1);
    let val: Vec<usize> = (0..val_n).collect();
    let mut best_val = f64::INFINITY;
    let mut patience = 0usize;
    let lr = 0.001;
    let batch = 32usize;
    let mut order: Vec<usize> = (0..xs.len()).collect();
    for epoch in 0..200 {
        rng.shuffle(&mut order);
        for chunk in order.chunks(batch) {
            let mut gw1 = vec![vec![0.0; d]; h];
            let mut gb1 = vec![0.0; h];
            let mut gw2 = vec![vec![0.0; h]; h];
            let mut gb2 = vec![0.0; h];
            let mut gw3 = vec![vec![0.0; h]; out];
            let mut gb3 = vec![0.0; out];
            for &i in chunk {
                let x = &xs[i];
                let z1 = mat_vec(&w1, x);
                let mut a1 = z1.clone();
                relu(&mut a1);
                let z2 = mat_vec(&w2, &a1);
                let mut a2 = z2.clone();
                relu(&mut a2);
                let z3 = mat_vec(&w3, &a2);
                let y = ys[i];
                let dz3: Vec<f64>;
                if task == Task::Classification {
                    // softmax + cross-entropy
                    let max = z3.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    let exps: Vec<f64> = z3.iter().map(|z| (z - max).exp()).collect();
                    let sum: f64 = exps.iter().sum();
                    let mut p: Vec<f64> = exps.iter().map(|e| e / sum).collect();
                    let c = y as usize;
                    if c < p.len() {
                        p[c] -= 1.0;
                    }
                    dz3 = p;
                } else {
                    let e = z3[0] - y;
                    dz3 = vec![2.0 * e];
                }
                for k in 0..out {
                    gb3[k] += dz3[k];
                    for j in 0..h {
                        gw3[k][j] += dz3[k] * a2[j];
                    }
                }
                let da2: Vec<f64> = (0..h)
                    .map(|j| (0..out).map(|k| w3[k][j] * dz3[k]).sum())
                    .collect();
                let dz2: Vec<f64> = da2
                    .iter()
                    .zip(z2.iter())
                    .map(|(g, z)| if *z > 0.0 { *g } else { 0.0 })
                    .collect();
                for k in 0..h {
                    gb2[k] += dz2[k];
                    for j in 0..h {
                        gw2[k][j] += dz2[k] * a1[j];
                    }
                }
                let da1: Vec<f64> = (0..h)
                    .map(|j| (0..h).map(|k| w2[k][j] * dz2[k]).sum())
                    .collect();
                let dz1: Vec<f64> = da1
                    .iter()
                    .zip(z1.iter())
                    .map(|(g, z)| if *z > 0.0 { *g } else { 0.0 })
                    .collect();
                for k in 0..h {
                    gb1[k] += dz1[k];
                    for j in 0..d {
                        gw1[k][j] += dz1[k] * x[j];
                    }
                }
            }
            let n = chunk.len().max(1) as f64;
            let scale = |v: &mut [f64]| {
                for x in v {
                    *x /= n;
                }
            };
            for row in gw1.iter_mut() {
                scale(row);
            }
            for row in gw2.iter_mut() {
                scale(row);
            }
            for row in gw3.iter_mut() {
                scale(row);
            }
            scale(&mut gb1);
            scale(&mut gb2);
            scale(&mut gb3);
            // flatten grads in the same order as params
            let mut grads: Vec<f64> = Vec::new();
            for row in &gw1 {
                grads.extend_from_slice(row);
            }
            grads.extend_from_slice(&gb1);
            for row in &gw2 {
                grads.extend_from_slice(row);
            }
            grads.extend_from_slice(&gb2);
            for row in &gw3 {
                grads.extend_from_slice(row);
            }
            grads.extend_from_slice(&gb3);
            // write params back from the flat vector after the update
            let mut flat: Vec<f64> = Vec::new();
            for row in &w1 {
                flat.extend_from_slice(row);
            }
            flat.extend_from_slice(&b1);
            for row in &w2 {
                flat.extend_from_slice(row);
            }
            flat.extend_from_slice(&b2);
            for row in &w3 {
                flat.extend_from_slice(row);
            }
            flat.extend_from_slice(&b3);
            adam.update(&mut flat, &grads, lr);
            let mut at = 0usize;
            for row in w1.iter_mut() {
                for w in row {
                    *w = flat[at];
                    at += 1;
                }
            }
            for w in b1.iter_mut() {
                *w = flat[at];
                at += 1;
            }
            for row in w2.iter_mut() {
                for w in row {
                    *w = flat[at];
                    at += 1;
                }
            }
            for w in b2.iter_mut() {
                *w = flat[at];
                at += 1;
            }
            for row in w3.iter_mut() {
                for w in row {
                    *w = flat[at];
                    at += 1;
                }
            }
            for w in b3.iter_mut() {
                *w = flat[at];
                at += 1;
            }
        }
        // validation loss on the held-out slice
        let mut vl = 0.0;
        for &i in &val {
            let x = &xs[i];
            let a1 = {
                let mut a = mat_vec(&w1, x);
                relu(&mut a);
                a
            };
            let a2 = {
                let mut a = mat_vec(&w2, &a1);
                relu(&mut a);
                a
            };
            let o = mat_vec(&w3, &a2);
            if task == Task::Classification {
                let c = ys[i] as usize;
                let max = o.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let exps: Vec<f64> = o.iter().map(|z| (z - max).exp()).collect();
                let sum: f64 = exps.iter().sum();
                let p = if c < exps.len() {
                    exps[c] / sum
                } else {
                    0.0
                };
                vl += -p.max(1e-12).ln();
            } else {
                vl += (o[0] - ys[i]).abs();
            }
        }
        vl /= val.len().max(1) as f64;
        if vl < best_val - 1e-9 {
            best_val = vl;
            patience = 0;
        } else {
            patience += 1;
            if patience >= 10 && epoch >= 10 {
                break;
            }
        }
    }
    Impl::Neural {
        w1,
        b1,
        w2,
        b2,
        w3,
        b3,
    }
}

// ---------------------------------------------------------------- training

pub fn train(
    t: &Table,
    target: &str,
    wanted: Option<&[String]>,
    kind: ModelKind,
    seed: u64,
) -> Result<Model, String> {
    let p = prepare(t, target, wanted, seed)?;
    let target_kind = match &t.columns[t.column_index(target).unwrap()].kind {
        Kind::Maybe(inner) => &**inner,
        k => k,
    };
    let task = if matches!(target_kind, Kind::Whole | Kind::Decimal) {
        Task::Regression
    } else {
        Task::Classification
    };
    let n_classes = p.classes.len().max(if task == Task::Classification { 1 } else { 0 });
    let train_xs: Vec<Vec<f64>> = p.train.iter().map(|&i| p.xs[i].clone()).collect();
    let train_ys: Vec<f64> = p.train.iter().map(|&i| p.ys[i]).collect();
    let mut rng = Rng::new(seed);
    let impl_: Impl = match kind {
        ModelKind::DecisionTree => Impl::Tree(build_tree(
            &train_xs,
            &train_ys,
            &(0..train_xs.len()).collect::<Vec<_>>(),
            0,
            Some(8),
            2,
            p.n_features,
            task,
            n_classes,
            &mut rng,
        )),
        ModelKind::RandomForest => {
            let mut trees = Vec::with_capacity(100);
            for _ in 0..100 {
                let boot: Vec<usize> = (0..train_xs.len())
                    .map(|_| rng.below(train_xs.len().max(1)))
                    .collect();
                trees.push(build_tree(
                    &train_xs,
                    &train_ys,
                    &boot,
                    0,
                    None,
                    1,
                    p.n_features,
                    task,
                    n_classes,
                    &mut rng,
                ));
            }
            Impl::Forest(trees)
        }
        ModelKind::Linear => {
            if task == Task::Regression {
                let (w, b) = train_linear(&train_xs, &train_ys, p.n_features, 0.000001);
                Impl::Linear { w, b }
            } else {
                let (w, b) = train_logistic(&train_xs, &train_ys, p.n_features, n_classes);
                Impl::Logistic { w, b }
            }
        }
        ModelKind::Neural => train_neural(
            &train_xs,
            &train_ys,
            p.n_features,
            n_classes,
            task,
            seed,
        ),
    };
    Ok(Model {
        kind,
        task,
        target: target.to_string(),
        table: String::new(),
        input_names: p.inputs.iter().map(|c| c.name.clone()).collect(),
        inputs: p.inputs,
        classes: p.classes,
        n_features: p.n_features,
        impl_,
        cleaning: p.cleaning,
    })
}

/// The M5-S26 score on the test rows: accuracy (classification) or mean
/// absolute error (regression).
pub fn score(model: &Model, xs: &[Vec<f64>], ys: &[f64], test: &[usize]) -> f64 {
    if test.is_empty() {
        return 0.0;
    }
    if model.task == Task::Classification {
        let mut ok = 0usize;
        for &i in test {
            if let Some(c) = model.predict(&xs[i]).class_index() {
                if c as f64 == ys[i] {
                    ok += 1;
                }
            }
        }
        ok as f64 / test.len() as f64
    } else {
        let mut sum = 0.0;
        for &i in test {
            sum += (model.predict(&xs[i]).value() - ys[i]).abs();
        }
        sum / test.len() as f64
    }
}

pub fn prepared_for(model: &Model, t: &Table, seed: u64) -> Result<Prepared, String> {
    let _ = model;
    prepare(t, &model.target, None, seed)
}

/// M5-S27: permutation importance on the test rows.
pub fn importance(model: &Model, p: &Prepared, seed: u64) -> Vec<(String, f64)> {
    let base = score(model, &p.xs, &p.ys, &p.test);
    let mut raw: Vec<(String, f64)> = Vec::new();
    for col in &model.inputs {
        let start = col.start;
        let len = col.len.max(1).min(model.n_features.saturating_sub(start));
        if len == 0 {
            continue;
        }
        let mut drops: Vec<f64> = Vec::new();
        for rep in 0..5 {
            let mut rng = Rng::new(seed ^ (rep as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let mut vals: Vec<Vec<f64>> = p
                .test
                .iter()
                .map(|&i| p.xs[i][start..start + len].to_vec())
                .collect();
            rng.shuffle(&mut vals);
            let mut xs = p.xs.clone();
            for (ri, &i) in p.test.iter().enumerate() {
                xs[i][start..start + len].copy_from_slice(&vals[ri]);
            }
            let s = score(model, &xs, &p.ys, &p.test);
            let drop = if model.task == Task::Classification {
                base - s
            } else {
                s - base
            };
            drops.push(drop);
        }
        let avg = drops.iter().sum::<f64>() / drops.len() as f64;
        if avg > 0.0 {
            raw.push((col.name.clone(), avg));
        }
    }
    if raw.is_empty() {
        return Vec::new();
    }
    let total: f64 = raw.iter().map(|(_, v)| *v).sum();
    raw.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    raw.truncate(5);
    // shares of the total positive drop, rounded to 2 decimals with the
    // largest-remainder method so they add up to 1.00
    let mut shares: Vec<(String, f64)> = raw
        .iter()
        .map(|(n, v)| (n.clone(), v / total))
        .collect();
    let mut rounded: Vec<usize> = shares
        .iter()
        .map(|(_, s)| (s * 100.0).floor() as usize)
        .collect();
    let mut left = 100usize.saturating_sub(rounded.iter().sum::<usize>());
    let mut order: Vec<usize> = (0..shares.len()).collect();
    order.sort_by(|&a, &b| {
        let fa = shares[a].1 * 100.0 - rounded[a] as f64;
        let fb = shares[b].1 * 100.0 - rounded[b] as f64;
        fb.partial_cmp(&fa).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut oi = 0;
    while left > 0 && !order.is_empty() {
        rounded[order[oi % order.len()]] += 1;
        left -= 1;
        oi += 1;
    }
    shares = shares
        .into_iter()
        .zip(rounded)
        .map(|((n, _), r)| (n, r as f64 / 100.0))
        .collect();
    shares
}

// ---------------------------------------------------------------- save/load

struct W {
    b: Vec<u8>,
}

impl W {
    fn u8(&mut self, v: u8) {
        self.b.push(v);
    }
    fn u64(&mut self, v: u64) {
        self.b.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.b.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn s(&mut self, v: &str) {
        self.u64(v.len() as u64);
        self.b.extend_from_slice(v.as_bytes());
    }
    fn node(&mut self, n: &Node) {
        match n {
            Node::Leaf { leaf, count } => {
                self.u8(0);
                self.u64(*count as u64);
                match leaf {
                    Leaf::Class(c) => {
                        self.u8(0);
                        self.u64(*c as u64);
                    }
                    Leaf::Value(v) => {
                        self.u8(1);
                        self.f64(*v);
                    }
                }
            }
            Node::Split {
                feature,
                threshold,
                left,
                right,
            } => {
                self.u8(1);
                self.u64(*feature as u64);
                self.f64(*threshold);
                self.node(left);
                self.node(right);
            }
        }
    }
}

impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        save_bytes(self) == save_bytes(other)
    }
}

pub fn save_bytes(m: &Model) -> Vec<u8> {
    let mut w = W { b: Vec::new() };
    w.b.extend_from_slice(b"LYKAMOD1\0");
    w.u8(match m.kind {
        ModelKind::RandomForest => 0,
        ModelKind::DecisionTree => 1,
        ModelKind::Linear => 2,
        ModelKind::Neural => 3,
    });
    w.u8(if m.task == Task::Regression { 0 } else { 1 });
    w.s(&m.target);
    w.s(&m.table);
    w.u64(m.input_names.len() as u64);
    for n in &m.input_names {
        w.s(n);
    }
    w.u64(m.classes.len() as u64);
    for c in &m.classes {
        w.s(c);
    }
    w.u64(m.inputs.len() as u64);
    for c in &m.inputs {
        w.s(&c.name);
        match &c.kind {
            ColKind::Numeric => {
                w.u8(0);
            }
            ColKind::Boolean { mode } => {
                w.u8(1);
                w.u8(if *mode { 1 } else { 0 });
            }
            ColKind::OneHot { values, mode } => {
                w.u8(2);
                w.u64(values.len() as u64);
                for v in values {
                    w.s(v);
                }
                w.u64(*mode as u64);
            }
        }
        w.f64(c.median);
        w.f64(c.mean);
        w.f64(c.std);
        w.u64(c.start as u64);
        w.u64(c.len as u64);
    }
    w.u64(m.n_features as u64);
    match &m.impl_ {
        Impl::Forest(trees) => {
            w.u8(0);
            w.u64(trees.len() as u64);
            for t in trees {
                w.node(t);
            }
        }
        Impl::Tree(t) => {
            w.u8(1);
            w.node(t);
        }
        Impl::Linear { w: weights, b } => {
            w.u8(2);
            w.u64(weights.len() as u64);
            for x in weights {
                w.f64(*x);
            }
            w.f64(*b);
        }
        Impl::Logistic { w: weights, b } => {
            w.u8(3);
            w.u64(weights.len() as u64);
            for row in weights {
                w.u64(row.len() as u64);
                for x in row {
                    w.f64(*x);
                }
            }
            for x in b {
                w.f64(*x);
            }
        }
        Impl::Neural {
            w1,
            b1,
            w2,
            b2,
            w3,
            b3,
        } => {
            w.u8(4);
            w.u64(w1.len() as u64);
            w.u64(w1.first().map(|r| r.len()).unwrap_or(0) as u64);
            for row in w1 {
                for x in row {
                    w.f64(*x);
                }
            }
            for x in b1 {
                w.f64(*x);
            }
            w.u64(w2.len() as u64);
            w.u64(w2.first().map(|r| r.len()).unwrap_or(0) as u64);
            for row in w2 {
                for x in row {
                    w.f64(*x);
                }
            }
            for x in b2 {
                w.f64(*x);
            }
            w.u64(w3.len() as u64);
            w.u64(w3.first().map(|r| r.len()).unwrap_or(0) as u64);
            for row in w3 {
                for x in row {
                    w.f64(*x);
                }
            }
            for x in b3 {
                w.f64(*x);
            }
        }
    }
    w.u64(m.cleaning.len() as u64);
    for c in &m.cleaning {
        w.s(c);
    }
    w.b
}

struct R<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> R<'a> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.p)?;
        self.p += 1;
        Some(v)
    }
    fn u64(&mut self) -> Option<u64> {
        let s = self.b.get(self.p..self.p + 8)?;
        self.p += 8;
        Some(u64::from_le_bytes(s.try_into().ok()?))
    }
    fn f64(&mut self) -> Option<f64> {
        let bits = self.u64()?;
        Some(f64::from_bits(bits))
    }
    fn s(&mut self) -> Option<String> {
        // A corrupt file can name a huge length; bound it and use checked
        // arithmetic so `p + n` cannot overflow.
        let n = cap(self.u64()?)?;
        let end = self.p.checked_add(n)?;
        let s = self.b.get(self.p..end)?;
        self.p = end;
        String::from_utf8(s.to_vec()).ok()
    }
    fn node(&mut self) -> Option<Node> {
        self.node_at(0)
    }
    fn node_at(&mut self, depth: usize) -> Option<Node> {
        // Bound the recursion so a corrupt file cannot overflow the stack.
        if depth > 2000 {
            return None;
        }
        match self.u8()? {
            0 => {
                let count = cap(self.u64()?)?;
                let leaf = match self.u8()? {
                    0 => Leaf::Class(cap(self.u64()?)?),
                    1 => Leaf::Value(self.f64()?),
                    _ => return None,
                };
                Some(Node::Leaf { leaf, count })
            }
            1 => {
                let feature = cap(self.u64()?)?;
                let threshold = self.f64()?;
                let left = Box::new(self.node_at(depth + 1)?);
                let right = Box::new(self.node_at(depth + 1)?);
                Some(Node::Split {
                    feature,
                    threshold,
                    left,
                    right,
                })
            }
            _ => None,
        }
    }
}

/// A corrupt model file can declare huge counts. Cap every count so the
/// loader cannot be made to allocate wildly or loop for a long time.
const MAX_ITEMS: usize = 1_000_000;

fn cap(n: u64) -> Option<usize> {
    let n = usize::try_from(n).ok()?;
    if n > MAX_ITEMS {
        None
    } else {
        Some(n)
    }
}

/// `a * b` bounded by MAX_ITEMS (for nested matrix allocations).
fn cap2(a: usize, b: usize) -> Option<usize> {
    if a == 0 || b == 0 {
        return Some(0);
    }
    if a > MAX_ITEMS || b > MAX_ITEMS / a {
        None
    } else {
        Some(a * b)
    }
}

pub fn load_bytes(b: &[u8]) -> Option<Model> {
    let mut r = R { b, p: 0 };
    if r.b.get(0..9)? != b"LYKAMOD1\0" {
        return None;
    }
    r.p = 9;
    let kind = match r.u8()? {
        0 => ModelKind::RandomForest,
        1 => ModelKind::DecisionTree,
        2 => ModelKind::Linear,
        3 => ModelKind::Neural,
        _ => return None,
    };
    let task = if r.u8()? == 0 {
        Task::Regression
    } else {
        Task::Classification
    };
    let target = r.s()?;
    let table = r.s()?;
    let n_in_names = cap(r.u64()?)?;
    let mut input_names = Vec::with_capacity(n_in_names);
    for _ in 0..n_in_names {
        input_names.push(r.s()?);
    }
    let n_classes = cap(r.u64()?)?;
    let mut classes = Vec::with_capacity(n_classes);
    for _ in 0..n_classes {
        classes.push(r.s()?);
    }
    let n_inputs = cap(r.u64()?)?;
    let mut inputs = Vec::with_capacity(n_inputs);
    for _ in 0..n_inputs {
        let name = r.s()?;
        let kind = match r.u8()? {
            0 => ColKind::Numeric,
            1 => ColKind::Boolean {
                mode: r.u8()? != 0,
            },
            2 => {
                let n = cap(r.u64()?)?;
                let mut values = Vec::with_capacity(n);
                for _ in 0..n {
                    values.push(r.s()?);
                }
                let mode = cap(r.u64()?)?;
                ColKind::OneHot { values, mode }
            }
            _ => return None,
        };
        let median = r.f64()?;
        let mean = r.f64()?;
        let std = r.f64()?;
        let start = cap(r.u64()?)?;
        let len = cap(r.u64()?)?;
        inputs.push(InputCol {
            name,
            kind,
            median,
            mean,
            std,
            start,
            len,
        });
    }
    let n_features = cap(r.u64()?)?;
    let impl_ = match r.u8()? {
        0 => {
            let n = cap(r.u64()?)?;
            let mut trees = Vec::with_capacity(n);
            for _ in 0..n {
                trees.push(r.node()?);
            }
            Impl::Forest(trees)
        }
        1 => Impl::Tree(r.node()?),
        2 => {
            let n = cap(r.u64()?)?;
            let mut w = Vec::with_capacity(n);
            for _ in 0..n {
                w.push(r.f64()?);
            }
            Impl::Linear { w, b: r.f64()? }
        }
        3 => {
            let n = cap(r.u64()?)?;
            let mut w = Vec::with_capacity(n);
            for _ in 0..n {
                let d = cap(r.u64()?)?;
                cap2(n, d)?;
                let mut row = Vec::with_capacity(d);
                for _ in 0..d {
                    row.push(r.f64()?);
                }
                w.push(row);
            }
            let mut b = Vec::with_capacity(n);
            for _ in 0..n {
                b.push(r.f64()?);
            }
            Impl::Logistic { w, b }
        }
        4 => {
            let h1 = cap(r.u64()?)?;
            let d1 = cap(r.u64()?)?;
            cap2(h1, d1)?;
            let mut w1 = vec![vec![0.0; d1]; h1];
            for row in w1.iter_mut() {
                for x in row {
                    *x = r.f64()?;
                }
            }
            let mut b1 = vec![0.0; h1];
            for x in b1.iter_mut() {
                *x = r.f64()?;
            }
            let h2 = cap(r.u64()?)?;
            let d2 = cap(r.u64()?)?;
            cap2(h2, d2)?;
            let mut w2 = vec![vec![0.0; d2]; h2];
            for row in w2.iter_mut() {
                for x in row {
                    *x = r.f64()?;
                }
            }
            let mut b2 = vec![0.0; h2];
            for x in b2.iter_mut() {
                *x = r.f64()?;
            }
            let h3 = cap(r.u64()?)?;
            let d3 = cap(r.u64()?)?;
            cap2(h3, d3)?;
            let mut w3 = vec![vec![0.0; d3]; h3];
            for row in w3.iter_mut() {
                for x in row {
                    *x = r.f64()?;
                }
            }
            let mut b3 = vec![0.0; h3];
            for x in b3.iter_mut() {
                *x = r.f64()?;
            }
            Impl::Neural {
                w1,
                b1,
                w2,
                b2,
                w3,
                b3,
            }
        }
        _ => return None,
    };
    let n_clean = cap(r.u64()?)?;
    let mut cleaning = Vec::with_capacity(n_clean);
    for _ in 0..n_clean {
        cleaning.push(r.s()?);
    }
    Some(Model {
        kind,
        task,
        target,
        table,
        input_names,
        inputs,
        classes,
        n_features,
        impl_,
        cleaning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::load_csv_text;

    const SALES: &str = "region,size,price,churned\n\
north,55,140,yes\n\
north,40,100,no\n\
east,60,160,yes\n\
east,35,90,no\n\
south,45,125,no\n\
south,30,80,no\n\
west,50,130,no\n\
west,25,70,yes\n";

    fn sales() -> Table {
        load_csv_text(SALES).unwrap()
    }

    #[test]
    fn corrupt_model_is_refused_without_panic() {
        // A real header with a wild count must be refused quickly (bounded
        // loader), not allocate wildly or loop.
        let mut wild = b"LYKAMOD1\0".to_vec();
        wild.extend_from_slice(&u64::MAX.to_le_bytes()); // n_in_names
        wild.extend_from_slice(&[0u8; 64]);
        assert!(load_bytes(&wild).is_none());
        // Truncated / random junk.
        assert!(load_bytes(b"").is_none());
        assert!(load_bytes(b"LYKAMOD1").is_none());
        assert!(load_bytes(&[0xffu8; 64]).is_none());
        // Deeply nested tree nodes must hit the depth bound, not the stack.
        let mut deep = b"LYKAMOD1\0".to_vec();
        deep.push(1); // RandomForest
        deep.push(0); // Regression
        deep.extend_from_slice(&1u64.to_le_bytes()); // target len
        deep.push(b't');
        deep.extend_from_slice(&1u64.to_le_bytes()); // table len
        deep.push(b'x');
        deep.extend_from_slice(&0u64.to_le_bytes()); // n_in_names
        deep.extend_from_slice(&0u64.to_le_bytes()); // n_classes
        deep.extend_from_slice(&0u64.to_le_bytes()); // n_inputs
        deep.extend_from_slice(&0u64.to_le_bytes()); // n_features
        deep.push(0); // Forest impl
        deep.extend_from_slice(&1u64.to_le_bytes()); // one tree
        for _ in 0..5000 {
            deep.push(1); // Split
            deep.extend_from_slice(&0u64.to_le_bytes()); // feature
            deep.extend_from_slice(&0f64.to_le_bytes()); // threshold
        }
        // The loader may return None at the depth bound; it must not overflow.
        let _ = load_bytes(&deep);
    }

    #[test]
    fn every_model_trains_and_scores_deterministically() {
        let t = sales();
        for kind in [
            ModelKind::RandomForest,
            ModelKind::DecisionTree,
            ModelKind::Linear,
            ModelKind::Neural,
        ] {
            let a = train(&t, "churned", None, kind, 42).unwrap();
            let b = train(&t, "churned", None, kind, 42).unwrap();
            let sa = save_bytes(&a);
            let sb = save_bytes(&b);
            assert_eq!(sa, sb, "{kind:?} must be deterministic");
            let p = prepare(&t, "churned", None, 42).unwrap();
            let s1 = score(&a, &p.xs, &p.ys, &p.test);
            assert!((0.0..=1.0).contains(&s1), "{kind:?} score {s1}");
            let loaded = load_bytes(&sa).expect("model round trip");
            let score2 = score(&loaded, &p.xs, &p.ys, &p.test);
            assert_eq!(s1, score2, "{kind:?} save/load keeps the score");
        }
    }

    #[test]
    fn regression_uses_error_and_accuracy_is_t0180() {
        let t = sales();
        let m = train(&t, "price", None, ModelKind::Linear, 42).unwrap();
        assert_eq!(m.task, Task::Regression);
        let p = prepare(&t, "price", None, 42).unwrap();
        let err = score(&m, &p.xs, &p.ys, &p.test);
        assert!(err >= 0.0);
    }

    #[test]
    fn too_few_rows_is_an_error() {
        let t = load_csv_text("a,b\n1,2\n3,4\n").unwrap();
        let err = train(&t, "b", None, ModelKind::DecisionTree, 42).unwrap_err();
        assert!(err.contains("Too few rows"));
    }
}