
CMD="cargo run --release --bin tcp-bench -- -c 2 -t 30 -s 1000 -l 512 --packets 1000000 $@"
echo $CMD
$CMD
