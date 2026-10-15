//! K04: a fake approval window that reproduces the verified Desktop 2.17.0 card with real Win32 controls, so the real UI Automation
//! binding is exercised end to end and deterministically. Each card sits in its own container window (like the DOM card), with a
//! title, the working directory, the command split into text fragments, the status text, five radio options (the first selected by
//! default, three of them persistent), a write-in box and `Skip` and `Submit` buttons. An unrelated `Approve` button sits beside
//! it. The window records which options were selected when `Submit` was pressed, so a test can prove no persistent option was.
//!
//! It deviates from the ticket's wording in one respect: the page is native Win32 rather than WebView2, because native controls
//! expose the same roles and patterns to UI Automation without a browser runtime, which keeps the harness deterministic.

#![allow(unsafe_code)]

use std::ffi::c_void;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BS_AUTORADIOBUTTON, BS_PUSHBUTTON, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, ES_AUTOHSCROLL, GetMessageW, GetParent, HMENU, KillTimer, MSG, PostMessageW,
    RegisterClassW, SW_MINIMIZE, SendMessageW, SetTimer, ShowWindow, TranslateMessage,
    WINDOW_EX_STYLE, WM_APP, WM_COMMAND, WM_TIMER, WNDCLASSW, WS_CHILD, WS_GROUP,
    WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

pub const OPTIONS: [&str; 5] = [
    "Yes, allow this time",
    "Yes, always allow in this conversation",
    "Yes, always allow in this project",
    "Yes, always allow",
    "No (tell the agent what to do instead)",
];

/// One card to show.
#[derive(Clone)]
pub struct CardSpec {
    pub fragments: Vec<String>,
    pub dir: String,
}

impl CardSpec {
    pub fn git_status() -> Self {
        Self {
            fragments: vec!["git".into(), " status ".into(), "--short".into()],
            dir: r"C:\vela-probe\wt1".into(),
        }
    }
}

/// How the window behaves.
#[derive(Clone)]
pub struct Scenario {
    pub title: String,
    pub cards: Vec<CardSpec>,
    /// Controls appear this long after the window (lazy tree population).
    pub delay_ms: u32,
    pub minimized: bool,
    /// An unrelated `Approve` button next to the card(s).
    pub approve_button: bool,
}

impl Scenario {
    pub fn one_card() -> Self {
        Self {
            title: "wt1 - vela-probe - Antigravity (fake)".into(),
            cards: vec![CardSpec::git_status()],
            delay_ms: 0,
            minimized: false,
            approve_button: true,
        }
    }
}

/// What the test can observe about the window.
#[derive(Default)]
pub struct Record {
    /// The options checked at each `Submit`, in order.
    pub submits: Vec<Vec<String>>,
}

struct Shared {
    scenario: Scenario,
    record: Mutex<Record>,
    // Window handles as integers (HWND is not Send).
    cards: Mutex<Vec<isize>>,
}

static ACTIVE: Mutex<Option<Arc<Shared>>> = Mutex::new(None);

const ID_RADIO: i32 = 100;
const ID_SUBMIT: i32 = 201;
const TIMER_POPULATE: usize = 1;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn hinst() -> HINSTANCE {
    // SAFETY: the module handle of this process.
    HINSTANCE(unsafe { GetModuleHandleW(None) }.expect("module").0)
}

fn child(
    parent: HWND,
    class: PCWSTR,
    text: &str,
    style: u32,
    id: i32,
    y: i32,
    height: i32,
) -> HWND {
    let t = wide(text);
    // SAFETY: creating a child control of a window this thread owns.
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            PCWSTR(t.as_ptr()),
            windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE(style) | WS_CHILD | WS_VISIBLE,
            8,
            y,
            560,
            height,
            Some(parent),
            Some(HMENU(id as usize as *mut c_void)),
            Some(hinst()),
            None,
        )
    }
    .expect("child control")
}

fn build_card(host: HWND, spec: &CardSpec, top: i32) -> HWND {
    let container = {
        let t = wide("");
        // SAFETY: a container child window of our own class.
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("VelaFakeApproval"),
                PCWSTR(t.as_ptr()),
                WS_CHILD | WS_VISIBLE,
                4,
                top,
                600,
                330,
                Some(host),
                None,
                Some(hinst()),
                None,
            )
        }
        .expect("card container")
    };
    let mut y = 4;
    let mut add = |class: PCWSTR, text: &str, style: u32, id: i32| {
        let h = child(container, class, text, style, id, y, 20);
        y += 24;
        h
    };
    add(w!("STATIC"), "Allow checking git status?", 0, 0);
    add(w!("STATIC"), &spec.dir, 0, 0);
    for f in &spec.fragments {
        add(w!("STATIC"), f, 0, 0);
    }
    add(w!("STATIC"), "Waiting for user input", 0, 0);
    add(
        w!("EDIT"),
        "Edit permission target",
        ES_AUTOHSCROLL as u32 | WS_TABSTOP.0,
        0,
    );
    for (i, o) in OPTIONS.iter().enumerate() {
        let group = if i == 0 { WS_GROUP.0 } else { 0 };
        let h = add(
            w!("BUTTON"),
            o,
            BS_AUTORADIOBUTTON as u32 | WS_TABSTOP.0 | group,
            ID_RADIO + i as i32,
        );
        if i == 0 {
            // Option 1 is selected by default on the real card.
            // SAFETY: checking our own radio button.
            unsafe {
                SendMessageW(h, 0x00F1 /* BM_SETCHECK */, Some(WPARAM(1)), None)
            };
        }
    }
    add(
        w!("BUTTON"),
        "Skip",
        BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
        202,
    );
    add(
        w!("BUTTON"),
        "Submit",
        BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
        ID_SUBMIT,
    );
    container
}

fn populate(hwnd: HWND, shared: &Shared) {
    let mut top = 4;
    let mut cards = shared.cards.lock().unwrap();
    for spec in &shared.scenario.cards {
        cards.push(build_card(hwnd, spec, top).0 as isize);
        top += 340;
    }
    if shared.scenario.approve_button {
        child(
            hwnd,
            w!("BUTTON"),
            "Approve",
            BS_PUSHBUTTON as u32 | WS_TABSTOP.0,
            300,
            top,
            30,
        );
    }
}

fn checked_labels(container: HWND) -> Vec<String> {
    let mut out = Vec::new();
    for (i, o) in OPTIONS.iter().enumerate() {
        // SAFETY: reading a radio button of the card.
        let h = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetDlgItem(
                Some(container),
                ID_RADIO + i as i32,
            )
        };
        if let Ok(h) = h {
            // SAFETY: querying the button's check state.
            let state = unsafe { SendMessageW(h, BM_GETCHECK, None, None) };
            if state.0 == 1 {
                out.push((*o).to_owned());
            }
        }
    }
    out
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let active = ACTIVE.lock().unwrap().clone();
    // SAFETY: standard window procedure; handles are the ones Windows passes in.
    unsafe {
        let parent = GetParent(hwnd).ok();
        match msg {
            WM_TIMER if wparam.0 == TIMER_POPULATE => {
                let _ = KillTimer(Some(hwnd), TIMER_POPULATE);
                if let Some(s) = &active {
                    populate(hwnd, s);
                }
                LRESULT(0)
            }
            m if m == WM_APP => {
                if let Some(s) = &active {
                    populate(hwnd, s);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;
                if let Some(p) = parent {
                    // Inside a card: pass the click up to the top-level window with the card's handle.
                    return SendMessageW(
                        p,
                        WM_COMMAND,
                        Some(wparam),
                        Some(LPARAM(hwnd.0 as isize)),
                    );
                }
                if id == ID_SUBMIT
                    && let Some(s) = &active
                {
                    let card = HWND(lparam.0 as *mut c_void);
                    let labels = checked_labels(card);
                    s.record.lock().unwrap().submits.push(labels);
                    // Answering a card removes it.
                    let _ = DestroyWindow(card);
                    s.cards.lock().unwrap().retain(|c| *c != card.0 as isize);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// A running fake window. Dropping it closes it.
pub struct FakeWindow {
    pub pid: u32,
    shared: Arc<Shared>,
    hwnd: isize,
}

impl FakeWindow {
    pub fn start(scenario: Scenario) -> Self {
        let shared = Arc::new(Shared {
            scenario: scenario.clone(),
            record: Mutex::new(Record::default()),
            cards: Mutex::new(Vec::new()),
        });
        *ACTIVE.lock().unwrap() = Some(shared.clone());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            // SAFETY: a window and its message loop on this thread.
            unsafe {
                let wc = WNDCLASSW {
                    lpfnWndProc: Some(proc),
                    hInstance: hinst(),
                    lpszClassName: w!("VelaFakeApproval"),
                    hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as usize as *mut c_void),
                    ..Default::default()
                };
                // Registering twice (a second scenario in the same process) is harmless.
                RegisterClassW(&wc);
                let title = wide(&scenario.title);
                let hwnd = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("VelaFakeApproval"),
                    PCWSTR(title.as_ptr()),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    100,
                    100,
                    700,
                    scenario.cards.len().max(1) as i32 * 340 + 120,
                    None,
                    None,
                    Some(hinst()),
                    None,
                )
                .expect("fake window");
                if scenario.minimized {
                    let _ = ShowWindow(hwnd, SW_MINIMIZE);
                }
                if scenario.delay_ms == 0 {
                    let _ = PostMessageW(Some(hwnd), WM_APP, WPARAM(0), LPARAM(0));
                } else {
                    SetTimer(Some(hwnd), TIMER_POPULATE, scenario.delay_ms, None);
                }
                tx.send(hwnd.0 as isize).unwrap();
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        });
        let hwnd = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("window started");
        // Give the window a moment to paint so UI Automation sees it.
        std::thread::sleep(Duration::from_millis(300));
        Self {
            pid: std::process::id(),
            shared,
            hwnd,
        }
    }

    /// The option sets that were checked at each `Submit`.
    pub fn submits(&self) -> Vec<Vec<String>> {
        self.shared.record.lock().unwrap().submits.clone()
    }

    /// How many cards are still showing.
    pub fn cards_left(&self) -> usize {
        self.shared.cards.lock().unwrap().len()
    }
}

impl Drop for FakeWindow {
    fn drop(&mut self) {
        // SAFETY: closing the window this harness created.
        unsafe {
            let _ = PostMessageW(
                Some(HWND(self.hwnd as *mut c_void)),
                windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                WPARAM(0),
                LPARAM(0),
            );
            let _ = PostMessageW(
                Some(HWND(self.hwnd as *mut c_void)),
                windows::Win32::UI::WindowsAndMessaging::WM_QUIT,
                WPARAM(0),
                LPARAM(0),
            );
        }
        *ACTIVE.lock().unwrap() = None;
    }
}
