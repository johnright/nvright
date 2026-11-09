//! Box drawing and block elements drawn to the cell instead of taken from the
//! font. The font's glyphs only span its own line, so with 'linespace' they
//! don't meet, and diagonals and ClearType fringes never line up across rows.
//! Double lines aren't covered yet, those still come from the font.

use super::font::CellMetrics;
use super::raster::{Coverage, RasterGlyph};

// arms as (up, right, down, left): 0 none, 1 light, 2 heavy
const LINES: [[u8; 4]; 76] = [
    [0, 1, 0, 1], // ─
    [0, 2, 0, 2],
    [1, 0, 1, 0], // │
    [2, 0, 2, 0],
    [0; 4], // dashes, see dashes()
    [0; 4],
    [0; 4],
    [0; 4],
    [0; 4],
    [0; 4],
    [0; 4],
    [0; 4],
    [0, 1, 1, 0], // ┌
    [0, 2, 1, 0],
    [0, 1, 2, 0],
    [0, 2, 2, 0],
    [0, 0, 1, 1], // ┐
    [0, 0, 1, 2],
    [0, 0, 2, 1],
    [0, 0, 2, 2],
    [1, 1, 0, 0], // └
    [1, 2, 0, 0],
    [2, 1, 0, 0],
    [2, 2, 0, 0],
    [1, 0, 0, 1], // ┘
    [1, 0, 0, 2],
    [2, 0, 0, 1],
    [2, 0, 0, 2],
    [1, 1, 1, 0], // ├
    [1, 2, 1, 0],
    [2, 1, 1, 0],
    [1, 1, 2, 0],
    [2, 1, 2, 0],
    [2, 2, 1, 0],
    [1, 2, 2, 0],
    [2, 2, 2, 0],
    [1, 0, 1, 1], // ┤
    [1, 0, 1, 2],
    [2, 0, 1, 1],
    [1, 0, 2, 1],
    [2, 0, 2, 1],
    [2, 0, 1, 2],
    [1, 0, 2, 2],
    [2, 0, 2, 2],
    [0, 1, 1, 1], // ┬
    [0, 1, 1, 2],
    [0, 2, 1, 1],
    [0, 2, 1, 2],
    [0, 1, 2, 1],
    [0, 1, 2, 2],
    [0, 2, 2, 1],
    [0, 2, 2, 2],
    [1, 1, 0, 1], // ┴
    [1, 1, 0, 2],
    [1, 2, 0, 1],
    [1, 2, 0, 2],
    [2, 1, 0, 1],
    [2, 1, 0, 2],
    [2, 2, 0, 1],
    [2, 2, 0, 2],
    [1, 1, 1, 1], // ┼
    [1, 1, 1, 2],
    [1, 2, 1, 1],
    [1, 2, 1, 2],
    [2, 1, 1, 1],
    [1, 1, 2, 1],
    [2, 1, 2, 1],
    [2, 1, 1, 2],
    [2, 2, 1, 1],
    [1, 1, 2, 2],
    [1, 2, 2, 1],
    [2, 2, 1, 2],
    [1, 2, 2, 2],
    [2, 1, 2, 2],
    [2, 2, 2, 1],
    [2, 2, 2, 2], // ╋
];

const HALF_LINES: [[u8; 4]; 12] = [
    [0, 0, 0, 1], // ╴
    [1, 0, 0, 0],
    [0, 1, 0, 0],
    [0, 0, 1, 0],
    [0, 0, 0, 2],
    [2, 0, 0, 0],
    [0, 2, 0, 0],
    [0, 0, 2, 0],
    [0, 2, 0, 1], // ╼
    [1, 0, 2, 0],
    [0, 1, 0, 2],
    [2, 0, 1, 0],
];

struct Canvas {
    w: usize,
    h: usize,
    light: usize,
    heavy: usize,
    alpha: Vec<f32>,
}

impl Canvas {
    fn rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, a: f32) {
        for y in y0..y1.min(self.h) {
            for x in x0..x1.min(self.w) {
                let p = &mut self.alpha[y * self.w + x];
                *p = p.max(a);
            }
        }
    }

    fn stroke(&mut self, thickness: f32, dist: impl Fn(f32, f32) -> f32) {
        for y in 0..self.h {
            for x in 0..self.w {
                let d = dist(x as f32 + 0.5, y as f32 + 0.5);
                let a = (thickness / 2.0 + 0.5 - d).clamp(0.0, 1.0);
                let p = &mut self.alpha[y * self.w + x];
                *p = p.max(a);
            }
        }
    }

    fn thickness(&self, weight: u8) -> usize {
        match weight {
            0 => 0,
            1 => self.light,
            _ => self.heavy,
        }
    }

    fn x0(&self, t: usize) -> usize {
        (self.w - t.min(self.w)) / 2
    }

    fn y0(&self, t: usize) -> usize {
        (self.h - t.min(self.h)) / 2
    }

    fn lines(&mut self, [up, right, down, left]: [u8; 4]) {
        let [tu, tr, td, tl] = [up, right, down, left].map(|a| self.thickness(a));
        let across_h = tl.max(tr);
        let across_v = tu.max(td);
        if tu > 0 {
            let t = across_h.max(tu);
            let x = self.x0(tu);
            self.rect(x, 0, x + tu, self.y0(t) + t, 1.0);
        }
        if td > 0 {
            let x = self.x0(td);
            let y = self.y0(across_h.max(td));
            self.rect(x, y, x + td, self.h, 1.0);
        }
        if tl > 0 {
            let t = across_v.max(tl);
            let y = self.y0(tl);
            self.rect(0, y, self.x0(t) + t, y + tl, 1.0);
        }
        if tr > 0 {
            let y = self.y0(tr);
            let x = self.x0(across_v.max(tr));
            self.rect(x, y, self.w, y + tr, 1.0);
        }
    }

    fn dashes(&mut self, n: usize, heavy: bool, vertical: bool) {
        let t = if heavy { self.heavy } else { self.light };
        let len = if vertical { self.h } else { self.w } as f32;
        let segment = len / n as f32;
        let gap = (segment * 0.35).round().max(1.0);
        for i in 0..n {
            let a = (i as f32 * segment + gap / 2.0).round() as usize;
            let b = ((i + 1) as f32 * segment - gap / 2.0).round() as usize;
            if vertical {
                let x = self.x0(t);
                self.rect(x, a, x + t, b, 1.0);
            } else {
                let y = self.y0(t);
                self.rect(a, y, b, y + t, 1.0);
            }
        }
    }

    fn arc(&mut self, sx: f32, sy: f32) {
        let t = self.light;
        let cx = self.x0(t) as f32 + t as f32 / 2.0;
        let cy = self.y0(t) as f32 + t as f32 / 2.0;
        let room_x = if sx > 0.0 { self.w as f32 - cx } else { cx };
        let room_y = if sy > 0.0 { self.h as f32 - cy } else { cy };
        let r = room_x.min(room_y);
        let (ox, oy) = (cx + sx * r, cy + sy * r);
        self.stroke(t as f32, |x, y| {
            let qx = (x - ox) * sx;
            let qy = (y - oy) * sy;
            let mut d = f32::MAX;
            if qy >= 0.0 {
                d = d.min((x - cx).abs());
            }
            if qx >= 0.0 {
                d = d.min((y - cy).abs());
            }
            if qx <= 0.0 && qy <= 0.0 {
                d = d.min(((qx * qx + qy * qy).sqrt() - r).abs());
            }
            d
        });
    }

    // padded, otherwise the joins between rows get a dark notch
    fn diagonal(&mut self, rising: bool, pad: usize) {
        let (w, h) = ((self.w - 2 * pad) as f32, self.h as f32);
        let len = (w * w + h * h).sqrt();
        let pad = pad as f32;
        self.stroke(self.light as f32, |x, y| {
            let x = x - pad;
            let y = if rising { h - y } else { y };
            (x * h - y * w).abs() / len
        });
    }

    fn block(&mut self, c: char) -> bool {
        let (w, h) = (self.w as f32, self.h as f32);
        let ys = |k: f32| (h * k / 8.0).round() as usize;
        let xs = |k: f32| (w * k / 8.0).round() as usize;
        let (mx, my) = (xs(4.0), ys(4.0));
        let (fw, fh) = (self.w, self.h);
        match c {
            '\u{2580}' => self.rect(0, 0, fw, my, 1.0),
            '\u{2581}'..='\u{2588}' => {
                let k = (c as u32 - 0x2580) as f32;
                self.rect(0, ys(8.0 - k), fw, fh, 1.0);
            }
            '\u{2589}'..='\u{258f}' => {
                let k = (0x2590 - c as u32) as f32;
                self.rect(0, 0, xs(k), fh, 1.0);
            }
            '\u{2590}' => self.rect(mx, 0, fw, fh, 1.0),
            '\u{2591}'..='\u{2593}' => {
                let a = (c as u32 - 0x2590) as f32 / 4.0;
                self.rect(0, 0, fw, fh, a);
            }
            '\u{2594}' => self.rect(0, 0, fw, ys(1.0), 1.0),
            '\u{2595}' => self.rect(xs(7.0), 0, fw, fh, 1.0),
            '\u{2596}'..='\u{259f}' => {
                let quads: u8 = match c {
                    '\u{2596}' => 0b0010,
                    '\u{2597}' => 0b0001,
                    '\u{2598}' => 0b1000,
                    '\u{2599}' => 0b1011,
                    '\u{259a}' => 0b1001,
                    '\u{259b}' => 0b1110,
                    '\u{259c}' => 0b1101,
                    '\u{259d}' => 0b0100,
                    '\u{259e}' => 0b0110,
                    _ => 0b0111,
                };
                let cells = [(0, 0, mx, my), (mx, 0, fw, my), (0, my, mx, fh), (mx, my, fw, fh)];
                for (i, (x0, y0, x1, y1)) in cells.into_iter().enumerate() {
                    if quads & (0b1000 >> i) != 0 {
                        self.rect(x0, y0, x1, y1, 1.0);
                    }
                }
            }
            _ => return false,
        }
        true
    }
}

pub fn is_drawn(c: char) -> bool {
    matches!(c, '\u{2500}'..='\u{254f}' | '\u{256d}'..='\u{259f}')
}

pub fn rasterize(c: char, m: &CellMetrics) -> Option<RasterGlyph> {
    if !is_drawn(c) {
        return None;
    }
    let (w, h) = (m.width as usize, m.height as usize);
    let light = m.box_stroke.max(1.0) as usize;
    let pad = if ('\u{2571}'..='\u{2573}').contains(&c) {
        light + 1
    } else {
        0
    };
    let mut canvas = Canvas {
        w: w + 2 * pad,
        h,
        light,
        heavy: light * 2,
        alpha: vec![0.0; (w + 2 * pad) * h],
    };
    let i = c as usize - 0x2500;
    match c {
        '\u{2504}'..='\u{250b}' => {
            let k = c as u32 - 0x2504;
            canvas.dashes(if k < 4 { 3 } else { 4 }, k % 2 == 1, k % 4 >= 2);
        }
        '\u{2500}'..='\u{254b}' => canvas.lines(LINES[i]),
        '\u{254c}'..='\u{254f}' => {
            let k = c as u32 - 0x254c;
            canvas.dashes(2, k % 2 == 1, k >= 2);
        }
        '\u{256d}' => canvas.arc(1.0, 1.0),
        '\u{256e}' => canvas.arc(-1.0, 1.0),
        '\u{256f}' => canvas.arc(-1.0, -1.0),
        '\u{2570}' => canvas.arc(1.0, -1.0),
        '\u{2571}' => canvas.diagonal(true, pad),
        '\u{2572}' => canvas.diagonal(false, pad),
        '\u{2573}' => {
            canvas.diagonal(true, pad);
            canvas.diagonal(false, pad);
        }
        '\u{2574}'..='\u{257f}' => canvas.lines(HALF_LINES[c as usize - 0x2574]),
        _ => {
            canvas.block(c);
        }
    }
    Some(RasterGlyph {
        width: canvas.w as u32,
        height: h as u32,
        left: -(pad as i32),
        top: 0,
        coverage: Coverage::Mask,
        rgba: canvas
            .alpha
            .iter()
            .flat_map(|&a| [255, 255, 255, (a * 255.0).round() as u8])
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(width: f32, height: f32) -> CellMetrics {
        CellMetrics {
            width,
            height,
            baseline: height * 0.75,
            underline_y: height - 2.0,
            strikeout_y: height / 2.0,
            stroke: 1.0,
            box_stroke: 1.0,
            gap_top: 0.0,
            gap_bottom: 0.0,
        }
    }

    fn alpha(g: &RasterGlyph, x: u32, y: u32) -> u8 {
        g.rgba[((y * g.width + x) * 4 + 3) as usize]
    }

    #[test]
    fn vertical_line_spans_the_cell() {
        let g = rasterize('│', &metrics(11.0, 29.0)).unwrap();
        assert_eq!(alpha(&g, 5, 0), 255);
        assert_eq!(alpha(&g, 5, 28), 255);
        assert_eq!(alpha(&g, 4, 14), 0);
    }

    #[test]
    fn corner_arms_meet() {
        let g = rasterize('┌', &metrics(11.0, 29.0)).unwrap();
        assert_eq!(alpha(&g, 10, 14), 255);
        assert_eq!(alpha(&g, 5, 28), 255);
        assert_eq!(alpha(&g, 5, 13), 0);
        assert_eq!(alpha(&g, 4, 14), 0);
    }

    #[test]
    fn diagonal_reaches_the_corners() {
        let g = rasterize('╲', &metrics(11.0, 29.0)).unwrap();
        let pad = (-g.left) as u32;
        assert!(alpha(&g, pad, 0) > 100);
        assert!(alpha(&g, pad + 10, 28) > 100);
        assert_eq!(alpha(&g, pad + 10, 0), 0);
    }

    #[test]
    fn double_lines_left_to_the_font() {
        assert!(rasterize('═', &metrics(11.0, 29.0)).is_none());
    }
}
