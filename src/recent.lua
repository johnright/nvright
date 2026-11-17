local chan = ...
local distro = vim.env.WSL_DISTRO_NAME or ''
local home = vim.uv.os_homedir()

local function skip(path)
  return path == '' or path == home or path:find('^/tmp/') or path:find('/%.git/')
end

local function send(kind, path)
  if not skip(path) then
    pcall(vim.rpcnotify, chan, 'nvright_recent', kind, path, vim.fn.getcwd(), distro)
  end
end

local group = vim.api.nvim_create_augroup('nvright_recent', { clear = true })
local function file(buf)
  local name = vim.api.nvim_buf_get_name(buf)
  if vim.bo[buf].buftype == '' and name ~= '' then
    send('file', vim.fn.fnamemodify(name, ':p'))
  end
end
local function dir()
  send('dir', vim.fn.getcwd())
end
vim.api.nvim_create_autocmd('BufReadPost', {
  group = group,
  callback = function(ev)
    file(ev.buf)
  end,
})
vim.api.nvim_create_autocmd('DirChanged', { group = group, callback = dir })
if vim.v.vim_did_enter == 1 then
  dir()
  file(0)
else
  vim.api.nvim_create_autocmd('VimEnter', {
    group = group,
    once = true,
    callback = function()
      dir()
      file(0)
    end,
  })
end
