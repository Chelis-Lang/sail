//! Checked selection from the actual decoded worker request journal.
//!
//! This preserves data and submission correspondence. Decoding, canonical path
//! observation, journal origin and physical compiler launch remain native joins.
#![allow(
    clippy::redundant_as_str,
    reason = "The pinned stock Verus specifies str::as_bytes, not String::as_bytes."
)]

use crate::acceptance::{bind_submission, request_corresponds};
use crate::jobs::Mode;
use crate::project::{ProjectResult, REQUEST_CAPTURE_VERSION, RequestCapture, ResolvedRequest};
use sail_acceptance::authority::exact_bytes;
use sail_acceptance::{sail_proof, sail_spec, sail_verified, sail_while};

#[cfg(sail_verus)]
use vstd::prelude::*;

sail_spec! {
    pub open spec fn original_capture_header(capture: &RequestCapture, worker: &str) -> bool {
        capture.version == REQUEST_CAPTURE_VERSION
            && crate::acceptance::string_bytes(&capture.worker_id) == vstd::utf8::encode_utf8(worker@)
    }

    pub open spec fn original_capture_run(capture: &RequestCapture, project: &ProjectResult) -> bool {
        crate::acceptance::string_bytes(&capture.request.run_id)
            == crate::acceptance::string_bytes(&project.run_id)
    }

    pub open spec fn original_capture_payload(capture: &RequestCapture, invocation: &str,
        arguments: Seq<String>, mode: &Mode, project: &ProjectResult) -> bool {
        crate::acceptance::string_bytes(&capture.request.invocation)
                == vstd::utf8::encode_utf8(invocation@)
            && (exists |kind: crate::acceptance::SubmissionKind|
                crate::acceptance::submission_correspondence(arguments, mode, &capture.request, kind))
            && crate::acceptance::request_header_matches(&capture.request, project)
            && crate::acceptance::resolved_coverage(&capture.request, project)
    }

    pub open spec fn selected_original_capture(records: Seq<RequestCapture>, worker: &str,
        invocation: &str, arguments: Seq<String>, mode: &Mode, project: &ProjectResult,
        request: &ResolvedRequest) -> bool {
        (forall |j: int| 0 <= j < records.len() ==> original_capture_header(&records[j], worker))
            && exists |i: int| 0 <= i < records.len()
                && records[i].request == *request
                && original_capture_run(&records[i], project)
                && original_capture_payload(&records[i], invocation, arguments, mode, project)
                && forall |j: int| 0 <= j < records.len()
                    && original_capture_run(&records[j], project) ==> j == i
    }

    /// A real nonempty constructive domain, including arbitrary earlier runs.
    /// Every expected field comes from original journal/submission inputs.
    pub open spec fn constructive_original_capture(records: Seq<RequestCapture>, worker: &str,
        invocation: &str, arguments: Seq<String>, mode: &Mode, project: &ProjectResult) -> bool {
        records.len() > 0
            && (forall |j: int| 0 <= j < records.len() ==> original_capture_header(&records[j], worker))
            && (forall |j: int| 0 <= j < records.len() - 1 ==> !original_capture_run(&records[j], project))
            && original_capture_run(&records.last(), project)
            && crate::acceptance::string_bytes(&records.last().request.invocation)
                == vstd::utf8::encode_utf8(invocation@)
            && arguments.len() == 0 && records.last().request.args@.len() == 0
            && mode is Stock && records.last().request.engine.is_none()
            && records.last().request.function.is_none()
            && crate::acceptance::small_request_match(&records.last().request, project)
    }
}

sail_verified! {
    /// Consumes the actual decoded rows and returns their unique matching
    /// original request. A selected data record is not positive proof authority.
    #[allow(clippy::question_mark,
        reason = "Retain the original explicit refusal branches in the checked operation and executable guard mutations.")]
    pub fn select_original_request(records: Vec<RequestCapture>, worker: &str, invocation: &str,
        arguments: &[String], mode: &Mode, project: &ProjectResult) -> (out: Option<ResolvedRequest>)
        requires(true)
        ensures(out.is_some() ==> selected_original_capture(records@, worker, invocation,
                arguments@, mode, project, &out.unwrap()),
            constructive_original_capture(records@, worker, invocation, arguments@, mode, project)
                ==> out.is_some() && out.unwrap() == records@.last().request)
    {
        let mut rows = records;
        let mut found: Option<usize> = None;
        let mut i = 0usize;
        sail_while! { (i < rows.len())
            invariant(i <= rows.len(), rows@ == records@,
                forall |j: int| 0 <= j < i ==> original_capture_header(&rows@[j], worker),
                match found {
                    None => forall |j: int| 0 <= j < i ==> !original_capture_run(&rows@[j], project),
                    Some(index) => index < i
                        && original_capture_run(&rows@[index as int], project)
                        && original_capture_payload(&rows@[index as int], invocation, arguments@, mode, project)
                        && forall |j: int| 0 <= j < i && original_capture_run(&rows@[j], project)
                            ==> j == index as int,
                },
                constructive_original_capture(records@, worker, invocation, arguments@, mode, project)
                    ==> if i < rows.len() { found.is_none() }
                        else { found == Some(rows.len() - 1) })
            decreases(rows.len() - i)
            {
                let capture = &rows[i];
                if capture.version != REQUEST_CAPTURE_VERSION
                    || !exact_bytes(capture.worker_id.as_str().as_bytes(), worker.as_bytes()) {
                    return None;
                }
                if !exact_bytes(capture.request.run_id.as_str().as_bytes(), project.run_id.as_str().as_bytes()) {
                    i += 1;
                    continue;
                }
                if found.is_some()
                    || !exact_bytes(capture.request.invocation.as_str().as_bytes(), invocation.as_bytes()) {
                    return None;
                }
                sail_proof! {
                    if constructive_original_capture(records@, worker, invocation, arguments@, mode, project) {
                        if i < rows.len() - 1 {
                            assert(!original_capture_run(capture, project));
                        }
                        assert(i == rows.len() - 1);
                    }
                }
                let binding = bind_submission(arguments, mode, &capture.request);
                if binding.is_none() { return None; }
                if !request_corresponds(&capture.request, project) { return None; }
                sail_proof! {
                    assert(binding.is_some());
                    assert(crate::acceptance::submission_correspondence(arguments@, mode,
                        &capture.request, crate::acceptance::bound_kind(&binding.unwrap())));
                    assert(original_capture_payload(capture, invocation, arguments@, mode, project));
                }
                found = Some(i);
                i += 1;
            }
        }
        let Some(index) = found else { return None; };
        sail_proof! { assert(selected_original_capture(records@, worker, invocation,
            arguments@, mode, project, &rows@[index as int].request)); }
        Some(rows.remove(index).request)
    }
}
