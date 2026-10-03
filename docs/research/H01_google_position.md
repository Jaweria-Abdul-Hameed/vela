# H01: Google's Position on External Orchestration of the Headless `agy` (Evidence Record)

Date accessed: **2026-10-03**. Author: Claude (research only; no policy was inferred from Vela's technical probes). This is a dated snapshot of external sources, not a permanent truth, and it is **not legal advice**.

**Status: OPEN, release gate only (Z06). Evidence classification: PARTIALLY-DOCUMENTED** (approved by the user, 2026-10-03; see "Decisions of record" and "Remaining action for H01" at the end). `agy` and headless automation are officially documented. External orchestration at Vela's level (a local desktop app repeatedly launching the official binary, with concurrent workers, unattended operation, fresh conversations, and approval delivery) is **not explicitly addressed** by the Terms, the FAQ, or the documentation. The only affirmative statement found is an informal reply from a Google-flagged forum account, which is not a term and is in tension with the literal text of the Terms and FAQ. **H01 is not resolved.**

The repository's equivalent labels (`VERIFIED`, `UNDOCUMENTED`, `RUNTIME`, `UNSUPPORTED`, `CHANGED`, in `EXTERNAL_VERIFICATION_2026-10-02.md`) describe individual facts; the four H01 states below describe the overall posture. Closest mapping: facts A and B are `VERIFIED`; C is `UNDOCUMENTED`; the credential-harvesting prohibition is `VERIFIED` (as already recorded in B9).

## What H01 requires

`H01` (`docs/issues/graph/tickets/H01.md`, ADR-016, FR-049): record a dated vendor position that orchestrating the official headless CLI with the user's own authenticated session is supported, **or** record the absence of one with the risk and a decision request. ADR-016 decision 6 already says this standing is "not confirmed by a vendor source" and is a **release prerequisite**. H01 has no blockers; its only dependent is Z06 (release candidate validation). The ticket's parallelization note recommends resolving it before A03 starts; the graph has no H01 to A03 edge.

## Sources

Kinds: **P** = official primary (Google-published documentation, terms, or repository); **F** = Google-hosted community forum (the forum's own metadata is reported; a forum reply is not a Google term); **G** = Google-employee statement in a Google repository.

| # | Source | URL | Kind | Notes |
|---|---|---|---|---|
| S1 | Antigravity CLI overview | https://antigravity.google/docs/cli/overview | P | No date shown. |
| S2 | Headless mode | https://antigravity.google/docs/cli/headless | P | No date shown. |
| S3 | Installation and auth | https://antigravity.google/docs/cli/install | P | No date shown. |
| S4 | Google Antigravity Additional Terms of Service | https://antigravity.google/terms/ | P | No effective date shown. The section number "6" is used by forum participants; it is not visible in the extracted text, so quote by wording, not number. |
| S5 | Antigravity FAQ | https://antigravity.google/docs/faq/ | P | No date shown. |
| S6 | Plans | https://antigravity.google/docs/plans/ | P | |
| S7 | Antigravity changelog | https://antigravity.google/docs/changelog | P | v1.2.x CLI entries, 2026-09; v2.5.0 on 2026-07-31. |
| S8 | Google Terms of Service | https://policies.google.com/terms | P | Effective 2026-07-30. |
| S9 | Generative AI Additional Terms | https://policies.google.com/terms/generative-ai | P | Last modified 2023-08-09; nothing relevant to this question. |
| S10 | Gemini CLI discussion 20632, "Addressing Antigravity Bans & Reinstating Access" | https://github.com/google-gemini/gemini-cli/discussions/20632 | G | Posted 2026-02-27 by `jackwotherspoon` (GitHub company `@google`, association COLLABORATOR) in the `google-gemini` organization. |
| S11 | `google-antigravity/antigravity-cli` | https://github.com/google-antigravity/antigravity-cli | P | Official repository ("Antigravity CLI brings the reasoning, execution, and orchestration capabilities of Antigravity agent harness directly into your terminal."). |
| S12 | ACP Registry entry `antigravity-acp` | https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json | P (Google-authored entry) | Authors "Google LLC", binaries at `dl.google.com`. See "Other official interfaces". |
| S13 | Forum thread 185992 | https://discuss.ai.google.dev/t/is-invoking-the-official-agy-cli-from-a-local-ai-coding-assistant-allowed-on-an-individual-google-ai-account/185992 | F | Reply by `chunduriv`, 2026-09-30. The forum's JSON reports this account as `staff=true`, `moderator=true`, flair `google`, group `tfteam`. |
| S14 | Forum threads 181088, 184829, 184785, 183051 | https://discuss.ai.google.dev/t/181088 and .../184829, .../184785, .../183051 | F | Replies by `Ambati_Rajendra` (181088, 184829, 184785) and `Engineer760` (183051). The forum's JSON reports **no** staff, admin, moderator, or Google flair for either account (`Ambati_Rajendra` is a community "Regular", trust level 3). **A first-pass fetch described `Ambati_Rajendra` as an official respondent; the account metadata contradicts that, so those replies are given no evidentiary weight.** |

Not used as evidence: Reddit, blogs, Stack Overflow, tutorials, third-party ACP wrappers, and Vela's own successful probes. Pages were fetched with a summarizing fetcher and every quote below that matters was re-verified against the raw page text on 2026-10-03.

## Questions A to F

### A. Is `agy` an officially documented and supported Antigravity interface? **Yes (explicit).**

- S1: "The Antigravity CLI is the lightweight Terminal User Interface (TUI) surface of Antigravity."
- S11 is a Google-organization repository for the CLI; S7 records ongoing CLI releases.

### B. Does official documentation describe non-interactive or headless automation? **Yes (explicit).**

- S2: "Run Antigravity CLI non-interactively to script agent tasks, integrate with CI pipelines, and capture machine-readable output."
- S2: "Headless mode uses your cached credentials. Authenticate once with an interactive `agy` session first." This is the documented design: headless uses the credentials of the user's own interactive sign-in.
- S3 documents a second headless/CI authentication route, a Gemini API key: "This suits headless and CI runs, where no browser is available to complete a sign-in." (Vela's ADR-016 decision 9 keeps API-key billing out of the normal v1 runtime.)

### C. Does Google explicitly permit an external local application orchestrating repeated `agy` invocations? **No explicit permission in any term, FAQ, or documentation page. One informal affirmative statement exists.**

- Explicit documents are silent on this pattern.
- Informal (S13, staff-flagged account, 2026-09-30, answering whether a local AI coding assistant may spawn the official binary): spawning the official, unmodified `agy` "locally as a single-user child process on your own machine using its documented headless flags is a supported use of the CLI, provided that" the user does not "extract, read, or forward Antigravity's cached OAuth tokens", all requests "execute through the unmodified `agy` binary under your own local `agy login` session", and the setup "is used strictly by you as an individual user on your local machine (not exposed as a shared server, multi-user relay, or unattended high-frequency polling loop)". The same reply says headless runs "draw from the exact same Google AI account quota" and advises reusing sessions with `-c` or `--conversation` to conserve quota.
- **Weight:** a forum reply, not a term. The asker's follow-up (2026-10-01) pointed out that the FAQ names Claude Code and that the Terms forbid use "in connection with products not provided by us", and asked for confirmation that this is "Google's official reading". **No reply followed.**

### D. Does Google explicitly prohibit such orchestration? **No explicit prohibition of launching the official binary was found. Broad prohibitions exist whose reach over it is not stated.**

Explicit and verified:

- S4: "You must not abuse, harm, interfere with, or disrupt the Service. This includes, but is not limited to, using the Service in connection with products not provided by us. Using third party software, tools, or services to access the Service (e.g. using OpenClaw with Antigravity OAuth) is a breach of this Agreement. Such actions may be grounds for suspension or termination of your Antigravity and/or Gemini CLI accounts."
- S5 (question: "Why can't I use third-party software (such as Claude Code, OpenClaw, or OpenCode) with my Antigravity login?"): "Using third-party software, tools, or services to access Antigravity is a violation of our Terms of Service and severely degrades the experience for legitimate product users. Such actions can result in suspension or termination of your account. To use a third-party coding agent with Gemini, we recommend using a Gemini Enterprise or Google AI Studio API key."
- S10: "Using third-party software, tools, or services to harvest or piggyback on Gemini CLI's OAuth authentication to access our backend services is a direct violation of Gemini CLI's applicable terms and policies."

Interpretation (mine, not Google's): the OAuth-harvesting, proxying, and direct-backend patterns are clearly prohibited. Whether an application that launches Google's own unmodified binary is "third party software ... to access the Service" or "use of the Service in connection with products not provided by us" is **not stated** in S4 or S5, and the one staff-flagged forum reply reads it narrowly while the FAQ and Terms wording is broader. That is a conflict between a narrow informal reading and broad literal text, recorded and not resolved.

### E. Restrictions on authentication, credentials, sharing, quota circumvention, abuse, or bypassing safety that affect Vela

| Topic | What the sources say | Status |
|---|---|---|
| Credentials and tokens | Harvesting or piggybacking on Antigravity or Gemini CLI OAuth is prohibited (S4, S5, S10). Credentials live in the OS keyring (S3). | Explicit. Matches ADR-016 decisions 7 and 8 and FR-049. |
| Account sharing | S4 does not address it. The staff-flagged reply excludes "a shared server, multi-user relay". | Informal only. |
| Quota circumvention | S10: recertification includes "acknowledging that bypassing system measures or circumventing usage limits is prohibited." S6: "baseline rate limits ... exist to prevent abuse." S8: no "bypassing our systems or protective measures". | Explicit as a rule; Vela does none of it by design. |
| Automated abuse | S4: "must not abuse, harm, interfere with, or disrupt the Service." S13 warns against "unattended high-frequency polling loop". | Explicit rule; scale thresholds undefined. |
| Permission and safety mechanisms | S4: the user is "solely responsible for ... authorizing an AI Agent's access and connection to data, applications, and systems". S2 documents `permissions.allow` rules and warns that `--dangerously-skip-permissions` "approves all tool calls". | Allow-rules and hooks are documented features; nothing found prohibits them. **Automating clicks on the Desktop approval card (UIA) is not addressed by any source** (separate from H06). |
| Enforcement | S7 records account blocks for Terms violations with an appeal link (2026-07-31). S10: a second violation is a permanent ban, and Antigravity bans also blocked Gemini CLI and Gemini Code Assist. | Explicit. The consequence falls on the user's own Google account. |

### F. Is the answer simply undocumented or ambiguous? **Yes, for Vela's level of orchestration.** Not addressed anywhere official: a packaged desktop application (as opposed to "your own script"), repeated and concurrent invocations, long unattended autonomous runs, a fresh conversation per worker (ADR-020), UIA delivery of approvals, and distribution to other users who each use their own session.

"Not explicitly prohibited" is not recorded as permitted, and "not documented" is not recorded as prohibited.

## Mapping Vela's intended behavior to the evidence

| Vela behavior | Evidence |
|---|---|
| Launch the official unmodified `agy` binary headless | Headless is documented (B). Informally affirmed for a local single-user child process (C). |
| User's own authenticated session; Vela never touches tokens | Documented headless design uses cached credentials (S2). Vela's token boundary matches every explicit prohibition. |
| Local orchestration, single user | Matches the conditions in the staff-flagged reply; not stated by any term. |
| Repeated invocations | Not addressed officially; the reply warns against high-frequency polling loops. |
| Concurrent workers where capacity permits | Not addressed. Existing design defaults to one session at a time (ADAPTERS.md CAP-07). |
| Unattended autonomous runs | In tension with the reply's "unattended high-frequency polling loop" condition; the Terms prohibit abuse without defining it. |
| Fresh substantive conversations (ADR-020) | Tension with the reply's quota advice to reuse sessions; advice, not a rule. |
| Generated allow-rules and fail-closed hooks | Documented features (S2); they restrict rather than bypass. |
| Approval Broker and UIA fallback | Not addressed. |
| No CAPTCHA, password, or 2FA automation; no credential extraction; no silent quota or account circumvention | Consistent with every explicit rule. No multi-account rotation or quota evasion is in scope. |

## Other official interfaces (discovered; not adopted; no change proposed)

- Gemini API key mode of the CLI (S3), documented for headless and CI. The FAQ recommends an API key for third-party coding agents (S5). ADR-016 decision 9 keeps it out of the normal v1 runtime unless a separate scope is approved.
- An ACP server published by Google LLC in the ACP Registry (S12). The Antigravity documentation page named as its website does not mention ACP. A forum staff-flagged reply mentions it; third-party ACP wrappers are not evidence of policy.

## Consequences for the current architecture

- No frozen ADR or the authentication boundary needs to change on this evidence. ADR-016 already states the position is unconfirmed and a release prerequisite; this record confirms that, and replaces its "only a forum answer" description with the classified evidence above.
- The architecture already has safe-by-construction elements: no token access, no account switching, no silent API-key substitution, one session at a time by default, no CAPTCHA, password, or 2FA automation.
- **Gap observed and since closed:** when this record was first written the repository documented no handling for a Terms-blocked or suspended account state (the changelog shows the CLI returns a distinct sign-in error with an appeal link). Following the user's decision of 2026-10-03 (see "Decisions of record"), FR-024 and AT-007 now require non-retryable, no-workaround, durable, user-surfaced handling.
- H01 cannot be marked resolved. Resolving it requires either a vendor or legal clarification that addresses Vela's pattern, or a decision by the user.

## What would change the classification

- To VERIFIED-SUPPORTED: an official Google source (terms, FAQ, documentation, or a written support response) stating that a local application may launch the official `agy` repeatedly under the user's own session, ideally covering unattended runs and concurrency.
- To VERIFIED-PROHIBITED: an official statement that launching the unmodified binary from third-party software is a breach.
- To UNRESOLVED: the staff-flagged reply being retracted, or an official source stating the opposite of it.

## Draft question for Google (for the user to send; not sent)

"Vela is a local Windows desktop application that I run myself, on my own machine, under my own signed-in Antigravity account. It repeatedly launches the unmodified official `agy` binary in headless mode (`-p` / `--output-format stream-json`), one or a few at a time, sometimes while I am away. It never reads, copies, stores, or reuses credentials or tokens, never calls backend endpoints, and does not rotate accounts or work around quotas. Does this fall within the Antigravity Additional Terms and FAQ ('third party software, tools, or services to access the Service' and 'in connection with products not provided by us')? Does anything change for unattended runs, a few concurrent `agy` processes within quota, or for an application distributed to other users who each use their own account? Please point to the official document that states this."

## Decisions of record (user, 2026-10-03)

1. **H01 stays OPEN and the classification stays PARTIALLY-DOCUMENTED.** The evidence review is approved; it does not resolve H01.
2. **Release gate only.** H01 gates Z06 (release-candidate completion) and nothing earlier. There is no H01 to A03 edge and none may be added. Development, local integration, spikes, and validation of the Antigravity adapter may continue while H01 is open. This is permission to continue implementation under documented uncertainty. **It is not a conclusion that Google officially permits Vela's orchestration pattern**, and Vela v1 must not be represented as release-ready while this question is unresolved.
3. **An authoritative written answer from Google is wanted.** The draft question above is kept for that purpose and nothing has been sent on the user's behalf. The staff-flagged forum reply is **not** sufficient to resolve H01.
4. **The Gemini API-key route is out of scope for Vela v1.** It remains recorded as a possible future integration path only. ADR-016 is not modified and does not authorize it; Vela v1 stays Antigravity-first with Antigravity-owned authentication.
5. **Provider-policy-block handling is required** and was added by extending FR-024 and AT-007 (no new FR or AT): a provider-neutral `PROVIDER_POLICY_BLOCK` is non-retryable, never worked around, stops provider work safely, is surfaced to the user, survives restart, and clears only by an explicit user decision after a user-initiated access re-check. See `docs/orchestration/CAPACITY_AND_PROFILES.md` ("Provider Policy Blocks"). The gap noted under "Consequences" is therefore closed as a requirement; its Antigravity signal mapping stays unverified `[U]` (a real block must never be provoked to obtain evidence).

## Remaining action for H01

Obtain an **authoritative written answer from Google** to the question above (through an official support or legal channel, not an informal forum reply), and record it here with its date, source, and exact wording. Until then H01 stays open, Z06 stays gated, and no source or Vela behavior is to be represented as Google's approval.
