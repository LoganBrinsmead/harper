mod mirror;

use clap::Parser;
use harper_brill::UPOS;
use harper_core::Document;
use harper_core::expr::{ExprExt, SequenceExpr};
use harper_core::patterns::UPOSSet;
use harper_core::spell::{Dictionary, FstDictionary};
use rand::seq::IndexedRandom;
use rand::seq::SliceRandom;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rayon::slice::ParallelSliceMut;
use std::fs;
use std::time::Instant;

use self::mirror::{Mirror, MirrorAtom, MirrorLayer};

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

    /// The maximum number of mutations to apply to a child
    #[arg(long, default_value_t = 5)]
    max_mutations: usize,

    /// The number of jobs (threads) to use for parallel processing.
    /// If not specified, Rayon will use the default number of threads.
    #[arg(short, long)]
    jobs: Option<usize>,
}

fn main() {
    let args = Args::parse();

    if let Some(jobs) = args.jobs {
        rayon::ThreadPoolBuilder::new()
            .num_threads(jobs)
            .build_global()
            .unwrap();
    }

    let problems = load_documents(&args.problem_file);
    let clean = load_documents(&args.clean_file);

    let mut mirs = vec![Mirror {
        layers: vec![MirrorLayer {
            seq: vec![MirrorAtom::Word("to".to_string())],
        }],
    }];

    let mut last_best_score = 0;

    for i in 0..args.generations {
        let start_time = Instant::now();

        mirs.par_sort_by_cached_key(|s| {
            let score = score(&s, &problems, &clean);
            usize::MAX - score
        });

        let best_score = if let Some(best_mir) = mirs.first() {
            score(best_mir, &problems, &clean)
        } else {
            0
        };

        let delta = best_score as i64 - last_best_score as i64;
        let elapsed = start_time.elapsed();
        let candidates_per_second = (mirs.len() as f64 / elapsed.as_secs_f64()) as usize;

        println!(
            "Generation {:<4} | Best Score: {:<10} | Max Score: {:<10} | Delta: {:<+10} | Candidates/sec: {:<10}",
            i,
            best_score,
            max_possible_score(&problems, &clean),
            delta,
            candidates_per_second
        );

        if let Some(best_mir) = mirs.first() {
            println!("Best mirror: {:#?}", best_mir);
        }

        last_best_score = best_score;

        mirs.truncate(args.min_pop);

        let mut perm_mirs = Vec::new();

        for mir in &mirs {
            perm_mirs.append(&mut mir.create_children_with_mutations(
                args.child_ratio,
                args.max_mutations,
                &mut rand::thread_rng(),
            ));
        }

        mirs.append(&mut perm_mirs);

        mirs.shuffle(&mut rand::thread_rng());
    }
}

fn load_documents(path: &str) -> Vec<Document> {
    fs::read_to_string(path)
        .expect("Unable to read file.")
        .lines()
        .map(|s| Document::new_plain_english_curated(s))
        .collect()
}

// Treat correctness as the dominant term and use simplicity as a tiebreaker.
// "Simpler" = fewer non-whitespace atoms and smaller UPOS sets.
fn mirror_complexity(m: &Mirror) -> usize {
    use MirrorAtom::*;
    let mut cost = 0usize;

    for layer in &m.layers {
        for atom in &layer.seq {
            match atom {
                Whitespace => {} // free
                Word(_w) => {
                    cost += 1;
                }
                UPOS(set) => {
                    cost += set.len().max(1);
                }
            }
        }
    }

    cost
}

fn score(candidate: &Mirror, problems: &[Document], clean: &[Document]) -> usize {
    let expr = candidate.to_expr();

    let mut correct = 0usize;

    for problem in problems {
        if expr.iter_matches_in_doc(problem).count() == 1 {
            correct += 50;
        }
    }

    for clean in clean {
        if expr.iter_matches_in_doc(clean).count() == 0 {
            correct += 100;
        }
    }

    const TIE_SCALE: usize = 25;
    let simplicity_bonus = TIE_SCALE.saturating_sub(mirror_complexity(candidate).min(TIE_SCALE));

    correct.saturating_mul(TIE_SCALE) + simplicity_bonus
}

pub fn max_possible_score(problems: &[Document], clean: &[Document]) -> usize {
    const TIE_SCALE: usize = 25;
    let per_problem = 50usize;
    let per_clean = 100usize;

    let correctness = per_problem
        .saturating_mul(problems.len())
        .saturating_add(per_clean.saturating_mul(clean.len()));

    correctness
        .saturating_mul(TIE_SCALE)
        .saturating_add(TIE_SCALE)
}
