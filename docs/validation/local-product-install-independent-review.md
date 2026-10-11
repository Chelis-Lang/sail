# Local product installation independent review

The file-only review covers commits `1276670` through
`c5775892361d08f8e696555aa9d40c5e692650d4`.
The exact `scripts/local-product-install.py` SHA is
`fc69f2146c443e8861e969f9a623ef2e3533e71a9e305ed980fbb4ca000bbe36`;
the shared lifecycle helper SHA is
`dd6a5a0be115a56dbb93d50e721332552e0deeeef71d79dc8cdae10193d977b1`.
No static blocker was found. Eleven pure product controls independently passed
on the host in 0.019 seconds, using fake processes and temporary files. This
review started no Docker, compiler, verifier or installation actor and read no
native compiler target.

The prerequisite requires both original combined phases to pass with their exact
commands and caps, recorded group closure, source revision/archive and unchanged
container identity. Partial, failed or uncertain results cannot authorize a
product. Native checks bind the entire source inventory, target symlink, PID 1
birth, protected files, original phase logs and selected complete 13-artifact
engine pin/manifest before and after each product phase.

The ordinary release build must return zero before the existing installer runs.
The installer creates new owned private files, and the final grant requires
matching ELF sizes/hashes, file/directory modes and service ownership. The grant
retains the actual container creation/start identity and explicit execution
label, and passes the existing common grant validator. Source output remains in
the external target and new evidence directories; frozen source files are not
modified.

The extracted `OwnedPhaseRunner` preserves the earlier session creation,
birth-checked observation, owned wait, known-group postflight and sticky
uncertainty. Moving the checkpoint inside the owned wait's cleanup region closes
the earlier checkpoint-error window. Controller signals are conservatively
uncertain even between phases. A timeout, interrupted transport, uncertain
descendant closure or failed final guard prevents further execution or a grant.
Known-group closure remains narrower than arbitrary orphan absence.

Installation-only evidence does not certify proof Currentness, repair or reader
acceptance, source-engine qualification, or performance. Native build/install
execution and the forthcoming combined product gate remain separate scopes.
