#!/bin/bash

expect_cmd() {
    expected="$1"
    read -r cmd
    if [[ "$cmd" != "$expected" ]]; then
        echo "unexpected command: '$cmd' expected '$expected'"
        exit 1
    fi
}

echo "fakefish test"
expect_cmd uci
echo "id name Stockfish 15"
echo "id author the Stockfish developers (see AUTHORS file)"
echo ""
echo "option name Debug Log File type string default"
echo "option name Threads type spin default 1 min 1 max 512"
echo "option name Hash type spin default 16 min 1 max 2048"
echo "option name Clear Hash type button"
echo "option name Ponder type check default false"
echo "option name MultiPV type spin default 1 min 1 max 500"
echo "option name Skill Level type spin default 20 min 0 max 20"
echo "option name Move Overhead type spin default 10 min 0 max 5000"
echo "option name Slow Mover type spin default 100 min 10 max 1000"
echo "option name nodestime type spin default 0 min 0 max 10000"
echo "option name UCI_Chess960 type check default false"
echo "option name UCI_AnalyseMode type check default false"
echo "option name UCI_LimitStrength type check default false"
echo "option name UCI_Elo type spin default 1350 min 1350 max 2850"
echo "option name UCI_ShowWDL type check default false"
echo "option name SyzygyPath type string default <empty>"
echo "option name SyzygyProbeDepth type spin default 1 min 1 max 100"
echo "option name Syzygy50MoveRule type check default true"
echo "option name SyzygyProbeLimit type spin default 7 min 0 max 7"
echo "option name Use NNUE type check default true"
echo "option name EvalFile type string default nn-6877cd24400e.nnue"
echo "uciok"
expect_cmd isready
echo "readyok"

