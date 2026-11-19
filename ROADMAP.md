<p align="center"><img src="assets/icon-256.png" alt="" width="64"></p>

<h1 align="center">Roadmap</h1>

<p align="center">What I'd like to do next, roughly in order.<br>
Nothing here is a promise, and anything that makes typing slower or text worse won't go in.</p>

## Soon

- [x] **Box drawing** – borders and block characters drawn by the GUI, so they
  stay joined with any `linespace`
- [x] **Images in Neovim** – snacks.image through kitty graphics placeholders,
  no ImageMagick
- [x] **Vulkan by default** – about half the memory of DX12 on my machine
- [x] **Images in markdown** – paste a screenshot or drop an image file and get
  a link
- [x] **New name: nvright** – right_editor said nothing about Neovim
- [x] **Releases** – a prebuilt `.exe` on GitHub Releases and `scoop install nvright`,
  no cross-compiling toolchain needed
- [x] **Scrollbar** – thin, drawn over the text while you scroll, with marks for
  search matches, diagnostics and git changes. You can drag it
- [x] **Floating window animation** – pickers, completion and hover fade in
  instead of just appearing
- [x] **Taskbar jump list** – recent files and folders on right click
- [x] **Any WSL distro** – a new window in Arch, Ubuntu or whatever else is
  installed, from a shortcut or the taskbar

## Later

- [ ] Ctrl+click on URLs and `file:line` paths, `:terminal` output too
- [ ] hollow cursor when the window isn't focused, blinking from `guicursor`
- [ ] startup errors shown in the window (the release build has no console)
- [ ] dropping a folder does `:cd`, Ctrl while dropping opens a split
- [ ] Windows paths as command line arguments
- [ ] IME candidate window and emoji panel at the cursor

## Maybe

- [ ] PDF preview with SyncTeX, for LaTeX and Typst
- [ ] paste an image as text, with the OCR that ships with Windows

## Done

- [x] ClearType text through DirectWrite, ligatures, color emoji
- [x] cursor trail, smooth scrolling, fade-in of typed text
- [x] ext_multigrid with shadows under floats, `winblend`, backdrop
- [x] markdown preview with images, tables, highlighted code
- [x] translucent window with acrylic blur
- [x] `:restart`
- [x] images from `vim.ui.img` (Neovim 0.13)
- [x] Neovim 0.13 multicursors drawn as real cursors

## Not planned

Native Linux/macOS builds (it compiles on Linux, but DirectWrite, DWM and WSL
are the whole point). Tabs, file tree, command palette: plugins do these
better. A minimap.
