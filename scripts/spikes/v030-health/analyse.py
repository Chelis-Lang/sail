import csv, os, statistics, collections
D = os.path.dirname(os.path.abspath(__file__))
def load(n):
    return list(csv.DictReader(open(f'{D}/{n}'), delimiter='\t', quoting=csv.QUOTE_NONE))
frac = {}
for p in ['ironkv', 'vest']:
    for r in load(f'census-{p}.tsv'):
        if r['seq'] != '-':
            frac[(os.path.basename(r['input']), r['seq'])] = (r['fraction'], r['answer'], r['used'])
for p in ['ironkv', 'vest']:
    print('==', p)
    rows = load(f'slice-{p}.tsv')
    ok = [r for r in rows if r.get('same_answer') in ('true', 'false')]
    same = [r for r in ok if r['same_answer'] == 'true']
    print(' slice rows', len(rows), 'comparable', len(ok), 'same answer', len(same), 'notes', collections.Counter(r['note'] for r in rows).most_common(3))
    for r in ok:
        if r['same_answer'] != 'true':
            print('  DIFF', os.path.basename(r['input']), r['seq'], r['full_answer'], r['full_rlimit'], r['slice_answer'], r['slice_rlimit'], frac.get((os.path.basename(r['input']), r['seq'])))
    ratios = sorted(float(r['ratio']) for r in ok if r['ratio'] != '-')
    if ratios:
        q = lambda x: ratios[min(len(ratios) - 1, int(x * len(ratios)))]
        print('  effort ratio slice/full: n=%d min=%.3f p10=%.3f median=%.3f p90=%.3f max=%.3f; within 10%%: %d; within 2x: %d; exact: %d' % (
            len(ratios), ratios[0], q(.1), q(.5), q(.9), ratios[-1], sum(0.9 <= x <= 1.1 for x in ratios), sum(0.5 <= x <= 2 for x in ratios), sum(x == 1 for x in ratios)))
    # recorded vs full replay
    rec = [(r, frac.get((os.path.basename(r['input']), r['seq']))) for r in ok]
    print('  full replay == recorded answer:', sum(1 for r, f in rec if f and f[1] == r['full_answer']), '/', len(rec),
          ' rlimit equal:', sum(1 for r, f in rec if f and f[2] == r['full_rlimit']))
    def fr_(r):
        try: return float(frac[(os.path.basename(r['input']), r['seq'])][0])
        except Exception: return 0.0
    top = [r for r in ok if fr_(r) > 0.05]
    print('  checks >5%% of limit: %d, same answer %d, ratios %s' % (len(top), sum(r['same_answer'] == 'true' for r in top), ['%s' % r['ratio'] for r in top]))
    print('  slice ms median %s vs full-session ms median %s; slice bytes median %s' % (
        statistics.median(int(r['slice_ms']) for r in ok), statistics.median(int(r['full_session_ms']) for r in ok), statistics.median(int(r['slice_bytes']) for r in ok)))
    for name in ['perturb', 'perturb-full']:
        rows = load(f'{name}-{p}.tsv')
        ok = [r for r in rows if r.get('same_answer') in ('true', 'false')]
        print(' %s rows %d comparable %d same answer %d same rlimit %d' % (name, len(rows), len(ok), sum(r['same_answer'] == 'true' for r in ok), sum(r['same_rlimit'] == 'true' for r in ok)))
        bad = [r for r in rows if r not in ok or r['same_answer'] != 'true' or r['same_rlimit'] != 'true']
        for r in bad[:8]:
            print('   ', {k: (os.path.basename(v) if k == 'input' else v) for k, v in r.items()})
        if ok:
            pm = sum(int(r['plain_ms']) for r in ok); tm = sum(int(r['traced_ms']) for r in ok)
            if name == 'perturb':
                print('   wall: plain total %d ms, traced total %d ms (x%.1f); max traced %d ms' % (pm, tm, tm / max(pm, 1), max(int(r['traced_ms']) for r in ok)))
                fr = [fr_(r) for r in ok]
                print('   used fraction of these checks: min %.3f max %.3f; >25%%: %d' % (min(fr), max(fr), sum(f > .25 for f in fr)))
