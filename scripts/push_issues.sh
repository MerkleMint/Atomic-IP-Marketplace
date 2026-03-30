#!/usr/bin/env bash
# Parses issues.md and creates GitHub issues via the gh CLI.
# Usage: ./scripts/push_issues.sh
set -euo pipefail

REPO="MerkleMint/Atomic-IP-Marketplace"
ISSUES_FILE="$(cd "$(dirname "$0")/.." && pwd)/issues.md"

if ! command -v gh &>/dev/null; then
  echo "Error: gh CLI not found." >&2
  exit 1
fi

created=0
failed=0

# Read the whole file
content=$(<"$ISSUES_FILE")

# Extract issue blocks using Python for reliable multi-line parsing
python3 - "$ISSUES_FILE" <<'PYEOF'
import sys, re, subprocess, time

with open(sys.argv[1], 'r') as f:
    text = f.read()

# Split on issue headers
blocks = re.split(r'\n(?=## Issue #)', text)
issues = [b for b in blocks if b.strip().startswith('## Issue #')]

print(f"Found {len(issues)} issues to push to MerkleMint/Atomic-IP-Marketplace\n")

created = 0
failed = 0

for block in issues:
    lines = block.strip().splitlines()

    # Title: "## Issue #N: Some Title"
    header = lines[0]
    title_match = re.match(r'^## Issue #\d+:\s*(.+)$', header)
    if not title_match:
        print(f"  ✗ Could not parse title from: {header}")
        failed += 1
        continue
    title = title_match.group(1).strip()

    # Labels: line like **Labels:** `bug`, `enhancement`
    labels = []
    for line in lines:
        lm = re.match(r'\*\*Labels:\*\*\s*(.+)', line)
        if lm:
            labels = re.findall(r'`([^`]+)`', lm.group(1))
            break

    # Body: everything after the first line
    body = '\n'.join(lines[1:]).strip()

    num_match = re.match(r'^## Issue #(\d+)', header)
    num = num_match.group(1) if num_match else '?'
    print(f"Creating: #{num} — {title}")

    cmd = ['gh', 'issue', 'create',
           '--repo', 'MerkleMint/Atomic-IP-Marketplace',
           '--title', title,
           '--body', body]
    if labels:
        cmd += ['--label', ','.join(labels)]

    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode == 0:
        print(f"  ✓ {result.stdout.strip()}")
        created += 1
    else:
        err = result.stderr.strip()
        # Retry without labels if label not found
        if 'label' in err.lower() or 'Label' in err:
            print(f"  ⚠ Label(s) not found — retrying without labels")
            cmd2 = ['gh', 'issue', 'create',
                    '--repo', 'MerkleMint/Atomic-IP-Marketplace',
                    '--title', title,
                    '--body', body]
            r2 = subprocess.run(cmd2, capture_output=True, text=True)
            if r2.returncode == 0:
                print(f"  ✓ {r2.stdout.strip()}")
                created += 1
            else:
                print(f"  ✗ Failed: {r2.stderr.strip()}")
                failed += 1
        else:
            print(f"  ✗ Failed: {err}")
            failed += 1

    time.sleep(2)  # avoid GitHub secondary rate limit

print(f"\nDone. Created: {created} | Failed: {failed}")
PYEOF
