//! A fake `gh` (T02, shared with T03, T04 and S07): a [`ProcessRunner`] that answers the `gh` subcommands Vela uses from an
//! in-memory repository, records every invocation, and can model "gh missing", "not logged in" and outages. It is a fake at
//! the process port, so adapter code runs unchanged against it and no real GitHub account or token is ever involved.

use std::sync::Mutex;

use vela_domain::errors::{ErrorClass, VelaError};
use vela_domain::ports::{
    CommandSpec, OutputChunk, OutputSink, OutputStream, PortFuture, ProcessOutcome, ProcessRunner,
    ready,
};

/// One issue in the fake repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeIssue {
    /// Issue number.
    pub number: u32,
    /// Title.
    pub title: String,
    /// Body.
    pub body: String,
    /// Label names.
    pub labels: Vec<String>,
    /// Comment bodies, in order.
    pub comments: Vec<String>,
    /// Numbers of issues that block this one (native dependencies).
    pub blocked_by: Vec<u32>,
    /// True once closed.
    pub closed: bool,
}

impl FakeIssue {
    /// A new open issue.
    pub fn new(number: u32, title: &str, body: &str) -> Self {
        Self {
            number,
            title: title.to_owned(),
            body: body.to_owned(),
            labels: Vec::new(),
            comments: Vec::new(),
            blocked_by: Vec::new(),
            closed: false,
        }
    }
}

#[derive(Debug)]
struct State {
    installed: bool,
    authenticated: bool,
    outage: bool,
    issues: Vec<FakeIssue>,
    calls: Vec<Vec<String>>,
}

/// The fake `gh`.
#[derive(Debug)]
pub struct FakeGh {
    state: Mutex<State>,
}

impl Default for FakeGh {
    fn default() -> Self {
        Self::new()
    }
}

fn json_string(s: &str) -> String {
    serde_json::to_string(s).expect("string serializes")
}

fn issue_json(i: &FakeIssue, with_comments: bool) -> String {
    let labels: Vec<String> = i
        .labels
        .iter()
        .map(|l| format!("{{\"name\":{}}}", json_string(l)))
        .collect();
    let mut out = format!(
        "{{\"number\":{},\"title\":{},\"body\":{},\"labels\":[{}]",
        i.number,
        json_string(&i.title),
        json_string(&i.body),
        labels.join(",")
    );
    if with_comments {
        let comments: Vec<String> = i
            .comments
            .iter()
            .map(|c| format!("{{\"body\":{}}}", json_string(c)))
            .collect();
        out.push_str(&format!(",\"comments\":[{}]", comments.join(",")));
    }
    out.push('}');
    out
}

impl FakeGh {
    /// Installed, authenticated, empty.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                installed: true,
                authenticated: true,
                outage: false,
                issues: Vec::new(),
                calls: Vec::new(),
            }),
        }
    }

    /// Adds an issue.
    pub fn add_issue(&self, issue: FakeIssue) {
        self.state.lock().expect("gh lock").issues.push(issue);
    }

    /// Makes `gh` absent (spawning it fails with "not found").
    pub fn set_installed(&self, installed: bool) {
        self.state.lock().expect("gh lock").installed = installed;
    }

    /// Logs in or out.
    pub fn set_authenticated(&self, authenticated: bool) {
        self.state.lock().expect("gh lock").authenticated = authenticated;
    }

    /// While set, every network-backed command fails as if GitHub were unreachable.
    pub fn set_outage(&self, down: bool) {
        self.state.lock().expect("gh lock").outage = down;
    }

    /// A copy of an issue.
    pub fn issue(&self, number: u32) -> Option<FakeIssue> {
        self.state
            .lock()
            .expect("gh lock")
            .issues
            .iter()
            .find(|i| i.number == number)
            .cloned()
    }

    /// Every invocation's arguments, in order (without the program name).
    pub fn calls(&self) -> Vec<Vec<String>> {
        self.state.lock().expect("gh lock").calls.clone()
    }

    /// Number of write invocations (comment, edit, close).
    pub fn write_count(&self) -> usize {
        self.calls()
            .iter()
            .filter(|c| {
                c.first().map(String::as_str) == Some("issue")
                    && matches!(
                        c.get(1).map(String::as_str),
                        Some("comment" | "edit" | "close")
                    )
            })
            .count()
    }

    fn handle(&self, args: &[String]) -> Result<(i32, String, String), VelaError> {
        let mut s = self.state.lock().expect("gh lock");
        s.calls.push(args.to_vec());
        if !s.installed {
            return Err(VelaError::of_class(
                ErrorClass::UserActionRequired,
                "could not start `gh`: NotFound",
            ));
        }
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        if a.first() == Some(&"auth") && a.get(1) == Some(&"status") {
            return Ok(if s.authenticated {
                (0, "Logged in to github.com\n".into(), String::new())
            } else {
                (
                    1,
                    String::new(),
                    "You are not logged into any GitHub hosts. To log in, run: gh auth login\n"
                        .into(),
                )
            });
        }
        if !s.authenticated {
            return Ok((
                4,
                String::new(),
                "gh: To get started with GitHub CLI, please run:  gh auth login\n".into(),
            ));
        }
        if s.outage {
            return Ok((
                1,
                String::new(),
                "error connecting to api.github.com: network is unreachable\n".into(),
            ));
        }
        let number_arg = |i: usize| a.get(i).and_then(|n| n.parse::<u32>().ok());
        match (a.first().copied(), a.get(1).copied()) {
            (Some("issue"), Some("list")) => {
                let items: Vec<String> = s
                    .issues
                    .iter()
                    .filter(|i| !i.closed)
                    .map(|i| issue_json(i, false))
                    .collect();
                Ok((0, format!("[{}]", items.join(",")), String::new()))
            }
            (Some("issue"), Some("view")) => {
                let with_comments = a.iter().any(|x| x.contains("comments"));
                match number_arg(2).and_then(|n| s.issues.iter().find(|i| i.number == n)) {
                    Some(i) => Ok((0, issue_json(i, with_comments), String::new())),
                    None => Ok((
                        1,
                        String::new(),
                        "GraphQL: Could not resolve to an issue\n".into(),
                    )),
                }
            }
            (Some("api"), Some(path)) if path.ends_with("/dependencies/blocked_by") => {
                let n = path
                    .split('/')
                    .rev()
                    .nth(2)
                    .and_then(|n| n.parse::<u32>().ok());
                let blockers = n
                    .and_then(|n| s.issues.iter().find(|i| i.number == n))
                    .map(|i| i.blocked_by.clone())
                    .unwrap_or_default();
                let items: Vec<String> = blockers
                    .iter()
                    .map(|b| format!("{{\"number\":{b}}}"))
                    .collect();
                Ok((0, format!("[{}]", items.join(",")), String::new()))
            }
            (Some("issue"), Some(verb @ ("comment" | "edit" | "close"))) => {
                let Some(n) = number_arg(2) else {
                    return Ok((1, String::new(), "invalid issue number\n".into()));
                };
                let Some(issue) = s.issues.iter_mut().find(|i| i.number == n) else {
                    return Ok((
                        1,
                        String::new(),
                        "GraphQL: Could not resolve to an issue\n".into(),
                    ));
                };
                match verb {
                    "comment" => {
                        if let Some(body) = a.iter().find_map(|x| x.strip_prefix("--body=")) {
                            issue.comments.push(body.to_owned());
                        }
                    }
                    "edit" => {
                        for x in &a {
                            if let Some(l) = x.strip_prefix("--add-label=")
                                && !issue.labels.iter().any(|e| e == l)
                            {
                                issue.labels.push(l.to_owned());
                            }
                            if let Some(l) = x.strip_prefix("--remove-label=") {
                                issue.labels.retain(|e| e != l);
                            }
                        }
                    }
                    _ => issue.closed = true,
                }
                Ok((0, String::new(), String::new()))
            }
            _ => Ok((
                2,
                String::new(),
                format!("unknown command: gh {}\n", args.join(" ")),
            )),
        }
    }
}

impl ProcessRunner for FakeGh {
    fn run<'a>(
        &'a self,
        spec: &'a CommandSpec,
        sink: &'a dyn OutputSink,
    ) -> PortFuture<'a, ProcessOutcome> {
        if spec.program != "gh" {
            return ready(Err(VelaError::of_class(
                ErrorClass::UserActionRequired,
                format!("could not start `{}`: NotFound", spec.program),
            )));
        }
        match self.handle(&spec.args) {
            Ok((code, out, err)) => {
                if !out.is_empty() {
                    sink.on_output(OutputChunk {
                        stream: OutputStream::Stdout,
                        text: out,
                    });
                }
                if !err.is_empty() {
                    sink.on_output(OutputChunk {
                        stream: OutputStream::Stderr,
                        text: err,
                    });
                }
                ready(Ok(ProcessOutcome {
                    exit_code: Some(code),
                    timed_out: false,
                    cancelled: false,
                    duration_ms: 1,
                }))
            }
            Err(e) => ready(Err(e)),
        }
    }

    fn kill_tree<'a>(&'a self, _id: &'a str) -> PortFuture<'a, ()> {
        ready(Ok(()))
    }
}
