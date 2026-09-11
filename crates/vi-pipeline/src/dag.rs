//! Derive the execution graph from operators' declared inputs and outputs.

use std::collections::{BTreeMap, BTreeSet};

use vi_core::{Error, Result};

use crate::operator::{ItemKind, Operator};

/// Operators in topological order, each operator's consumer indices, and the
/// indices of root operators.
pub type DagParts = (Vec<Box<dyn Operator>>, Vec<Vec<usize>>, Vec<usize>);

/// A validated, topologically ordered set of operators.
pub struct Dag {
    ops: Vec<Box<dyn Operator>>,
    /// For each operator index, the indices of operators consuming any of
    /// its outputs.
    consumers: Vec<Vec<usize>>,
    /// Operators consuming the root `Media` item.
    roots: Vec<usize>,
}

impl std::fmt::Debug for Dag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dag")
            .field(
                "order",
                &self.ops.iter().map(|o| o.id()).collect::<Vec<_>>(),
            )
            .field("consumers", &self.consumers)
            .field("roots", &self.roots)
            .finish()
    }
}

impl Dag {
    /// Build from operators in any order. Every input kind must be produced
    /// by another operator or be [`ItemKind::Media`]; cycles are rejected.
    pub fn build(ops: Vec<Box<dyn Operator>>) -> Result<Self> {
        // producers: kind -> operator indices
        let mut producers: BTreeMap<ItemKind, Vec<usize>> = BTreeMap::new();
        for (i, op) in ops.iter().enumerate() {
            for k in op.outputs() {
                producers.entry(*k).or_default().push(i);
            }
        }
        let mut ids = BTreeSet::new();
        for op in &ops {
            if !ids.insert(op.id()) {
                return Err(Error::invalid(format!("duplicate operator '{}'", op.id())));
            }
        }
        // deps: operator -> operators it depends on
        let mut deps: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); ops.len()];
        let mut roots = Vec::new();
        for (i, op) in ops.iter().enumerate() {
            let mut is_root = false;
            for k in op.inputs() {
                if *k == ItemKind::Media {
                    is_root = true;
                    continue;
                }
                match producers.get(k) {
                    Some(p) if !p.is_empty() => deps[i].extend(p.iter().copied()),
                    _ => {
                        return Err(Error::invalid(format!(
                            "operator '{}' needs {:?} but nothing in the policy produces it",
                            op.id(),
                            k
                        )))
                    }
                }
            }
            if is_root {
                roots.push(i);
            }
            if op.inputs().is_empty() {
                return Err(Error::invalid(format!(
                    "operator '{}' declares no inputs",
                    op.id()
                )));
            }
        }
        // Kahn's algorithm for a topological order and cycle detection.
        let mut indeg: Vec<usize> = deps.iter().map(BTreeSet::len).collect();
        let mut ready: Vec<usize> = (0..ops.len()).filter(|i| indeg[*i] == 0).collect();
        let mut order = Vec::with_capacity(ops.len());
        while let Some(i) = ready.pop() {
            order.push(i);
            for (j, d) in deps.iter().enumerate() {
                if d.contains(&i) {
                    indeg[j] -= 1;
                    if indeg[j] == 0 {
                        ready.push(j);
                    }
                }
            }
        }
        if order.len() != ops.len() {
            return Err(Error::invalid("operator graph has a cycle"));
        }
        // Reindex into topological order.
        let mut pos = vec![0usize; ops.len()];
        for (new, old) in order.iter().enumerate() {
            pos[*old] = new;
        }
        let mut sorted: Vec<Option<Box<dyn Operator>>> = ops.into_iter().map(Some).collect();
        let mut ops_sorted: Vec<Box<dyn Operator>> = Vec::with_capacity(sorted.len());
        for old in &order {
            if let Some(op) = sorted[*old].take() {
                ops_sorted.push(op);
            }
        }
        let mut consumers = vec![Vec::new(); ops_sorted.len()];
        for (j_old, d) in deps.iter().enumerate() {
            for i_old in d {
                consumers[pos[*i_old]].push(pos[j_old]);
            }
        }
        for c in &mut consumers {
            c.sort_unstable();
            c.dedup();
        }
        let roots = roots.into_iter().map(|r| pos[r]).collect();
        Ok(Self {
            ops: ops_sorted,
            consumers,
            roots,
        })
    }

    /// Operators in topological order.
    pub fn operators(&self) -> &[Box<dyn Operator>] {
        &self.ops
    }

    /// Indices of consumers of operator `i`'s outputs.
    pub fn consumers_of(&self, i: usize) -> &[usize] {
        &self.consumers[i]
    }

    /// Indices of operators that take the root `Media` item.
    pub fn roots(&self) -> &[usize] {
        &self.roots
    }

    /// Stage names in order.
    pub fn stage_names(&self) -> Vec<String> {
        self.ops.iter().map(|o| o.id().to_string()).collect()
    }

    /// Consume the DAG.
    pub fn into_parts(self) -> DagParts {
        (self.ops, self.consumers, self.roots)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator::*;
    use async_trait::async_trait;

    struct Fake {
        id: &'static str,
        i: Vec<ItemKind>,
        o: Vec<ItemKind>,
    }

    #[async_trait]
    impl Operator for Fake {
        fn id(&self) -> &'static str {
            self.id
        }
        fn version(&self) -> u32 {
            1
        }
        fn inputs(&self) -> &[InputKind] {
            &self.i
        }
        fn outputs(&self) -> &[OutputKind] {
            &self.o
        }
        fn cost_estimate(&self, _: &InputSummary) -> CostEstimate {
            CostEstimate::default()
        }
        async fn run(&self, _: &OpContext, _: OpInput) -> Result<OpOutput> {
            Ok(OpOutput::default())
        }
    }

    fn op(id: &'static str, i: &[ItemKind], o: &[ItemKind]) -> Box<dyn Operator> {
        Box::new(Fake {
            id,
            i: i.to_vec(),
            o: o.to_vec(),
        })
    }

    #[test]
    fn orders_and_wires_consumers() {
        let dag = Dag::build(vec![
            op("thumbnail", &[ItemKind::Frame], &[ItemKind::Thumbnail]),
            op("phash", &[ItemKind::Frame], &[ItemKind::Hashed]),
            op("sample", &[ItemKind::Media], &[ItemKind::Frame]),
        ])
        .unwrap();
        let names = dag.stage_names();
        assert_eq!(names[0], "sample");
        assert_eq!(dag.roots(), &[0]);
        assert_eq!(dag.consumers_of(0).len(), 2);
        assert!(dag.consumers_of(1).is_empty());
    }

    #[test]
    fn rejects_missing_producer_cycle_and_duplicates() {
        assert!(Dag::build(vec![op("phash", &[ItemKind::Frame], &[ItemKind::Hashed])]).is_err());
        assert!(Dag::build(vec![
            op("a", &[ItemKind::Hashed], &[ItemKind::Frame]),
            op("b", &[ItemKind::Frame], &[ItemKind::Hashed]),
        ])
        .is_err());
        assert!(Dag::build(vec![
            op("sample", &[ItemKind::Media], &[ItemKind::Frame]),
            op("sample", &[ItemKind::Media], &[ItemKind::Frame]),
        ])
        .is_err());
    }
}
