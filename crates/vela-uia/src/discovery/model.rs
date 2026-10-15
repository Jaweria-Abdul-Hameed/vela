//! The abstract accessibility tree Vela reasons about. It carries only what the verified card exposes: a role, a name, whether the
//! control can be selected or invoked, and whether it is selected. Identifiers are deliberately absent: they contain a
//! session-unique part, so controls are matched by role and label, never by id (ADR-018).

use vela_domain::errors::VelaError;

/// The kind of control, as far as Vela cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A top-level window or the root of one.
    Window,
    /// A text run.
    Text,
    /// A radio option (`SelectionItem`).
    Radio,
    /// A push button (`Invoke`).
    Button,
    /// An edit box.
    Edit,
    /// Anything else.
    Other,
}

/// Where a node is: child indices from the window root.
pub type NodePath = Vec<usize>;

/// One control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiNode {
    /// The kind.
    pub role: Role,
    /// The accessible name (the visible label or text).
    pub name: String,
    /// Whether it is enabled.
    pub enabled: bool,
    /// Whether it supports the `SelectionItem` pattern.
    pub can_select: bool,
    /// Whether it supports the `Invoke` pattern.
    pub can_invoke: bool,
    /// Whether it is currently selected.
    pub selected: bool,
    /// Its children, in order.
    pub children: Vec<UiNode>,
}

impl UiNode {
    /// A node with nothing special about it.
    pub fn new(role: Role, name: impl Into<String>) -> Self {
        Self {
            role,
            name: name.into(),
            enabled: true,
            can_select: false,
            can_invoke: false,
            selected: false,
            children: Vec::new(),
        }
    }

    /// A radio option.
    pub fn radio(name: impl Into<String>, selected: bool) -> Self {
        Self {
            can_select: true,
            can_invoke: true,
            selected,
            ..Self::new(Role::Radio, name)
        }
    }

    /// A push button.
    pub fn button(name: impl Into<String>) -> Self {
        Self {
            can_invoke: true,
            ..Self::new(Role::Button, name)
        }
    }

    /// Adds children.
    pub fn with(mut self, children: Vec<UiNode>) -> Self {
        self.children = children;
        self
    }

    /// Nodes in this subtree, itself included.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(UiNode::count).sum::<usize>()
    }

    /// The node at `path`.
    pub fn at(&self, path: &[usize]) -> Option<&UiNode> {
        let mut n = self;
        for i in path {
            n = n.children.get(*i)?;
        }
        Some(n)
    }

    /// Every node with its path, depth first, in reading order.
    pub fn walk(&self) -> Vec<(NodePath, &UiNode)> {
        fn go<'a>(n: &'a UiNode, path: &mut NodePath, out: &mut Vec<(NodePath, &'a UiNode)>) {
            out.push((path.clone(), n));
            for (i, c) in n.children.iter().enumerate() {
                path.push(i);
                go(c, path, out);
                path.pop();
            }
        }
        let mut out = Vec::new();
        go(self, &mut Vec::new(), &mut out);
        out
    }
}

/// A top-level window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    /// An opaque handle the surface understands.
    pub handle: u64,
    /// The owning process.
    pub pid: u32,
    /// The window title.
    pub title: String,
    /// Whether it is on screen.
    pub visible: bool,
    /// Whether it is minimized.
    pub minimized: bool,
}

/// Something the accessibility tree can be read from and acted on through. Acting is only ever selection and invocation of a
/// control found by path; there are no coordinates and no keystrokes.
pub trait Surface: Send + Sync {
    /// Top-level windows.
    fn windows(&self) -> Result<Vec<WindowInfo>, VelaError>;
    /// The tree of a window.
    fn tree(&self, window: &WindowInfo) -> Result<UiNode, VelaError>;
    /// `SelectionItemPattern.Select` on the node at `path`.
    fn select(&self, window: &WindowInfo, path: &NodePath) -> Result<(), VelaError>;
    /// `InvokePattern.Invoke` on the node at `path`.
    fn invoke(&self, window: &WindowInfo, path: &NodePath) -> Result<(), VelaError>;
}
