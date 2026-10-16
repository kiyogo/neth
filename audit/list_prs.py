import json
import sys

sys.stdout.reconfigure(encoding='utf-8')

with open('audit/all_open_prs.json', 'r', encoding='utf-8') as f:
    prs = json.load(f)

prs = sorted(prs, key=lambda x: x['number'])
print('Total open PRs:', len(prs))

for p in prs:
    num = p['number']
    if 3970 <= num <= 4015:
        author = p.get('author', {}).get('login', 'unknown')
        title = p['title']
        print(f"#{num}: {title} ({author})")
