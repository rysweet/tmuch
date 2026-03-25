#!/bin/bash
# E2E test runner for tmuch using tmux as the test harness.
#
# tmux provides:
# - A real PTY for tmuch to render into
# - capture-pane to grab the rendered screen
# - send-keys to simulate user input
# - Scripted session lifecycle for setup/teardown
#
# Usage: ./tests/run-e2e.sh [test_name]
#   No args = run all tests
#   test_name = run only that test

set -euo pipefail

BINARY="./target/release/tmuch"
TEST_SESSION="tmuch-e2e-runner"
EVIDENCE_DIR="./tests/evidence/$(date +%Y%m%d-%H%M%S)"
PASS=0
FAIL=0
SKIP=0

mkdir -p "$EVIDENCE_DIR"

# Build release binary
echo "Building release binary..."
cargo build --release 2>&1 | tail -1

# Cleanup any leftover test sessions
cleanup() {
    tmux kill-session -t "$TEST_SESSION" 2>/dev/null || true
    for s in $(tmux list-sessions -F '#{session_name}' 2>/dev/null | grep '^e2e-'); do
        tmux kill-session -t "$s" 2>/dev/null || true
    done
    for s in $(tmux list-sessions -F '#{session_name}' 2>/dev/null | grep '^tmuch-'); do
        tmux kill-session -t "$s" 2>/dev/null || true
    done
}
trap cleanup EXIT

# Helper: launch tmuch in a tmux session, wait for it to render
launch_tmuch() {
    local args="$*"
    tmux new-session -d -s "$TEST_SESSION" -x 180 -y 50 \
        "$BINARY $args 2>/tmp/tmuch-e2e-stderr.log"
    sleep 2  # Let TUI render
}

# Helper: capture the current screen
capture() {
    local name="$1"
    tmux capture-pane -t "$TEST_SESSION" -p > "$EVIDENCE_DIR/$name.txt" 2>/dev/null
}

# Helper: send keys to tmuch
send() {
    tmux send-keys -t "$TEST_SESSION" "$@"
    sleep 0.5
}

# Helper: assert screen contains text
assert_screen_contains() {
    local text="$1"
    local desc="${2:-screen contains '$text'}"
    local screen
    screen=$(tmux capture-pane -t "$TEST_SESSION" -p 2>/dev/null)
    if echo "$screen" | grep -qF -- "$text"; then
        echo "  ✅ $desc"
        return 0
    else
        echo "  ❌ $desc (expected: '$text')"
        return 1
    fi
}

# Helper: assert command output
assert_output_contains() {
    local cmd="$1"
    local text="$2"
    local desc="${3:-output contains '$text'}"
    local output
    output=$(eval "$cmd" 2>&1)
    if echo "$output" | grep -qF -- "$text"; then
        echo "  ✅ $desc"
        return 0
    else
        echo "  ❌ $desc (expected: '$text', got: $(echo "$output" | head -3))"
        return 1
    fi
}

# Helper: run a test
run_test() {
    local name="$1"
    local filter="${2:-}"
    if [ -n "$filter" ] && [ "$name" != "$filter" ]; then
        return
    fi
    echo ""
    echo "━━━ TEST: $name ━━━"
}

# Helper: record result
pass() { PASS=$((PASS + 1)); }
fail() { FAIL=$((FAIL + 1)); }

# ============================================================
# TEST 1: CLI Smoke Tests (no TUI needed)
# ============================================================
run_test "cli-smoke" "${1:-}"
if true; then
    assert_output_contains "$BINARY --help" "TUI tmux multiplexer" "help shows description" && pass || fail
    assert_output_contains "$BINARY --help" "update" "help shows update command" && pass || fail
    assert_output_contains "$BINARY --help" "--new" "help shows --new flag" && pass || fail
    assert_output_contains "$BINARY --help" "--bind" "help shows --bind flag" && pass || fail
    assert_output_contains "$BINARY --version" "tmuch 0.1.0" "version shows 0.1.0" && pass || fail
    assert_output_contains "$BINARY update --help" "Update tmuch" "update help works" && pass || fail
fi

# ============================================================
# TEST 2: TUI Multi-Pane Display
# ============================================================
run_test "tui-multi-pane" "${1:-}"
if true; then
    # Setup: create test sessions
    tmux new-session -d -s e2e-pane-a "echo 'content-from-pane-a'; sleep 300"
    tmux new-session -d -s e2e-pane-b "echo 'content-from-pane-b'; sleep 300"
    sleep 1

    launch_tmuch "e2e-pane-a e2e-pane-b"
    capture "test2-two-panes"

    assert_screen_contains "e2e-pane-a" "shows first pane title" && pass || fail
    assert_screen_contains "e2e-pane-b" "shows second pane title" && pass || fail
    assert_screen_contains "NORMAL" "status bar shows NORMAL mode" && pass || fail
    assert_screen_contains "[2/2]" "status bar shows 2/2 panes" && pass || fail
    assert_screen_contains "content-from-pane-a" "first pane content visible" && pass || fail
    assert_screen_contains "content-from-pane-b" "second pane content visible" && pass || fail

    send C-q
    sleep 1
    cleanup
fi

# ============================================================
# TEST 3: Add and Drop Panes
# ============================================================
run_test "tui-add-drop" "${1:-}"
if true; then
    tmux new-session -d -s e2e-pane-a "echo 'base-pane'; sleep 300"
    sleep 1

    launch_tmuch "e2e-pane-a"
    assert_screen_contains "[1/1]" "starts with 1 pane" && pass || fail

    send C-a  # Add pane
    sleep 2
    capture "test3-after-add"
    assert_screen_contains "[2/2]" "after Ctrl-A shows 2 panes" && pass || fail

    send C-a  # Add another
    sleep 2
    capture "test3-three-panes"
    assert_screen_contains "[3/3]" "after second Ctrl-A shows 3 panes" && pass || fail

    send C-d  # Drop pane
    sleep 2
    capture "test3-after-drop"
    assert_screen_contains "[2/2]" "after Ctrl-D shows 2 panes" && pass || fail

    send C-q
    sleep 1
    cleanup
fi

# ============================================================
# TEST 4: Pane Focus Mode
# ============================================================
run_test "tui-focus-mode" "${1:-}"
if true; then
    tmux new-session -d -s e2e-pane-a "sleep 300"
    sleep 1

    launch_tmuch "e2e-pane-a"
    sleep 1

    send Enter  # Enter focus mode
    sleep 1
    capture "test4-focused"
    assert_screen_contains "FOCUSED" "Enter switches to FOCUSED mode" && pass || fail
    assert_screen_contains "ATTACHED" "pane title shows ATTACHED" && pass || fail

    send Escape  # Exit focus mode
    sleep 1
    capture "test4-unfocused"
    assert_screen_contains "NORMAL" "Esc returns to NORMAL mode" && pass || fail

    send C-q
    sleep 1
    cleanup
fi

# ============================================================
# TEST 5: Session Picker
# ============================================================
run_test "tui-session-picker" "${1:-}"
if true; then
    tmux new-session -d -s e2e-pane-a "sleep 300"
    tmux new-session -d -s e2e-pane-b "sleep 300"
    sleep 1

    launch_tmuch "e2e-pane-a"
    sleep 1

    send C-s  # Open picker
    sleep 1
    capture "test5-picker-open"
    assert_screen_contains "tmux sessions" "picker overlay shows title" && pass || fail
    assert_screen_contains "PICKER" "status bar shows PICKER mode" && pass || fail

    send j  # Navigate down
    sleep 0.3
    send Enter  # Select
    sleep 2
    capture "test5-after-select"
    assert_screen_contains "[2/2]" "selecting session adds a pane" && pass || fail

    send C-q
    sleep 1
    cleanup
fi

# ============================================================
# TEST 6: Key Bindings
# ============================================================
run_test "tui-key-bindings" "${1:-}"
if true; then
    tmux new-session -d -s e2e-pane-a "sleep 300"
    sleep 1

    launch_tmuch "-b '1=echo binding-one' -b '2=echo binding-two' e2e-pane-a"
    sleep 1
    assert_screen_contains "[1/1]" "starts with 1 pane" && pass || fail

    send 1  # Trigger binding 1
    sleep 2
    capture "test6-after-bind-1"
    assert_screen_contains "[2/2]" "pressing 1 creates second pane" && pass || fail

    send 2  # Trigger binding 2
    sleep 2
    capture "test6-after-bind-2"
    assert_screen_contains "[3/3]" "pressing 2 creates third pane" && pass || fail

    send C-q
    sleep 1
    cleanup
fi

# ============================================================
# TEST 7: Tab Navigation
# ============================================================
run_test "tui-tab-navigation" "${1:-}"
if true; then
    tmux new-session -d -s e2e-pane-a "sleep 300"
    tmux new-session -d -s e2e-pane-b "sleep 300"
    sleep 1

    launch_tmuch "e2e-pane-a e2e-pane-b"
    sleep 1
    # Focus should be on pane 2 (last added)
    capture "test7-initial"
    assert_screen_contains "e2e-pane-b" "initially focused on last pane" && pass || fail

    send Tab  # Navigate to next (wraps to first)
    sleep 1
    capture "test7-after-tab"
    assert_screen_contains "[1/2]" "Tab moves focus to pane 1" && pass || fail

    send Tab  # Back to second
    sleep 1
    assert_screen_contains "[2/2]" "Tab again moves to pane 2" && pass || fail

    send C-q
    sleep 1
    cleanup
fi

# ============================================================
# TEST 8: --new Flag
# ============================================================
run_test "tui-new-flag" "${1:-}"
if true; then
    launch_tmuch "-n 'echo new-session-output; sleep 300'"
    capture "test8-new-session"
    assert_screen_contains "[1/1]" "new session shows as 1 pane" && pass || fail
    assert_screen_contains "tmuch-" "pane title has tmuch- prefix" && pass || fail

    send C-q
    sleep 1
    cleanup
fi

# ============================================================
# TEST 9: Update Command (graceful failure)
# ============================================================
run_test "cli-update-graceful" "${1:-}"
if true; then
    output=$($BINARY update 2>&1 || true)
    if echo "$output" | grep -qE "(Not Found|No release|already at)"; then
        echo "  ✅ update fails gracefully (no crash)"
        pass
    else
        echo "  ❌ update did not fail gracefully: $output"
        fail
    fi
fi

# ============================================================
# TEST 10: First Launch (no args, sessions exist)
# ============================================================
run_test "tui-first-launch-picker" "${1:-}"
if true; then
    tmux new-session -d -s e2e-pane-a "sleep 300"
    sleep 1

    launch_tmuch ""
    sleep 1
    capture "test10-first-launch"
    # Should open session picker since sessions exist
    assert_screen_contains "tmux sessions" "first launch opens session picker" && pass || fail

    send Enter  # Select first session
    sleep 2
    assert_screen_contains "[1/1]" "selecting creates a pane" && pass || fail

    send C-q
    sleep 1
    cleanup
fi

# ============================================================
# RESULTS
# ============================================================
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "RESULTS: $PASS passed, $FAIL failed"
echo "Evidence: $EVIDENCE_DIR"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
