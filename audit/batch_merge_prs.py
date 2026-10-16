import json
import subprocess
import sys
import os

GROUP3_REJECT = [3819, 3469, 4074]

def run_cmd(cmd, cwd=None):
    res = subprocess.run(cmd, cwd=cwd, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding='utf-8', errors='replace')
    return res.returncode, res.stdout.strip(), res.stderr.strip()

def main():
    with open('audit/all_open_prs.json', 'r', encoding='utf-8') as f:
        prs = json.load(f)

    # Sort oldest to newest
    prs.sort(key=lambda x: x['number'])

    # Filter out Group 3
    valid_prs = [p for p in prs if p['number'] not in GROUP3_REJECT]

    print(f"Total valid PRs to process: {len(valid_prs)}")

    # Load progress if exists
    progress_file = 'audit/batch_merge_progress.json'
    processed = {}
    if os.path.exists(progress_file):
        try:
            with open(progress_file, 'r', encoding='utf-8') as f:
                processed = json.load(f)
        except Exception:
            processed = {}

    batch_clean_count = 0

    for p in valid_prs:
        num = p['number']
        title = p['title']
        author = p.get('author', {}).get('login', '') if isinstance(p.get('author'), dict) else p.get('author', '')

        # Skip if already marked processed
        if str(num) in processed and processed[str(num)].get('status') in ['already_merged', 'clean_merged', 'conflict_resolved']:
            continue

        print(f"\n==========================================")
        print(f"Processing PR #{num}: {title} ({author})")

        # 1. Fetch PR
        code, out, err = run_cmd(f"git fetch origin pull/{num}/head")
        if code != 0:
            print(f"[-] Failed to fetch PR #{num}: {err}")
            processed[str(num)] = {"status": "fetch_failed", "title": title, "author": author}
            with open(progress_file, 'w', encoding='utf-8') as f:
                json.dump(processed, f, indent=2)
            continue

        # 2. Check if already in master (zero diff or all commits already in master)
        code, out, _ = run_cmd("git log master..FETCH_HEAD --oneline")
        if not out.strip():
            print(f"[+] PR #{num} has no new commits (already merged in master).")
            processed[str(num)] = {"status": "already_merged", "title": title, "author": author}
            with open(progress_file, 'w', encoding='utf-8') as f:
                json.dump(processed, f, indent=2)
            continue

        code, out, _ = run_cmd("git diff master...FETCH_HEAD")
        if not out.strip():
            print(f"[+] PR #{num} diff against master is empty (already present).")
            processed[str(num)] = {"status": "already_merged", "title": title, "author": author}
            with open(progress_file, 'w', encoding='utf-8') as f:
                json.dump(processed, f, indent=2)
            continue

        # 3. Attempt clean merge
        print(f"[*] Merging PR #{num}...")
        merge_code, merge_out, merge_err = run_cmd(f'git merge FETCH_HEAD -m "Merge PR #{num}: {title}"')
        
        if merge_code == 0:
            print(f"[+] Successfully clean merged PR #{num}!")
            processed[str(num)] = {"status": "clean_merged", "title": title, "author": author}
            with open(progress_file, 'w', encoding='utf-8') as f:
                json.dump(processed, f, indent=2)
            batch_clean_count += 1
            if batch_clean_count >= 10:
                print(f"[!] Reached batch of {batch_clean_count} clean merges. Pausing for verification.")
                break
        else:
            print(f"[!] Merge conflict on PR #{num}!")
            print(f"Conflict output:\n{merge_out}\n{merge_err}")
            # Identify conflict files
            code, status_out, _ = run_cmd("git status --porcelain")
            conflict_files = [line[3:] for line in status_out.splitlines() if line.startswith('UU ') or line.startswith('AA ') or line.startswith('UD ') or line.startswith('DU ')]
            print(f"Conflict files: {conflict_files}")
            processed[str(num)] = {"status": "in_conflict", "title": title, "author": author, "conflict_files": conflict_files}
            with open(progress_file, 'w', encoding='utf-8') as f:
                json.dump(processed, f, indent=2)
            # Break so we can resolve conflict properly by hand
            break

if __name__ == '__main__':
    main()
