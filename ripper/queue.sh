#! /bin/bash

cargo run --release -- -m 1000000 -g 75 -c 10 --problem-file rules/to_too.txt --clean-file clean.txt --max-mutations 60 --seed to -o to_too.sol 
cargo run --release -- -m 1000000 -g 75 -c 10 --problem-file rules/missing_article.txt --clean-file clean.txt --max-mutations 60 -o missing_article.sol
cargo run --release -- -m 1000000 -g 75 -c 10 --problem-file rules/indp.txt --clean-file clean.txt --max-mutations 60 -o best.txt

