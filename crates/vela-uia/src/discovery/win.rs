//! The Windows UI Automation binding behind [`Surface`] (K05, S-UIA-BINDINGS). Each call attaches to COM on the calling thread and
//! talks to UI Automation directly; nothing here ever uses coordinates, key presses or the clipboard. Selection and invocation go
//! through `SelectionItemPattern` and `InvokePattern` only.
//!
//! A tree is read in control-view order and the same order resolves a path, so a [`NodePath`] read from [`Surface::tree`] finds the
//! same control in [`Surface::select`] and [`Surface::invoke`]; the caller re-reads and re-checks role and name before acting.

use std::ffi::c_void;

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationInvokePattern,
    IUIAutomationSelectionItemPattern, IUIAutomationWindowPattern, TreeScope_Children,
    UIA_ButtonControlTypeId, UIA_EditControlTypeId, UIA_InvokePatternId,
    UIA_RadioButtonControlTypeId, UIA_SelectionItemPatternId, UIA_TextControlTypeId,
    UIA_WindowControlTypeId, UIA_WindowPatternId, WindowVisualState_Minimized,
};

use vela_domain::errors::{ErrorCode, VelaError};

use super::model::{NodePath, Role, Surface, UiNode, WindowInfo};

/// The tree is cut off here: a Chromium window has hundreds of nodes, never this many that matter.
const MAX_NODES: usize = 8_000;
const MAX_DEPTH: usize = 80;

/// The real UI Automation surface.
#[derive(Debug, Default, Clone, Copy)]
pub struct WinSurface;

fn fail(what: &str, e: windows::core::Error) -> VelaError {
    VelaError::from_code(
        ErrorCode::ApprovalSurfaceUnavailable,
        format!("UI Automation: {what}: {e}"),
    )
}

fn automation() -> Result<IUIAutomation, VelaError> {
    // S_FALSE (already initialised) and a different apartment model on this thread are both fine for our use.
    // SAFETY: plain COM initialisation with no reserved pointer.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    // SAFETY: creating the system's UI Automation object.
    unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }
        .map_err(|e| fail("could not start", e))
}

fn hwnd(handle: u64) -> HWND {
    HWND(handle as usize as *mut c_void)
}

fn role_of(el: &IUIAutomationElement) -> Role {
    // SAFETY: reading a property of a live element.
    let t = unsafe { el.CurrentControlType() }.unwrap_or_default();
    if t == UIA_TextControlTypeId {
        Role::Text
    } else if t == UIA_RadioButtonControlTypeId {
        Role::Radio
    } else if t == UIA_ButtonControlTypeId {
        Role::Button
    } else if t == UIA_EditControlTypeId {
        Role::Edit
    } else if t == UIA_WindowControlTypeId {
        Role::Window
    } else {
        Role::Other
    }
}

fn node_of(el: &IUIAutomationElement) -> UiNode {
    let role = role_of(el);
    // SAFETY: property reads and pattern lookups on a live element.
    let name = unsafe { el.CurrentName() }
        .map(|b| b.to_string())
        .unwrap_or_default();
    let enabled = unsafe { el.CurrentIsEnabled() }
        .map(|b| b.as_bool())
        .unwrap_or(true);
    let mut node = UiNode::new(role, name);
    node.enabled = enabled;
    if matches!(role, Role::Radio | Role::Button | Role::Other) {
        // SAFETY: asking for a pattern; an element without it returns an error.
        if let Ok(p) = unsafe {
            el.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId)
        } {
            node.can_select = true;
            node.selected = unsafe { p.CurrentIsSelected() }
                .map(|b| b.as_bool())
                .unwrap_or(false);
        }
        if unsafe { el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) }
            .is_ok()
        {
            node.can_invoke = true;
        }
    }
    node
}

fn children_of(
    uia: &IUIAutomation,
    el: &IUIAutomationElement,
) -> Result<Vec<IUIAutomationElement>, VelaError> {
    // SAFETY: walking the control view.
    let walker = unsafe { uia.ControlViewWalker() }.map_err(|e| fail("no tree walker", e))?;
    let mut out = Vec::new();
    let mut next = unsafe { walker.GetFirstChildElement(el) }.ok();
    while let Some(child) = next {
        next = unsafe { walker.GetNextSiblingElement(&child) }.ok();
        out.push(child);
        if out.len() > MAX_NODES {
            break;
        }
    }
    Ok(out)
}

fn build(
    uia: &IUIAutomation,
    el: &IUIAutomationElement,
    depth: usize,
    budget: &mut usize,
) -> Result<UiNode, VelaError> {
    *budget = budget.saturating_sub(1);
    let mut node = node_of(el);
    if depth < MAX_DEPTH && *budget > 0 {
        for child in children_of(uia, el)? {
            if *budget == 0 {
                break;
            }
            node.children.push(build(uia, &child, depth + 1, budget)?);
        }
    }
    Ok(node)
}

fn element_at(
    uia: &IUIAutomation,
    window: &WindowInfo,
    path: &NodePath,
) -> Result<IUIAutomationElement, VelaError> {
    // SAFETY: resolving a window handle to its element.
    let mut el = unsafe { uia.ElementFromHandle(hwnd(window.handle)) }
        .map_err(|e| fail("the window is gone", e))?;
    for index in path {
        let kids = children_of(uia, &el)?;
        el = kids.into_iter().nth(*index).ok_or_else(|| {
            VelaError::from_code(
                ErrorCode::ApprovalSurfaceNotFound,
                "the control is no longer where it was read",
            )
        })?;
    }
    Ok(el)
}

impl Surface for WinSurface {
    fn windows(&self) -> Result<Vec<WindowInfo>, VelaError> {
        let uia = automation()?;
        // SAFETY: enumerating the desktop's top-level windows.
        let root = unsafe { uia.GetRootElement() }.map_err(|e| fail("no desktop", e))?;
        let all = unsafe { uia.CreateTrueCondition() }.map_err(|e| fail("no condition", e))?;
        let found = unsafe { root.FindAll(TreeScope_Children, &all) }
            .map_err(|e| fail("could not list windows", e))?;
        let n = unsafe { found.Length() }.unwrap_or(0);
        let mut out = Vec::new();
        for i in 0..n {
            let Ok(el) = (unsafe { found.GetElement(i) }) else {
                continue;
            };
            let handle = unsafe { el.CurrentNativeWindowHandle() }
                .map(|h| h.0 as usize as u64)
                .unwrap_or(0);
            if handle == 0 {
                continue;
            }
            let minimized = unsafe {
                el.GetCurrentPatternAs::<IUIAutomationWindowPattern>(UIA_WindowPatternId)
            }
            .ok()
            .and_then(|p| unsafe { p.CurrentWindowVisualState() }.ok())
            .is_some_and(|s| s == WindowVisualState_Minimized);
            let offscreen = unsafe { el.CurrentIsOffscreen() }
                .map(|b| b.as_bool())
                .unwrap_or(false);
            out.push(WindowInfo {
                handle,
                pid: u32::try_from(unsafe { el.CurrentProcessId() }.unwrap_or(0)).unwrap_or(0),
                title: unsafe { el.CurrentName() }
                    .map(|b| b.to_string())
                    .unwrap_or_default(),
                visible: !offscreen && !minimized,
                minimized,
            });
        }
        Ok(out)
    }

    fn tree(&self, window: &WindowInfo) -> Result<UiNode, VelaError> {
        let uia = automation()?;
        let root = element_at(&uia, window, &Vec::new())?;
        let mut budget = MAX_NODES;
        build(&uia, &root, 0, &mut budget)
    }

    fn select(&self, window: &WindowInfo, path: &NodePath) -> Result<(), VelaError> {
        let uia = automation()?;
        let el = element_at(&uia, window, path)?;
        // SAFETY: SelectionItem.Select on the control found by path.
        let p = unsafe {
            el.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId)
        }
        .map_err(|_| {
            VelaError::from_code(
                ErrorCode::ApprovalDeliveryFailed,
                "the option does not support selection",
            )
        })?;
        unsafe { p.Select() }.map_err(|e| {
            VelaError::from_code(
                ErrorCode::ApprovalDeliveryFailed,
                format!("selecting failed: {e}"),
            )
        })
    }

    fn invoke(&self, window: &WindowInfo, path: &NodePath) -> Result<(), VelaError> {
        let uia = automation()?;
        let el = element_at(&uia, window, path)?;
        // SAFETY: Invoke on the control found by path.
        let p =
            unsafe { el.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) }
                .map_err(|_| {
                    VelaError::from_code(
                        ErrorCode::ApprovalDeliveryFailed,
                        "the button does not support invoking",
                    )
                })?;
        unsafe { p.Invoke() }.map_err(|e| {
            VelaError::from_code(
                ErrorCode::ApprovalDeliveryFailed,
                format!("invoking failed: {e}"),
            )
        })
    }
}
