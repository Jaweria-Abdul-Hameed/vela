# Spike S-PROC-JOB (SP03): Job Object tree termination

**Result: VERIFIED (executed on the development host, 2026-10-12, Windows 11).** Run by the build agent without the user; no Antigravity or credentials were involved (a synthetic tree stands in for the `agy` process tree).

## Method
`crates/vela-process/src/tree/tests.rs` spawns `powershell -Command "Start-Process ping ... ; Start-Process ping ... ; Start-Sleep 120"` (a root plus two grandchildren), assigns the root to a Job Object created with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, and checks `ActiveProcesses` from `QueryInformationJobObject`.

## Evidence
| Check | Result |
|---|---|
| Job contains the root and both PowerShell-started children (count >= 3) | PASS |
| `TerminateJobObject` leaves 0 processes | PASS |
| Dropping the last job handle (kill-on-close) ends the root | PASS |
| Graceful request (`taskkill /T`, no `/F`) then forced terminate leaves 0 processes; cancelled flag set; registry entry tracked and unknown ids are harmless | PASS |

## Decision
Use the Job Object mechanism (no `taskkill /F /T` fallback needed on the primary path).

## Known limits
- A descendant spawned in the microseconds between `spawn` and `AssignProcessToJobObject` is not in the job. The runner assigns immediately; the real `agy` tree was not exercised (SP03 is about the mechanism).
- A process created with `CREATE_BREAKAWAY_FROM_JOB` can leave the job; Vela does not set the breakaway-OK limit, so breakaway fails for children.
