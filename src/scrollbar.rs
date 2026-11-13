use std::time::{Duration, Instant};

use rmpv::Value;

use crate::nvim::redraw::Viewport;

pub const LUA: &str = include_str!("scrollbar.lua");

pub const SCROLL_TO: &str = r#"
local win, top = ...
vim.api.nvim_win_call(win, function()
  local h = vim.api.nvim_win_get_height(0)
  local so = math.min(vim.wo.scrolloff, math.floor((h - 1) / 2))
  local lnum = math.max(top + 1 + so, math.min(vim.fn.line('.'), top + h - so))
  vim.fn.winrestview({ topline = top + 1, lnum = math.min(lnum, vim.fn.line('$')) })
end)
"#;

const HOLD: Duration = Duration::from_millis(1000);
const FADE: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub enum MarksEvent {
    Colors(Vec<u32>),
    Window(u64, Vec<(u32, u8)>),
}

pub fn parse_notification(args: Vec<Value>) -> Option<MarksEvent> {
    let mut args = args.into_iter();
    let kind = args.next()?;
    match kind.as_str()? {
        "colors" => {
            let colors = args.next()?;
            Some(MarksEvent::Colors(
                colors
                    .as_array()?
                    .iter()
                    .map(|c| c.as_u64().unwrap_or(0x808080) as u32)
                    .collect(),
            ))
        }
        "win" => {
            let win = args.next()?.as_u64()?;
            let flat = args.next()?;
            let flat = flat.as_array().map(Vec::as_slice).unwrap_or_default();
            let marks = flat
                .as_chunks::<2>()
                .0
                .iter()
                .filter_map(|[line, kind]| Some((line.as_u64()? as u32, kind.as_u64()? as u8)))
                .collect();
            Some(MarksEvent::Window(win, marks))
        }
        _ => None,
    }
}

pub fn scrollable(view: &Viewport) -> bool {
    view.lines > view.bot.saturating_sub(view.top)
}

pub fn thumb(view: &Viewport, height: f32, min: f32) -> (f32, f32) {
    let lines = view.lines.max(1) as f32;
    let shown = view.bot.saturating_sub(view.top).max(1) as f32;
    let h = (shown / lines * height).clamp(min.min(height), height);
    let y = view.top as f32 / (lines - shown).max(1.0) * (height - h);
    (y.clamp(0.0, height - h), h)
}

pub fn mark_y(line: u32, lines: u64, height: f32) -> f32 {
    line as f32 / lines.max(1) as f32 * height
}

pub fn top_at(view: &Viewport, frac: f32) -> u64 {
    let shown = view.bot.saturating_sub(view.top);
    let max = view.lines.saturating_sub(shown);
    let top = frac.clamp(0.0, 1.0) * view.lines as f32 - shown as f32 / 2.0;
    (top.max(0.0).round() as u64).min(max)
}

#[derive(Debug, Clone, Copy)]
pub struct Bar {
    pub view: Viewport,
    pub marks: u64,
    pub until: Instant,
}

impl Bar {
    pub fn poke(&mut self, now: Instant) {
        self.until = now + HOLD;
    }

    pub fn alpha(&self, now: Instant) -> f32 {
        let past = now.saturating_duration_since(self.until);
        1.0 - (past.as_secs_f32() / FADE.as_secs_f32()).min(1.0)
    }

    pub fn fading(&self, now: Instant) -> bool {
        now < self.until + FADE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(top: u64, bot: u64, lines: u64) -> Viewport {
        Viewport {
            win: 1000,
            top,
            bot,
            lines,
        }
    }

    #[test]
    fn thumb_spans_the_track() {
        assert_eq!(thumb(&view(0, 50, 100), 200.0, 20.0), (0.0, 100.0));
        assert_eq!(thumb(&view(50, 100, 100), 200.0, 20.0), (100.0, 100.0));
        let (y, h) = thumb(&view(9990, 10000, 10000), 200.0, 20.0);
        assert_eq!((y + h, h), (200.0, 20.0));
        assert!(!scrollable(&view(0, 30, 30)));
    }

    #[test]
    fn drag_targets() {
        let v = view(0, 20, 100);
        assert_eq!(top_at(&v, 0.0), 0);
        assert_eq!(top_at(&v, 0.5), 40);
        assert_eq!(top_at(&v, 1.0), 80);
    }

    #[test]
    fn notification() {
        let args = vec![
            Value::from("win"),
            Value::from(1001),
            Value::Array(vec![3.into(), 2.into(), 10.into(), 1.into()]),
        ];
        match parse_notification(args) {
            Some(MarksEvent::Window(1001, marks)) => assert_eq!(marks, [(3, 2), (10, 1)]),
            other => panic!("{other:?}"),
        }
    }
}
