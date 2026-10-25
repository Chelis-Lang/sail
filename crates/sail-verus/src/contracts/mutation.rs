//! Deterministic body-only mutations over the admitted neutral scalar subset.
use quote::ToTokens as _;
use sail_api::mutation::{Candidate, MAX_MUTANTS, POLICY, Plan, VERSION, digest};
use syn::spanned::Spanned as _;

/// Generate a bounded source inventory, admitting every emitted candidate again.
/// All bytes outside exactly one executable body remain unchanged.
///
/// # Errors
/// Rejects invalid source, budgets, source ranges or changed contract bindings.
#[allow(
    clippy::too_many_lines,
    reason = "One bounded inventory preserves original spans and unchanged binding checks for each emitted source."
)]
pub fn plan(input: &sail_api::contracts::Submission, budget: usize) -> crate::Result<Plan> {
    input.validate().map_err(super::unsupported)?;
    if !(1..=MAX_MUTANTS).contains(&budget) {
        return Err(super::unsupported(
            "mutation budget must be between one and eight",
        ));
    }
    let baseline = super::lower_profile(&input.source, &input.crate_name, input.profile)?;
    let admitted = super::admission::admit(&input.source, input.profile)?;
    let mut functions = admitted
        .model
        .functions
        .iter()
        .filter(|f| !f.function.predicate)
        .collect::<Vec<_>>();
    functions.sort_by_key(|f| super::admission::identity(f));
    let mut result = Plan {
        version: VERSION,
        policy: POLICY.into(),
        baseline_source_sha256: input.source_sha256.clone(),
        candidates: Vec::new(),
        unsupported: Vec::new(),
        omitted: 0,
    };
    for function in functions {
        let source_start = function.function.item.span().start();
        let binding = baseline
            .manifest
            .bindings
            .iter()
            .find(|b| b.source.line == source_start.line && b.source.column == source_start.column)
            .ok_or_else(|| super::unsupported("mutation target has no exact guarantee binding"))?;
        let identity = binding.function.clone();
        let literals = if function.owner.is_none() && function.function.requirements.len() == 1 {
            match &function.function.item.sig.output {
                syn::ReturnType::Type(_, ty) => match ty.as_ref() {
                    syn::Type::Path(path)
                        if path.qself.is_none() && path.path.segments.len() == 1 =>
                    {
                        match path.path.segments[0].ident.to_string().as_str() {
                            "bool" => vec!["false", "true"],
                            "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16"
                            | "i32" | "i64" | "i128" | "isize" => vec!["0", "1"],
                            _ => Vec::new(),
                        }
                    }
                    _ => Vec::new(),
                },
                syn::ReturnType::Default => Vec::new(),
            }
        } else {
            Vec::new()
        };
        if literals.is_empty() {
            result.unsupported.push(format!(
                "{identity}: policy supports only single-requirement free scalar-return functions"
            ));
            continue;
        }
        let body = &function.function.item.block;
        let begin = offset(&input.source, body.span().start())?;
        let end = offset(&input.source, body.span().end())?;
        for literal in literals {
            let replacement = format!("{{ {literal} }}");
            let replacement_ast: syn::Block =
                syn::parse_str(&replacement).map_err(super::syntax)?;
            if body.to_token_stream().to_string() == replacement_ast.to_token_stream().to_string() {
                continue;
            }
            if result.candidates.len() >= budget {
                result.omitted += 1;
                continue;
            }
            let mut source = input.source.clone();
            if begin >= end || !source.is_char_boundary(begin) || !source.is_char_boundary(end) {
                return Err(super::unsupported("invalid mutation body range"));
            }
            source.replace_range(begin..end, &replacement);
            let candidate = sail_api::contracts::Submission {
                profile: input.profile,
                source_sha256: digest(source.as_bytes()),
                source,
                crate_name: input.crate_name.clone(),
            };
            let lowered =
                super::lower_profile(&candidate.source, &candidate.crate_name, candidate.profile)?;
            let identity_fields = |manifest: &sail_api::contracts::LoweringManifest| {
                manifest
                    .bindings
                    .iter()
                    .map(|b| {
                        (
                            &b.requirement_id,
                            &b.function,
                            &b.lowered_function,
                            &b.guarantee_sha256,
                        )
                    })
                    .map(|(a, b, c, d)| (a.clone(), b.clone(), c.clone(), d.clone()))
                    .collect::<Vec<_>>()
            };
            if identity_fields(&baseline.manifest) != identity_fields(&lowered.manifest) {
                return Err(super::unsupported(
                    "mutation changed a contract or function identity",
                ));
            }
            result.candidates.push(Candidate {
                id: digest(
                    format!("{POLICY}:{}:{identity}:{literal}", input.source_sha256).as_bytes(),
                ),
                function: identity.clone(),
                requirement_id: binding.requirement_id.clone(),
                guarantee_sha256: binding.guarantee_sha256.clone(),
                replacement: literal.into(),
                body_start: begin,
                body_end: end,
                source: candidate,
            });
        }
    }
    result.unsupported.sort();
    Ok(result)
}
fn offset(source: &str, position: proc_macro2::LineColumn) -> crate::Result<usize> {
    let mut start = 0;
    for (index, line) in source.split_inclusive('\n').enumerate() {
        if index + 1 == position.line {
            let column = line
                .char_indices()
                .nth(position.column)
                .map(|(i, _)| i)
                .or_else(|| (line.chars().count() == position.column).then_some(line.len()))
                .ok_or_else(|| {
                    super::unsupported("mutation source column is outside its UTF-8 line")
                })?;
            return Ok(start + column);
        }
        start += line.len();
    }
    Err(super::unsupported(
        "mutation source line is outside the source",
    ))
}
