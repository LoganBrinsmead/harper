use harper_brill::UPOS;
use harper_core::expr::{All, LongestMatchOf, SequenceExpr};
use harper_core::patterns::{UPOSSet, WordSet};
use rand::seq::{IndexedRandom, SliceRandom};
use rand::{Rng, random_bool};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::IntoEnumIterator;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Mirror {
    pub layers: Vec<MirrorLayer>,
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

    fn create_random_layer(rng: &mut impl Rng) -> Self {
        let len = rng.random_range(1..=3);
        let mut seq = Vec::with_capacity(len);
        for _ in 0..len {
            seq.push(Self::create_random_step(rng));
        }
        Self { seq }
    }
}

impl Mirror {
    pub fn to_expr(&self) -> All {
        let mut all = All::default();
        for layer in &self.layers {
            all.add(layer.to_seq_expr());
        }
        all
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
        if self.layers.is_empty() || rng.random_bool(0.2) {
            let i = rng.random_range(0..=self.layers.len());
            self.layers.insert(i, MirrorLayer::create_random_layer(rng));
            return;
        }

        if rng.random_bool(0.1) && !self.layers.is_empty() {
            let i = rng.random_range(0..self.layers.len());
            self.layers.remove(i);
            return;
        }

        if rng.random_bool(0.1) && self.layers.len() >= 2 {
            self.layers.shuffle(rng);
            return;
        }

        let i = rng.random_range(0..self.layers.len());
        let layer = &mut self.layers[i];

        if rng.random_bool(0.6) {
            layer.mutate(rng);
        } else {
            if rng.random_bool(0.5) {
                let insert_at = rng.random_range(0..=layer.seq.len());
                layer
                    .seq
                    .insert(insert_at, MirrorLayer::create_random_step(rng));
            } else if !layer.seq.is_empty() {
                let remove_at = rng.random_range(0..layer.seq.len());
                layer.seq.remove(remove_at);
            }
        }
    }
}
