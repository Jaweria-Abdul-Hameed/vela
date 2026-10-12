//! Remote operations (G05, ADR-011, ADR-015): fetch and push through the hardened invocation. Vela pushes with an explicit
//! `refs/heads/<branch>:refs/heads/<branch>` refspec and never forces; a push that already happened is recognized by
//! comparing the remote ref and not repeated; network, authentication and non-fast-forward failures are distinct typed errors.

use std::collections::BTreeMap;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ids::{BranchName, CommitSha, WorktreePath};
use vela_domain::ports::{
    FetchOutcome, GitContext, GitRemote, PortFuture, PushOutcome, RemoteName,
};

use crate::read::hardening::Mode;
use crate::read::{GitCli, classify_failure};

/// The arguments of `git fetch`: no tags, prune stale remote-tracking refs, the named remote only.
pub fn fetch_args(remote: &RemoteName) -> Vec<String> {
    vec![
        "fetch".to_owned(),
        "--no-tags".to_owned(),
        "--prune".to_owned(),
        remote.as_str().to_owned(),
    ]
}

/// The explicit, non-forcing refspec for pushing one branch to the same name.
pub fn push_refspec(branch: &BranchName) -> String {
    format!("refs/heads/{0}:refs/heads/{0}", branch.as_str())
}

/// The arguments of `git push`: the remote and one non-forcing refspec, nothing else.
pub fn push_args(remote: &RemoteName, branch: &BranchName) -> Vec<String> {
    vec![
        "push".to_owned(),
        "--no-follow-tags".to_owned(),
        remote.as_str().to_owned(),
        push_refspec(branch),
    ]
}

fn parse_ref_listing(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let (sha, name) = line.split_once(['\t', ' '])?;
            Some((name.trim().to_owned(), sha.trim().to_owned()))
        })
        .collect()
}

impl GitCli {
    async fn remote_branch_sha(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
        remote: &RemoteName,
        branch: &BranchName,
    ) -> Result<Option<CommitSha>, VelaError> {
        let reference = format!("refs/heads/{}", branch.as_str());
        let text = self
            .run_ok(
                ctx,
                repo,
                Mode::Remote,
                &["ls-remote", remote.as_str(), &reference],
            )
            .await?;
        Ok(parse_ref_listing(&text)
            .get(&reference)
            .and_then(|sha| CommitSha::new(sha).ok()))
    }

    async fn local_branch_sha(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
        branch: &BranchName,
    ) -> Result<CommitSha, VelaError> {
        let spec = format!("refs/heads/{}^{{commit}}", branch.as_str());
        let text = self
            .run_ok(
                ctx,
                repo,
                Mode::Read,
                &["rev-parse", "--verify", "--quiet", &spec],
            )
            .await
            .map_err(|_| {
                VelaError::of_class(
                    ErrorClass::ValidationFailure,
                    format!("the branch `{}` does not exist locally", branch.as_str()),
                )
            })?;
        CommitSha::new(text.trim())
            .map_err(|e| VelaError::of_class(ErrorClass::InternalBug, e.to_string()))
    }

    /// Pushes `branch` to `remote` unless the remote already has exactly its tip. Returns the remote head after the push.
    pub async fn push_branch(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
        remote: &RemoteName,
        branch: &BranchName,
    ) -> Result<PushOutcome, VelaError> {
        let local = self.local_branch_sha(ctx, repo, branch).await?;
        if self
            .remote_branch_sha(ctx, repo, remote, branch)
            .await?
            .as_ref()
            == Some(&local)
        {
            return Ok(PushOutcome { remote_head: local });
        }
        vela_domain::fault_point!(vela_domain::fault::FaultPoint::GitPushBefore);
        let args = push_args(remote, branch);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = self.run_raw(ctx, repo, Mode::Remote, &refs).await?;
        if !out.success() {
            return Err(classify_failure("push", &out));
        }
        vela_domain::fault_point!(vela_domain::fault::FaultPoint::GitPushAfter);
        match self.remote_branch_sha(ctx, repo, remote, branch).await? {
            Some(remote_head) if remote_head == local => Ok(PushOutcome { remote_head }),
            _ => Err(VelaError::of_class(
                ErrorClass::ExternalTemporary,
                "the push finished but the remote branch does not show the pushed commit; it will be verified again",
            )),
        }
    }

    async fn ref_listing(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
        prefix: &str,
    ) -> Result<BTreeMap<String, String>, VelaError> {
        let text = self
            .run_ok(
                ctx,
                repo,
                Mode::Read,
                &["for-each-ref", "--format=%(objectname) %(refname)", prefix],
            )
            .await?;
        Ok(parse_ref_listing(&text))
    }

    /// Fetches `remote` and reports which remote-tracking refs changed.
    pub async fn fetch_remote(
        &self,
        ctx: &GitContext,
        repo: &WorktreePath,
        remote: &RemoteName,
    ) -> Result<FetchOutcome, VelaError> {
        let prefix = format!("refs/remotes/{}/", remote.as_str());
        let before = self.ref_listing(ctx, repo, &prefix).await?;
        let args = fetch_args(remote);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = self.run_raw(ctx, repo, Mode::Remote, &refs).await?;
        if !out.success() {
            return Err(classify_failure("fetch", &out));
        }
        let after = self.ref_listing(ctx, repo, &prefix).await?;
        let mut updated: Vec<String> = after
            .iter()
            .filter(|(name, sha)| before.get(*name) != Some(*sha))
            .map(|(name, _)| name.clone())
            .collect();
        updated.sort();
        Ok(FetchOutcome {
            updated_refs: updated,
        })
    }
}

impl GitRemote for GitCli {
    fn fetch<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        remote: &'a RemoteName,
    ) -> PortFuture<'a, FetchOutcome> {
        Box::pin(async move { self.fetch_remote(ctx, repo, remote).await })
    }

    fn push<'a>(
        &'a self,
        ctx: &'a GitContext,
        repo: &'a WorktreePath,
        remote: &'a RemoteName,
        branch: &'a BranchName,
    ) -> PortFuture<'a, PushOutcome> {
        Box::pin(async move { self.push_branch(ctx, repo, remote, branch).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn the_refspec_names_one_branch_and_never_forces() {
        let branch = BranchName::new("vela/run-1/ticket-a").unwrap();
        assert_eq!(
            push_refspec(&branch),
            "refs/heads/vela/run-1/ticket-a:refs/heads/vela/run-1/ticket-a"
        );
        let remote = RemoteName::new("origin").unwrap();
        assert_eq!(
            push_args(&remote, &branch),
            [
                "push",
                "--no-follow-tags",
                "origin",
                "refs/heads/vela/run-1/ticket-a:refs/heads/vela/run-1/ticket-a"
            ]
        );
    }

    #[test]
    fn ref_listings_are_parsed_from_ls_remote_and_for_each_ref() {
        let sha = "a".repeat(40);
        let m = parse_ref_listing(&format!(
            "{sha}\trefs/heads/main\n{sha} refs/remotes/origin/x\n"
        ));
        assert_eq!(m.get("refs/heads/main"), Some(&sha));
        assert_eq!(m.get("refs/remotes/origin/x"), Some(&sha));
        assert!(parse_ref_listing("").is_empty());
    }

    proptest! {
        /// No force flag, forcing refspec or lease option can ever be produced, whatever the validated names are.
        #[test]
        fn push_arguments_never_force(remote in "[a-z][a-z0-9_-]{0,20}", branch in "[a-zA-Z][a-zA-Z0-9/_.-]{0,40}") {
            let (Ok(remote), Ok(branch)) = (RemoteName::new(remote), BranchName::new(branch)) else {
                return Ok(());
            };
            let args = push_args(&remote, &branch);
            prop_assert_eq!(args.len(), 4);
            for arg in &args {
                prop_assert!(!arg.starts_with("--force"), "{}", arg);
                prop_assert!(!arg.contains("force-with-lease"), "{}", arg);
                prop_assert!(arg != "-f" && !arg.starts_with('+'), "{}", arg);
            }
            prop_assert_eq!(args[0].as_str(), "push");
            prop_assert!(args[3].starts_with("refs/heads/"));
            prop_assert!(!args[3].starts_with('+'));
            let fetch = fetch_args(&remote);
            prop_assert!(!fetch.iter().any(|a| a.starts_with("--force") || a == "-f"));
        }
    }
}
