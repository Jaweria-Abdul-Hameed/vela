use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::graph::DependencyGraph;
use crate::ids::TicketId;

/// The evidence label shown for a pair (never a numeric probability).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLabel {
    /// Disjoint modules and no hazards.
    SafeByExplicitSeparation,
    /// Same module but different files and no hazards.
    SafeWithCaution,
    /// One ticket (transitively) blocks the other.
    SerializedByDependency,
    /// A shared file, contract, schema, generated artifact, lockfile, build config, resource key or policy area.
    SerializedByContract,
    /// Missing or unreliable evidence: run sequentially.
    UnknownSequential,
}

impl EvidenceLabel {
    /// True only for the two safe labels.
    pub fn is_parallel_safe(self) -> bool {
        matches!(self, Self::SafeByExplicitSeparation | Self::SafeWithCaution)
    }
}

/// Why a pair got its label (inspectable evidence).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// Dependency path between the tickets.
    Dependency,
    /// Both write the same file.
    SharedFile(String),
    /// One renames or moves a path the other touches.
    Rename(String),
    /// Both touch schema or migrations.
    Schema,
    /// Both touch a generated artifact or its source of truth.
    GeneratedArtifact,
    /// Both touch a lockfile.
    Lockfile,
    /// Both touch shared build or configuration files.
    BuildConfig,
    /// One changes a contract the other changes or consumes.
    Contract(String),
    /// Both claim the same exclusive resource key.
    ResourceKey(String),
    /// Both touch an area the repository policy serializes.
    PolicyArea(String),
    /// The analyst reported a hazard (adds only).
    Analyst(String),
    /// No predicted write set for this ticket.
    NoPrediction(TicketId),
    /// Both write inside the same directory.
    SameDirectory(String),
}

/// What a ticket is predicted to change. Predictions are hints, never permissions.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PredictedWriteSet {
    /// Exact files, repository-relative with forward slashes.
    pub files: BTreeSet<String>,
    /// Directories or modules (prefix match).
    pub directories: BTreeSet<String>,
    /// Paths renamed or moved.
    pub renames: BTreeSet<String>,
    /// Contracts (API, event or type names) this ticket changes.
    pub contracts_changed: BTreeSet<String>,
    /// Contracts this ticket consumes.
    pub contracts_consumed: BTreeSet<String>,
    /// Exclusive local resources claimed.
    pub resource_keys: BTreeSet<String>,
}

/// A hazard the dependency analyst found. It can only serialize a pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalystHazard {
    /// One ticket of the pair.
    pub a: TicketId,
    /// The other.
    pub b: TicketId,
    /// Short reason, stored as evidence.
    pub note: String,
}

/// Repository policy: areas that are always serialized.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SurfaceRules {
    /// Path prefixes the repository policy marks serialized.
    pub serialized_areas: BTreeSet<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Surface {
    Schema,
    Generated,
    Lockfile,
    BuildConfig,
}

fn classify_path(path: &str) -> Option<Surface> {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    if matches!(
        name,
        "cargo.lock"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "poetry.lock"
            | "go.sum"
    ) {
        return Some(Surface::Lockfile);
    }
    if lower
        .split('/')
        .any(|s| s == "migrations" || s == "migration")
        || name.ends_with(".sql")
    {
        return Some(Surface::Schema);
    }
    if lower
        .split('/')
        .any(|s| s == "generated" || s == "__generated__")
    {
        return Some(Surface::Generated);
    }
    if matches!(
        name,
        "cargo.toml"
            | "package.json"
            | "rust-toolchain.toml"
            | "rustfmt.toml"
            | "build.rs"
            | "tauri.conf.json"
    ) || name.starts_with("tsconfig")
        || lower.starts_with(".github/")
    {
        return Some(Surface::BuildConfig);
    }
    None
}

fn surfaces(set: &PredictedWriteSet) -> BTreeSet<Surface> {
    set.files
        .iter()
        .chain(set.directories.iter())
        .filter_map(|p| classify_path(p))
        .collect()
}

fn overlaps(a: &str, b: &str) -> bool {
    let a = a.trim_end_matches('/');
    let b = b.trim_end_matches('/');
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}

/// The verdict for one pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairVerdict {
    /// First ticket (smaller id).
    pub a: TicketId,
    /// Second ticket.
    pub b: TicketId,
    /// The label.
    pub label: EvidenceLabel,
    /// Every reason found, for the evidence view.
    pub reasons: Vec<Reason>,
}

fn reaches(graph: &DependencyGraph, from: &TicketId, to: &TicketId) -> bool {
    let mut seen = BTreeSet::new();
    let mut stack = vec![from.clone()];
    while let Some(t) = stack.pop() {
        if &t == to {
            return true;
        }
        if !seen.insert(t.clone()) {
            continue;
        }
        if let Ok(next) = graph.dependents_of(&t) {
            stack.extend(next);
        }
    }
    false
}

/// One side of a pair: the ticket and its prediction, if any.
pub type Side<'a> = (&'a TicketId, Option<&'a PredictedWriteSet>);

/// Classifies one pair. Uncertain input gives [`EvidenceLabel::UnknownSequential`].
pub fn classify_pair(
    graph: &DependencyGraph,
    a: Side<'_>,
    b: Side<'_>,
    analyst: &[AnalystHazard],
    rules: &SurfaceRules,
) -> PairVerdict {
    let (x, y) = if a.0 <= b.0 { (a, b) } else { (b, a) };
    let verdict = |label, reasons| PairVerdict {
        a: x.0.clone(),
        b: y.0.clone(),
        label,
        reasons,
    };
    let mut reasons = Vec::new();

    if reaches(graph, x.0, y.0) || reaches(graph, y.0, x.0) {
        return verdict(
            EvidenceLabel::SerializedByDependency,
            vec![Reason::Dependency],
        );
    }
    for h in analyst {
        if (&h.a == x.0 && &h.b == y.0) || (&h.a == y.0 && &h.b == x.0) {
            reasons.push(Reason::Analyst(h.note.clone()));
        }
    }
    let (Some(sa), Some(sb)) = (x.1, y.1) else {
        for (t, s) in [x, y] {
            if s.is_none() {
                reasons.push(Reason::NoPrediction(t.clone()));
            }
        }
        return verdict(EvidenceLabel::UnknownSequential, reasons);
    };

    let mut hard = false;
    for f in sa.files.intersection(&sb.files) {
        reasons.push(Reason::SharedFile(f.clone()));
        hard = true;
    }
    for (renames, other) in [(&sa.renames, sb), (&sb.renames, sa)] {
        for r in renames {
            if other
                .files
                .iter()
                .chain(other.directories.iter())
                .any(|p| overlaps(r, p))
            {
                reasons.push(Reason::Rename(r.clone()));
                hard = true;
            }
        }
    }
    for s in surfaces(sa).intersection(&surfaces(sb)) {
        reasons.push(match s {
            Surface::Schema => Reason::Schema,
            Surface::Generated => Reason::GeneratedArtifact,
            Surface::Lockfile => Reason::Lockfile,
            Surface::BuildConfig => Reason::BuildConfig,
        });
        hard = true;
    }
    for (changed, other) in [(&sa.contracts_changed, sb), (&sb.contracts_changed, sa)] {
        for c in changed {
            if other.contracts_changed.contains(c) || other.contracts_consumed.contains(c) {
                reasons.push(Reason::Contract(c.clone()));
                hard = true;
            }
        }
    }
    for k in sa.resource_keys.intersection(&sb.resource_keys) {
        reasons.push(Reason::ResourceKey(k.clone()));
        hard = true;
    }
    for area in &rules.serialized_areas {
        let hits = |s: &PredictedWriteSet| {
            s.files
                .iter()
                .chain(s.directories.iter())
                .any(|p| overlaps(p, area))
        };
        if hits(sa) && hits(sb) {
            reasons.push(Reason::PolicyArea(area.clone()));
            hard = true;
        }
    }
    if hard || reasons.iter().any(|r| matches!(r, Reason::Analyst(_))) {
        reasons.sort_by_key(|r| format!("{r:?}"));
        reasons.dedup();
        return verdict(EvidenceLabel::SerializedByContract, reasons);
    }
    let all_a: Vec<&String> = sa.directories.iter().chain(sa.files.iter()).collect();
    let all_b: Vec<&String> = sb.directories.iter().chain(sb.files.iter()).collect();
    for pa in &all_a {
        for pb in &all_b {
            let dir_involved = sa.directories.contains(*pa) || sb.directories.contains(*pb);
            if dir_involved && overlaps(pa, pb) {
                reasons.push(Reason::SameDirectory((*pa).clone()));
            }
        }
    }
    reasons.sort_by_key(|r| format!("{r:?}"));
    reasons.dedup();
    let label = if reasons.is_empty() {
        EvidenceLabel::SafeByExplicitSeparation
    } else {
        EvidenceLabel::SafeWithCaution
    };
    verdict(label, reasons)
}

/// Picks tickets from the ready `frontier` (in order) that are pairwise safe, up to `capacity`. A ticket that is unsafe with
/// any already chosen one waits for a later round; the first ticket is always chosen so progress is never blocked.
pub fn select_parallel(
    graph: &DependencyGraph,
    frontier: &[TicketId],
    sets: &BTreeMap<TicketId, PredictedWriteSet>,
    analyst: &[AnalystHazard],
    rules: &SurfaceRules,
    capacity: usize,
) -> Vec<TicketId> {
    let mut chosen: Vec<TicketId> = Vec::new();
    for t in frontier {
        if chosen.len() >= capacity {
            break;
        }
        let ok = chosen.iter().all(|c| {
            classify_pair(graph, (c, sets.get(c)), (t, sets.get(t)), analyst, rules)
                .label
                .is_parallel_safe()
        });
        if ok || chosen.is_empty() {
            chosen.push(t.clone());
        }
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{DependencyEdge, EdgeOrigin};
    use proptest::prelude::*;

    fn id(s: &str) -> TicketId {
        TicketId::new(s).unwrap()
    }
    fn set(files: &[&str]) -> PredictedWriteSet {
        PredictedWriteSet {
            files: files.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }
    fn label(a: &PredictedWriteSet, b: &PredictedWriteSet) -> EvidenceLabel {
        let g = DependencyGraph::new();
        classify_pair(
            &g,
            (&id("A"), Some(a)),
            (&id("B"), Some(b)),
            &[],
            &SurfaceRules::default(),
        )
        .label
    }

    #[test]
    fn disjoint_is_safe() {
        assert_eq!(
            label(&set(&["crates/a/src/x.rs"]), &set(&["apps/b/src/y.ts"])),
            EvidenceLabel::SafeByExplicitSeparation
        );
    }

    #[test]
    fn same_dir_different_files_is_caution() {
        let mut a = set(&["crates/a/src/x.rs"]);
        a.directories.insert("crates/a/src".into());
        assert_eq!(
            label(&a, &set(&["crates/a/src/y.rs"])),
            EvidenceLabel::SafeWithCaution
        );
    }

    #[test]
    fn hard_blockers_serialize() {
        for (a, b) in [
            (
                "crates/x/migrations/0001.sql",
                "crates/y/migrations/0002.sql",
            ),
            ("Cargo.lock", "Cargo.lock"),
            ("pkg/generated/a.ts", "pkg/generated/b.ts"),
            ("apps/a/Cargo.toml", "apps/b/Cargo.toml"),
            ("src/same.rs", "src/same.rs"),
        ] {
            assert_eq!(
                label(&set(&[a]), &set(&[b])),
                EvidenceLabel::SerializedByContract,
                "{a} {b}"
            );
        }
    }

    #[test]
    fn contract_resource_and_rename() {
        let mut a = set(&["a.rs"]);
        let mut b = set(&["b.rs"]);
        a.contracts_changed.insert("EventEnvelope".into());
        b.contracts_consumed.insert("EventEnvelope".into());
        assert_eq!(label(&a, &b), EvidenceLabel::SerializedByContract);
        let (mut c, mut d) = (set(&["c.rs"]), set(&["d.rs"]));
        c.resource_keys.insert("RK-X".into());
        d.resource_keys.insert("RK-X".into());
        assert_eq!(label(&c, &d), EvidenceLabel::SerializedByContract);
        let mut e = set(&["e.rs"]);
        e.renames.insert("src/old".into());
        assert_eq!(
            label(&e, &set(&["src/old/f.rs"])),
            EvidenceLabel::SerializedByContract
        );
    }

    #[test]
    fn policy_area_serializes() {
        let g = DependencyGraph::new();
        let mut r = SurfaceRules::default();
        r.serialized_areas.insert("infra".into());
        let (a, b) = (set(&["infra/a.tf"]), set(&["infra/b.tf"]));
        let v = classify_pair(&g, (&id("A"), Some(&a)), (&id("B"), Some(&b)), &[], &r);
        assert_eq!(v.label, EvidenceLabel::SerializedByContract);
    }

    #[test]
    fn dependency_and_unknown() {
        let mut g = DependencyGraph::new();
        for t in ["A", "B", "C"] {
            g.add_ticket(id(t)).unwrap();
        }
        g.add_edge(&DependencyEdge::new(id("A"), id("B"), EdgeOrigin::Authored).unwrap())
            .unwrap();
        g.add_edge(&DependencyEdge::new(id("B"), id("C"), EdgeOrigin::Authored).unwrap())
            .unwrap();
        let s = set(&["x"]);
        let r = SurfaceRules::default();
        assert_eq!(
            classify_pair(&g, (&id("A"), Some(&s)), (&id("C"), Some(&s)), &[], &r).label,
            EvidenceLabel::SerializedByDependency
        );
        assert_eq!(
            classify_pair(&g, (&id("A"), None), (&id("B"), Some(&s)), &[], &r).label,
            EvidenceLabel::SerializedByDependency
        );
        let mut g2 = DependencyGraph::new();
        g2.add_ticket(id("A")).unwrap();
        g2.add_ticket(id("B")).unwrap();
        assert_eq!(
            classify_pair(&g2, (&id("A"), None), (&id("B"), Some(&s)), &[], &r).label,
            EvidenceLabel::UnknownSequential
        );
    }

    #[test]
    fn select_respects_pairs_and_capacity() {
        let g = DependencyGraph::new();
        let mut sets = BTreeMap::new();
        sets.insert(id("A"), set(&["a/x.rs"]));
        sets.insert(id("B"), set(&["a/x.rs"]));
        sets.insert(id("C"), set(&["c/y.rs"]));
        let f = [id("A"), id("B"), id("C")];
        let r = SurfaceRules::default();
        assert_eq!(
            select_parallel(&g, &f, &sets, &[], &r, 3),
            vec![id("A"), id("C")]
        );
        assert_eq!(select_parallel(&g, &f, &sets, &[], &r, 1), vec![id("A")]);
    }

    proptest! {
        #[test]
        fn analyst_never_unserializes(
            files_a in prop::collection::btree_set("[a-c]/[x-z]\\.rs", 0..3),
            files_b in prop::collection::btree_set("[a-c]/[x-z]\\.rs", 0..3),
        ) {
            let a = PredictedWriteSet { files: files_a, ..Default::default() };
            let b = PredictedWriteSet { files: files_b, ..Default::default() };
            let g = DependencyGraph::new();
            let r = SurfaceRules::default();
            let hz = vec![AnalystHazard { a: id("A"), b: id("B"), note: "n".into() }];
            let base = classify_pair(&g, (&id("A"), Some(&a)), (&id("B"), Some(&b)), &[], &r).label;
            let with = classify_pair(&g, (&id("A"), Some(&a)), (&id("B"), Some(&b)), &hz, &r).label;
            if !base.is_parallel_safe() {
                prop_assert!(!with.is_parallel_safe());
            }
            prop_assert!(!with.is_parallel_safe());
        }
    }
}
