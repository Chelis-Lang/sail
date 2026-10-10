#!/usr/bin/env python3
"""Static coverage gate for an applied graph-ledger candidate; does not build."""
import argparse
import pathlib
import re


def audit(source):
    source = pathlib.Path(source)
    context = (source / 'vir/src/context.rs').read_text()
    for field in ['func_call_graph', 'func_call_sccs']:
        assert re.search(r'^    ' + field + r':', context, re.M), field + ' must be private'
        assert not re.search(r'pub(?:\([^)]*\))?\s+' + field + r':', context), field
    for path in source.rglob('*.rs'):
        if path == source / 'vir/src/context.rs':
            continue
        text = path.read_text()
        assert not re.search(r'\.\s*(func_call_graph|func_call_sccs)\b', text), str(path)
    # Constructor locals are independent builders. These are the only reads of
    # the retained private field: fresh-clone, four wrappers and two validators.
    assert len(re.findall(r'\bself\s*\.\s*func_call_graph\b', context)) == 7
    assert len(re.findall(r'\bself\s*\.\s*func_call_sccs\b', context)) == 4
    expected = {
        'vir/src/ast_to_sst.rs': {'call_graph_scc_rep': 1},
        'vir/src/ast_to_sst_func.rs': {'call_graph_scc_rep': 1},
        'vir/src/ast_to_sst_crate.rs': {'call_graph_function_order': 1},
        'vir/src/recursion.rs': {'call_graph_scc_rep': 3, 'call_graph_is_cycle': 1},
        'vir/src/recursive_types.rs': {'call_graph_all_sccs': 1, 'call_graph_scc_nodes': 1,
                                       'call_graph_is_cycle': 1, 'call_graph_shortest_cycle': 1},
        'vir/src/sst_to_air_func.rs': {'call_graph_shortest_cycle': 1},
        'rust_verify/src/commands.rs': {'call_graph_all_sccs': 2, 'call_graph_scc_nodes': 3},
    }
    for name, counts in expected.items():
        actual = (source / name).read_text()
        for method, count in counts.items():
            assert len(re.findall(r'\b' + method + r'\(', actual)) == count, (name, method)
    engine = (source / 'rust_verify/src/sail_engine.rs').read_text()
    assert engine.index('sail_validate_graph_reads(portable.graph_reads.frames())') < engine.index('ctx.sail_restore_effects')
    assert 'portable.version!=3' in engine and 'sail-prepared/graph-ledger/v3' in engine
    assert 'self.global.func_call_graph.sail_encoding()' not in context
    assert 'self.global.datatype_graph.sail_encoding()' in context
    verifier = (source / 'rust_verify/src/verifier.rs').read_text()
    start = verifier.index('sail_begin_graph_reads')
    assert start < verifier.index('vir::ast_to_sst_crate::ast_to_sst_krate', start)
    assert verifier.index('vir::poly::poly_krate_for_module', start) < verifier.index('sail_finish_graph_reads', start)
    print('PASS: private graph coverage, routed read inventory, replay-before-effects and capture boundaries')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', required=True, help='applied candidate source/ directory')
    audit(parser.parse_args().source)
