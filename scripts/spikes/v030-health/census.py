import csv, collections, sys, os
D = os.path.dirname(os.path.abspath(__file__))
for p in ['ironkv', 'vest']:
    rows = list(csv.DictReader(open(f'{D}/census-{p}.tsv'), delimiter='\t', quoting=csv.QUOTE_NONE))
    sess = {}
    for r in rows:
        sess[r['session']] = (int(r['in_bytes']), int(r['out_bytes']))
    checks = [r for r in rows if r['seq'] != '-']
    tot_in = sum(v[0] for v in sess.values()); tot_out = sum(v[1] for v in sess.values())
    def frac(r):
        try: return float(r['fraction'])
        except Exception: return None
    ret = set(); near = []; nonunsat = []
    for r in checks:
        f = frac(r)
        if r['answer'] != 'unsat': nonunsat.append(r); ret.add(r['session'])
        if f is not None and f > 0.25: near.append(r); ret.add(r['session'])
    nofrac = [r for r in checks if frac(r) is None]
    print(p, 'sessions', len(sess), 'checks', len(checks), 'in', tot_in, 'out', tot_out,
          'max_in', max(v[0] for v in sess.values()), 'max_inout', max(sum(v) for v in sess.values()))
    print('  nonunsat', len(nonunsat), collections.Counter((r['answer'], r['reason']) for r in nonunsat))
    print('  >25%', len(near), ' no-effort checks', len(nofrac), 'unpaired sessions',
          len({r['session'] for r in rows if r['unpaired'] != '0'}))
    print('  retained sessions', len(ret), 'raw in bytes', sum(sess[s][0] for s in ret), 'in+out', sum(sum(sess[s]) for s in ret))
    for th in (0.1, 0.05, 0.01):
        s2 = {r['session'] for r in checks if (frac(r) or 0) > th or r['answer'] != 'unsat'}
        print('   policy >%.2f: sessions %d bytes %d' % (th, len(s2), sum(sess[s][0] for s in s2)))
    big = [s for s, v in sess.items() if sum(v) > 256 * 1024 * 1024]
    print('  sessions over 256MiB', len(big))
    print('  limits', collections.Counter(r['limit'] for r in checks).most_common(6))
    # lists for the replay experiments: top by fraction
    ranked = sorted([r for r in checks if frac(r) is not None], key=lambda r: -frac(r))
    with open(f'{D}/retain-{p}.txt', 'w') as f:
        for s in sorted(ret): f.write(s + '\n')
    with open(f'{D}/top-{p}.tsv', 'w') as f:
        for r in ranked[:40]: f.write('\t'.join([r['input'], r['seq'], r['fraction'], r['answer'], r['limit']]) + '\n')
    # stratified sample for slicing: top 40 + every non-unsat + every 60th of the rest
    pick = ranked[:40] + nonunsat + ranked[40::max(1, len(ranked) // 40)]
    seen = set(); out = []
    for r in pick:
        k = (r['input'], r['seq'])
        if k not in seen: seen.add(k); out.append(r)
    with open(f'{D}/slice-{p}.tsv', 'w') as f:
        for r in out: f.write(f"/home/ubuntu/spike/{r['input']}\t{r['seq']}\n")
    with open(f'{D}/perturb-{p}.tsv', 'w') as f:
        for r in ranked[:25]: f.write(f"/home/ubuntu/spike/{r['input']}\t{r['seq']}\n")
    print('  slice list', len(out), 'top fractions', [r['fraction'] for r in ranked[:26]])
