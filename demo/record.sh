#!/bin/sh
set -eu
udid=$1
decider=$2
out=$3
plan=${4:-examples/news-browse.yaml}
argent run stop-simulator-server --udid "$udid" >/dev/null 2>&1 || true
pkill -f "simulator-server ios --id $udid" || true
sleep 1
tmp=$(mktemp -d)/recording.mp4
xcrun simctl io "$udid" recordVideo --codec h264 --force "$tmp" &
recorder=$!
sleep 2
status=0
cargo run -q -- run "$plan" --udid "$udid" --decider "$decider" || status=$?
sleep 1
kill -INT "$recorder"
wait "$recorder" || true
mv "$tmp" "$out"
exit "$status"
