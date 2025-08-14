mod mirror;

use clap::Parser;
use harper_brill::UPOS;
use harper_core::Document;
use harper_core::expr::{ExprExt, SequenceExpr};
use harper_core::patterns::UPOSSet;
use harper_core::spell::{Dictionary, FstDictionary};
use rand::seq::IndexedRandom;
use rand::{Rng, seq::SliceRandom};
use rayon::slice::ParallelSliceMut;
use std::fs;
use strum::IntoEnumIterator;

use self::mirror::{Mirror, MirrorAtom};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The ratio of children to generate for each generation
    #[arg(short, long, default_value_t = 10000)]
    child_ratio: usize,

    /// The minimum population size to maintain
    #[arg(short, long, default_value_t = 10)]
    min_pop: usize,

    /// A file containing newline-separated sentences that should be flagged
    #[arg(long)]
    problem_file: String,

    /// A file containing newline-separated sentences that should not be flagged
    #[arg(long)]
    clean_file: String,

    /// The number of generations to run
    #[arg(short, long)]
    generations: usize,
}

fn main() {
    let args = Args::parse();

    let problems = load_documents(&args.problem_file);
    let clean = load_documents(&args.clean_file);

    let mut mirs = vec![Mirror {
        seq: vec![MirrorAtom::Word("too".to_string())],
    }];

    for _i in 0..args.generations {
        mirs.par_sort_by_cached_key(|s| {
            let score = score(&s.to_seq_expr(), &problems, &clean);
            usize::MAX - score
        });

        for i in 0..4.min(mirs.len()) {
            dbg!(
                &mirs[i],
                score(&mirs[i].to_seq_expr(), &problems, &clean),
                problems.len() + clean.len()
            );
        }

        mirs.truncate(args.min_pop);

        let mut perm_mirs = Vec::new();

        for mir in &mirs {
            perm_mirs.append(
                &mut mir.create_children_with_mutations(args.child_ratio, &mut rand::rng()),
            );
        }

        mirs.append(&mut perm_mirs);
        mirs.shuffle(&mut rand::rng());
    }
}

fn load_documents(path: &str) -> Vec<Document> {
    fs::read_to_string(path)
        .expect("Unable to read file")
        .lines()
        .map(|s| Document::new_plain_english_curated(s))
        .collect()
}

fn score(candidate: &SequenceExpr, problems: &[Document], clean: &[Document]) -> usize {
    let mut correct = 0;

    let mut matches = Vec::new();

    for problem in problems {
        matches.clear();
        matches.extend(candidate.iter_matches_in_doc(problem));

        if matches.len() == 1 {
            correct += 1;
        }
    }

    for clean in clean {
        if candidate.iter_matches_in_doc(clean).count() == 0 {
            correct += 1;
        }
    }

    correct
}
