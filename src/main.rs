mod mirror;

use clap::Parser;
use harper_core::Document;
use harper_core::expr::ExprExt;
use rand::seq::SliceRandom;
use rayon::iter::ParallelIterator;
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
        root: MirrorNode::Leaf(MirrorLayer {
            seq: vec![MirrorAtom::Word("to".to_owned())],
        }),
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

// Treat correctness as the dominant term and use simplicity as a tiebreaker.
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
                    cost += set.len().max(1);
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

const TIE_SCALE: usize = 400;
const PROBLEM_REWARD: usize = 500;
const CLEAN_REWARD: usize = 1000;

fn score(candidate: &Mirror, problems: &[Document], clean: &[Document]) -> usize {
    let expr = candidate.to_expr();

    let mut correct = 0usize;

    // Early-exit counting: avoid scanning entire document when not necessary.
    for problem in problems {
        let mut it = expr.iter_matches_in_doc(problem);
        let first = it.next().is_some();
        let second = it.next().is_none();
        if first && second {
            // Exactly one match
            correct += PROBLEM_REWARD;
        }
    }

    for clean in clean {
        if expr.iter_matches_in_doc(clean).next().is_none() {
            correct += CLEAN_REWARD;
        }
    }

    let simplicity_bonus = TIE_SCALE.saturating_sub(mirror_complexity(candidate).min(TIE_SCALE));

    correct.saturating_mul(TIE_SCALE) + simplicity_bonus
}

pub fn max_possible_score(problems: &[Document], clean: &[Document]) -> usize {
    let correctness = PROBLEM_REWARD
        .saturating_mul(problems.len())
        .saturating_add(CLEAN_REWARD.saturating_mul(clean.len()));

    correctness
        .saturating_mul(TIE_SCALE)
        .saturating_add(TIE_SCALE)
}
