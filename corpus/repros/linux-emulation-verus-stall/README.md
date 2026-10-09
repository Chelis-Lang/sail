# Local emulation stall during plain Verus warm-up

On macOS arm64 with the Linux amd64 CI image under Docker's Rosetta
emulation, `ci/local.sh e2e` stalled while its initial **plain**
`cargo verus verify` verified the fresh project's vstd dependency. No Sail
solver process had started. The workload is the two-function fixture embedded
in `ci/run.sh`'s end-to-end step.

Evidence captured on 2026-10-09:

- Image input tag: `df4ba79c5c7c3cc1`.
- Pinned Verus `0.2026.10.04.426d8b0`, Z3 `4.16.0`.
- The verifier's CPU counters did not change between snapshots. Five threads
  waited in `rt_mutex_schedule`; another waited in `futex_do_wait`.
- The real Z3 child waited in `anon_pipe_read` for more commands.
- Native Linux CI passed the unchanged end-to-end step. The integrated local
  gate and all 35 solver/fault/portfolio tests had already passed.

The raw process snapshots, image identity, fixture and warm-up streams were
retained locally. The interrupted run is not acceptance evidence. Only its
owned test container was stopped; unrelated containers and Docker itself
were left running. Retry the same command and comparison unchanged. This
observation implicates the local emulation environment; it does not establish
a Verus or Sail defect, and no issue was filed upstream.
