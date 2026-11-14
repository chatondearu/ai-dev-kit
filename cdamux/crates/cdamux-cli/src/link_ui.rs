//! Clickable PR / issue links (OSC 8) and the `o` opener.
//!
//! ratatui 0.28 counts the bytes of an escape sequence as cell width, so a link cannot live
//! in a buffer cell. Draw code [`record`]s the cells a link covers; [`flush`] re-prints them
//! right after the frame, wrapped in OSC 8, through the same backend (styles included);
//! a link something was drawn over (overlay, modal) is dropped.
//! tmux needs `terminal-features ',*:hyperlinks'` (>= 3.4) to forward them; elsewhere the
//! plain text is all you get.

use ratatui::backend::Backend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::{Frame, Terminal};
use std::cell::RefCell;
use std::io;
use std::process::Command;

/// A link inside one row, as columns relative to the row start.
pub(crate) struct RowLink {
    pub col: u16,
    pub len: u16,
    pub url: String,
}

struct Spot {
    url: String,
    cells: Vec<(u16, u16, Cell)>,
}

thread_local! {
    static SPOTS: RefCell<Vec<Spot>> = const { RefCell::new(Vec::new()) };
}

/// Only plain web URLs become links: the value comes from state files, and a control
/// character in it would inject terminal escapes.
fn safe_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && !url.chars().any(|c| c.is_control() || c == ' ')
}

/// Remember the `len` cells at `(x, y)` of the frame being drawn as a link to `url`.
/// Call after the widget under it rendered, so the cells carry their final style.
pub(crate) fn record(buf: &Buffer, x: u16, y: u16, len: u16, url: &str) {
    let area = buf.area;
    if len == 0 || !safe_url(url) || y >= area.bottom() || x < area.x || x.saturating_add(len) > area.right() {
        return;
    }
    let cells = (x..x + len).map(|cx| (cx, y, buf[(cx, y)].clone())).collect();
    SPOTS.with_borrow_mut(|s| s.push(Spot { url: url.to_string(), cells }));
}

/// Drop the links whose cells changed since [`record`] (drawn over); run on the final frame.
fn settle(buf: &Buffer) {
    SPOTS.with_borrow_mut(|spots| {
        spots.retain(|s| s.cells.iter().all(|(x, y, cell)| &buf[(*x, *y)] == cell));
    });
}

/// `terminal.draw(ui)` plus the OSC 8 links recorded by `ui`.
pub(crate) fn draw<B: Backend>(term: &mut Terminal<B>, ui: impl FnOnce(&mut Frame)) -> io::Result<()> {
    term.draw(|f| {
        ui(f);
        settle(f.buffer_mut());
    })?;
    flush(term.backend_mut())
}

/// Re-print the recorded links wrapped in OSC 8.
fn flush<B: Backend>(backend: &mut B) -> io::Result<()> {
    let spots = SPOTS.with_borrow_mut(std::mem::take);
    if spots.is_empty() {
        return Ok(());
    }
    for spot in spots {
        let last = spot.cells.len() - 1;
        let cells: Vec<(u16, u16, Cell)> = spot
            .cells
            .into_iter()
            .enumerate()
            .map(|(i, (x, y, mut cell))| {
                let mut sym = cell.symbol().to_string();
                if i == 0 {
                    sym.insert_str(0, &format!("\x1b]8;;{}\x1b\\", spot.url));
                }
                if i == last {
                    sym.push_str("\x1b]8;;\x1b\\");
                }
                cell.set_symbol(&sym);
                (x, y, cell)
            })
            .collect();
        backend.draw(cells.iter().map(|(x, y, c)| (*x, *y, c)))?;
    }
    backend.flush()
}

/// What `o` opens: the PR when there is one, else the issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenTarget {
    pub cwd: String,
    pub pr: bool,
    pub number: u32,
    pub url: Option<String>,
}

impl OpenTarget {
    fn label(&self) -> String {
        format!("{} #{}", if self.pr { "PR" } else { "issue" }, self.number)
    }
}

/// Run `program args` in `cwd`; `Err` carries the failure text.
type Runner<'a> = &'a dyn Fn(&str, &[&str], &str) -> Result<(), String>;

pub(crate) fn run_command(program: &str, args: &[&str], cwd: &str) -> Result<(), String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// `gh <pr|issue> view N --web`; when that fails and the URL is known, the OS opener.
pub(crate) fn open_target(t: &OpenTarget, run: Runner) -> Result<String, String> {
    let n = t.number.to_string();
    let kind = if t.pr { "pr" } else { "issue" };
    let gh_err = match run("gh", &[kind, "view", &n, "--web"], &t.cwd) {
        Ok(()) => return Ok(format!("opened {}", t.label())),
        Err(e) => e,
    };
    match t.url.as_deref().filter(|u| safe_url(u)) {
        Some(url) => {
            let opener = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
            run(opener, &[url], &t.cwd)
                .map(|()| format!("opened {}", t.label()))
                .map_err(|e| format!("{opener}: {e}"))
        }
        None => Err(format!("gh {kind} view failed: {gh_err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::{CrosstermBackend, TestBackend};
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Style};
    use ratatui::{Terminal, TerminalOptions, Viewport};
    use std::cell::RefCell;
    use std::io::Write;
    use std::rc::Rc;

    /// A cloneable sink so the test can read what the backend wrote.
    #[derive(Clone, Default)]
    struct Sink(Rc<RefCell<Vec<u8>>>);

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    const URL: &str = "https://github.com/o/r/pull/12";

    #[test]
    fn flush_wraps_cells_in_osc8_and_keeps_layout() {
        let sink = Sink::default();
        let opts = TerminalOptions { viewport: Viewport::Fixed(Rect::new(0, 0, 20, 1)) };
        let mut term = Terminal::with_options(CrosstermBackend::new(sink.clone()), opts).unwrap();
        draw(&mut term, |f| {
            f.render_widget(
                ratatui::widgets::Paragraph::new("ab PR#12 cd").style(Style::default().fg(Color::Red)),
                Rect::new(0, 0, 20, 1),
            );
            record(f.buffer_mut(), 3, 0, 5, URL);
            // Rejected: not a web URL, control character, outside the frame.
            record(f.buffer_mut(), 0, 0, 2, "file:///etc/passwd");
            record(f.buffer_mut(), 0, 0, 2, "https://x/\x1b]0;pwn\x07");
            record(f.buffer_mut(), 18, 0, 5, URL);
        })
        .unwrap();
        let out = String::from_utf8(sink.0.borrow().clone()).unwrap();
        assert_eq!(out.matches("\x1b]8;;").count(), 2, "{out:?}");
        // One link: label contiguous (same style, no cursor jump), opened and closed once.
        let link = format!("\x1b]8;;{URL}\x1b\\PR#12\x1b]8;;\x1b\\");
        assert!(out.contains(&link), "{out:?}");
        // Nothing is replayed on the next frame, and a link drawn over is dropped.
        sink.0.borrow_mut().clear();
        draw(&mut term, |f| {
            f.render_widget(ratatui::widgets::Paragraph::new("ab PR#12 cd"), Rect::new(0, 0, 20, 1));
            record(f.buffer_mut(), 3, 0, 5, URL);
            f.render_widget(ratatui::widgets::Paragraph::new("overlay"), Rect::new(0, 0, 20, 1));
        })
        .unwrap();
        assert!(!String::from_utf8(sink.0.borrow().clone()).unwrap().contains("\x1b]8;;"));
    }

    #[test]
    fn draw_into_test_backend_keeps_visible_text() {
        let mut term = Terminal::new(TestBackend::new(12, 1)).unwrap();
        draw(&mut term, |f| {
            f.render_widget(ratatui::widgets::Paragraph::new("go PR#12"), f.area());
            record(f.buffer_mut(), 3, 0, 5, URL);
        })
        .unwrap();
        let row: String = (0..12).map(|x| term.backend().buffer()[(x, 0)].symbol().to_string()).collect();
        assert_eq!(row.replace(&format!("\x1b]8;;{URL}\x1b\\"), "").replace("\x1b]8;;\x1b\\", "").trim_end(), "go PR#12");
    }

    #[test]
    fn open_prefers_gh_then_falls_back_to_the_url() {
        let calls = RefCell::new(Vec::<String>::new());
        let stub = |gh_ok: bool| {
            let calls = &calls;
            move |p: &str, a: &[&str], _cwd: &str| {
                calls.borrow_mut().push(format!("{p} {}", a.join(" ")));
                if p == "gh" && !gh_ok { Err("not logged in".to_string()) } else { Ok(()) }
            }
        };
        let pr = OpenTarget { cwd: "/w".into(), pr: true, number: 12, url: Some(URL.into()) };
        assert_eq!(open_target(&pr, &stub(true)).unwrap(), "opened PR #12");
        assert_eq!(calls.borrow().as_slice(), ["gh pr view 12 --web"]);
        calls.borrow_mut().clear();
        assert_eq!(open_target(&pr, &stub(false)).unwrap(), "opened PR #12");
        assert!(calls.borrow()[1].ends_with(URL), "{:?}", calls.borrow());
        let bare = OpenTarget { cwd: "/w".into(), pr: false, number: 7, url: None };
        let err = open_target(&bare, &stub(false)).unwrap_err();
        assert_eq!(err, "gh issue view failed: not logged in");
    }
}
