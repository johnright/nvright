local chan = ...
if vim.fn.has('nvim-0.13') == 0 then
  return
end

local ns = vim.api.nvim_create_namespace('nvim.multicursor')
local pending = false
local last = ''

local function hide_hl()
  vim.api.nvim_set_hl(0, 'MCursor', {})
end

local function screen(win, row, col)
  local p = vim.fn.screenpos(win, row + 1, col + 1)
  if p.row > 0 then
    return p.row, p.col
  end
  local line = vim.api.nvim_buf_get_lines(vim.api.nvim_win_get_buf(win), row, row + 1, false)[1] or ''
  if col < #line or #line == 0 then
    return nil
  end
  local q = vim.fn.screenpos(win, row + 1, #line)
  if q.row == 0 then
    return nil
  end
  return q.row, q.endcol + 1
end

local function send()
  pending = false
  local win = vim.api.nvim_get_current_win()
  local buf = vim.api.nvim_win_get_buf(win)
  local cur = vim.api.nvim_win_get_cursor(win)
  local out = {}
  for _, m in ipairs(vim.api.nvim_buf_get_extmarks(buf, ns, 0, -1, {})) do
    if not (m[2] + 1 == cur[1] and m[3] == cur[2]) then
      local r, c = screen(win, m[2], m[3])
      if r then
        out[#out + 1] = r
        out[#out + 1] = c
      end
    end
  end
  local key = table.concat(out, ',')
  if key ~= last then
    last = key
    vim.rpcnotify(chan, 'nvright_mcursors', out)
  end
end

local function later()
  if not pending then
    pending = true
    vim.schedule(send)
  end
end

hide_hl()
local group = vim.api.nvim_create_augroup('nvright_mcursors', { clear = true })
vim.api.nvim_create_autocmd('ColorScheme', { group = group, callback = hide_hl })
vim.api.nvim_create_autocmd({
  'CmdAtom',
  'CursorMoved',
  'CursorMovedI',
  'TextChanged',
  'TextChangedI',
  'ModeChanged',
  'WinScrolled',
  'WinEnter',
  'BufEnter',
  'VimResized',
}, { group = group, callback = later })
