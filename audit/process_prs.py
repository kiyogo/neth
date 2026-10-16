import json, subprocess

with open('audit/all_open_prs.json', 'r', encoding='utf-8') as f:
    prs = json.load(f)

prs.sort(key=lambda x: x['number'])

group3 = [3819, 3469, 4074]
valid_prs = [p for p in prs if p['number'] not in group3]

print(f"Total valid open PRs (Group 1 & 2): {len(valid_prs)}")
print("Oldest 15 PRs:")
for p in valid_prs[:15]:
    num = p['number']
    title = p['title']
    author = p.get('author', {}).get('login', '') if isinstance(p.get('author'), dict) else p.get('author', '')
    print(f"#{num}: {title} ({author})")
