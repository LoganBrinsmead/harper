mod mirror;

use clap::Parser;
use harper_brill::UPOS;
use harper_core::Document;
use harper_core::expr::{ExprExt, SequenceExpr};
use harper_core::patterns::{UPOSSet, WordSet};
use harper_core::spell::{Dictionary, FstDictionary};
use rand::seq::IndexedRandom;
use rand::{Rng, seq::SliceRandom};
use rayon::slice::ParallelSliceMut;
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
}

fn main() {
    let args = Args::parse();

    let dict = FstDictionary::curated();

    let mut mirs = vec![Mirror {
        seq: vec![MirrorAtom::AllowedWords(vec!["too".to_string()])],
    }];

    for i in 0..200 {
        mirs.par_sort_by_cached_key(|s| usize::MAX - score(&s.to_seq_expr()));

        for i in 0..3.min(mirs.len()) {
            dbg!(
                &mirs[i],
                score(&mirs[i].to_seq_expr()),
                problems().len() + clean().len()
            );
        }

        mirs.truncate(args.min_pop);

        let mut perm_mirs = Vec::new();

        for mir in &mirs {
            perm_mirs.append(&mut permute_mirror_unique(
                mir,
                args.child_ratio,
                &dict,
            ));
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

fn problems() -> &'static [&'static str] {
    &[
        "I need too finish this report before noon.",
        "She went too the grocery store after work.",
        "We decided too postpone the meeting.",
        "He forgot too send the invitation.",
        "Remember too lock the door.",
        "They plan too travel next spring.",
        "I tried too explain the discrepancy.",
        "Please add this file too the repository.",
        "He refused too apologize for the oversight.",
        "I'm happy too help with debugging.",
        "Set the thermostat back too sixty-eight at night.",
        "We need too align our objectives.",
        "She learned how too solder the components.",
        "The CEO chose too prioritize profitability.",
        "Use the adapter too connect the cable.",
        "He ran from end too end of the field.",
        "It's critical too verify each assumption.",
        "Attach the label too the container.",
        "I meant too forward you the email.",
        "They drove too Denver before sunrise.",
        "Tap here too continue.",
        "I neglected too charge the laptop.",
        "We hope too avoid scope creep.",
        "Click 'Build' too compile the project.",
        "The change needs too propagate across services.",
        "I'm going too grab lunch.",
        "She agreed too mentor the interns.",
        "He promised too follow up tomorrow.",
        "Use this form too request access.",
        "I prefer too work asynchronously.",
        "The team aims too reduce latency.",
        "Press Esc too cancel.",
        "Switch lanes from left too right safely.",
        "We're moving from draft too production.",
        "I waited an hour just too speak with support.",
        "Route the packet too the primary gateway.",
        "This needs to adhere too the spec.",
        "According too our records, your payment cleared.",
        "Due too network congestion, the upload stalled.",
        "We intend too deprecate this endpoint.",
    ]
}

fn clean() -> &'static [&'static str] {
    &[
        "I need to finish this report before noon.",
        "She went to the grocery store after work.",
        "We decided to postpone the meeting.",
        "He forgot to send the invitation.",
        "Remember to lock the door.",
        "They plan to travel next spring.",
        "I tried to explain the discrepancy.",
        "Please add this file to the repository.",
        "He refused to apologize for the oversight.",
        "I'm happy to help with debugging.",
        "Set the thermostat back to sixty-eight at night.",
        "We need to align our objectives.",
        "She learned how to solder the components.",
        "The CEO chose to prioritize profitability.",
        "Use the adapter to connect the cable.",
        "He ran from end to end of the field.",
        "It's critical to verify each assumption.",
        "Attach the label to the container.",
        "I meant to forward you the email.",
        "They drove to Denver before sunrise.",
        "Tap here to continue.",
        "I neglected to charge the laptop.",
        "We hope to avoid scope creep.",
        "Click 'Build' to compile the project.",
        "The change needs to propagate across services.",
        "I'm going to grab lunch.",
        "She agreed to mentor the interns.",
        "He promised to follow up tomorrow.",
        "Use this form to request access.",
        "I prefer to work asynchronously.",
        "The team aims to reduce latency.",
        "Press Esc to cancel.",
        "Switch lanes from left to right safely.",
        "We're moving from draft to production.",
        "I waited an hour just to speak with support.",
        "Route the packet to the primary gateway.",
        "This needs to adhere to the spec.",
        "According to our records, your payment cleared.",
        "Due to network congestion, the upload stalled.",
        "We intend to deprecate this endpoint.",
        "It was far too late to salvage the schedule.",
        "The espresso tasted too bitter for my palate.",
        "She is joining the expedition too, despite the warnings.",
        "Your proposal is too vague for the board.",
        "The server grew too hot under sustained load.",
        "He spoke too quickly to be understood.",
        "I found the contract too onerous to sign.",
        "They arrived too early and waited in silence.",
        "This dataset is too noisy for reliable inference.",
        "The film was too long yet strangely compelling.",
        "The cliff looked too sheer for novice climbers.",
        "Her apology felt too rehearsed to be sincere.",
        "That price is too steep for a prototype.",
        "He ate too much and regretted it.",
        "The room grew too quiet to ignore.",
        "Your tone is too abrasive for diplomacy.",
        "The deadline is too tight for thorough testing.",
        "She laughed too hard at the misprint.",
        "The instructions are too convoluted for speed.",
        "I, too, questioned the methodology.",
        "He goes too far with bets.",
        "Just be prepared to occasionally troubleshoot the debugger itself.",
        "It takes a great deal of energy to consistently operate under that kind of pressure.",
        "I am too hungry.",
        "Please remember to eat your vegetables.",
        "I’m going to Nashville next week.",
        "Talk to you later.",
        "The coffee is too hot to drink.",
        "The music was too loud, making it hard to hear.",
        "He's too shy to speak in public.",
        "The cake is too sweet for my taste.",
        "It's too expensive for me to buy right now.",
        "She worked too hard and ended up getting sick.",
        "The instructions were too complicated to understand.",
        "I like apples, and my brother does too.",
        "She's coming to the party, and he is too.",
        "I want to go to the beach, and you do too?",
        "He's a talented musician, and a great friend too.",
        "The movie was good, and the popcorn was delicious too.",
        "The problem is too difficult, and the deadline is too close.",
        "He's too good at the game, and he's too nice to win.",
        "Bringing Hope and Opportunity to Young Musicians",
        "Attendees can look forward to:",
        "We're empowering them to build brighter futures.",
        "I’d like you to consciously delegate one task",
        "Soundscapes are not merely environmental features; they are integral to human identity and cultural expression.",
        "Its speed, flexibility, and seamless integration with FZF make it a compelling alternative to traditional fuzzy finding solutions.",
        "Attempted to explicitly cast the result back to a string",
        "They felt buried under the data, unable to proactively address emerging threats.",
        "Familiarize yourself with these resources to learn how to effectively utilize the plugin’s features.",
        "It takes a great deal of energy to consistently operate under that kind of pressure.",
        "Just be prepared to occasionally troubleshoot the debugger itself.",
        "He goes too far with bets.",
    ]
}

fn score(candidate: &SequenceExpr) -> usize {
    let mut correct = 0;

    let mut matches = Vec::new();

    for problem in problems() {
        let doc = Document::new_plain_english_curated(&problem);

        matches.clear();
        matches.extend(candidate.iter_matches_in_doc(&doc));

        if matches.len() == 1 {
            correct += 1;
        }
    }

    for clean in clean() {
        let doc = Document::new_plain_english_curated(&clean);

        if candidate.iter_matches_in_doc(&doc).count() == 0 {
            correct += 1;
        }
    }

    correct
}
