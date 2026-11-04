//! Small ratatui form widgets: Select (enum cycle), checkbox, TextInput,
//! FieldList, and a text modal. Pure helpers so forms stay thin and testable.

use crate::app_tabs::{
    screen_block, CLR_ACCENT, CLR_INPUT_BG, CLR_MUTED, CLR_SEL_BG, CLR_TEXT, CLR_WARN,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};

pub const MODAL_BG: Color = Color::Rgb(22, 26, 34);

/// Default visual width for text fields; `text_field` clamps to `available`.
pub const TEXT_FIELD_WIDTH: usize = 24;

/// Select chrome: `[ value ▾ ]`.
pub fn select(value: &str) -> String {
    format!("[ {value} ▾ ]")
}

/// Checkbox chrome: `[x]` / `[ ]`.
pub fn checkbox(on: bool) -> &'static str {
    if on { "[x]" } else { "[ ]" }
}

/// Text field: pad/truncate to `TEXT_FIELD_WIDTH.min(available)`, darker input bg,
/// block cursor when `editing`.
pub fn text_field(value: &str, editing: bool, available: usize) -> Span<'static> {
    let width = TEXT_FIELD_WIDTH.min(available.max(1));
    let mut content = value.to_string();
    if editing {
        content.push('█');
    }
    let display = match content.chars().count() {
        n if n > width => content.chars().take(width).collect(),
        n => format!("{content}{}", " ".repeat(width - n)),
    };
    Span::styled(display, Style::default().fg(CLR_TEXT).bg(CLR_INPUT_BG))
}

/// Select: step `cur` by `delta` through `all`, wrapping both ways.
pub fn cycle<T: Copy + PartialEq>(all: &[T], cur: T, delta: i32) -> T {
    let pos = all.iter().position(|v| *v == cur).unwrap_or(0) as i32;
    all[(pos + delta).rem_euclid(all.len() as i32) as usize]
}

/// Mouse wheel: move `cur` by `delta` rows, clamped to `0..len`.
pub fn wheel_step(cur: usize, delta: isize, len: usize) -> usize {
    cur.saturating_add_signed(delta).min(len.saturating_sub(1))
}

/// Render `list` keeping `selected` in view: the list scrolls only when the
/// cursor reaches an edge. `offset` persists the scroll position across frames.
/// `bordered` lists get their scrollbar on the right border, inside the corners.
pub fn draw_scrolled_list(
    f: &mut Frame,
    area: Rect,
    list: List,
    selected: usize,
    offset: &mut usize,
    bordered: bool,
) {
    let len = list.len();
    let mut state = ListState::default().with_offset(*offset).with_selected(Some(selected));
    f.render_stateful_widget(list, area, &mut state);
    *offset = state.offset();
    let bar = if bordered { area.inner(Margin { vertical: 1, horizontal: 0 }) } else { area };
    draw_scrollbar(f, bar, len, *offset);
}

/// Vertical scrollbar on the right column of `area` (one row per visible list
/// row), drawn only when `len` rows overflow it; `offset` is the first visible row.
pub fn draw_scrollbar(f: &mut Frame, area: Rect, len: usize, offset: usize) {
    let visible = area.height as usize;
    if len <= visible {
        return;
    }
    // Content length = offsets + 1 so the thumb reaches the bottom at the last offset.
    let mut state = ScrollbarState::new(len - visible + 1)
        .position(offset)
        .viewport_content_length(visible);
    let bar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .track_style(Style::default().fg(CLR_MUTED))
        .thumb_style(Style::default().fg(CLR_ACCENT));
    f.render_stateful_widget(bar, area, &mut state);
}

/// TextInput: Backspace pops; plain/shifted chars passing `accept` are pushed.
pub fn edit_text(buf: &mut String, key: KeyEvent, accept: impl Fn(char) -> bool) {
    match key.code {
        KeyCode::Backspace => {
            buf.pop();
        }
        KeyCode::Char(c)
            if (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
                && !c.is_control()
                && accept(c) =>
        {
            buf.push(c);
        }
        _ => {}
    }
}

/// FieldList: clamp focus after moving by `delta` in a list of `len` rows.
pub fn move_focus(focus: usize, len: usize, delta: i32) -> usize {
    (focus as i64 + delta as i64).clamp(0, len.saturating_sub(1) as i64) as usize
}

/// One `label = value` row; a new `section` emits a heading above it.
pub struct FieldRow {
    pub section: &'static str,
    pub label: &'static str,
    pub value: String,
    pub value_color: Color,
    /// Action rows render the label only (no ` = `).
    pub action: bool,
    /// Paint the value on `CLR_INPUT_BG` (Settings scalar / root text fields).
    pub input_bg: bool,
}

/// FieldList: header, sectioned rows with focus marker, then a status line.
/// Returns the items and the list index of the focused row.
pub fn field_list_items(
    header: ListItem<'static>,
    rows: Vec<FieldRow>,
    focus: usize,
    status: &str,
) -> (Vec<ListItem<'static>>, usize) {
    let mut items = vec![header];
    let mut selected = 0;
    let mut section = "";
    for (i, row) in rows.into_iter().enumerate() {
        if row.section != section {
            section = row.section;
            items.push(ListItem::new(""));
            items.push(ListItem::new(Span::styled(section, Style::default().fg(CLR_ACCENT))));
        }
        let is_focus = i == focus;
        if is_focus {
            selected = items.len();
        }
        let value_span = if row.input_bg {
            Span::styled(
                {
                    let width = TEXT_FIELD_WIDTH;
                    let n = row.value.chars().count();
                    if n > width {
                        row.value.chars().take(width).collect()
                    } else {
                        format!("{}{}", row.value, " ".repeat(width - n))
                    }
                },
                Style::default().fg(row.value_color).bg(CLR_INPUT_BG),
            )
        } else {
            Span::styled(row.value, Style::default().fg(row.value_color))
        };
        let line = Line::from(vec![
            Span::raw(format!("{} {}", if is_focus { "›" } else { " " }, row.label)),
            Span::raw(if row.action { "" } else { " = " }),
            value_span,
        ]);
        let style = if is_focus {
            Style::default().bg(CLR_SEL_BG).fg(CLR_TEXT).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(CLR_TEXT)
        };
        items.push(ListItem::new(line).style(style));
    }
    items.push(ListItem::new(""));
    items.push(ListItem::new(Span::styled(status.to_string(), Style::default().fg(CLR_WARN))));
    (items, selected)
}

pub fn draw_field_list(
    f: &mut Frame,
    area: Rect,
    title: &str,
    header: ListItem<'static>,
    rows: Vec<FieldRow>,
    focus: usize,
    status: &str,
) {
    let (items, selected) = field_list_items(header, rows, focus, status);
    // Offset is recomputed each frame from the focused row (same as a fresh ListState).
    let mut offset = 0;
    draw_scrolled_list(
        f,
        area,
        List::new(items).block(screen_block(title)),
        selected,
        &mut offset,
        true,
    );
}

/// Text modal: 5-row box near the top third of `area`, clipped on tiny terminals.
pub fn draw_text_modal(f: &mut Frame, area: Rect, title: String, body: Text<'static>) {
    let rect = text_modal_rect(area);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(MODAL_BG).fg(CLR_TEXT));
    f.render_widget(Clear, rect);
    f.render_widget(Paragraph::new(body).block(block), rect);
}

/// Inner width of the text modal (for clamping text fields).
pub fn text_modal_inner_width(area: Rect) -> usize {
    text_modal_rect(area).width.saturating_sub(2) as usize
}

/// Top third of the room left once the box fits, so short terminals keep the whole box.
fn text_modal_rect(area: Rect) -> Rect {
    let w = area.width.saturating_sub(4).min(72);
    let y = area.y + area.height.saturating_sub(5) / 3;
    Rect::new(area.x + (area.width - w) / 2, y, w, 5).intersection(area)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn wheel_step_clamps() {
        assert_eq!(wheel_step(1, -3, 10), 0);
        assert_eq!(wheel_step(8, 3, 10), 9);
        assert_eq!(wheel_step(4, 3, 10), 7);
        assert_eq!(wheel_step(0, 3, 0), 0);
    }

    #[test]
    fn cycle_wraps_both_ways() {
        let all = [1, 2, 3];
        assert_eq!(cycle(&all, 3, 1), 1);
        assert_eq!(cycle(&all, 1, -1), 3);
        assert_eq!(cycle(&all, 2, 1), 3);
    }

    #[test]
    fn edit_text_pushes_pops_and_filters() {
        let mut buf = String::new();
        edit_text(&mut buf, key(KeyCode::Char('a'), KeyModifiers::NONE), |_| true);
        edit_text(&mut buf, key(KeyCode::Char('B'), KeyModifiers::SHIFT), |_| true);
        edit_text(&mut buf, key(KeyCode::Char('c'), KeyModifiers::CONTROL), |_| true);
        edit_text(&mut buf, key(KeyCode::Char('é'), KeyModifiers::NONE), |c| c.is_ascii());
        assert_eq!(buf, "aB");
        edit_text(&mut buf, key(KeyCode::Backspace, KeyModifiers::NONE), |_| true);
        assert_eq!(buf, "a");
    }

    #[test]
    fn text_modal_stays_inside_short_terminals() {
        let area = Rect::new(0, 0, 80, 6);
        assert_eq!(text_modal_rect(area), Rect::new(4, 0, 72, 5));
        assert_eq!(text_modal_rect(Rect::new(0, 0, 80, 24)).y, 6);
    }

    #[test]
    fn move_focus_clamps() {
        assert_eq!(move_focus(0, 3, -1), 0);
        assert_eq!(move_focus(1, 3, 1), 2);
        assert_eq!(move_focus(2, 3, 1), 2);
    }

    #[test]
    fn field_list_marks_focus_after_section_headings() {
        let row = |section, label| FieldRow {
            section,
            label,
            value: "v".into(),
            value_color: CLR_TEXT,
            action: false,
            input_bg: false,
        };
        let rows = vec![row("[a]", "x"), row("[a]", "y"), row("[b]", "z")];
        // header, blank, [a], x, y, blank, [b], z, blank, status
        let (items, selected) = field_list_items(ListItem::new("h"), rows, 2, "ok");
        assert_eq!((items.len(), selected), (10, 7));
    }

    /// Right column, inside the top/bottom borders (same seam as Sessions).
    fn right_column(term: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
        let buf = term.backend().buffer();
        let x = buf.area.width - 1;
        (1..buf.area.height - 1).map(|y| buf[(x, y)].symbol()).collect()
    }

    fn many_rows(n: usize) -> Vec<FieldRow> {
        (0..n)
            .map(|i| FieldRow {
                section: "[s]",
                label: "f",
                value: format!("{i}"),
                value_color: CLR_TEXT,
                action: false,
                input_bg: false,
            })
            .collect()
    }

    #[test]
    fn field_list_scrollbar_only_when_overflowing() {
        let mut term =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(40, 12)).unwrap();
        term.draw(|f| {
            draw_field_list(f, f.area(), "settings", ListItem::new("h"), many_rows(5), 0, "ok")
        })
        .unwrap();
        assert!(right_column(&term).chars().all(|c| c == '│'), "no scrollbar expected");
        term.draw(|f| {
            draw_field_list(f, f.area(), "settings", ListItem::new("h"), many_rows(30), 29, "ok")
        })
        .unwrap();
        assert!(right_column(&term).contains('█'), "{}", right_column(&term));
    }

    #[test]
    fn form_chrome_text_select_checkbox() {
        assert_eq!(select("mirror"), "[ mirror ▾ ]");
        assert_eq!(checkbox(true), "[x]");
        assert_eq!(checkbox(false), "[ ]");
        let span = text_field("hi", true, 10);
        assert!(span.content.contains('█'));
        assert_eq!(span.style.bg, Some(CLR_INPUT_BG));

        let rows = vec![
            FieldRow {
                section: "[s]",
                label: "enum",
                value: select("Project"),
                value_color: CLR_TEXT,
                action: false,
                input_bg: false,
            },
            FieldRow {
                section: "[s]",
                label: "flag",
                value: checkbox(true).into(),
                value_color: CLR_TEXT,
                action: false,
                input_bg: false,
            },
            FieldRow {
                section: "[s]",
                label: "num",
                value: "80".into(),
                value_color: CLR_TEXT,
                action: false,
                input_bg: true,
            },
        ];
        let mut term =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(60, 12)).unwrap();
        // Focus stays on the Enum row so the scalar's input bg is not under the focus fill.
        term.draw(|f| {
            draw_field_list(f, f.area(), "settings", ListItem::new("h"), rows, 0, "ok")
        })
        .unwrap();
        let buf = term.backend().buffer();
        let lines: Vec<String> = (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect();
        let screen = lines.join("\n");
        assert!(screen.contains('▾'), "{screen}");
        assert!(screen.contains("[x]"), "{screen}");
        // Walk cells (String::find is byte-based; box-drawing borders are multi-byte).
        let num_y = lines.iter().position(|l| l.contains(" num = ")).unwrap() as u16;
        let num_x = (0..buf.area.width.saturating_sub(6))
            .find(|&x| {
                (0..6).map(|i| buf[(x + i, num_y)].symbol()).collect::<String>() == "num = "
            })
            .map(|x| x + 6)
            .expect("num = cell run");
        assert_eq!(buf[(num_x, num_y)].symbol(), "8");
        assert_eq!(buf[(num_x, num_y)].bg, CLR_INPUT_BG);
    }
}
