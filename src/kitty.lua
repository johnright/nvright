local chan = ...
local distro = vim.env.WSL_DISTRO_NAME or ''

vim.env.SNACKS_KITTY = '1'

local function size()
  local cell = vim.g.nvright_cell or { 9, 18 }
  local cw, ch = cell[1], cell[2]
  return {
    width = vim.o.columns * cw,
    height = vim.o.lines * ch,
    columns = vim.o.columns,
    rows = vim.o.lines,
    cell_width = cw,
    cell_height = ch,
    scale = math.max(1, cw / 8),
  }
end

local function patch(m)
  m.size = size
  m.write = function(data)
    vim.rpcnotify(chan, 'nvright_kitty', 'data', data)
  end
  m._terminal = { terminal = 'nvright', version = '' }
end

local decoded = { png = true, jpg = true, jpeg = true, gif = true, webp = true, svg = true }

local function patch_convert(m)
  local convert = m.convert
  m.convert = function(opts)
    local c = convert(opts)
    local ft = vim.fn.fnamemodify(c.src, ':e'):lower()
    if m.is_uri(c.src) or not decoded[ft] then
      return c
    end
    local ok, size = pcall(vim.rpcrequest, chan, 'nvright_image_size', c.src, distro)
    if not ok or type(size) ~= 'table' then
      return c
    end
    c.steps = {}
    c.file = c.src
    c.meta.info = {
      format = ft,
      size = { width = size[1], height = size[2] },
      dpi = { width = 72, height = 72 },
    }
    return c
  end
end

local function hook(name, fn)
  if package.loaded[name] then
    fn(package.loaded[name])
    return
  end
  package.preload[name] = function()
    package.preload[name] = nil
    local path = 'lua/' .. name:gsub('%.', '/') .. '.lua'
    local file = vim.api.nvim_get_runtime_file(path, false)[1]
    local m = assert(loadfile(file))(name)
    fn(m)
    return m
  end
end

hook('snacks.image.terminal', patch)
hook('snacks.image.convert', patch_convert)

vim.rpcnotify(chan, 'nvright_kitty', 'distro', distro)
