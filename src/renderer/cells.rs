use std::collections::HashMap;
use std::sync::Arc;

use super::atlas::{Atlas, AtlasFull};
use super::font::{CellMetrics, FontStyle, Fonts};
use super::pipeline::{
    Instance, KIND_COLOR, KIND_DASHED, KIND_DOTTED, KIND_MASK, KIND_SOLID, KIND_SUBPIXEL,
    KIND_UNDERCURL, pack_rgb, pack_rgba,
};
use super::raster::{Coverage, Rasterizer};
use crate::editor::{CellText, HlTable, Row, TextInterner};
use crate::kitty::{FitKey, Fitted, Placeholder, Placements};

#[derive(Debug, Default, Clone)]
pub struct RowInstances {
    pub bg: Vec<Instance>,
    pub fg: Vec<Instance>,
    pub default_bg: bool,
}

#[derive(Default)]
pub struct RowBuilder {
    seg_text: String,
    seg_map: Vec<(u32, u16)>,
    prev_image: Option<(u32, u16, u16, usize)>,
    pub missing: Vec<FitKey>,
}

pub struct RowContext<'a> {
    pub hl: &'a HlTable,
    pub text: &'a TextInterner,
    pub metrics: CellMetrics,
    pub queue: &'a wgpu::Queue,
    pub paint_default_bg: bool,
    pub placements: &'a Placements,
    pub images: &'a HashMap<FitKey, Option<Arc<Fitted>>>,
}

fn font_style(hl: &HlTable, id: u32) -> FontStyle {
    let attr = hl.attr(id);
    FontStyle {
        bold: attr.bold,
        italic: attr.italic,
    }
}

impl RowBuilder {
    fn image_cell(
        &mut self,
        col: usize,
        hl: u32,
        p: Placeholder,
        ctx: &RowContext<'_>,
        atlas: &mut Atlas,
        out: &mut RowInstances,
    ) -> Result<(), AtlasFull> {
        let attr = ctx.hl.attr(hl);
        let Some(fg) = attr.foreground else {
            return Ok(());
        };
        let id = fg | (p.id_msb.unwrap_or(0) as u32) << 24;
        let prev = self
            .prev_image
            .filter(|&(prev_id, _, _, prev_col)| prev_id == id && prev_col + 1 == col);
        let row = p.row.or(prev.map(|p| p.1)).unwrap_or(0);
        let icol = p.col.or(prev.map(|p| p.2 + 1)).unwrap_or(0);
        self.prev_image = Some((id, row, icol, col));

        let Some((cols, rows)) = ctx.placements.get(id, attr.special.unwrap_or(0)) else {
            return Ok(());
        };
        let m = ctx.metrics;
        let key = (
            id,
            cols as u32 * m.width as u32,
            rows as u32 * m.height as u32,
        );
        let Some(entry) = ctx.images.get(&key) else {
            self.missing.push(key);
            return Ok(());
        };
        let Some(image) = entry else {
            return Ok(());
        };
        let ag = atlas.get_image(key, image, ctx.queue)?;
        let (cx, cy) = (icol as f32 * m.width, row as f32 * m.height);
        let (ix, iy) = (image.x as f32, image.y as f32);
        let x0 = cx.max(ix);
        let y0 = cy.max(iy);
        let x1 = (cx + m.width).min(ix + image.width as f32);
        let y1 = (cy + m.height).min(iy + image.height as f32);
        if x1 <= x0 || y1 <= y0 {
            return Ok(());
        }
        out.fg.push(Instance {
            pos: [col as f32 * m.width + x0 - cx, y0 - cy],
            size: [x1 - x0, y1 - y0],
            uv: [ag.uv[0] as f32 + x0 - ix, ag.uv[1] as f32 + y0 - iy],
            color: u32::MAX,
            kind: KIND_COLOR,
        });
        Ok(())
    }

    pub fn build(
        &mut self,
        row: &Row,
        ctx: &RowContext<'_>,
        fonts: &mut Fonts,
        raster: &mut Rasterizer,
        atlas: &mut Atlas,
        out: &mut RowInstances,
    ) -> Result<(), AtlasFull> {
        out.bg.clear();
        out.fg.clear();
        out.default_bg = ctx.paint_default_bg;
        let cells = &row.cells;
        let m = ctx.metrics;
        let default_bg = ctx.hl.defaults().bg;

        let bg_of = |hl: u32| (ctx.hl.colors(hl).bg, ctx.hl.attr(hl).blend);
        let mut col = 0;
        while col < cells.len() {
            let (bg, blend) = bg_of(cells[col].hl);
            let start = col;
            while col < cells.len() && bg_of(cells[col].hl) == (bg, blend) {
                col += 1;
            }
            if bg != default_bg || ctx.paint_default_bg {
                let alpha = (255 * (100 - blend.min(100) as u32) / 100) as u8;
                out.bg.push(Instance {
                    color: pack_rgba(bg, alpha),
                    ..Instance::solid(
                        start as f32 * m.width,
                        0.0,
                        (col - start) as f32 * m.width,
                        m.height,
                        bg,
                    )
                });
            }
        }

        self.prev_image = None;
        let mut col = 0;
        while col < cells.len() {
            let first = cells[col];
            if first.text == CellText::SPACE || first.text == CellText::EMPTY {
                col += 1;
                continue;
            }
            if let Some(p) = ctx.text.placeholder(first.text) {
                self.image_cell(col, first.hl, p, ctx, atlas, out)?;
                col += 1;
                continue;
            }
            let style = font_style(ctx.hl, first.hl);
            self.seg_text.clear();
            self.seg_map.clear();
            while col < cells.len() {
                let cell = cells[col];
                if cell.text == CellText::SPACE || ctx.text.placeholder(cell.text).is_some() {
                    break;
                }
                if cell.text != CellText::EMPTY {
                    if font_style(ctx.hl, cell.hl) != style {
                        break;
                    }
                    self.seg_map.push((self.seg_text.len() as u32, col as u16));
                    ctx.text.push_to(cell.text, &mut self.seg_text);
                }
                col += 1;
            }

            let glyphs = fonts.shape(&self.seg_text, style);
            for g in glyphs.iter() {
                let i = self
                    .seg_map
                    .partition_point(|&(byte, _)| byte <= g.byte)
                    .saturating_sub(1);
                let gcol = self.seg_map[i].1;
                let fg = ctx.hl.colors(cells[gcol as usize].hl).fg;
                let c = self.seg_text[g.byte as usize..].chars().next().unwrap_or(' ');
                if let Some(ag) = atlas.get_drawn(c, &m, ctx.queue)? {
                    out.fg.push(Instance {
                        pos: [gcol as f32 * m.width + ag.left as f32, 0.0],
                        size: [ag.size[0] as f32, ag.size[1] as f32],
                        uv: [ag.uv[0] as f32, ag.uv[1] as f32],
                        color: pack_rgb(fg),
                        kind: KIND_MASK,
                    });
                    continue;
                }
                let Some(ag) = atlas.get(g.key, fonts, raster, ctx.queue)? else {
                    continue;
                };
                out.fg.push(Instance {
                    pos: [
                        gcol as f32 * m.width + (g.x + ag.left as i32) as f32,
                        m.baseline + (g.y - ag.top as i32) as f32,
                    ],
                    size: [ag.size[0] as f32, ag.size[1] as f32],
                    uv: [ag.uv[0] as f32, ag.uv[1] as f32],
                    color: pack_rgb(fg),
                    kind: match ag.coverage {
                        Coverage::Mask => KIND_MASK,
                        Coverage::Subpixel => KIND_SUBPIXEL,
                        Coverage::Color => KIND_COLOR,
                    },
                });
            }
        }

        let mut col = 0;
        while col < cells.len() {
            let id = cells[col].hl;
            let start = col;
            while col < cells.len() && cells[col].hl == id {
                col += 1;
            }
            let attr = ctx.hl.attr(id);
            let (x, w) = (start as f32 * m.width, (col - start) as f32 * m.width);
            let mut deco = |kind: u32, y: f32, h: f32, rgb: u32| {
                out.fg.push(Instance {
                    pos: [x, y],
                    size: [w, h],
                    uv: [x, 0.0],
                    color: pack_rgb(rgb),
                    kind,
                })
            };
            let colors = ctx.hl.colors(id);
            if attr.underline {
                deco(KIND_SOLID, m.underline_y, m.stroke, colors.sp);
            }
            if attr.underdouble {
                deco(KIND_SOLID, m.underline_y, m.stroke, colors.sp);
                deco(
                    KIND_SOLID,
                    (m.underline_y - 2.0 * m.stroke).max(0.0),
                    m.stroke,
                    colors.sp,
                );
            }
            if attr.undercurl {
                let h = (4.0 * m.stroke).max(4.0);
                let y = (m.underline_y + m.stroke / 2.0 - h / 2.0).clamp(0.0, m.height - h);
                deco(KIND_UNDERCURL, y, h, colors.sp);
            }
            if attr.underdotted {
                deco(KIND_DOTTED, m.underline_y, m.stroke, colors.sp);
            }
            if attr.underdashed {
                deco(KIND_DASHED, m.underline_y, m.stroke, colors.sp);
            }
            if attr.strikethrough {
                deco(KIND_SOLID, m.strikeout_y, m.stroke, colors.fg);
            }
        }
        Ok(())
    }
}
