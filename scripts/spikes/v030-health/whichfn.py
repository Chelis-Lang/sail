"""whichfn.py LOGS_CRATE_DIR SESSION.in.smt2 SEQ: the Verus function of a session's check.

Finds the `.smt_transcript` whose QUERY bytes are the session's input (sha256),
then reads the AIR log of the same stem: the SEQ-th `(check-valid` and the
`;; Function-Def` / `;; Function-Recommend` comment before it."""
import sys, os, hashlib, re, glob
logs, session, seq = sys.argv[1], sys.argv[2], int(sys.argv[3])
want = hashlib.sha256(open(session, 'rb').read()).hexdigest()
for t in sorted(glob.glob(os.path.join(logs, '*.smt_transcript'))):
    data = open(t, 'rb').read()
    sent = b''.join(m.group(1) + b'\n(echo "<<DONE>>")\n' for m in re.finditer(rb';;;>>> QUERY\n(.*?);;;<<<\n', data, re.S))
    if hashlib.sha256(sent).hexdigest() != want:
        continue
    stem = t[:-len('.smt_transcript')]
    current = None; n = -1; funcs = []
    for line in open(stem + '.air', errors='replace'):
        if line.startswith(';; Function-'):
            current = line.strip()
        elif line.startswith('(check-valid'):
            n += 1; funcs.append(current)
    checks = sent.count(b'\n(check-sat)')
    print('transcript', os.path.basename(t), 'sha256 match; check-sat', checks, 'AIR check-valid', len(funcs))
    if len(funcs) == checks:
        print('check', seq, '->', funcs[seq])
    else:
        print('AIR queries and solver checks differ in number; nearby:', funcs[max(0, seq - 2):seq + 3])
    sys.exit(0)
print('no transcript sends exactly this session (sha256', want[:16], ')')
