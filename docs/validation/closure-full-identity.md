# Full closure-identity candidate

**Built on both platforms; unselected and unqualified as a Sail runtime.**
This private composition starts from Sail `e3322f4f` and binds fork
`cc7bbce4fdc8cf7e4256668b3ee8431343bdb6b9` to its actual normal Linux and
macOS runtime archives. The candidate keeps both halves of Rust's stable
crate-qualified closure identity for equality, hashing and internal SMT names.
This hardens the combined `8a373f81` candidate; no natural collision or wrong
verdict was observed. Earlier 8a performance and corpus results do not qualify
this subsequent source.

The accepted main runtime remains `73dea021`. These candidate pins describe
private artifacts; their canonical fork release names are prospective. No
public asset, default branch, pinned tag or Sail release is selected by this
composition. The repository and draft rc.5 remain private and unpublished.

| Gate | Actual status and scope |
|---|---|
| Focused native Linux | Pass: two VIR tests, 16 fresh imported-closure clients and ten verification matrix rows; not the complete suite or installed delivery |
| Normal macOS engine | Pass: genuine normal builder and original packer; fresh vstd 2,060 verified / 0 errors; all 15 archive members independently checked |
| Normal Linux engine | Pass: genuine normal builder, original packer and all 14 archive members independently checked |
| Separate complete Linux suite | Built and preserved: original manifest, 154 test executables, all 1,479 compiler source files and Git metadata; every nested archive member independently checked |
| Full fork suite | Not run: launcher refused its stock installation lookup before any test; the retained suite is available for a remaining-gates run |
| Ordinary bundled delivery, both platforms | Pending on this composition |
| Default gates, FAST, STANDARD, DEEP and determinism | Pending on this composition |
| Native edit and repair performance | Pending on this composition; the earlier 8a Vest repair judge remains failed at 1.7276x against its unchanged 2.0x gate |

All builds use upstream `426d8b01`, Rust `1.98.1` and the unchanged source lock
`80c71fdd00ef5073cd93edf6b5e37a635f7b4114fba8f6553ba2125eecb1395e`.
Runtime pins bind the actual archive, manifest and solver; the normal build is
kept distinct from the Vargo source-suite variant. No suite manifest is
converted into a normal-runtime manifest.

Raw evidence is retained outside the repository under the following records:

- `closure-full-identity-native-2026-10-17`: focused final archive SHA-256
  `18810f6898fc2108854d05430126bf47111b0ce9a3d3267b10aade6959839ed9`.
- `closure-full-identity-macos-2026-10-17`: normal archive SHA-256
  `65523848b990f49221035a087e685fe9666e65d34cb5b9d1b76f25f690f86d9b`;
  original manifest SHA-256
  `c35e88e0293bc6afbedddcd25d3750bcbac8202fd42d26108e6d9d3497cd445c`.
- `closure-full-identity-engine-linux-2026-10-17`: build checkpoint SHA-256
  `9b17f52d775dee79e5975e3754ee2e3df316f2cdea4ae1d76605003a1655c342`;
  normal archive SHA-256
  `717680b4cb126f48397b02f32dcbf54e0f2a320fa0c16c86a97237a8c3eee8b1`;
  original normal manifest SHA-256
  `8fd9ffb1ed9fe5bd82506d0b1b45571642c4886ad256b228b4bcd54b3f0db550`.
  The separate complete-suite archive has SHA-256
  `4f31e2dcff721eb2a274e21052f59c6f5689be3bb2f827ec323bee288f35c82a`:
  1,674 entries and 643,946,925 uncompressed bytes, including all 154 actual
  test executables and the original suite manifest.

Failed attempt, 2026-10-17: Sail `d422adf0` / fork `cc7bbce4`, full fork suite
launch exited 2 with `SAIL3003` before any test because its clean environment
omitted the explicit installed stock-toolchain path. Normal engine, complete
suite capture and two VIR tests had already passed. The failed attempt stays
failed; the remainder must restore the original suite prefix, recheck its
source and executable identities, exercise the actual runtime selection, then
run the complete unchanged suite. Passing build evidence supplies no missing
source-suite or application acceptance.
