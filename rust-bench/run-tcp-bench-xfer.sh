
CMD="cargo run --release --bin tcp-bench -- -c 1 -t 30 -l 512 --packets 2000000 $@"
echo $CMD
$CMD
