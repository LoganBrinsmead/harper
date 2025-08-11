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
}

fn main() {
    let args = Args::parse();

    let problems = load_sentences(&args.problem_file);
    let clean = load_sentences(&args.clean_file);

    let dict = FstDictionary::curated();

    let mut mirs = vec![Mirror {
        seq: vec![MirrorAtom::AllowedWords(vec!["too".to_string()])],
    }];

    for i in 0..200 {
        mirs.par_sort_by_cached_key(|s| {
            let score = score(&s.to_seq_expr(), &problems, &clean);
            usize::MAX - score
        });

        for i in 0..3.min(mirs.len()) {
            dbg!(
                &mirs[i],
                score(&mirs[i].to_seq_expr(), &problems, &clean),
                problems.len() + clean.len()
            );
        }

        mirs.truncate(args.min_pop);

        let mut perm_mirs = Vec::new();

        for mir in &mirs {
            perm_mirs.append(&mut permute_mirror_unique(mir, args.child_ratio, &dict));
        }

        mirs.append(&mut perm_mirs);
        mirs.shuffle(&mut rand::rng());
    }
}

fn build_word_pool<D: Dictionary>(dict: Option<&D>, rng: &mut impl Rng) -> Vec<String> {
    match dict {
        Some(d) => {
            let cap = d.word_count().min(5_000);
            let mut pool = Vec::with_capacity(cap);
            for w in d.words_iter().take(cap) {
                let s: String = w.iter().collect();
                if !s.chars().any(|c| c.is_whitespace()) {
                    pool.push(s);
                }
            }
            pool.shuffle(rng);
            if pool.is_empty() {
                fallback_words()
            } else {
                pool
            }
        }
        None => fallback_words(),
    }
}

fn choose_words(pool: &[String], k: usize, rng: &mut impl Rng) -> Vec<String> {
    if pool.is_empty() {
        return fallback_words()
            .choose_multiple(rng, k)
            .cloned()
            .map(|s| s.to_string())
            .collect();
    }
    pool.choose_multiple(rng, k).cloned().collect()
}

fn fallback_words() -> Vec<String> {
    [
        "alpha", "brisk", "candle", "delta", "ember", "flora", "glyph", "harbor", "ionic",
        "juniper", "keystone", "lumen", "modest", "nylon", "opal", "quartz", "rivet", "spruce",
        "topaz", "umber", "vivid", "willow", "xenon", "yonder", "zephyr",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

pub fn permute_mirror_unique<D: Dictionary>(base: &Mirror, n: usize, dict: &D) -> Vec<Mirror> {
    let mut rng = rand::thread_rng();
    let word_pool = build_word_pool(Some(dict), &mut rng);

    let mut out = Vec::with_capacity(n);

    let mut attempts = 0usize;
    while out.len() < n && attempts < n * 80 {
        attempts += 1;

        let mut m = Mirror {
            seq: base.seq.clone(),
        };
        if rng.gen_bool(0.35) {
            m.seq.push(random_non_ws_atom(&word_pool, &mut rng));
        } else {
            apply_random_edit(&mut m, &word_pool, &mut rng);
        }

        if rng.gen_bool(0.80) {
            m.seq.push(MirrorAtom::Whitespace);
        }

        out.push(m);
    }

    out
}

fn apply_random_edit(m: &mut Mirror, pool: &[String], rng: &mut impl Rng) {
    for _ in 0..6 {
        let i = rng.gen_range(0..m.seq.len());
        match &mut m.seq[i] {
            MirrorAtom::AllowedWords(words) => {
                if rng.gen_bool(0.65) {
                    *words = distinct_words(pool, words.len(), rng, Some(words));
                } else {
                    m.seq[i] = flip_from_words(words.len(), pool, rng);
                }
            }
            MirrorAtom::DisallowedWords(words) => {
                if rng.gen_bool(0.65) {
                    *words = distinct_words(pool, words.len(), rng, Some(words));
                } else {
                    m.seq[i] = flip_from_words(words.len(), pool, rng);
                }
            }
            MirrorAtom::UPOS(_) => {
                if rng.gen_bool(0.65) {
                    m.seq[i] = MirrorAtom::UPOS(random_upos_set(rng));
                } else {
                    m.seq[i] = flip_from_upos(pool, rng);
                }
            }
            MirrorAtom::Whitespace => {
                m.seq[i] = if rng.gen_bool(0.55) {
                    random_non_ws_atom(pool, rng)
                } else {
                    MirrorAtom::UPOS(random_upos_set(rng))
                };
            }
        }
    }
}

fn flip_from_words(len: usize, pool: &[String], rng: &mut impl Rng) -> MirrorAtom {
    match rng.gen_range(0..=2) {
        0 => MirrorAtom::DisallowedWords(distinct_words(pool, len.max(1), rng, None)),
        1 => MirrorAtom::UPOS(random_upos_set(rng)),
        _ => MirrorAtom::Whitespace,
    }
}

fn flip_from_upos(pool: &[String], rng: &mut impl Rng) -> MirrorAtom {
    match rng.gen_range(0..=2) {
        0 => MirrorAtom::AllowedWords(distinct_words(pool, rng.gen_range(1..=3), rng, None)),
        1 => MirrorAtom::DisallowedWords(distinct_words(pool, rng.gen_range(1..=3), rng, None)),
        _ => MirrorAtom::Whitespace,
    }
}

fn random_non_ws_atom(pool: &[String], rng: &mut impl Rng) -> MirrorAtom {
    match rng.gen_range(0..=2) {
        0 => MirrorAtom::AllowedWords(distinct_words(pool, rng.gen_range(1..=3), rng, None)),
        1 => MirrorAtom::DisallowedWords(distinct_words(pool, rng.gen_range(1..=3), rng, None)),
        _ => MirrorAtom::UPOS(random_upos_set(rng)),
    }
}

fn random_upos_set(rng: &mut impl Rng) -> UPOSSet {
    let v = UPOS::iter().collect::<Vec<_>>();
    let k = rng.gen_range(1..v.len());
    let mut v: Vec<UPOS> = v.choose_multiple(rng, k).cloned().collect();
    v.sort_unstable();
    UPOSSet::new(&v)
}

fn distinct_words(
    pool: &[String],
    k: usize,
    rng: &mut impl Rng,
    avoid: Option<&Vec<String>>,
) -> Vec<String> {
    let mut out: Vec<String> = pool.choose_multiple(rng, k.max(1)).cloned().collect();
    out.sort_unstable();
    out.dedup();
    if let Some(orig) = avoid {
        let mut a = orig.clone();
        a.sort_unstable();
        a.dedup();
        if a == out {
            return distinct_words(pool, k, rng, avoid);
        }
    }
    out
}

fn load_sentences(path: &str) -> Vec<String> {
    fs::read_to_string(path)
        .expect("Unable to read file")
        .lines()
        .map(|s| s.to_string())
        .collect()
}

fn score(candidate: &SequenceExpr, problems: &[String], clean: &[String]) -> usize {
    let mut correct = 0;

    let mut matches = Vec::new();

    for problem in problems {
        let doc = Document::new_plain_english_curated(&problem);

        matches.clear();
        matches.extend(candidate.iter_matches_in_doc(&doc));

        if matches.len() == 1 {
            correct += 1;
        }
    }

    for clean in clean {
        let doc = Document::new_plain_english_curated(&clean);

        if candidate.iter_matches_in_doc(&doc).count() == 0 {
            correct += 1;
        }
    }

    correct
}