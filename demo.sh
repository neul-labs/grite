#!/bin/bash
# Grit + Claude Code Demo
# Shows how grit provides persistent memory for AI coding agents
#
# Usage:
#   ./demo.sh          # Interactive mode (step-by-step with pauses)
#   ./demo.sh --auto   # Auto mode (runs all steps without pauses)

set -e

DEMO_DIR="/tmp/grit-demo"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
GRIT_BIN="${GRIT_BIN:-$SCRIPT_DIR/target/debug/grit}"
# Use --no-daemon to avoid IPC issues in demo
GRIT="$GRIT_BIN --no-daemon"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m'

AUTO_MODE=false

print_step() { echo -e "\n${GREEN}${BOLD}━━━ $1 ━━━${NC}\n"; }
print_info() { echo -e "${BLUE}$1${NC}"; }
print_cmd() { echo -e "${YELLOW}▶ $1${NC}"; }
print_output() { echo -e "${CYAN}$1${NC}"; }
print_error() { echo -e "${RED}Error: $1${NC}"; }

wait_for_user() {
    if [ "$AUTO_MODE" = false ]; then
        echo -e "\n${GREEN}Press Enter to continue...${NC}"
        read -r
    else
        sleep 0.5
    fi
}

run_cmd() {
    print_cmd "$1"
    eval "$1"
}

run_cmd_capture() {
    print_cmd "$1"
    eval "$1"
}

check_dependencies() {
    if ! command -v jq &> /dev/null; then
        print_error "jq is required but not installed. Please install jq first."
        exit 1
    fi

    if [ ! -x "$GRIT_BIN" ]; then
        print_error "grit binary not found at $GRIT_BIN"
        print_info "Build grit first with: cargo build"
        print_info "Or set GRIT_BIN environment variable"
        exit 1
    fi

    # Stop any existing daemon to avoid IPC issues
    "$GRIT_BIN" daemon stop 2>/dev/null || true
}

# Step 1: Setup project
setup_demo() {
    print_step "STEP 1: Setting up demo project"

    print_info "Creating a sample Python project in $DEMO_DIR..."
    echo ""

    # Clean previous demo
    rm -rf "$DEMO_DIR"
    mkdir -p "$DEMO_DIR"
    cd "$DEMO_DIR"

    # Create sample Python project
    cat > greet.py << 'PYEOF'
#!/usr/bin/env python3
"""Simple greeting CLI - Demo project for grit"""
import argparse

def main():
    parser = argparse.ArgumentParser(description="A greeting tool")
    parser.add_argument("name", help="Name to greet")
    args = parser.parse_args()
    print(f"Hello, {args.name}!")

if __name__ == "__main__":
    main()
PYEOF

    cat > README.md << 'MDEOF'
# Greeting CLI

A simple Python CLI tool for greetings.

## Usage

```bash
python greet.py <name>
```
MDEOF

    print_info "Created greet.py and README.md"
    echo ""

    # Initialize git
    run_cmd "git init -q"
    run_cmd "git add ."
    run_cmd "git commit -q -m 'Initial commit'"

    echo ""
    print_info "Initializing grit..."
    run_cmd "$GRIT init"

    echo ""
    print_info "Project created with git and grit initialized"

    wait_for_user
}

# Step 2: Show AGENTS.md
show_agents_md() {
    print_step "STEP 2: AGENTS.md - Agent Discovery"

    print_info "Grit automatically created AGENTS.md for Claude Code to discover."
    print_info "This file tells AI coding agents how to use grit for memory/tasks."
    echo ""

    run_cmd "cat AGENTS.md"

    echo ""
    print_info "Claude Code will read this file and use grit for tasks/memory"

    wait_for_user
}

# Global to store the task issue ID
TASK_ISSUE_ID=""

# Step 3: Create a task
create_task() {
    print_step "STEP 3: Creating a Task"

    print_info "Simulating Claude Code creating a task for a feature request..."
    print_info "Task: Add personalized greeting styles (formal, casual, enthusiastic)"
    echo ""

    # Create task and capture the issue ID
    CREATE_OUTPUT=$($GRIT issue create --title 'Add personalized greeting styles' --body 'Add support for formal, casual, and enthusiastic greeting styles via --style flag' --label agent:todo --json)
    # JSON output has structure: {"schema_version":1,"ok":true,"data":{"issue_id":"...","event_id":"...","wal_head":"..."}}
    TASK_ISSUE_ID=$(echo "$CREATE_OUTPUT" | jq -r '.data.issue_id // .issue_id')

    print_cmd "$GRIT issue create --title 'Add personalized greeting styles' --body '...' --label agent:todo --json"
    echo "$CREATE_OUTPUT" | jq '.'

    echo ""
    print_info "Created issue: $TASK_ISSUE_ID"
    echo ""
    print_info "Listing all issues:"
    run_cmd "$GRIT issue list"

    wait_for_user
}

# Step 4: Work with checkpoints
work_with_checkpoints() {
    print_step "STEP 4: Working with Checkpoints"

    # Use the task issue ID from step 3
    ISSUE_ID="$TASK_ISSUE_ID"
    SHORT_ID="${ISSUE_ID:0:8}"

    if [ -z "$ISSUE_ID" ] || [ "$ISSUE_ID" = "null" ]; then
        print_error "Could not get issue ID - was step 3 run?"
        return 1
    fi

    print_info "Working on issue $SHORT_ID..."
    echo ""

    # Post plan
    print_info "1. Claude posts a plan before starting work:"
    run_cmd "$GRIT issue comment $ISSUE_ID --body 'Plan: Add --style flag with options: formal, casual, enthusiastic. Will create greet() function with style mapping.'"

    echo ""

    # Simulate code change
    print_info "2. Claude modifies the code..."
    cat > greet.py << 'PYEOF'
#!/usr/bin/env python3
"""Simple greeting CLI - Demo project for grit"""
import argparse

def greet(name: str, style: str = "casual") -> str:
    """Generate a greeting based on style."""
    greetings = {
        "formal": f"Good day, {name}. It is a pleasure to meet you.",
        "casual": f"Hey {name}!",
        "enthusiastic": f"WOW! {name}! SO GREAT TO SEE YOU!"
    }
    return greetings.get(style, greetings["casual"])

def main():
    parser = argparse.ArgumentParser(description="A greeting tool")
    parser.add_argument("name", help="Name to greet")
    parser.add_argument("--style", choices=["formal", "casual", "enthusiastic"],
                        default="casual", help="Greeting style")
    args = parser.parse_args()
    print(greet(args.name, args.style))

if __name__ == "__main__":
    main()
PYEOF

    echo ""
    print_info "Updated greet.py:"
    run_cmd "cat greet.py"

    echo ""

    # Post checkpoint
    print_info "3. Claude posts a checkpoint after completing the change:"
    run_cmd "$GRIT issue comment $ISSUE_ID --body 'Checkpoint: Added greet() function with 3 styles. Tested: python greet.py World --style formal'"

    echo ""
    print_info "View the issue with all comments:"
    run_cmd "$GRIT issue show $ISSUE_ID"

    wait_for_user
}

# Step 5: Store memory
store_memory() {
    print_step "STEP 5: Storing Memory"

    print_info "Claude discovered a pattern about this project and stores it as memory."
    print_info "Memories persist across sessions and help future agents understand the codebase."
    echo ""

    run_cmd "$GRIT issue create --title '[Memory] CLI uses argparse pattern' --body 'This project uses argparse for CLI parsing. Pattern: create ArgumentParser, add_argument() for each flag, then parse_args() and use the args object.' --label memory --json"

    echo ""
    print_info "Memories can be queried with the 'memory' label:"
    run_cmd "$GRIT issue list --label memory"

    wait_for_user
}

# Step 6: Close task and show session resume
session_resume() {
    print_step "STEP 6: Session Resume"

    # Use the task issue ID from step 3
    print_info "Claude closes the completed task:"
    run_cmd "$GRIT issue close $TASK_ISSUE_ID"

    echo ""
    echo -e "${CYAN}━━━ Simulating New Session ━━━${NC}"
    echo ""
    print_info "In a new Claude Code session, the agent runs the startup routine"
    print_info "from AGENTS.md to restore context:"
    echo ""

    run_cmd "$GRIT sync --pull --json 2>/dev/null || echo '{\"ok\":true,\"message\":\"No remote configured\"}'"

    echo ""
    print_info "Check for open tasks:"
    run_cmd "$GRIT issue list --state open --label agent:todo"

    echo ""
    print_info "Retrieve stored memories:"
    run_cmd "$GRIT issue list --label memory"

    echo ""
    print_info "Claude can see all memories and open tasks from previous sessions!"

    wait_for_user
}

# Step 7: Health checks
run_doctor() {
    print_step "STEP 7: Health Checks"

    print_info "Run grit doctor to check database health:"
    run_cmd "$GRIT doctor"

    echo ""
    print_info "Doctor checks: git repo, WAL ref, actor config, store integrity, rebuild threshold"
    print_info "Use 'grit doctor --fix' to auto-repair issues"

    wait_for_user
}

# Final summary
show_summary() {
    print_step "DEMO COMPLETE"

    echo -e "${GREEN}${BOLD}What we demonstrated:${NC}"
    echo ""
    echo -e "  1. ${BLUE}grit init${NC} creates AGENTS.md for agent discovery"
    echo -e "  2. Tasks created with ${BLUE}grit issue create --label agent:todo${NC}"
    echo -e "  3. Progress tracked with ${BLUE}grit issue comment${NC} (checkpoints)"
    echo -e "  4. Learnings stored with ${BLUE}--label memory${NC}"
    echo -e "  5. New sessions retrieve context via ${BLUE}grit issue list${NC}"
    echo -e "  6. Health checks with ${BLUE}grit doctor${NC}"
    echo ""
    echo -e "${GREEN}${BOLD}Try it yourself:${NC}"
    echo ""
    echo -e "  ${YELLOW}cd $DEMO_DIR${NC}"
    echo -e "  ${YELLOW}claude${NC}"
    echo ""
    echo -e "${GREEN}${BOLD}Claude Code will:${NC}"
    echo ""
    echo -e "  - Read AGENTS.md automatically"
    echo -e "  - Use grit for task tracking"
    echo -e "  - Store/retrieve memories across sessions"
    echo ""
    echo -e "${GREEN}${BOLD}Demo project location:${NC} $DEMO_DIR"
    echo ""
}

show_help() {
    echo "Grit + Claude Code Demo"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  --auto    Run demo automatically without pauses"
    echo "  --help    Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0           # Interactive mode (step-by-step)"
    echo "  $0 --auto    # Automated mode (no pauses)"
    echo ""
}

main() {
    case "${1:-}" in
        --auto)
            AUTO_MODE=true
            ;;
        --help|-h)
            show_help
            exit 0
            ;;
        "")
            AUTO_MODE=false
            ;;
        *)
            print_error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac

    check_dependencies

    echo -e "${GREEN}"
    cat << 'BANNER'
    ╔════════════════════════════════════════════════════════════╗
    ║           GRIT + CLAUDE CODE DEMO                          ║
    ║   Persistent Memory for AI Coding Agents                   ║
    ╚════════════════════════════════════════════════════════════╝
BANNER
    echo -e "${NC}"

    if [ "$AUTO_MODE" = true ]; then
        print_info "Running in AUTO mode (no pauses)"
    else
        print_info "Running in INTERACTIVE mode (press Enter to advance)"
    fi
    echo ""

    setup_demo
    show_agents_md
    create_task
    work_with_checkpoints
    store_memory
    session_resume
    run_doctor
    show_summary
}

main "$@"
