use std::process::Stdio;

use tokio::process::Command;

use crate::config::NvimLaunch;

pub fn command(launch: &NvimLaunch) -> Command {
    let cmd = if cfg!(windows) && !launch.no_wsl {
        wsl_command(launch)
    } else {
        native_command(launch)
    };
    finish(cmd)
}

fn wsl_command(launch: &NvimLaunch) -> Command {
    let mut cmd = Command::new(&launch.wsl);
    if let Some(distro) = &launch.distro {
        cmd.args(["-d", distro]);
    }
    if let Some(cwd) = &launch.cwd {
        cmd.args(["--cd", cwd]);
    }
    cmd.arg("-e").arg(program(launch));
    nvim_args(&mut cmd, launch);
    cmd
}

fn native_command(launch: &NvimLaunch) -> Command {
    let mut cmd = Command::new(program(launch));
    nvim_args(&mut cmd, launch);
    if let Some(cwd) = &launch.cwd {
        cmd.current_dir(cwd);
    }
    cmd
}

fn program(launch: &NvimLaunch) -> &str {
    launch.shell.as_deref().unwrap_or(&launch.nvim)
}

fn nvim_args(cmd: &mut Command, launch: &NvimLaunch) {
    if launch.shell.is_some() {
        cmd.args(["-lc", r#"exec "$0" --embed "$@""#])
            .arg(&launch.nvim);
    } else {
        cmd.arg("--embed");
    }
    cmd.args(["--cmd", GUI_SETUP]);
    cmd.args(&launch.args);
}

const GUI_SETUP: &str = "let g:right_editor = 1 | set title";

const BRIDGE: &str = "local uv = vim.uv; os.remove(arg[0]); \
local s, i, o = uv.new_pipe(false), uv.new_pipe(false), uv.new_pipe(false); \
i:open(0); o:open(1); local done = false; \
s:connect(arg[1], function(err) \
if err then io.stderr:write('connect: ' .. err .. '\\n'); done = true; return end; \
s:read_start(function(_, d) if d then o:write(d) else done = true end end); \
i:read_start(function(_, d) if d then s:write(d) else done = true end end) end); \
while not done do vim.wait(1000, function() return done end) end";

const BRIDGE_SH: &str =
    r#"f=$(mktemp) && printf %s "$0" > "$f" && exec "$1" --clean --headless -l "$f" "$2""#;

pub fn bridge_command(launch: &NvimLaunch, addr: &str) -> Option<Command> {
    let mut cmd = if cfg!(windows) && !launch.no_wsl {
        let mut cmd = Command::new(&launch.wsl);
        if let Some(distro) = &launch.distro {
            cmd.args(["-d", distro]);
        }
        cmd.arg("-e");
        cmd
    } else if cfg!(windows) {
        return None;
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c");
        cmd.arg(BRIDGE_SH).arg(BRIDGE).arg(&launch.nvim).arg(addr);
        return Some(finish(cmd));
    };
    match &launch.shell {
        Some(shell) => cmd.args([shell.as_str(), "-lc"]),
        None => cmd.args(["sh", "-c"]),
    };
    cmd.arg(BRIDGE_SH).arg(BRIDGE).arg(&launch.nvim).arg(addr);
    Some(finish(cmd))
}

fn finish(mut cmd: Command) -> Command {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

#[cfg(windows)]
pub fn bind_to_parent(child: &tokio::process::Child) {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::*;

    let Some(process) = child.raw_handle() else {
        return;
    };
    let result = unsafe {
        CreateJobObjectW(None, None).and_then(|job| {
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )?;
            AssignProcessToJobObject(job, HANDLE(process))
        })
    };
    if let Err(err) = result {
        log::warn!("cannot tie nvim to the window: {err}");
    }
}

#[cfg(not(windows))]
pub fn bind_to_parent(_child: &tokio::process::Child) {}

pub fn parse_wsl_unc(path: &str) -> Option<(String, String)> {
    let rest = path
        .strip_prefix(r"\\?\UNC\")
        .or_else(|| path.strip_prefix(r"\\"))?;
    let (host, rest) = rest.split_once('\\')?;
    if !host.eq_ignore_ascii_case("wsl.localhost") && !host.eq_ignore_ascii_case("wsl$") {
        return None;
    }
    let (distro, linux) = rest.split_once('\\').unwrap_or((rest, ""));
    if distro.is_empty() {
        return None;
    }
    let linux = format!("/{}", linux.trim_end_matches('\\').replace('\\', "/"));
    Some((distro.to_string(), linux))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(cmd: &Command) -> Vec<String> {
        cmd.as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn gui_setup_comes_before_user_args() {
        let launch = NvimLaunch {
            args: vec!["-u".into(), "x.lua".into()],
            no_wsl: true,
            ..NvimLaunch::default()
        };
        assert_eq!(
            args(&command(&launch)),
            ["--embed", "--cmd", GUI_SETUP, "-u", "x.lua"]
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn bridge_runs_the_lua_script_with_nvim() {
        let launch = NvimLaunch {
            no_wsl: true,
            ..NvimLaunch::default()
        };
        let cmd = bridge_command(&launch, "/tmp/nvim.sock").unwrap();
        assert_eq!(cmd.as_std().get_program(), "sh");
        assert_eq!(
            args(&cmd),
            ["-c", BRIDGE_SH, BRIDGE, "nvim", "/tmp/nvim.sock"]
        );
        assert!(
            !BRIDGE.contains('"'),
            "the bridge must travel as one argument"
        );
    }

    #[test]
    fn parses_wsl_unc_paths() {
        assert_eq!(
            parse_wsl_unc(r"\\wsl.localhost\archlinux\home\john\dev"),
            Some(("archlinux".into(), "/home/john/dev".into()))
        );
        assert_eq!(
            parse_wsl_unc(r"\\?\UNC\wsl$\Ubuntu\tmp\"),
            Some(("Ubuntu".into(), "/tmp".into()))
        );
        assert_eq!(
            parse_wsl_unc(r"\\wsl.localhost\Ubuntu"),
            Some(("Ubuntu".into(), "/".into()))
        );
        assert_eq!(parse_wsl_unc(r"C:\Users\john"), None);
        assert_eq!(parse_wsl_unc(r"\\server\share\x"), None);
    }
}
