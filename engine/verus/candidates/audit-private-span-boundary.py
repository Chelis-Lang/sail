#!/usr/bin/env python3
"""Static routed span-read boundary gate for the unselected private-span candidate."""
import argparse
import pathlib
import re
from importlib.machinery import SourceFileLoader


def audit(source):
    source = pathlib.Path(source)
    SourceFileLoader('graph_audit', str(pathlib.Path(__file__).with_name('audit-graph-ledger.py'))).load_module().audit(source)
    context = (source / 'vir/src/context.rs').read_text()
    production = context.split('#[cfg(test)]')[0]
    assert re.search(r'^    datatype_graph_span_infos: Vec<Span>,$', production, re.M)
    assert not re.search(r'pub(?:\([^)]*\))?\s+datatype_graph_span_infos\s*:', production)
    for path in source.rglob('*.rs'):
        if path == source / 'vir/src/context.rs':
            continue
        text = path.read_text()
        uses = re.findall(r'\.\s*datatype_graph_span_infos\b(?!\s*\()', text)
        assert not uses, ('unrouted private span read', str(path))
    # The only raw self-field owners are the fresh factory copy and guarded
    # accessor; Ctx seed registration calls the same accessor. Construction
    # locals remain complete fresh trait-check inputs.
    assert len(re.findall(r'\bself\.datatype_graph_span_infos\b', production)) == 2
    assert '(&self.global.no_span,self.global.datatype_graph_span_infos())' in production
    method = production.split('fn datatype_graph_span_infos(&self)', 1)[1].split('\n    }', 1)[0]
    assert method.index('sail_graph_capture.unsupported()') < method.index('&self.datatype_graph_span_infos')
    factory = production.split('pub fn from_self_with_log', 1)[1].split('pub(crate) fn datatype_graph_span_infos', 1)[0]
    assert factory.index('sail_graph_capture.unsupported()') < factory.index('sail_graph_capture: Default::default()')
    fork = production.split('pub fn sail_oracle_fork', 1)[1].split('pub fn sail_oracle_effect_fault', 1)[0]
    assert fork.index('sail_graph_capture.unsupported()') < fork.index('global.sail_graph_capture=')
    baseline = production.split('pub fn sail_baseline', 1)[1].split('pub(crate) fn checking_spec_preconditions', 1)[0]
    assert 'datatype_graph_span_infos' not in baseline
    components = production.split('pub fn sail_oracle_components', 1)[1].split('pub fn sail_baseline', 1)[0]
    assert 'datatype_graph_span_infos' not in components
    assert len(re.findall(r'capture!\(', components)) == 30
    traits = (source / 'vir/src/recursive_types.rs').read_text()
    assert traits.count('ctx.datatype_graph_span_infos()') == 1
    verifier = (source / 'rust_verify/src/verifier.rs').read_text()
    inner = verifier.split('fn verify_crate_inner', 1)[1]
    assert inner.index('recursive_types::check_traits') < inner.index('verify_bucket_outer(')
    outer = verifier.split('fn verify_bucket_outer', 1)[1].split('fn verify_crate_inner', 1)[0]
    assert outer.index('sail_begin_graph_reads') < outer.index('ast_to_sst_krate')
    print('PASS: private seed read, both clone-escape guards, 30 exact remaining components, fresh trait checks')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True)
    audit(parser.parse_args().source)
