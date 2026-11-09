#!/usr/bin/env bash
# SPIKE: stop every spike process on this host and clear ROOTs and traces.
pkill -f 'spike-v030-bugs\.py' || true
pkill -f 'spike_neutral_loop' || true
pkill -f 'sail serve' || true
pkill stress-ng || true
sleep 1
rm -rf /tmp/sail-spike "$@"
mkdir -p /tmp/sail-spike
echo reset
