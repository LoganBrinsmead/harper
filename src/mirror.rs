use harper_brill::UPOS;
use harper_core::expr::{All, Expr, LongestMatchOf, SequenceExpr};
use harper_core::patterns::{UPOSSet, WordSet};
use rand::seq::{IndexedRandom, SliceRandom};
use rand::{Rng, random_bool};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::IntoEnumIterator;

/// A tree of expressions.
/// - `And`: every child must match (logical AND).
/// - `Or`: at least one child must match (logical OR, longest-match semantics).
/// - `Leaf`: a concrete `SequenceExpr` built from a `MirrorLayer`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MirrorNode {
    And(Vec<MirrorNode>),
    Or(Vec<MirrorNode>),
    Leaf(MirrorLayer),
}

impl Default for MirrorNode {
    fn default() -> Self {
        Self::Leaf(MirrorLayer { seq: vec![] })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Mirror {
    pub root: MirrorNode,
}

/// Back-compat constructor for callers that still think in “layers = AND of sequences”.
impl From<Vec<MirrorLayer>> for Mirror {
    fn from(layers: Vec<MirrorLayer>) -> Self {
        Mirror {
            root: MirrorNode::And(layers.into_iter().map(MirrorNode::Leaf).collect()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MirrorLayer {
    pub seq: Vec<MirrorAtom>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MirrorAtom {
    Word(String),
    UPOS(SmallVec<[UPOS; 16]>),
    Whitespace,
}

impl MirrorLayer {
    pub fn to_seq_expr(&self) -> SequenceExpr {
        let mut seq = SequenceExpr::default();
        for atom in &self.seq {
            match atom {
                MirrorAtom::UPOS(uposset) => seq = seq.then(UPOSSet::new(&uposset)),
                MirrorAtom::Whitespace => seq = seq.t_ws(),
                MirrorAtom::Word(word) => {
                    let mut set = WordSet::default();
                    set.add_chars(&word.chars().collect::<Vec<_>>());
                    seq = seq.then(set)
                }
            }
        }
        seq
    }

    pub fn mutate(&mut self, rng: &mut impl Rng) {
        if !self.seq.is_empty() && rng.random_bool(0.5) {
            let i = rng.random_range(0..self.seq.len());
            let step = &mut self.seq[i];
            Self::mutate_step(step, rng);
        } else {
            let i = rng.random_range(0..=self.seq.len());
            self.seq.insert(i, Self::create_random_step(rng));
        }

        if !self.seq.is_empty() && rng.random_bool(0.1) {
            let i = rng.random_range(0..self.seq.len());
            self.seq.remove(i);
        }
    }

    fn mutate_step(atom: &mut MirrorAtom, rng: &mut impl Rng) {
        match atom {
            MirrorAtom::UPOS(uposset) => {
                if !uposset.is_empty() && rng.random_bool(0.5) {
                    uposset.remove(rng.random_range(0..uposset.len()));
                }
                if rng.random_bool(0.5) {
                    uposset.push(UPOS::iter().nth(rng.random_range(0..16)).unwrap());
                }
            }
            MirrorAtom::Word(w) => {
                if rng.random_bool(0.2) && !w.is_empty() {
                    let chars: Vec<char> = w.chars().collect();
                    let take = rng.random_range(1..=chars.len());
                    let new_w: String = chars.into_iter().take(take).collect();
                    *w = new_w;
                }
            }
            MirrorAtom::Whitespace => {}
        }
    }

    fn create_random_step(rng: &mut impl Rng) -> MirrorAtom {
        if random_bool(0.5) {
            let mut all_upos: SmallVec<[UPOS; 16]> = UPOS::iter().collect();
            all_upos.shuffle(rng);
            all_upos.truncate(rng.random_range(0..all_upos.len()));
            MirrorAtom::UPOS(all_upos)
        } else if random_bool(0.2) {
            MirrorAtom::Word(String::from("the"))
        } else {
            MirrorAtom::Whitespace
        }
    }

    pub fn create_random_layer(rng: &mut impl Rng) -> Self {
        let len = rng.random_range(1..=3);
        let mut seq = Vec::with_capacity(len);
        for _ in 0..len {
            seq.push(Self::create_random_step(rng));
        }
        Self { seq }
    }
}

impl MirrorNode {
    /// Build a boxed `Expr` for this subtree.
    /// AND => `All`, OR => `LongestMatchOf`, Leaf => `SequenceExpr`.
    pub fn to_owned_expr(&self) -> Box<dyn Expr> {
        match self {
            MirrorNode::Leaf(layer) => Box::new(layer.to_seq_expr()),
            MirrorNode::And(children) => {
                let mut v: Vec<Box<dyn Expr>> = Vec::with_capacity(children.len().max(1));
                for c in children {
                    v.push(c.to_owned_expr());
                }
                Box::new(All::new(v))
            }
            MirrorNode::Or(children) => {
                let mut v: Vec<Box<dyn Expr>> = Vec::with_capacity(children.len().max(1));
                for c in children {
                    v.push(c.to_owned_expr());
                }
                Box::new(LongestMatchOf::new(v))
            }
        }
    }

    /// Randomly mutate the tree:
    /// - Recurse into a child and mutate it
    /// - Insert/remove a child
    /// - Flip AND <-> OR
    /// - Wrap/unwrap a leaf to introduce structure
    pub fn mutate(&mut self, rng: &mut impl Rng) {
        match self {
            MirrorNode::Leaf(layer) => {
                // 70%: tweak the leaf; 30%: promote to a tiny AND/OR subtree
                if rng.random_bool(0.7) {
                    layer.mutate(rng);
                } else {
                    let new_leaf = MirrorNode::Leaf(MirrorLayer::create_random_layer(rng));
                    if rng.random_bool(0.5) {
                        *self = MirrorNode::And(vec![MirrorNode::Leaf(layer.clone()), new_leaf]);
                    } else {
                        *self = MirrorNode::Or(vec![MirrorNode::Leaf(layer.clone()), new_leaf]);
                    }
                }
            }
            MirrorNode::And(children) | MirrorNode::Or(children) => {
                let len = children.len();

                // Occasionally flip operator
                if rng.random_bool(0.1) {
                    let swapped = match std::mem::take(self) {
                        MirrorNode::And(c) => MirrorNode::Or(c),
                        MirrorNode::Or(c) => MirrorNode::And(c),
                        MirrorNode::Leaf(_) => unreachable!(),
                    };
                    *self = swapped;
                    return;
                }

                // Sometimes reorder for variety
                if len >= 2 && rng.random_bool(0.1) {
                    children.shuffle(rng);
                }

                // Insert / remove
                if rng.random_bool(0.25) {
                    let idx = rng.random_range(0..=len);
                    children.insert(idx, MirrorNode::Leaf(MirrorLayer::create_random_layer(rng)));
                } else if len > 1 && rng.random_bool(0.15) {
                    let idx = rng.random_range(0..len);
                    children.remove(idx);
                } else if !children.is_empty() {
                    // Recurse into a child
                    let idx = rng.random_range(0..children.len());
                    children[idx].mutate(rng);
                }

                // Occasionally collapse a singleton (unwrap) or wrap a pair
                if children.len() == 1 && rng.random_bool(0.15) {
                    // Replace this node with its only child
                    let only = children.remove(0);
                    *self = only;
                } else if children.len() >= 2 && rng.random_bool(0.15) {
                    // Wrap two adjacent children into a new AND/OR group
                    let idx = rng.random_range(0..children.len() - 1);
                    let a = children.remove(idx);
                    let b = children.remove(idx); // same idx: list shrank
                    let wrapped = if rng.random_bool(0.5) {
                        MirrorNode::And(vec![a, b])
                    } else {
                        MirrorNode::Or(vec![a, b])
                    };
                    children.insert(idx, wrapped);
                }
            }
        }
    }
}

impl Mirror {
    pub fn to_expr(&self) -> Box<dyn Expr> {
        self.root.to_owned_expr()
    }

    pub fn create_children_with_mutations(
        &self,
        child_count: usize,
        max_mutations: usize,
        rng: &mut impl Rng,
    ) -> Vec<Self> {
        let mut children = Vec::with_capacity(child_count);
        for _ in 0..child_count {
            let mut child = self.clone();
            let mutation_count = rng.gen_range(1..=max_mutations);
            for _ in 0..mutation_count {
                child.mutate(rng);
            }
            children.push(child);
        }
        children
    }

    pub fn mutate(&mut self, rng: &mut impl Rng) {
        // 20%: wrap root to add a new level (AND/OR); 10%: replace root with a fresh leaf; otherwise recurse.
        if rng.random_bool(0.2) {
            let wrapper_is_and = rng.random_bool(0.5);
            let sibling = MirrorNode::Leaf(MirrorLayer::create_random_layer(rng));
            let old = std::mem::replace(
                &mut self.root,
                MirrorNode::Leaf(MirrorLayer::create_random_layer(rng)),
            );
            self.root = if wrapper_is_and {
                MirrorNode::And(vec![old, sibling])
            } else {
                MirrorNode::Or(vec![old, sibling])
            };
            return;
        }

        if rng.random_bool(0.1) {
            self.root = MirrorNode::Leaf(MirrorLayer::create_random_layer(rng));
            return;
        }

        self.root.mutate(rng);
    }
}
