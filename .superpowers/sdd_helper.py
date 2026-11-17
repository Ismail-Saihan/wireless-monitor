import os
import sys
import subprocess
import re

def get_repo_root():
    res = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True)
    return res.stdout.strip().replace('/', '\\')

def get_workspace(plan_file):
    root = get_repo_root()
    slug = os.path.splitext(os.path.basename(plan_file))[0]
    workspace = os.path.join(root, ".superpowers", "sdd", slug)
    os.makedirs(workspace, exist_ok=True)
    marker = os.path.join(workspace, "plan-path")
    if not os.path.exists(marker):
        with open(marker, "w") as f:
            f.write(plan_file.replace('\\', '/') + "\n")
    return workspace

def extract_brief(plan_file, task_n, out_file=None):
    workspace = get_workspace(plan_file)
    if not out_file:
        out_file = os.path.join(workspace, f"task-{task_n}-brief.md")
    
    with open(plan_file, "r", encoding="utf-8") as f:
        content = f.read()

    # Search for ### Task N: ... until next ### Task or end of file
    pattern = rf'(###\s+Task\s+{task_n}:[\s\S]*?)(?=\n###\s+Task|\Z)'
    match = re.search(pattern, content)
    if not match:
        raise ValueError(f"Task {task_n} not found in {plan_file}")

    task_content = match.group(1).strip()
    with open(out_file, "w", encoding="utf-8") as f:
        f.write(task_content + "\n")
    print(out_file)
    return out_file

def review_package(plan_file, base, head, out_file=None):
    workspace = get_workspace(plan_file)
    root = get_repo_root()
    if not out_file:
        base7 = subprocess.run(["git", "rev-parse", "--short", base], capture_output=True, text=True, check=True).stdout.strip()
        head7 = subprocess.run(["git", "rev-parse", "--short", head], capture_output=True, text=True, check=True).stdout.strip()
        out_file = os.path.join(workspace, f"review-{base7}..{head7}.diff")

    log_res = subprocess.run(["git", "log", "--oneline", f"{base}..{head}"], capture_output=True, text=True, check=True).stdout
    stat_res = subprocess.run(["git", "diff", "--stat", f"{base}..{head}"], capture_output=True, text=True, check=True).stdout
    diff_res = subprocess.run(["git", "diff", "-U10", f"{base}..{head}"], capture_output=True, text=True, check=True).stdout

    with open(out_file, "w", encoding="utf-8") as f:
        f.write(f"# Review package: {base}..{head}\n\n")
        f.write("## Commits\n" + log_res + "\n\n")
        f.write("## Files changed\n" + stat_res + "\n\n")
        f.write("## Diff\n" + diff_res + "\n")

    print(out_file)
    return out_file

if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "workspace":
        print(get_workspace(sys.argv[2]))
    elif cmd == "brief":
        extract_brief(sys.argv[2], sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else None)
    elif cmd == "review-package":
        review_package(sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5] if len(sys.argv) > 5 else None)
