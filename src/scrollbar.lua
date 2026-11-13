local chan = ...
local api = vim.api

local SEARCH, ERROR, WARN, INFO, ADD, CHANGE, DELETE = 1, 2, 3, 4, 5, 6, 7
local MAX_LINES = 20000
local MAX_MARKS = 2000

local function color(groups, field)
  for _, name in ipairs(groups) do
    local hl = api.nvim_get_hl(0, { name = name, link = false })
    if hl[field] then
      return hl[field]
    end
  end
  return 0x808080
end

local function send_colors()
  pcall(vim.rpcnotify, chan, 'nvright_marks', 'colors', {
    color({ 'Normal' }, 'fg'),
    color({ 'DiagnosticError' }, 'fg'),
    color({ 'DiagnosticWarn' }, 'fg'),
    color({ 'DiagnosticInfo' }, 'fg'),
    color({ 'GitSignsAdd', 'Added', 'DiffAdd' }, 'fg'),
    color({ 'GitSignsChange', 'Changed', 'DiffChange' }, 'fg'),
    color({ 'GitSignsDelete', 'Removed', 'DiffDelete' }, 'fg'),
  })
end

local function search(buf, out)
  local pat = vim.fn.getreg('/')
  if vim.v.hlsearch == 0 or pat == '' then
    return
  end
  local n = api.nvim_buf_line_count(buf)
  if n > MAX_LINES then
    return
  end
  local ignore = vim.o.ignorecase and not (vim.o.smartcase and pat:find('%u'))
  local ok, re = pcall(vim.regex, (ignore and '\\c' or '\\C') .. pat)
  if not ok then
    return
  end
  for i = 0, n - 1 do
    if re:match_line(buf, i) then
      out[#out + 1] = i
      out[#out + 1] = SEARCH
      if #out >= 2 * MAX_MARKS then
        return
      end
    end
  end
end

local function collect(buf)
  local out = {}
  for _, d in ipairs(vim.diagnostic.get(buf)) do
    out[#out + 1] = d.lnum
    out[#out + 1] = d.severity == 1 and ERROR or d.severity == 2 and WARN or INFO
  end
  local gs = package.loaded.gitsigns
  local ok, hunks = pcall(function()
    return gs and gs.get_hunks(buf)
  end)
  for _, h in ipairs(ok and hunks or {}) do
    local first = math.max(h.added.start - 1, 0)
    if h.type == 'delete' then
      out[#out + 1] = first
      out[#out + 1] = DELETE
    else
      for l = first, first + math.max(h.added.count, 1) - 1 do
        out[#out + 1] = l
        out[#out + 1] = h.type == 'add' and ADD or CHANGE
      end
    end
  end
  search(buf, out)
  return out
end

local sent = {}
local pending = false

local function update()
  pending = false
  local seen, cache = {}, {}
  for _, win in ipairs(api.nvim_tabpage_list_wins(0)) do
    if api.nvim_win_get_config(win).relative == '' then
      local buf = api.nvim_win_get_buf(win)
      cache[buf] = cache[buf] or collect(buf)
      local marks = cache[buf]
      local key = buf .. ':' .. table.concat(marks, ',')
      if sent[win] ~= key then
        sent[win] = key
        pcall(vim.rpcnotify, chan, 'nvright_marks', 'win', win, marks)
      end
      seen[win] = true
    end
  end
  for win in pairs(sent) do
    if not seen[win] then
      sent[win] = nil
    end
  end
end

local function schedule(delay)
  if not pending then
    pending = true
    vim.defer_fn(update, delay)
  end
end

local group = api.nvim_create_augroup('nvright_scrollbar', { clear = true })
api.nvim_create_autocmd(
  { 'DiagnosticChanged', 'BufWinEnter', 'WinNew', 'TabEnter', 'TextChanged', 'InsertLeave' },
  { group = group, callback = function() schedule(150) end }
)
api.nvim_create_autocmd('User', {
  group = group,
  pattern = 'GitSignsUpdate',
  callback = function() schedule(50) end,
})
local last = ''
api.nvim_create_autocmd({ 'CursorMoved', 'CmdlineLeave' }, {
  group = group,
  callback = function()
    local now = vim.v.hlsearch .. vim.fn.getreg('/')
    if now ~= last then
      last = now
      schedule(30)
    end
  end,
})
api.nvim_create_autocmd({ 'ColorScheme', 'VimEnter' }, {
  group = group,
  callback = function()
    send_colors()
    schedule(0)
  end,
})
send_colors()
