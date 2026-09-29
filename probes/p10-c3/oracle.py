"""Phase 10 C3: cases for the matcher, answered by C3's text half's reader
(`target/p10-c3/evidence.py`, untracked: the rules as that half read them
from UTS #35 Part 1), and the comparison with the matcher's own answers.

  python3 probes/p10-c3/oracle.py gen DIR       # DIR/in.txt and DIR/py.txt
  python3 probes/p10-c3/oracle.py compare DIR   # DIR/py.txt against DIR/rust.txt

`oracle.sh` runs both around the matcher's side (`oracle_test.rs`).
"""
import json, random, sys
sys.path.insert(0, 'target/p10-c3')
import evidence as ev
random.seed(7)
mode, out = sys.argv[1], sys.argv[2]

def compare():
    py = sorted(open(f"{out}/py.txt").read().splitlines())
    rs = sorted(open(f"{out}/rust.txt").read().splitlines())
    only = sorted(set(py) ^ set(rs))
    def offers_und(line):
        body = line[2:]
        if line.startswith("d "):
            return body.split(" ")[1].split("-")[0] == "und"
        supported = body.split(" | ")[1].split(" => ")[0].split(" ")
        return any(t.split("-")[0] == "und" for t in supported)
    other = [l for l in only if not offers_und(l)]
    print(f"{len(py)} cases; {len(only)} lines differ, all of them where the application offers `und`: {not other}")
    for l in other[:20]:
        print("  ", l)
    sys.exit(1 if other else 0)

if mode == "compare":
    compare()
L = ev.LIKELY
tags = set()
tags |= {k for k in L if not k.startswith('und') and '-' in k}
tags |= set(ev.PARADIGMS)
tags |= set("""en en-US en-GB en-AU en-IN en-001 en-150 en-CA en-PH en-SA en-GU en-IE en-ZA en-XA
es es-419 es-MX es-AR es-ES es-US es-CR es-CO es-EA pt pt-BR pt-PT pt-AO pt-MO pt-CH zh zh-TW zh-HK zh-MO zh-CN
zh-Hant zh-Hans zh-SG zh-Hani zh-Latn yue yue-Hans sr sr-Latn sr-Cyrl sr-ME sr-RS sr-BA pa pa-PK pa-Arab pa-Guru ar ar-EG ar-MA ar-DZ ar-SA ar-Latn ar-XB
fr fr-CA fr-FR fr-BE fr-CH fr-MA de de-AT de-CH gsw lb nb no nn da sv ca br cy und und-US und-Latn ja ja-Latn ja-Hira ko ko-Kore ko-Hang
hi hi-Latn ur bn bn-Latn ta az az-IR az-Arab uz uz-AF mn mn-CN sd sd-IN ru uk be kk hy ka ach af haw sh hr bs qaa qaa-Latn
it it-CH eu gl oc tl fil ceb ms id jv su vi th lo km my si am ti so sw zu xh yo ig ha ha-NG ff ff-Adlm ks ks-Deva shi vai""".split())
langs = [k for k in L if '-' not in k and k != 'und']
tags |= set(random.sample(langs, 150))
for l in random.sample(langs, 60):
    tags.add(l + '-' + random.choice(['US','GB','FR','CN','IN','BR','MX','419','001','DE','RU','TW']))
tags = sorted(tags)
pairs = [(a, b) for a in tags for b in tags if random.random() < 0.12]
lists = []
for _ in range(3000):
    d = random.sample(tags, random.randint(1, 5))
    s = random.sample(tags, random.randint(1, 6))
    lists.append((d, s))
with open(f'{out}/in.txt', 'w') as f:
    for a, b in pairs: f.write(f"d {a} {b}\n")
    for d, s in lists: f.write(f"b {' '.join(d)} | {' '.join(s)}\n")
with open(f'{out}/py.txt', 'w') as f:
    for a, b in pairs:
        f.write(f"d {a} {b} {ev.distance(a, b)[2]}\n")
    for d, s in lists:
        got = ev.best(d, s)[0]
        f.write(f"b {' '.join(d)} | {' '.join(s)} => {got or '-'}\n")
print(len(tags), 'tags', len(pairs), 'pairs', len(lists), 'lists')
