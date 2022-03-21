#!/bin/bash
set -e

if [ -z $STITCH_DIR ]
then
    echo "[launch_stitch_run.sh] Please set environment variable \$STITCH_DIR to the stitch/ directory"
    exit 1
fi

if [ -z $2 ]
then
    echo "[launch_stitch_run.sh] Usage: ./launch_stitch_run.sh DOMAIN RUN"
    exit 1
fi

DOMAIN=$1
RUN=$2


# compile Stitch
pushd $STITCH_DIR
cargo build --release --bin=compress
popd

mkdir -p "out/$DOMAIN/$RUN/stitch"

# run Stitch on all the input files from the run
for INFILE in `python3 analyze.py to_input_files out/$DOMAIN/$RUN`; do
    echo "[launch_stitch_run.sh] Running Stitch on: $INFILE"
    /usr/bin/time -v $STITCH_DIR/target/release/compress $INFILE -i1 -a3 --fmt=dreamcoder --out="out/$DOMAIN/$RUN/stitch/out_$(basename $INFILE)"
done

