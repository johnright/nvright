pub const LUA: &str = r#"
local chan = ...
local function spawn(distro)
  pcall(vim.rpcnotify, chan, 'nvright_spawn', distro)
end
_G.nvright_new = function(distro)
  local list = vim.g.nvright_distros or {}
  if distro and distro ~= '' then
    return spawn(distro)
  end
  if #list <= 1 then
    return spawn(list[1] or '')
  end
  vim.ui.select(list, { prompt = 'New window in' }, function(choice)
    if choice then
      spawn(choice)
    end
  end)
end
vim.api.nvim_create_user_command('NvrightNew', function(o)
  _G.nvright_new(o.args)
end, {
  nargs = '?',
  desc = 'nvright: new window in a WSL distro',
  complete = function()
    return vim.g.nvright_distros or {}
  end,
})
"#;

pub const NEW: &str = "_G.nvright_new()";

pub fn installed() -> Vec<String> {
    #[cfg(windows)]
    return registry();
    #[cfg(not(windows))]
    Vec::new()
}

pub fn spawn(distro: &str) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut cmd = std::process::Command::new(exe);
    if !distro.is_empty() {
        cmd.args(["--distro", distro, "--cwd", "~"]);
    }
    if let Err(err) = cmd.spawn() {
        log::warn!("new window in {distro}: {err}");
    }
}

#[cfg(windows)]
fn registry() -> Vec<String> {
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW,
        RegGetValueW, RegOpenKeyExW,
    };
    use windows::core::{PCWSTR, PWSTR, w};
    let mut out = Vec::new();
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Lxss"),
            None,
            KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return out;
        }
        for i in 0.. {
            let mut sub = [0u16; 128];
            let mut len = sub.len() as u32;
            if RegEnumKeyExW(key, i, Some(PWSTR(sub.as_mut_ptr())), &mut len, None, None, None, None)
                .is_err()
            {
                break;
            }
            let mut name = [0u16; 256];
            let mut size = (name.len() * 2) as u32;
            if RegGetValueW(
                key,
                PCWSTR(sub.as_ptr()),
                w!("DistributionName"),
                RRF_RT_REG_SZ,
                None,
                Some(name.as_mut_ptr().cast()),
                Some(&mut size),
            )
            .is_ok()
            {
                let n = name.iter().position(|&c| c == 0).unwrap_or(0);
                let distro = String::from_utf16_lossy(&name[..n]);
                if !distro.starts_with("docker-desktop") {
                    out.push(distro);
                }
            }
        }
        let _ = RegCloseKey(key);
    }
    out
}
