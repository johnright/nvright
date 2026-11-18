use std::collections::BTreeMap;

use rmpv::Value;

pub const LUA: &str = include_str!("ui_img.lua");

const MAX_BOX: u32 = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opts {
    pub row: i64,
    pub col: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub zindex: i64,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            row: 1,
            col: 1,
            width: None,
            height: None,
            zindex: 0,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Event {
    Set {
        id: u32,
        opts: Opts,
        data: Option<Vec<u8>>,
    },
    Del(Option<u32>),
}

pub fn parse_notification(args: Vec<Value>) -> Option<Event> {
    let mut args = args.into_iter();
    let op = args.next()?;
    let id = args.next()?.as_i64()?;
    match op.as_str()? {
        "del" => Some(Event::Del((id >= 0).then_some(id as u32))),
        "set" => {
            let opts = parse_opts(args.next());
            let data = match args.next() {
                Some(Value::String(s)) => Some(s.into_bytes()),
                Some(Value::Binary(b)) => Some(b),
                _ => None,
            };
            Some(Event::Set {
                id: id as u32,
                opts,
                data,
            })
        }
        _ => None,
    }
}

fn parse_opts(v: Option<Value>) -> Opts {
    let mut opts = Opts::default();
    let Some(Value::Map(map)) = v else {
        return opts;
    };
    for (k, v) in map {
        let n = v.as_i64();
        match k.as_str() {
            Some("row") => opts.row = n.unwrap_or(1),
            Some("col") => opts.col = n.unwrap_or(1),
            Some("width") => opts.width = n.filter(|&n| n > 0).map(|n| n as u32),
            Some("height") => opts.height = n.filter(|&n| n > 0).map(|n| n as u32),
            Some("zindex") => opts.zindex = n.unwrap_or(0),
            _ => {}
        }
    }
    opts
}

pub fn image_key(id: u32) -> u32 {
    0xff00_0000 | (id & 0x00ff_ffff)
}

pub fn natural_size(data: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(data))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

#[derive(Debug, Clone, Copy)]
pub struct UiImage {
    pub key: u32,
    pub opts: Opts,
    pub natural: Option<(u32, u32)>,
}

impl UiImage {
    pub fn box_px(&self, cell: (f32, f32)) -> Option<(u32, u32)> {
        let (cw, ch) = cell;
        let aspect = self.natural.map(|(w, h)| h as f32 / w.max(1) as f32);
        let (w, h) = match (self.opts.width, self.opts.height) {
            (Some(w), Some(h)) => (w as f32 * cw, h as f32 * ch),
            (Some(w), None) => {
                let w = w as f32 * cw;
                (w, w * aspect?)
            }
            (None, Some(h)) => {
                let h = h as f32 * ch;
                (h / aspect?, h)
            }
            (None, None) => {
                let (w, h) = self.natural?;
                (w as f32, h as f32)
            }
        };
        let shrink = (MAX_BOX as f32 / w.max(h)).min(1.0);
        let (w, h) = ((w * shrink).round() as u32, (h * shrink).round() as u32);
        (w > 0 && h > 0).then_some((w, h))
    }
}

pub type UiImages = BTreeMap<u32, UiImage>;

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, i64)]) -> Value {
        Value::Map(
            pairs
                .iter()
                .map(|(k, v)| (Value::from(*k), Value::from(*v)))
                .collect(),
        )
    }

    #[test]
    fn parses_set_and_del() {
        let args = vec![
            "set".into(),
            3.into(),
            map(&[("row", 5), ("col", 10), ("width", 40), ("zindex", 50)]),
            Value::Binary(vec![1, 2, 3]),
        ];
        assert_eq!(
            parse_notification(args),
            Some(Event::Set {
                id: 3,
                opts: Opts {
                    row: 5,
                    col: 10,
                    width: Some(40),
                    height: None,
                    zindex: 50
                },
                data: Some(vec![1, 2, 3]),
            })
        );
        let update = vec!["set".into(), 3.into(), Value::Array(vec![])];
        assert_eq!(
            parse_notification(update),
            Some(Event::Set {
                id: 3,
                opts: Opts::default(),
                data: None
            })
        );
        assert_eq!(
            parse_notification(vec!["del".into(), (-1).into()]),
            Some(Event::Del(None))
        );
        assert_eq!(
            parse_notification(vec!["del".into(), 7.into()]),
            Some(Event::Del(Some(7)))
        );
    }

    #[test]
    fn box_sizes() {
        let img = |width, height| UiImage {
            key: 0,
            opts: Opts {
                width,
                height,
                ..Opts::default()
            },
            natural: Some((200, 100)),
        };
        let cell = (10.0, 20.0);
        assert_eq!(img(Some(4), Some(3)).box_px(cell), Some((40, 60)));
        assert_eq!(img(Some(4), None).box_px(cell), Some((40, 20)));
        assert_eq!(img(None, Some(2)).box_px(cell), Some((80, 40)));
        assert_eq!(img(None, None).box_px(cell), Some((200, 100)));
        assert_eq!(img(Some(1000), None).box_px(cell), Some((2000, 1000)));
        let unknown = UiImage {
            natural: None,
            ..img(None, None)
        };
        assert_eq!(unknown.box_px(cell), None);
    }
}
