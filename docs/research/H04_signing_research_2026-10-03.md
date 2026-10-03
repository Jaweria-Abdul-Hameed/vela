# H04: Release Signing Research Record (2026-10-03)

Date accessed: **2026-10-03**. Research only, not legal or purchasing advice. Nothing was purchased, no account was created, no production credential or key was generated, and no machine security setting was changed. Production signing choices are intentionally unresolved (see `docs/issues/graph/tickets/H04.md`).

## Two different signatures

| | Tauri updater artifact signing | Windows Authenticode code signing |
|---|---|---|
| Purpose | Lets the Tauri updater verify update artifacts | Signs the installer, `vela-hook`, and other Windows binaries |
| Keys | Its own keypair (`tauri signer generate`); the public key goes in `plugins.updater.pubkey`; the private key is passed as `TAURI_SIGNING_PRIVATE_KEY` (and an optional password) | A certificate and key from a route the user chooses (not yet chosen) |
| Required by | The updater itself: "This cannot be disabled." | FR-047 and the release checklist; not required for Windows to run the app |
| Status in v1 | Test keypair for development (Z02); production keypair at release (H04) | Development self-signed certificate (Z01); production route at release (H04) |

Sources (Tauri): https://v2.tauri.app/plugin/updater/ (quotes re-verified against the raw page: "This cannot be disabled", the public key "cannot be a file path", `.env` files do not work, and "if you lose this key you will NOT be able to publish new updates to the users that have the app already installed"); https://v2.tauri.app/distribute/sign/windows/ (code signing "is not required to execute your application on Windows, as long as your end user is okay with ignoring the SmartScreen warning"; its certificate-file CI guidance covers only OV certificates acquired before 2023-06-01); https://v2.tauri.app/distribute/windows-installer/ (NSIS and WiX MSI; installer packaging itself needs no signing).

## Evidence labels used for costs

-   **Verified official current price:** a price shown on the provider's own current pricing page.
-   **Unverified or illustrative:** a figure stated by a source but not confirmed on the provider's own pricing, or a general range not checked against providers.
-   **Unknown:** no figure found.

| Item | Figure | Label |
|---|---|---|
| Azure Artifact Signing (formerly Trusted Signing) | Microsoft Learn says "~$9.99/month"; the Azure pricing page shows plan names (Basic, Premium) but no prices ("$-", request a quote or use the calculator) | **Unverified or illustrative** (an official Microsoft statement of an approximate figure, not confirmed on the pricing page) |
| OV certificate from a certification authority | "$150-300/year" appears on a Microsoft page as a typical range | **Unverified or illustrative** (not checked against any certification authority) |
| EV certificate | "$400+/year" appears on the same Microsoft page | **Unverified or illustrative** (not checked against any certification authority) |
| Self-signed development certificate; Tauri updater test keypair | No purchase involved | Verified (no cost) |
| Microsoft Store (MSIX) route | Microsoft says code signing is free and handled for you; out of v1 scope (the specification uses NSIS and the Tauri updater) | Unverified beyond Microsoft's statement |
| SignPath Foundation (named by Microsoft as free for qualifying open-source projects) | Eligibility and terms not checked; the repository is public and GitHub reports no license for it | Unknown |
| Total production signing spend | Not determined | Unknown |

Source: https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options (page dated 2026-08-29), https://azure.microsoft.com/en-us/pricing/details/artifact-signing/ (shows no prices).

## Facts about the routes (Microsoft sources, accessed 2026-10-03)

-   Artifact Signing: needs a paid Azure subscription (free, trial, and sponsored subscriptions are not supported); identity validation is required and cannot be expedited; individuals are limited to the USA and Canada, organizations to the USA, Canada, the EU, and the UK; the certificate is never given to the developer (it is held in the service) and CI authenticates to Azure instead; the billed amount is the full SKU for each month. https://learn.microsoft.com/en-us/azure/artifact-signing/faq (page dated 2026-08-14).
-   Certificates from a certification authority: Microsoft states private keys for OV certificates must be on a hardware module or token (CA/Browser Forum requirement since June 2023), so a downloadable certificate file with a password in CI applies only to older certificates.
-   SmartScreen: EV no longer bypasses it; reputation builds from publisher identity and file hash over time for every non-Store route. https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation (page dated 2026-08-17).
-   Smart App Control: in enforcement mode it blocks unknown unsigned code and allows code signed by a certificate chaining to the Microsoft Trusted Root Program. https://learn.microsoft.com/en-us/windows/apps/develop/smart-app-control/overview (page dated 2025-11-18). A self-signed certificate does not chain to that program.

## Observation on the current development machine

Smart App Control was observed **ON** (enforcement) on 2026-10-03 (read-only registry query). It may affect unsigned or self-signed development artifacts. It was not changed, and changing it is not a prerequisite for Vela development; any actual blocker found during implementation is to be surfaced, not worked around by weakening the setting. The H05 clean environment is the intended place for installer and signing tests.

## What is decided, deferred, and where it lives

-   Decided (2026-10-03): H04 is a production-release prerequisite only; development and tests use development and test material; Authenticode and updater signing are separate; Z01, Z02, H04, and Z06 own the pieces listed in their tickets; H04 gates Z06 directly.
-   Deferred to the release-signing phase, all the user's: production Authenticode provider or route, purchase and spend, identity-validation route, production credentials, production updater private-key generation and custody and backup, and the final updater hosting location.
