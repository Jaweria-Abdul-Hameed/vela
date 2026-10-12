//! Window placement safety (L04): a restored window must never end up off every monitor (a monitor was unplugged, the
//! resolution or scaling changed). The geometry rules are plain functions so they are tested without a window.

use tauri::{Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

/// A rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

/// The smallest overlap with a monitor (in each direction) that counts as reachable: enough to grab the title bar.
pub const MIN_VISIBLE: u32 = 120;

fn overlap(a: Rect, b: Rect) -> (u32, u32) {
    let left = a.x.max(b.x);
    let right = (a.x + a.width as i32).min(b.x + b.width as i32);
    let top = a.y.max(b.y);
    let bottom = (a.y + a.height as i32).min(b.y + b.height as i32);
    (
        u32::try_from((right - left).max(0)).unwrap_or(0),
        u32::try_from((bottom - top).max(0)).unwrap_or(0),
    )
}

/// True when the window is reachable on at least one monitor.
pub fn is_reachable(window: Rect, monitors: &[Rect]) -> bool {
    monitors.iter().any(|m| {
        let (w, h) = overlap(window, *m);
        w >= MIN_VISIBLE.min(window.width) && h >= MIN_VISIBLE.min(window.height)
    })
}

/// A placement that is reachable: the window unchanged when it already is, otherwise shrunk to fit the first monitor and
/// centred on it. With no monitors the window is returned unchanged.
pub fn clamp_to_monitors(window: Rect, monitors: &[Rect]) -> Rect {
    if monitors.is_empty() || is_reachable(window, monitors) {
        return window;
    }
    let m = monitors[0];
    let width = window.width.min(m.width);
    let height = window.height.min(m.height);
    Rect {
        x: m.x + ((m.width - width) / 2) as i32,
        y: m.y + ((m.height - height) / 2) as i32,
        width,
        height,
    }
}

/// Moves the window onto a monitor when its restored placement is unreachable.
pub fn ensure_visible<R: Runtime>(window: &WebviewWindow<R>) {
    let (Ok(position), Ok(size), Ok(monitors)) = (
        window.outer_position(),
        window.outer_size(),
        window.available_monitors(),
    ) else {
        return;
    };
    let monitors: Vec<Rect> = monitors
        .iter()
        .map(|m| Rect {
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
        })
        .collect();
    let current = Rect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    };
    let fixed = clamp_to_monitors(current, &monitors);
    if fixed != current {
        let _ = window.set_size(PhysicalSize::new(fixed.width, fixed.height));
        let _ = window.set_position(PhysicalPosition::new(fixed.x, fixed.y));
    }
}

/// Applies [`ensure_visible`] to the main window if it exists.
pub fn ensure_main_visible<R: Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        ensure_visible(&window);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMARY: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    };
    const SECOND: Rect = Rect {
        x: 1920,
        y: 0,
        width: 1920,
        height: 1080,
    };

    fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn a_window_on_a_monitor_is_left_alone() {
        let w = rect(100, 100, 1280, 800);
        assert_eq!(clamp_to_monitors(w, &[PRIMARY]), w);
        let on_second = rect(2000, 50, 1280, 800);
        assert_eq!(clamp_to_monitors(on_second, &[PRIMARY, SECOND]), on_second);
    }

    #[test]
    fn a_window_left_on_an_unplugged_monitor_is_brought_back_and_centred() {
        let stranded = rect(2000, 50, 1280, 800);
        let fixed = clamp_to_monitors(stranded, &[PRIMARY]);
        assert_eq!(fixed, rect(320, 140, 1280, 800));
        assert!(is_reachable(fixed, &[PRIMARY]));
    }

    #[test]
    fn a_window_larger_than_the_monitor_is_shrunk_to_fit() {
        let big = rect(5000, 5000, 4000, 3000);
        assert_eq!(clamp_to_monitors(big, &[PRIMARY]), rect(0, 0, 1920, 1080));
    }

    #[test]
    fn a_window_barely_peeking_in_is_unreachable_but_a_title_bar_width_is_enough() {
        assert!(!is_reachable(rect(1900, 100, 1280, 800), &[PRIMARY]));
        assert!(is_reachable(rect(1800, 100, 1280, 800), &[PRIMARY]));
        assert!(!is_reachable(rect(100, 1070, 1280, 800), &[PRIMARY]));
    }

    #[test]
    fn a_smaller_work_area_after_a_dpi_change_is_handled_and_no_monitors_changes_nothing() {
        let smaller = rect(0, 0, 1280, 720);
        let w = rect(600, 400, 1280, 800);
        assert!(is_reachable(clamp_to_monitors(w, &[smaller]), &[smaller]));
        assert_eq!(clamp_to_monitors(w, &[]), w);
    }
}
