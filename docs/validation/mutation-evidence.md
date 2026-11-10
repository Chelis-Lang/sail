# Mutation kill evidence on real projects (#664, ADR 0111)

| | |
|---|---|
| Status | **Spike observation, not acceptance.** It supports the evidence rule in [ADR 0111](../decisions/0111-mutation-evidence.md). `sail mutate` is not built, and nothing here qualifies it. |
| Source | Sail `b0c6b3a5` plus the spike's oracle patch (branch `spike/v030-mutev`, commits `f3ca5d9b` and `75c86cb8`); `sail` sha256 `1c44e73b…f9`, `sail-z3` shim sha256 `5cda4be0…a9` |
| Runtime | Stock Verus 0.2026.10.18.7d82994 (source `7d829946`); fork engine `a0e11cff` used only for Sail's report; Z3 4.16.0 (sha256 `e583c418…65`); Rust 1.99.1 for Verus |
| Host | Native EC2 c7i.4xlarge in us-east-2 (Xeon Platinum 8488C, 16 vCPUs), Linux 7.0.0-1014-aws, CI image `sail-ci:f57ed8a3e26baaff-core` |
| Projects | verified-ironkv `7e35ed09` (unit `ironsht`), 38 mutants; Vest `11af01dd` (unit `vest_lib`), 30 mutants; registry dependencies from `scripts/shipped-deps/Cargo.lock` |
| Date | 2026-11-09/10 |

## Scope

- Each mutant ran five times under stock `cargo verus`, with the pinned Z3
  behind the byte-forwarding `sail-z3` shim and its tap on. That came to 340
  runs, 44,650 solver sessions and 554,920 checks. Each mutant also ran once
  with no Sail binary in the process tree, and once through Sail for its
  structured report.
- Two overlapping sets: 20 catalogue mutants and 20 exec-token mutants per
  project, 68 distinct mutants in all. Exec-token mutants change only
  executable code.
- Plain-Verus versions of the #664 minimal cases (Boolean identity,
  increment, body edit, a bit-vector control) ran under stock Verus and Sail.
- Three oracle workers per project ran alongside other steps. The timings
  are not measurements.

## Result

| Exec-token set (40) | Count |
|---|---:|
| Rejected, Verus error attributed to the mutated function | 39 |
| of which also hit an rlimit in another function (Vest) | 2 |
| No failure beyond the baseline's own (`vest-compare-005`) | 1 |
| Rejections whose only non-`unsat` answers were `unknown "(incomplete quantifiers)"` (plus `canceled` for the rlimit elsewhere) | 39 |
| Rejections with native `sat` | 0 |
| Rejections Sail could not attribute answers to, because of duplicate friendly names (`ironkv-arith-006`, `vest-constant-011`, `vest-statement-026`) | 3 |

- Stability: 68 of 68 mutants gave the same outcome, exit, failing functions
  and diagnostics in all five runs. None had byte-identical solver input
  across runs. Stderr line order varied for 6 IronKV mutants.
- Native `sat` appeared in only two catalogue mutants (`vest-constant-006`,
  `vest-logic-003`), both on bit-vector assertions.
- Baseline: IronKV's unmutated baseline verifies. Vest's always fails
  `parse_cbor_with_child` on its rlimit (`unknown "canceled"`), so its
  mutants exit 101 even when they survive.
- Outside the exec-token set, 5 IronKV catalogue mutants verified, 1 more
  Vest mutant showed no new failure, 5 were rejected only in other functions,
  and 1 (`vest-logic-003`) had a span in the mutated function but no
  function attribution.
- Minimal cases: each mutant fails `postcondition not satisfied` on an
  `unknown "(incomplete quantifiers)"` check, followed by `unsat` checks.
  Each baseline proves with one `unsat`. The bit-vector control fails on a
  native `sat`. Sail's output matched stock's for every case in all five
  runs.
- Retrospectively, under ADR 0111, the exec-token set would score 36
  `verifier-rejected`, 0 `counterexample-confirmed`, 1 survived and 3
  inconclusive (attribution). That is a score of 36/37. This reading assumes
  the baseline twins had headroom, which the spike did not measure.

## Limits

- Two projects and 68 mutants on one host, stock Verus only, Linux only.
- The spike did not record each baseline twin's rlimit use, so it did not
  apply ADR 0111's headroom condition.
- The Vest survivor ran unscoped, but the classification is from the spike's
  scripts, not from an implementation of the rule.
- Raw logs were not kept; the host was terminated. The analysed records are
  on branch `spike/v030-mutev` at `75c86cb8`, under
  `spikes/v030-mutev/results/`:

| Record | sha256 |
|---|---|
| `ironkv-traced.json` (stored gzipped) | `035f6c2b1376fcaf826b44a665bdd66d7f0cbfdaf35f01e1c25e05c4db42ada7` |
| `vest-traced.json` (stored gzipped) | `2d54038e2e453c3b2dd63f6144efc1a1888c574eda81f5631188da87d12e4bad` |
| `ironkv-plain.json` (stored gzipped) | `fbe0c35083765e80bb7751c4a8121cf32afb6abe531ad545f4ed74c590eff3e6` |
| `vest-plain.json` (stored gzipped) | `c0482f3261b727f072fb8ad600caea24567ec25419e17a1e7fd44c427ad12f4f` |
| `minimal.json` | `ad04eccd13533518f73595fcb79a3d6f09216c974b6ac691dfd5831b133af5d9` |
| `ironkv-stock.tsv` | `b3d4237a4f892024204a7164f69822d1d1517c7385a9ec69512b66bcc9af182d` |
| `vest-stock.tsv` | `77e77eca4da7ed93895182158fb717de283104c542ef51203f20fcfe5258b7ed` |
| `ironkv-sail-table.json` | `5d3a4baa3724a13b7a6cbc7c463f8fa0575037e74ac39bae33f48bfb5a5ec5ac` |
| `vest-sail-table.json` | `99fb1fff0a48befefb84e1225edb84cfc428c07ca4f8b7b8d74826c74787fbe9` |
