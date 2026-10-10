# F02 Decision Requests

Questions found while implementing F02 that the frozen specification does not settle. F02 does not decide product behavior, so
each is recorded here for the owning ticket or the user. None blocks F02.

## DR-F02-1: Which group module registers `onboarding.set` (and the other commands whose prefix is not a group)?

- **Specification facts.** `IMPLEMENTATION_ARCHITECTURE.md` section 6 lists the IPC commands, including `onboarding.set` and
  `project.trust.set|revoke`. `SHARED_SURFACE_PROTOCOL.md` section 4 lists fifteen command *group modules* (`project`, `settings`,
  `trust`, `preflight`, `graph`, `run`, `ticket`, `timeline`, `evidence`, `intervention`, `approval`, `policy`, `diagnostics`,
  `update`, `snapshot`). There is no `onboarding` group, and a `trust` group exists although the trust commands are named
  `project.trust.*`. So command names and group modules are not the same namespace, and the specification does not say which group
  registers which command.
- **What F02 did.** It did not invent commands and did not add a group (the group list is part of the frozen protocol). The registry
  type treats a command name as `segment.segment[...]` that is independent of the group module registering it, so `onboarding.set`
  can be registered under whichever group is chosen without editing F02 code. F02 registers no real commands.
- **Likely answer.** `onboarding.set` registers in the `settings` group (onboarding choices are user-global settings,
  `IMPLEMENTATION_ARCHITECTURE.md` section 9, and S02 owns the onboarding keys); `project.trust.*` registers in the `trust` group.
- **Decision needed from.** The user (or the S02 and S03 ticket owners): confirm those two assignments, or add a group to
  `SHARED_SURFACE_PROTOCOL.md` section 4 (a specification change).

## DR-F02-2: Closed-set payload fields are strings

- Several payload fields hold a closed set that is documented in prose only (`ApprovalClassified.decision`: `ALLOW|ASK|DENY`,
  `ApprovalDetected.evidence_tier`, `HumanActionRequired.intervention_kind`, `RunCompleted.outcome`, `WorkerResumed.from_state`,
  `ReviewFindingResolved.disposition`). The sets are defined by documents owned by later tickets (the approval domain objects, the
  run and worker state tables of D01), so F02 keeps them as strings. Tightening them to enums later changes the generated contract.
- **Decision needed from.** The owning tickets (D01, K01): promote each to an enum when its set is implemented, as a documented
  contract change.
