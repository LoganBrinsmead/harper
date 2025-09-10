#! /bin/bash

cargo run --release -- -m 1000000 -g 75 -c 10 --problem-file rules/to_too.txt --clean-file clean.txt --max-mutations 60 --seed to -o to_too.sol 
cargo run --release -- -m 1000000 -g 75 -c 10 --problem-file rules/missing_article.txt --clean-file clean.txt --max-mutations 60 -o missing_article.sol
cargo run --release -- -m 1000000 -g 75 -c 10 --problem-file rules/indp.txt --clean-file clean.txt --max-mutations 60 -o best.txt
cargo run --release -- -m 1600000 -g 75 -c 10 --problem-file rules/missing_noun.txt --clean-file clean.txt --max-mutations 60 -o missing_noun.txt
cargo run --release -- -m 1600000 -g 75 -c 10 --problem-file rules/effect_affect.txt --clean-file clean.txt --max-mutations 60 -o effect_affect.txt
cargo run --release -- -m 1600000 -g 75 -c 10 --problem-file rules/affect_effect.txt --clean-file clean.txt --max-mutations 60 -o affect_effect.txt
cargo run --release -- -m 1600000 -g 75 -c 10 --problem-file rules/missing_to.txt --clean-file clean.txt --max-mutations 60 -o missing_to.txt
