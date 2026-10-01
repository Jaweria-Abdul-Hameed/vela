# ADR-009: Vela Policy Sovereignty and Native Permission Posture

## Status

Accepted (2026-10-02). Refines ADR-007; does not supersede it.

## Context

ADR-007 makes native Antigravity permission mechanisms the first approval-delivery tier. The
Prompt 2 audit (SA-02, SA-03) found two gaps:

- If a native mode executes operations without surfacing them, Vela's `ALLOW`/`ASK`/`DENY`
  policy is never consulted, so `DENY` and `ASK` cannot be enforced.
- The evidence used to bind an approval prompt to a concrete operation was undefined, so a
  misleading or spoofed prompt could be classified from untrusted display text.

Human decision (2026-10-02): Vela must not depend on Antigravity `Always Proceed`; Vela's own
policy must remain enforceable; Prompt 4 must verify the exact native mechanism before it is
frozen.

## Decision

### 1. Policy sovereignty

Vela's policy engine is the authority for what an unattended worker may do. Vela shall not
depend on, configure, or recommend an unconditional auto-execution mode (Antigravity
`Always Proceed` or any equivalent) as its unattended-operation mechanism. Native mechanisms
are used only to the extent that they are compatible with the Native Permission Posture below.

### 2. Native Permission Posture (NPP)

The effective Antigravity permission posture for a Vela-supervised session must satisfy:

- **NPP-1.** Any operation outside the native envelope surfaces to a channel Vela can observe
  before the operation executes (approval prompt, provider event, or equivalent), so Vela can
  classify it.
- **NPP-2.** The native envelope (operations Antigravity executes without surfacing anything)
  is bounded to the assigned worktree/workspace and contains no operation class that Vela
  policy classifies `ASK` or `DENY`.
- **NPP-3.** Vela never sets an unconditional auto-execute mode. If a user- or machine-level
  unconditional mode is detectable, the posture does not meet NPP.

Which concrete Antigravity settings or modes satisfy NPP is **not decided here**. It is an
external fact that Prompt 4 must verify against current primary documentation and the installed
environment, and that the real-environment suite (Prompt 15) must confirm. Until verified, the
posture is `UNKNOWN`.

### 3. Effective Posture Check (preflight)

Preflight reports exactly one of:

- `MEETS`: NPP-1..3 are demonstrated for the installed environment.
- `DOES_NOT_MEET`: a violation is detected.
- `UNKNOWN`: NPP cannot be demonstrated.

Consequences: `MEETS` permits Autonomous mode. `DOES_NOT_MEET` or `UNKNOWN` blocks Autonomous
mode. Supervised mode is permitted only with a recorded, explicit user acknowledgement that
Vela cannot enforce policy over native auto-executed operations, a persistent banner while the
run is active, and Vela-controlled gates (push, merge, promotion) still enforced by Vela
itself.

### 4. Detective controls

Independently of the native posture, Vela treats the following, when observable through its own
adapters, as policy violations: unexpected ref or remote changes, pushes it did not perform,
and writes in the primary checkout or outside Vela-managed worktrees. A violation pauses the
worker, raises `NEEDS_HUMAN` (kind `POLICY_VIOLATION`), and is journaled.

### 5. Evidence-Binding Rules (EBR)

An approval prompt may be bound to a normalized operation only under these rules:

- **EBR-1.** Operation facts come from the highest-trust channel available, in order:
  (a) a structured provider/CLI/SDK event; (b) the accessibility subtree of the identified
  approval control; (c) visual recognition of a verified layout.
- **EBR-2.** Only text inside the approval control's own subtree, in a window verified to belong
  to a Vela-created Antigravity session, counts as evidence. Transcript, chat, tool output, or
  any agent-authored content is never an operation source.
- **EBR-3.** Truncated, elided, scrolled, multi-operation, or not fully normalizable text yields
  `ASK`.
- **EBR-4.** Tier (c) evidence may yield `ALLOW` only for operation classes the versioned layout
  fingerprint declares fully legible; otherwise `ASK`.
- **EBR-5.** Where a post-execution observation exists (event stream, command record), Vela
  compares it with the approved signature; a mismatch is a policy violation (section 4).
- **EBR-6.** The control's identity must match a versioned adapter fingerprint. An unknown or
  unrecognized surface fails closed.

## Consequences

- Preflight, `APPROVAL_BROKER.md`, `SECURITY_AND_PERMISSIONS.md`, `AUTONOMY_MODES.md` and
  `PRODUCT_SPEC.md` (FR-038, FR-039) carry the normative detail.
- Autonomous readiness depends on external verification that is not yet done; this is a
  deliberate dependency, not an invented capability.
- Some routine prompts will be `ASK` when evidence is weak; this trades autonomy for safety by
  design.
