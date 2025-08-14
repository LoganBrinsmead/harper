use harper_brill::UPOS;
use harper_core::expr::SequenceExpr;
use harper_core::patterns::{UPOSSet, Word, WordSet};
use harper_core::spell::Dictionary;
use rand::seq::{IndexedRandom, SliceRandom};
use rand::{Rng, random_bool};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use strum::IntoEnumIterator;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Mirror {
    pub seq: Vec<MirrorAtom>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MirrorAtom {
    Word(String),
    UPOS(SmallVec<[UPOS; 16]>),
    Whitespace,
}

impl Mirror {
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
            };
        }

        seq
    }

    /// Creates children with random mutations.
    pub fn create_children_with_mutations(
        &self,
        child_count: usize,
        rng: &mut impl Rng,
    ) -> Vec<Self> {
        let mut children = Vec::with_capacity(child_count);

        for _ in 0..child_count {
            let mut child = self.clone();
            child.mutate(rng);
            children.push(child);
        }

        children
    }

    pub fn mutate(&mut self, rng: &mut impl Rng) {
        if !self.seq.is_empty() && rng.random_bool(0.5) {
            let i = rng.random_range(0..self.seq.len());
            let step = &mut self.seq[i];
            Self::mutate_step(step, rng);
        } else {
            let i = rng.random_range(0..self.seq.len());
            self.seq.insert(i, Self::create_random_step(rng));
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
            _ => (),
        }
    }

    fn create_random_step(rng: &mut impl Rng) -> MirrorAtom {
        if random_bool(0.5) {
            let mut all_upos: SmallVec<[UPOS; 16]> = UPOS::iter().collect();
            all_upos.shuffle(rng);
            all_upos.truncate(rng.random_range(0..all_upos.len()));
            MirrorAtom::UPOS(all_upos)
        } else {
            MirrorAtom::Whitespace
        }
    }
}
