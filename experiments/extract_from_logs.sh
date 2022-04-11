#!/bin/bash

set -e

# $1 must match the path to the logs directory
if [ -z $2 ]
then
    echo "Usage: bash extract_all_data.sh PATH_TO_LOGS_DIRECTORY ARTIFACT_PATH" ;
    exit 1 ;
fi


ARTIFACT_PATH=$2
LOGS_DIR=$1
ARTIFACT_BIN_PATH="${ARTIFACT_PATH}/bin"  # Note: this directory must contain a copy of the "data_extractor.py" script

echo "copying log_extractor.py to $ARTIFACT_BIN_PATH"
cp log_extractor.py $ARTIFACT_BIN_PATH

OUT_DIR="data_dreamcoder"

mkdir -p $OUT_DIR


for DOMAIN in `ls $LOGS_DIR` ; do
    echo "Started extracting data from $DOMAIN"
    mkdir -p "$OUT_DIR/$DOMAIN"

    for RUN in `ls $LOGS_DIR/$DOMAIN` ; do
        echo "Started extracting data from $DOMAIN/$RUN"
        # echo "Logging extraction process to $OUT_DIR/$DOMAIN/$RUN/.log"
        # echo "$LOG_FILE" > "data/$DOMAIN/$RUN/.log"
        # python3 "$ARTIFACT_BIN_PATH/log_extractor.py" "$EXP_OUTS_PATH/$LOG_FILE" "data/$DOMAIN/$RUN" "8" >> "data/$DOMAIN/$RUN/.log" 2>&1
        python3 "$ARTIFACT_BIN_PATH/log_extractor.py" "$LOGS_DIR/$DOMAIN/$RUN" "$DOMAIN" "$RUN" "$OUT_DIR/$DOMAIN/$RUN.json"

    done
    echo "Finished extracting data from $DOMAIN"

done

# add info.json to all data folders
# echo "analyzing all run data"
# python3 analyze.py all_run_info
# echo "done"
