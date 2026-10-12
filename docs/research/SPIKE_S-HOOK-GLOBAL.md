# Spike S-HOOK-GLOBAL (SP06) and isolated-profile findings

**Result: VERIFIED (run by the build agent on the development host, 2026-10-12, Windows 11, agy 1.3.3; the CLI had
updated itself from 1.2.14 earlier in the session).** The real account was used for a few short headless turns; no credential
file was read or copied.

## Questions and answers

| Question | Answer | Evidence |
|---|---|---|
| Is a **global** hooks file in an isolated profile (`USERPROFILE` and `HOME` redirected) loaded? | **Yes.** `<profile>/.gemini/config/hooks.json` is loaded and its `PreToolUse` hook runs. | A deny hook placed there ended a `run_command` step in `ERROR` with `tool call denied by pre-tool hook: <reason>`. |
| Where do native allow rules live in an isolated profile, and do they work? | `<profile>/.gemini/antigravity-cli/settings.json` with `{"permissions":{"allow":["command(git status --short)"]}}`. **Yes, they work**, and authentication still works (the OS credential store is not under the profile). | The exact command ran with real output. The CLI created its own store (`brain`, `cache`, ...) inside the profile. |
| How is a hook command executed? | With the **hooks file's folder as the working directory**, and **not through a shell**: quotes are passed to the program literally, so a quoted absolute path fails (`Cannot find module '...\"C:\...\"'`). | Two failed runs, then a pass with a relative, space-free command (`node hook-deny.js`). |
| What does a hook crash look like? | Fail closed: the tool step ends in `ERROR` with `JSON hook "<name>" failed: command failed: exit status N, stderr: ...`. | Recorded as `tests/fixtures/transcripts/hook-crash`. |
| Does a hook deny show in the stream? | Yes: step `state: ERROR`, `tool_info.error.message = "tool call denied by pre-tool hook: <reason>"`. | `tests/fixtures/transcripts/hook-deny`. |
| How are global skills delivered to an isolated profile? | The CLI does **not** read `<profile>/.agents/skills`. It reads skills from **enabled plugins**: `<profile>/.gemini/config/plugins/<plugin>/skills/<skill>/SKILL.md` plus `{"plugins":{"<plugin>":{"enabled":true}}}` in `<profile>/.gemini/config/config.json`. Project skills (`<worktree>/.agents/skills`) work as before. | A `vela-probe` skill delivered this way was resolved by `-p "/vela-probe"` and answered `SKILL-PROBE-OK`. |

## Consequences implemented

- A05 writes the allow rules, the hooks file (`vela-hook.exe --table vela-table.json --spool spool`, relative and space-free), a
  copy of the hook binary, the rule table and the spool beside the hooks file.
- A08 `provide_to_profile` writes the `vela-skills` plugin. The hook allows reads under that plugin folder only (the policy
  engine otherwise refuses `.gemini` paths); the exception resolves a relative table path against the working directory.
- The user's own `config.json` carries `globalPermissionGrants.allow` (for example `command(python git gh)`). An isolated profile
  drops those, which is the intended containment.

## Known limits

- **Smart App Control.** On a machine with Smart App Control (or Device Guard) on, a freshly built, unsigned `vela-hook.exe`
  can be blocked (`exit status 4551`). The hook then fails closed (every tool is denied), which is safe but stops work. Release
  builds are code-signed (H04); development builds may need a retry or the control off.
- Windows 8.3 short names (`RUNNER~1`) are rejected by the path policy; callers must pass long paths.
- Whether Antigravity Desktop shows conversations created in an isolated profile (S-DESKTOP-VISIBILITY, SP08) still needs the
  Desktop UI and is not settled here: the CLI keeps those conversations in the profile's own store.
