use harper_core::expr::SequenceExpr;
use harper_core::patterns::{UPOSSet, WordSet};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Mirror {
    pub seq: Vec<MirrorAtom>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum MirrorAtom {
    AllowedWords(Vec<String>),
    DisallowedWords(Vec<String>),
    UPOS(UPOSSet),
    Whitespace,
}

impl Mirror {
    pub fn to_seq_expr(&self) -> SequenceExpr {
        let mut seq = SequenceExpr::default();
        for atom in &self.seq {
            match atom {
                MirrorAtom::AllowedWords(items) => {
                    let mut set = WordSet::default();

                    for item in items {
                        set.add(&item);
                    }

                    seq = seq.then(set)
                }
                MirrorAtom::DisallowedWords(items) => {
                    let mut set = WordSet::default();

                    for item in items {
                        set.add(&item);
                    }

                    seq = seq.then_unless(set)
                }
                MirrorAtom::UPOS(uposset) => seq = seq.then(uposset.clone()),
                MirrorAtom::Whitespace => seq = seq.t_ws(),
            };
        }

        seq
    }
}
