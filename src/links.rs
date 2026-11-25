#[derive(Debug, PartialEq, Eq)]
pub enum Link {
    Url(String),
    File {
        path: String,
        line: Option<u32>,
        col: Option<u32>,
    },
}

pub const OPEN: &str = r#"
local win, path, lnum, col = ...
lnum = lnum ~= vim.NIL and lnum or nil
col = col ~= vim.NIL and col or nil
if not vim.api.nvim_win_is_valid(win) then
  win = vim.api.nvim_get_current_win()
end
local buf = vim.api.nvim_win_get_buf(win)
if path:match('^%a:[\\/]') and vim.fn.has('wsl') == 1 then
  path = '/mnt/' .. path:sub(1, 1):lower() .. path:sub(3):gsub('\\', '/')
elseif path:sub(1, 2) == '~/' then
  path = vim.uv.os_homedir() .. path:sub(2)
end
local found
if path:sub(1, 1) == '/' or path:match('^%a:[\\/]') then
  found = vim.uv.fs_stat(path) and path
else
  local dirs = {}
  if vim.bo[buf].buftype == 'terminal' then
    local pid = vim.b[buf].terminal_job_pid
    dirs[#dirs + 1] = pid and vim.uv.fs_readlink('/proc/' .. pid .. '/cwd')
  end
  dirs[#dirs + 1] = vim.fn.getcwd(win)
  if vim.bo[buf].buftype == '' and vim.api.nvim_buf_get_name(buf) ~= '' then
    dirs[#dirs + 1] = vim.fn.fnamemodify(vim.api.nvim_buf_get_name(buf), ':p:h')
  end
  for _, dir in pairs(dirs) do
    if vim.uv.fs_stat(dir .. '/' .. path) then
      found = vim.fs.normalize(dir .. '/' .. path)
      break
    end
  end
end
if not found then
  vim.notify('nvright: no such file: ' .. path, vim.log.levels.WARN)
  return
end
local function plain(w)
  return vim.api.nvim_win_get_config(w).relative == '' and vim.bo[vim.api.nvim_win_get_buf(w)].buftype == ''
end
local target = plain(win) and win or nil
local prev = vim.fn.win_getid(vim.fn.winnr('#'))
if not target and prev ~= 0 and plain(prev) then
  target = prev
end
for _, w in ipairs(vim.api.nvim_tabpage_list_wins(0)) do
  target = target or (plain(w) and w or nil)
end
if target then
  vim.api.nvim_set_current_win(target)
else
  vim.api.nvim_set_current_win(win)
  vim.cmd(vim.api.nvim_win_get_config(win).relative == '' and 'aboveleft split' or 'tabnew')
end
vim.cmd.edit(vim.fn.fnameescape(found))
if lnum then
  local l = math.min(lnum, vim.api.nvim_buf_line_count(0))
  vim.api.nvim_win_set_cursor(0, { l, math.max((col or 1) - 1, 0) })
  vim.cmd('normal! zz')
end
"#;

pub fn open_url(url: &str) {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::{HSTRING, w};
        ShellExecuteW(None, w!("open"), &HSTRING::from(url), None, None, SW_SHOWNORMAL);
    }
    #[cfg(not(windows))]
    if let Err(err) = std::process::Command::new("xdg-open").arg(url).spawn() {
        log::warn!("xdg-open {url}: {err}");
    }
}

fn separator(s: &str) -> bool {
    s.chars().next().is_some_and(|c| {
        c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '<' | '>' | '|' | '(' | ')' | '[' | ']' | '{' | '}')
    })
}

fn trim_end(s: &str) -> &str {
    s.trim_end_matches(['.', ',', ';', ':', '!', '?'])
}

fn digits(s: &str) -> Option<u32> {
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

pub fn at(cells: &[String], col: usize) -> Option<Link> {
    if col >= cells.len() || separator(&cells[col]) {
        return None;
    }
    let start = (0..col).rev().find(|&i| separator(&cells[i])).map_or(0, |i| i + 1);
    let end = (col..cells.len()).find(|&i| separator(&cells[i])).unwrap_or(cells.len());
    let word: String = cells[start..end].concat();
    let rest: String = cells[end..].concat();
    let before: usize = cells[start..col].iter().map(String::len).sum();

    for scheme in ["https://", "http://"] {
        if let Some(i) = word.find(scheme) {
            let url = trim_end(&word[i..]);
            if before >= i && before < i + url.len() && url.len() > scheme.len() {
                return Some(Link::Url(url.to_string()));
            }
        }
    }

    let mut path = trim_end(word.strip_prefix("file://").unwrap_or(&word));
    let mut nums = Vec::new();
    while nums.len() < 2 {
        match path.rsplit_once(':') {
            Some((head, tail)) if digits(tail).is_some() => {
                nums.push(digits(tail));
                path = head;
            }
            _ => break,
        }
    }
    nums.reverse();
    if nums.is_empty()
        && let Some(inner) = rest.strip_prefix('(').and_then(|r| r.split_once(')')).map(|(i, _)| i)
    {
        let mut parts = inner.split(',').map(|p| digits(p.trim()));
        if let Some(Some(l)) = parts.next() {
            nums.push(Some(l));
            nums.push(parts.next().flatten());
        }
    }
    let looks_like_file = path.contains('/') || path.contains('\\') || !nums.is_empty();
    if path.is_empty() || !looks_like_file || path.contains("://") {
        return None;
    }
    Some(Link::File {
        path: path.to_string(),
        line: nums.first().copied().flatten(),
        col: nums.get(1).copied().flatten(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(s: &str) -> Vec<String> {
        s.chars().map(String::from).collect()
    }

    fn file(path: &str, line: Option<u32>, col: Option<u32>) -> Option<Link> {
        Some(Link::File { path: path.into(), line, col })
    }

    #[test]
    fn urls() {
        let c = cells("see https://example.com/a?b=1. ok");
        assert_eq!(at(&c, 10), Some(Link::Url("https://example.com/a?b=1".into())));
        assert_eq!(at(&c, 1), None);
        assert_eq!(at(&cells("(http://x.org/y)"), 5), Some(Link::Url("http://x.org/y".into())));
    }

    #[test]
    fn paths_with_lines() {
        let c = cells("  --> src/main.rs:12:5");
        assert_eq!(at(&c, 9), file("src/main.rs", Some(12), Some(5)));
        assert_eq!(at(&cells("a.ts:40: error"), 1), file("a.ts", Some(40), None));
        assert_eq!(at(&cells("app/a.component.ts(12,7): error"), 3), file("app/a.component.ts", Some(12), Some(7)));
        assert_eq!(at(&cells(r"C:\me\x.rs:3"), 4), file(r"C:\me\x.rs", Some(3), None));
        assert_eq!(at(&cells("~/notes/todo.md"), 4), file("~/notes/todo.md", None, None));
    }

    #[test]
    fn plain_words_are_not_links() {
        assert_eq!(at(&cells("hello world"), 2), None);
        assert_eq!(at(&cells("foo.bar"), 2), None);
    }
}
