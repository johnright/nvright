use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};

use image::RgbaImage;
use image::imageops::FilterType;

pub const LUA: &str = include_str!("kitty.lua");

pub const PLACEHOLDER: char = '\u{10EEEE}';

const MAX_PENDING: usize = 64 << 20;
const MAX_FIT: u32 = 2000;

const DIACRITICS: [u32; 297] = [
    0x0305, 0x030D, 0x030E, 0x0310, 0x0312, 0x033D, 0x033E, 0x033F, 0x0346, 0x034A,
    0x034B, 0x034C, 0x0350, 0x0351, 0x0352, 0x0357, 0x035B, 0x0363, 0x0364, 0x0365,
    0x0366, 0x0367, 0x0368, 0x0369, 0x036A, 0x036B, 0x036C, 0x036D, 0x036E, 0x036F,
    0x0483, 0x0484, 0x0485, 0x0486, 0x0487, 0x0592, 0x0593, 0x0594, 0x0595, 0x0597,
    0x0598, 0x0599, 0x059C, 0x059D, 0x059E, 0x059F, 0x05A0, 0x05A1, 0x05A8, 0x05A9,
    0x05AB, 0x05AC, 0x05AF, 0x05C4, 0x0610, 0x0611, 0x0612, 0x0613, 0x0614, 0x0615,
    0x0616, 0x0617, 0x0657, 0x0658, 0x0659, 0x065A, 0x065B, 0x065D, 0x065E, 0x06D6,
    0x06D7, 0x06D8, 0x06D9, 0x06DA, 0x06DB, 0x06DC, 0x06DF, 0x06E0, 0x06E1, 0x06E2,
    0x06E4, 0x06E7, 0x06E8, 0x06EB, 0x06EC, 0x0730, 0x0732, 0x0733, 0x0735, 0x0736,
    0x073A, 0x073D, 0x073F, 0x0740, 0x0741, 0x0743, 0x0745, 0x0747, 0x0749, 0x074A,
    0x07EB, 0x07EC, 0x07ED, 0x07EE, 0x07EF, 0x07F0, 0x07F1, 0x07F3, 0x0816, 0x0817,
    0x0818, 0x0819, 0x081B, 0x081C, 0x081D, 0x081E, 0x081F, 0x0820, 0x0821, 0x0822,
    0x0823, 0x0825, 0x0826, 0x0827, 0x0829, 0x082A, 0x082B, 0x082C, 0x082D, 0x0951,
    0x0953, 0x0954, 0x0F82, 0x0F83, 0x0F86, 0x0F87, 0x135D, 0x135E, 0x135F, 0x17DD,
    0x193A, 0x1A17, 0x1A75, 0x1A76, 0x1A77, 0x1A78, 0x1A79, 0x1A7A, 0x1A7B, 0x1A7C,
    0x1B6B, 0x1B6D, 0x1B6E, 0x1B6F, 0x1B70, 0x1B71, 0x1B72, 0x1B73, 0x1CD0, 0x1CD1,
    0x1CD2, 0x1CDA, 0x1CDB, 0x1CE0, 0x1DC0, 0x1DC1, 0x1DC3, 0x1DC4, 0x1DC5, 0x1DC6,
    0x1DC7, 0x1DC8, 0x1DC9, 0x1DCB, 0x1DCC, 0x1DD1, 0x1DD2, 0x1DD3, 0x1DD4, 0x1DD5,
    0x1DD6, 0x1DD7, 0x1DD8, 0x1DD9, 0x1DDA, 0x1DDB, 0x1DDC, 0x1DDD, 0x1DDE, 0x1DDF,
    0x1DE0, 0x1DE1, 0x1DE2, 0x1DE3, 0x1DE4, 0x1DE5, 0x1DE6, 0x1DFE, 0x20D0, 0x20D1,
    0x20D4, 0x20D5, 0x20D6, 0x20D7, 0x20DB, 0x20DC, 0x20E1, 0x20E7, 0x20E9, 0x20F0,
    0x2CEF, 0x2CF0, 0x2CF1, 0x2DE0, 0x2DE1, 0x2DE2, 0x2DE3, 0x2DE4, 0x2DE5, 0x2DE6,
    0x2DE7, 0x2DE8, 0x2DE9, 0x2DEA, 0x2DEB, 0x2DEC, 0x2DED, 0x2DEE, 0x2DEF, 0x2DF0,
    0x2DF1, 0x2DF2, 0x2DF3, 0x2DF4, 0x2DF5, 0x2DF6, 0x2DF7, 0x2DF8, 0x2DF9, 0x2DFA,
    0x2DFB, 0x2DFC, 0x2DFD, 0x2DFE, 0x2DFF, 0xA66F, 0xA67C, 0xA67D, 0xA6F0, 0xA6F1,
    0xA8E0, 0xA8E1, 0xA8E2, 0xA8E3, 0xA8E4, 0xA8E5, 0xA8E6, 0xA8E7, 0xA8E8, 0xA8E9,
    0xA8EA, 0xA8EB, 0xA8EC, 0xA8ED, 0xA8EE, 0xA8EF, 0xA8F0, 0xA8F1, 0xAAB0, 0xAAB2,
    0xAAB3, 0xAAB7, 0xAAB8, 0xAABE, 0xAABF, 0xAAC1, 0xFE20, 0xFE21, 0xFE22, 0xFE23,
    0xFE24, 0xFE25, 0xFE26, 0x10A0F, 0x10A38, 0x1D185, 0x1D186, 0x1D187, 0x1D188, 0x1D189,
    0x1D1AA, 0x1D1AB, 0x1D1AC, 0x1D1AD, 0x1D242, 0x1D243, 0x1D244,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Placeholder {
    pub row: Option<u16>,
    pub col: Option<u16>,
    pub id_msb: Option<u8>,
}

pub fn parse_placeholder(s: &str) -> Option<Placeholder> {
    let mut chars = s.chars();
    if chars.next()? != PLACEHOLDER {
        return None;
    }
    let mut marks = chars.map(|c| DIACRITICS.binary_search(&(c as u32)).ok());
    let mut next = || marks.next().flatten();
    Some(Placeholder {
        row: next().map(|i| i as u16),
        col: next().map(|i| i as u16),
        id_msb: next().map(|i| i as u8),
    })
}

#[derive(Debug, Default, PartialEq)]
pub struct Command {
    keys: Vec<(u8, String)>,
    payload: Vec<u8>,
}

impl Command {
    fn parse(body: &[u8]) -> Self {
        let (control, payload) = match body.iter().position(|&b| b == b';') {
            Some(i) => (&body[..i], &body[i + 1..]),
            None => (body, &[][..]),
        };
        let keys = control
            .split(|&b| b == b',')
            .filter_map(|kv| {
                let (&key, rest) = kv.split_first()?;
                let value = rest.strip_prefix(b"=")?;
                Some((key, String::from_utf8_lossy(value).into_owned()))
            })
            .collect();
        Self {
            keys,
            payload: payload.to_vec(),
        }
    }

    fn get(&self, key: u8) -> Option<&str> {
        self.keys
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
    }

    fn num(&self, key: u8) -> Option<u32> {
        self.get(key)?.parse().ok()
    }

    fn is_continuation(&self) -> bool {
        self.keys.iter().all(|(k, _)| matches!(k, b'm' | b'q'))
    }
}

#[derive(Debug, Default)]
pub struct Stream {
    buf: Vec<u8>,
}

impl Stream {
    pub fn push(&mut self, data: &[u8], out: &mut Vec<Command>) {
        self.buf.extend_from_slice(data);
        let consumed = Self::scan(&self.buf, out);
        self.buf.drain(..consumed);
        if self.buf.len() > MAX_PENDING {
            log::warn!("kitty: dropping {} bytes of an unfinished sequence", self.buf.len());
            self.buf.clear();
        }
    }

    fn scan(buf: &[u8], out: &mut Vec<Command>) -> usize {
        let mut i = 0;
        while let Some(esc) = buf[i..].iter().position(|&b| b == 0x1b) {
            let start = i + esc;
            let rest = &buf[start..];
            let Some(&kind) = rest.get(1) else {
                return start;
            };
            match kind {
                b'_' => {
                    let Some(end) = rest[2..].windows(2).position(|w| w == b"\x1b\\") else {
                        return start;
                    };
                    let body = &rest[2..2 + end];
                    if let Some(body) = body.strip_prefix(b"G") {
                        out.push(Command::parse(body));
                    }
                    i = start + 2 + end + 2;
                }
                b'[' => {
                    let Some(end) = rest[2..].iter().position(|b| (0x40..=0x7e).contains(b)) else {
                        return start;
                    };
                    i = start + 2 + end + 1;
                }
                _ => i = start + 2,
            }
        }
        buf.len()
    }
}

#[derive(Debug, Default)]
pub struct Placements {
    map: HashMap<(u32, u32), (u16, u16)>,
}

impl Placements {
    pub fn get(&self, image: u32, placement: u32) -> Option<(u16, u16)> {
        self.map.get(&(image, placement)).copied().or_else(|| {
            self.map
                .iter()
                .find(|((i, _), _)| *i == image)
                .map(|(_, &size)| size)
        })
    }
}

#[derive(Debug)]
pub struct Fitted {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub type FitKey = (u32, u32, u32);

#[derive(Debug, Default)]
pub struct Outcome {
    pub forget: Vec<u32>,
    pub placements_changed: bool,
}

enum Source {
    File(PathBuf),
    Data(Vec<u8>),
}

enum Job {
    Load(u32, Source),
    Fit(FitKey),
}

pub struct Kitty {
    stream: Stream,
    jobs: Sender<Job>,
    chunks: Option<(u32, Vec<u8>)>,
    distro: String,
}

impl Kitty {
    pub fn new(sink: impl Fn(FitKey, Arc<Fitted>) + Send + 'static) -> Self {
        Self {
            stream: Stream::default(),
            jobs: spawn_worker(sink),
            chunks: None,
            distro: String::new(),
        }
    }

    pub fn set_distro(&mut self, distro: String) {
        self.distro = distro;
    }

    pub fn fit(&self, key: FitKey) {
        let _ = self.jobs.send(Job::Fit(key));
    }

    pub fn feed(&mut self, data: &[u8], placements: &mut Placements) -> Outcome {
        let mut commands = Vec::new();
        self.stream.push(data, &mut commands);
        let mut outcome = Outcome::default();
        for cmd in commands {
            self.command(cmd, placements, &mut outcome);
        }
        outcome
    }

    fn command(&mut self, cmd: Command, placements: &mut Placements, outcome: &mut Outcome) {
        if cmd.is_continuation() {
            if let Some((_, data)) = &mut self.chunks {
                data.extend_from_slice(&cmd.payload);
                if cmd.num(b'm') != Some(1) {
                    let (id, data) = self.chunks.take().unwrap();
                    self.load(id, Source::Data(base64(&data)), outcome);
                }
            }
            return;
        }
        let id = cmd.num(b'i').unwrap_or(0);
        match cmd.get(b'a').unwrap_or("t") {
            action @ ("t" | "T") => {
                match cmd.get(b't').unwrap_or("d") {
                    "f" | "t" => {
                        let path = String::from_utf8_lossy(&base64(&cmd.payload)).into_owned();
                        let path = self.host_path(&path);
                        self.load(id, Source::File(path), outcome);
                    }
                    "d" if cmd.num(b'm') == Some(1) => {
                        self.chunks = Some((id, cmd.payload.clone()));
                    }
                    "d" => self.load(id, Source::Data(base64(&cmd.payload)), outcome),
                    other => log::debug!("kitty: transmission medium {other} not supported"),
                }
                if action == "T" {
                    self.place(id, &cmd, placements, outcome);
                }
            }
            "p" => self.place(id, &cmd, placements, outcome),
            "d" => {
                match (cmd.get(b'd').unwrap_or("a"), cmd.num(b'p')) {
                    ("i" | "I", Some(p)) => {
                        placements.map.remove(&(id, p));
                    }
                    ("i" | "I", None) => placements.map.retain(|&(i, _), _| i != id),
                    ("a" | "A", _) => placements.map.clear(),
                    (other, _) => log::debug!("kitty: delete {other} not supported"),
                }
                outcome.placements_changed = true;
            }
            other => log::debug!("kitty: action {other} not supported"),
        }
    }

    fn place(&self, id: u32, cmd: &Command, placements: &mut Placements, outcome: &mut Outcome) {
        if cmd.num(b'U') != Some(1) {
            log::debug!("kitty: only virtual placements are supported");
            return;
        }
        let (Some(cols), Some(rows)) = (cmd.num(b'c'), cmd.num(b'r')) else {
            log::debug!("kitty: placement without a size");
            return;
        };
        let p = cmd.num(b'p').unwrap_or(0);
        placements.map.insert((id, p), (cols as u16, rows as u16));
        outcome.placements_changed = true;
    }

    fn load(&self, id: u32, source: Source, outcome: &mut Outcome) {
        outcome.forget.push(id);
        let _ = self.jobs.send(Job::Load(id, source));
    }

    fn host_path(&self, linux: &str) -> PathBuf {
        host_path(&self.distro, linux)
    }
}

pub fn host_path(distro: &str, linux: &str) -> PathBuf {
    if distro.is_empty() {
        PathBuf::from(linux)
    } else {
        PathBuf::from(format!(
            r"\\wsl.localhost\{distro}{}",
            linux.replace('/', "\\")
        ))
    }
}

pub fn dimensions(path: &Path) -> Result<(u32, u32), String> {
    let is_svg = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    if is_svg {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        return crate::preview::decode_image(&bytes, 1, 1.0, None).map(|img| img.natural);
    }
    image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?
        .into_dimensions()
        .map_err(|e| e.to_string())
}

fn spawn_worker(sink: impl Fn(FitKey, Arc<Fitted>) + Send + 'static) -> Sender<Job> {
    let (tx, rx) = channel::<Job>();
    let spawned = std::thread::Builder::new()
        .name("kitty images".into())
        .spawn(move || {
            let mut images: HashMap<u32, RgbaImage> = HashMap::new();
            while let Ok(job) = rx.recv() {
                match job {
                    Job::Load(id, source) => match decode(source) {
                        Ok(image) => {
                            log::debug!("kitty: image {id} {}x{}", image.width(), image.height());
                            images.insert(id, image);
                        }
                        Err(err) => log::warn!("kitty: image {id}: {err}"),
                    },
                    Job::Fit(key) => {
                        if let Some(image) = images.get(&key.0)
                            && let Some(fitted) = fit(image, key.1, key.2)
                        {
                            sink(key, Arc::new(fitted));
                        }
                    }
                }
            }
        });
    if let Err(err) = spawned {
        log::error!("kitty: image thread not started: {err}");
    }
    tx
}

fn decode(source: Source) -> Result<RgbaImage, String> {
    let bytes = match source {
        Source::File(path) => std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?,
        Source::Data(bytes) => bytes,
    };
    let image = crate::preview::decode_image(&bytes, 0, 2.0, None)?;
    RgbaImage::from_raw(image.width, image.height, image.rgba)
        .ok_or_else(|| "bad image buffer".to_string())
}

fn fit(image: &RgbaImage, box_w: u32, box_h: u32) -> Option<Fitted> {
    if box_w == 0 || box_h == 0 || box_w > MAX_FIT || box_h > MAX_FIT {
        log::warn!("kitty: {box_w}x{box_h} is too large to draw");
        return None;
    }
    let (iw, ih) = (image.width() as f32, image.height() as f32);
    let scale = (box_w as f32 / iw).min(box_h as f32 / ih);
    let width = ((iw * scale).round() as u32).clamp(1, box_w);
    let height = ((ih * scale).round() as u32).clamp(1, box_h);
    let scaled = if (width, height) == image.dimensions() {
        image.clone()
    } else {
        image::imageops::resize(image, width, height, FilterType::CatmullRom)
    };
    Some(Fitted {
        x: (box_w - width) / 2,
        y: (box_h - height) / 2,
        width,
        height,
        rgba: scaled.into_raw(),
    })
}

fn base64(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for &b in input {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_with_row_and_col() {
        let s = format!("{PLACEHOLDER}\u{030D}\u{0305}");
        let p = parse_placeholder(&s).unwrap();
        assert_eq!((p.row, p.col, p.id_msb), (Some(1), Some(0), None));
        assert!(parse_placeholder("a").is_none());
    }

    #[test]
    fn stream_splits_sequences() {
        let mut stream = Stream::default();
        let mut out = Vec::new();
        stream.push(b"\x1b[3;4H\x1b_Ga=p,U=1,i=7,c=3", &mut out);
        assert!(out.is_empty());
        stream.push(b",r=2\x1b\\", &mut out);
        let cmd = &out[0];
        assert_eq!((cmd.get(b'a'), cmd.num(b'i'), cmd.num(b'r')), (Some("p"), Some(7), Some(2)));
    }

    #[test]
    fn base64_decodes() {
        assert_eq!(base64(b"L3RtcC9hLnBuZw=="), b"/tmp/a.png");
    }

    #[test]
    fn fit_centers() {
        let image = RgbaImage::new(100, 50);
        let f = fit(&image, 40, 40).unwrap();
        assert_eq!((f.x, f.y, f.width, f.height), (0, 10, 40, 20));
    }
}
