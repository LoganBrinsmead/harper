mod mirror;

use clap::Parser;
use harper_core::Document;
use harper_core::expr::ExprExt;
use rand::seq::SliceRandom;
use rayon::slice::ParallelSliceMut;
use std::fs;
use std::time::Instant;

use self::mirror::{Mirror, MirrorAtom, MirrorLayer, MirrorNode};

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

    /// Optional seed word to initialize the search with
    #[arg(long)]
    seed: Option<String>,
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

    let mut mirs = vec![if let Some(seed) = args.seed.clone() {
        Mirror {
            root: MirrorNode::Leaf(MirrorLayer {
                seq: vec![MirrorAtom::Word(seed)],
            }),
        }
    } else {
        // No seed provided: start with an empty leaf and let mutation explore
        Mirror {
            root: MirrorNode::Leaf(MirrorLayer { seq: vec![] }),
        }
    }];

    let mut last_best_score = 0;

    for i in 0..args.generations {
        let start_time = Instant::now();

        mirs.truncate(args.min_pop);

        let mut perm_mirs = Vec::new();
        let mut rng = rand::rng();

        for mir in &mirs {
            perm_mirs.append(&mut mir.create_children_with_mutations(
                args.child_ratio,
                args.max_mutations,
                &mut rng,
            ));
        }

        mirs.append(&mut perm_mirs);

        mirs.shuffle(&mut rand::rng());

        mirs.par_sort_by_cached_key(|s| {
            let score = score(s, &problems, &clean);
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

        if let Some(best_mir) = mirs.first() {
            println!("Best mirror: {:#?}", best_mir);
        }

        println!(
            "Generation {:<4} | Best Score: {:<10} | Max Score: {:<10} | Delta: {:<+10} | Candidates/sec: {:<10}",
            i,
            best_score,
            max_possible_score(&problems, &clean),
            delta,
            candidates_per_second
        );

        last_best_score = best_score;
    }
}

fn load_documents(path: &str) -> Vec<Document> {
    fs::read_to_string(path)
        .expect("Unable to read file.")
        .lines()
        .map(Document::new_plain_english_curated)
        .collect()
}

// Treat correctness as the dominant term and use simplicity as a small bonus.
// "Simpler" = fewer non-whitespace atoms and smaller UPOS sets.
fn mirror_complexity(m: &Mirror) -> usize {
    fn layer_cost(layer: &MirrorLayer) -> usize {
        let mut cost = 0usize;
        for atom in &layer.seq {
            match atom {
                MirrorAtom::Whitespace => {
                    cost += 1;
                }
                MirrorAtom::Word(_) => {
                    cost += 1;
                }
                MirrorAtom::UPOS(set) => {
                    cost += set.len();
                }
                MirrorAtom::NPMember => {
                    // Simple boolean check; treat similar to a basic atom
                    cost += 1;
                }
            }
        }
        cost
    }

    fn node_cost(node: &MirrorNode) -> usize {
        match node {
            MirrorNode::Leaf(layer) => layer_cost(layer),
            MirrorNode::And(children) | MirrorNode::Or(children) => {
                children.iter().map(node_cost).sum::<usize>() + 2
            }
        }
    }

    node_cost(&m.root)
}

// Upper bound for simplicity bonus points. This should be small relative
// to correctness so it cannot outweigh getting sentences right.
const SIMPLICITY_BONUS_MAX: usize = 150;

fn score(candidate: &Mirror, problems: &[Document], clean: &[Document]) -> usize {
    let expr = candidate.to_expr();

    // Clean correctness: percentage of clean sentences with zero matches.
    let mut clean_correct = 0usize;
    for c in clean {
        if expr.iter_matches_in_doc(c).next().is_none() {
            clean_correct += 1;
        }
    }
    let clean_pct = if clean.is_empty() {
        0usize
    } else {
        (clean_correct * 100) / clean.len()
    };

    // Problem correctness: percentage of problem sentences that are flagged.
    // Preserve the existing notion of a "correct" flag as exactly one match.
    let mut problem_correct = 0usize;
    for p in problems {
        let mut it = expr.iter_matches_in_doc(p);
        let first = it.next().is_some();
        let second = it.next().is_none();
        if first && second {
            problem_correct += 1;
        }
    }
    let problem_pct = if problems.is_empty() {
        0usize
    } else {
        (problem_correct * 100) / problems.len()
    };

    // Combined correctness: clean is weighted 2x as requested.
    let correctness_score = clean_pct * 2 + problem_pct;

    // Small simplicity bonus in 0..=SIMPLICITY_BONUS_MAX, decreasing with complexity.
    let complexity = mirror_complexity(candidate);
    let simplicity_bonus = SIMPLICITY_BONUS_MAX.saturating_sub(complexity);

    correctness_score * 100 + simplicity_bonus
}

pub fn max_possible_score(problems: &[Document], clean: &[Document]) -> usize {
    // Max correctness = (2x clean%) + (1x problem%).
    // Only count a component if its corpus is present.
    let mut max = 0usize;
    if !clean.is_empty() {
        max += 200; // 2 * 100%
    }
    if !problems.is_empty() {
        max += 100; // 1 * 100%
    }
    max * 100 + SIMPLICITY_BONUS_MAX
}
