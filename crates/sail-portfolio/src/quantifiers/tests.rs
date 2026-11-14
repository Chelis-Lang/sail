use super::*;

fn count(input: &[u8]) -> Result<Counts, Error> {
    let mut counter = Counter::new();
    counter.push(input)?;
    counter.finish()
}
fn trace(body: &str) -> Vec<u8> {
    format!("[tool-version] Z3 4.16.0\n{body}[eof]\n").into_bytes()
}

#[test]
fn actual_pinned_traces_count_ematching_mbqi_and_quoted_labels() -> Result<(), Error> {
    let input = include_bytes!("../../tests/fixtures/quantifiers/two-checks.trace");
    let result = count(input)?;
    assert_eq!(result.checks, 2);
    assert_eq!(result.quantifier_instances, 8);
    assert_eq!(result.theory_instances, 16);
    assert!(result.attribution_complete);
    for qid in ["q_fg", "q_g"] {
        let q = result.quantifiers.iter().find(|q| q.qid == qid);
        assert_eq!(q.map(|q| (q.matches, q.instances)), Some((4, 4)));
    }
    let quoted = count(include_bytes!(
        "../../tests/fixtures/quantifiers/quoted-qid.trace"
    ))?;
    assert_eq!(quoted.quantifier_instances, 8);
    assert!(quoted.attribution_complete);
    assert!(
        quoted
            .quantifiers
            .iter()
            .any(|q| q.qid == "|q fg space|" && q.instances == 4)
    );
    let mbqi = count(include_bytes!(
        "../../tests/fixtures/quantifiers/mbqi.trace"
    ))?;
    assert_eq!(mbqi.quantifier_instances, 1);
    assert!(mbqi.attribution_complete);
    assert!(
        mbqi.quantifiers
            .iter()
            .any(|q| q.qid == "mbqi_p" && q.mbqi_discoveries == 1 && q.instances == 1)
    );
    Ok(())
}

#[test]
fn actual_trace_is_invariant_under_every_two_chunk_split() -> Result<(), Error> {
    let input = include_bytes!("../../tests/fixtures/quantifiers/two-checks.trace");
    let expected = count(input)?;
    for at in 0..=input.len() {
        let mut counter = Counter::new();
        counter.push(&input[..at])?;
        counter.push(&input[at..])?;
        assert_eq!(counter.finish()?, expected, "split at {at}");
    }
    let mut counter = Counter::new();
    for byte in input {
        counter.push(&[*byte])?;
    }
    assert_eq!(counter.finish()?, expected);
    Ok(())
}

#[test]
fn reused_term_ids_and_colliding_fingerprints_do_not_invent_attribution() -> Result<(), Error> {
    let reused = count(&trace(
        "[mk-quant] #1 first 1 #9\n[mk-app] #1 other\n[new-match] 7 #1 #2 ; #3\n[instance] 7 ; 1\n[end-of-instance]\n",
    ))?;
    assert_eq!(reused.quantifier_instances, 0);
    assert_eq!(reused.unattributed_instances, 1);
    assert_eq!(reused.unattributed_discoveries, 1);
    assert!(!reused.attribution_complete);
    let collision = count(&trace(
        "[mk-quant] #1 first 1 #9\n[mk-quant] #2 second 1 #9\n[new-match] 7 #1 #3 ;\n[instance] 7 ; 1\n[end-of-instance]\n[new-match] 7 #2 #3 ;\n[instance] 7 ; 1\n[end-of-instance]\n[new-match] 7 #1 #3 ;\n[instance] 7 ; 1\n[end-of-instance]\n",
    ))?;
    assert_eq!(collision.quantifier_instances, 1);
    assert_eq!(collision.unattributed_instances, 2);
    assert_eq!(collision.ambiguous_fingerprints, 1);
    assert!(!collision.attribution_complete);
    Ok(())
}

#[test]
fn zero_quantifier_hash_and_theory_null_are_distinct() -> Result<(), Error> {
    let result = count(&trace(
        "[mk-quant] #1 zero 1 #9\n[new-match] 0 #1 #3 ;\n[instance] 0 ; 1\n[end-of-instance]\n[inst-discovered] theory-solving 0x0 arith# ; #9\n[instance] 0x0 #9\n[end-of-instance]\n[inst-discovered] theory-solving 0x0 arith#7\n[instance] 0 #9\n[end-of-instance]\n",
    ))?;
    assert_eq!(result.quantifier_instances, 1);
    assert_eq!(result.theory_instances, 2);
    assert!(result.attribution_complete);
    Ok(())
}

#[test]
fn malformed_truncated_trailing_and_unsupported_records_refuse_stickily() {
    for body in [
        "[mk-quant] #1 ( 1 #9\n",
        "[mk-quant] #1 |unfinished 1 #9\n",
        "[mk-quant] #1 q 0 #9\n",
        "[new-match] 1 #1 #2\n",
        "[new-match] 1 #1 #2 ; invalid\n",
        "[new-match] 01 #1 #2 ;\n",
        "[new-match] 1 #1 #2 ; (#3)\n",
        "[new-match] 1 #1 #2 ; (#3 #4 #5)\n",
        "[inst-discovered] theory-solving 0x0 # ; #9\n",
        "[instance] 7 #9\n",
        "[instance] 1 ; nope\n",
        "[instance] 4294967296 ; 1\n",
        "[instance] 1 ; 1\n",
        "[end-of-instance]\n",
        "[begin-check] 01\n",
        "[tool-version] Z3 4.16.0\n",
        "[unrecognized] #1\n",
        "[mk-app]#1 x\n",
        "[mk-app] #1 x\r\n",
    ] {
        let mut counter = Counter::new();
        let first = counter.push(&trace(body));
        let result = match first {
            Ok(()) => counter.finish(),
            Err(error) => {
                assert_eq!(counter.push(b"[eof]\n"), Err(error.clone()));
                Err(error)
            }
        };
        assert!(result.is_err(), "{body:?}");
    }
    let valid = trace("[begin-check] 0\n");
    for end in 0..valid.len() {
        assert!(count(&valid[..end]).is_err(), "length {end}");
    }
    let mut extra = valid;
    extra.extend_from_slice(b"[begin-check] 0\n");
    assert!(count(&extra).is_err());
    assert!(count(b"[tool-version] Z3 4.15.0\n[eof]\n").is_err());
    assert!(count(b"[tool-version] Z3 4.16.0\n\xff\n[eof]\n").is_err());
}

#[test]
fn over_budget_lines_and_fingerprints_refuse_without_partial_counts() {
    let mut counter = Counter::new();
    let result = counter.push(&vec![b'x'; MAX_LINE + 1]);
    assert!(result.is_err());
    assert!(counter.finish().is_err());
    let mut counter = Counter::new();
    assert!(
        counter
            .push(b"[tool-version] Z3 4.16.0\n[mk-quant] #1 q 1 #9\n")
            .is_ok()
    );
    for fp in 0..MAX_FINGERPRINTS {
        let line = format!("[new-match] {fp} #1 #2 ;\n");
        assert!(counter.push(line.as_bytes()).is_ok());
    }
    let extra = format!("[new-match] {MAX_FINGERPRINTS} #1 #2 ;\n");
    assert!(counter.push(extra.as_bytes()).is_err());
    assert!(counter.finish().is_err());
}

#[test]
fn null_theory_match_never_conflicts_with_zero_quantifier_hash() -> Result<(), Error> {
    let result = count(&trace(
        "[mk-quant] #1 zero 1 #9\n[new-match] 0 #1 #3 ;\n[instance] 0 ; 1\n[end-of-instance]\n[new-match] 0 datatype#15 datatype#12 #22 ; #23\n[instance] 0 #30\n[end-of-instance]\n[instance] 0 ; 2\n[end-of-instance]\n",
    ))?;
    assert_eq!(result.quantifier_instances, 2);
    assert_eq!(result.theory_instances, 1);
    assert_eq!(result.ambiguous_fingerprints, 0);
    assert!(result.attribution_complete);
    assert!(count(&trace("[new-match] 0x0 #1 #3 ;\n")).is_err());
    assert!(count(&trace("[new-match] 1 datatype#15 datatype#12 ;\n")).is_err());
    Ok(())
}

#[test]
fn actual_datatype_emitter_counts_explicit_theory_instance() -> Result<(), Error> {
    let result = count(include_bytes!(
        "../../tests/fixtures/quantifiers/datatype.trace"
    ))?;
    assert_eq!(result.checks, 1);
    assert_eq!(result.theory_instances, 1);
    assert_eq!(result.quantifier_instances, 0);
    assert_eq!(result.unattributed_instances, 0);
    assert!(result.attribution_complete);
    Ok(())
}
