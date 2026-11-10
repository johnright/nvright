#![allow(dead_code)]

use std::collections::HashMap;

pub const PLACEHOLDER: char = '\u{10EEEE}';

const MAX_PENDING: usize = 64 << 20;

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
}
