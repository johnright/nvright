local chan = ...
if not (vim.ui and vim.ui.img) then
  return
end

local state = {}
local last = 0

local M = {}

function M.set(data_or_id, opts)
  opts = opts or {}
  vim.validate('data_or_id', data_or_id, { 'string', 'number' })
  vim.validate('opts', opts, 'table')
  if type(data_or_id) == 'string' then
    last = last + 1
    state[last] = vim.deepcopy(opts)
    vim.rpcnotify(chan, 'nvright_img', 'set', last, opts, data_or_id)
    return last
  end
  local id = data_or_id
  assert(state[id], 'invalid image id: ' .. tostring(id))
  state[id] = vim.tbl_extend('force', state[id], opts)
  vim.rpcnotify(chan, 'nvright_img', 'set', id, state[id])
  return id
end

function M.get(id)
  vim.validate('id', id, 'number')
  return state[id] and vim.deepcopy(state[id])
end

function M.del(id)
  vim.validate('id', id, 'number')
  if id == math.huge then
    local any = next(state) ~= nil
    state = {}
    if any then
      vim.rpcnotify(chan, 'nvright_img', 'del', -1)
    end
    return any
  end
  if not state[id] then
    return false
  end
  state[id] = nil
  vim.rpcnotify(chan, 'nvright_img', 'del', id)
  return true
end

vim.ui.img = M
package.loaded['vim.ui.img'] = M
