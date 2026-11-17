use std::path::PathBuf;

use rmpv::Value;

pub const LUA: &str = include_str!("recent.lua");

const MAX_FILES: usize = 10;
const MAX_DIRS: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub cwd: String,
    pub distro: String,
}

impl Entry {
    fn args(&self, dir: bool) -> String {
        let mut args = String::new();
        if !self.distro.is_empty() {
            args.push_str(&format!("--distro \"{}\" ", self.distro));
        }
        if dir {
            args.push_str(&format!("--cwd \"{}\"", self.path));
        } else {
            args.push_str(&format!("--cwd \"{}\" \"{}\"", self.cwd, self.path));
        }
        args
    }

    #[cfg_attr(not(windows), allow(dead_code))]
    fn name(&self) -> &str {
        let p = self.path.trim_end_matches('/');
        p.rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or(p)
    }
}

#[derive(Debug)]
pub enum RecentEvent {
    File(Entry),
    Dir(Entry),
}

pub fn parse_notification(args: Vec<Value>) -> Option<RecentEvent> {
    let s: Vec<String> = args
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();
    let [kind, path, cwd, distro] = s.as_slice() else {
        return None;
    };
    let entry = Entry {
        path: path.clone(),
        cwd: cwd.clone(),
        distro: distro.clone(),
    };
    match kind.as_str() {
        "file" => Some(RecentEvent::File(entry)),
        "dir" => Some(RecentEvent::Dir(entry)),
        _ => None,
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Recent {
    pub files: Vec<Entry>,
    pub dirs: Vec<Entry>,
}

fn file() -> Option<PathBuf> {
    crate::config::config_dir().map(|d| d.join("recent"))
}

impl Recent {
    pub fn load() -> Self {
        let Some(text) = file().and_then(|f| std::fs::read_to_string(f).ok()) else {
            return Self::default();
        };
        Self::parse(&text)
    }

    fn parse(text: &str) -> Self {
        let mut recent = Self::default();
        for line in text.lines() {
            let mut parts = line.split('\t');
            let (Some(kind), Some(path), Some(cwd), distro) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let entry = Entry {
                path: path.into(),
                cwd: cwd.into(),
                distro: distro.unwrap_or_default().into(),
            };
            match kind {
                "file" => recent.files.push(entry),
                "dir" => recent.dirs.push(entry),
                _ => {}
            }
        }
        recent
    }

    fn to_text(&self) -> String {
        let mut out = String::new();
        for (kind, list) in [("file", &self.files), ("dir", &self.dirs)] {
            for e in list {
                out.push_str(&format!("{kind}\t{}\t{}\t{}\n", e.path, e.cwd, e.distro));
            }
        }
        out
    }

    pub fn add(&mut self, event: RecentEvent) -> bool {
        let before = self.clone();
        let (list, max, entry) = match event {
            RecentEvent::File(e) => (&mut self.files, MAX_FILES, e),
            RecentEvent::Dir(e) => (&mut self.dirs, MAX_DIRS, e),
        };
        list.retain(|e| e.path != entry.path || e.distro != entry.distro);
        list.insert(0, entry);
        list.truncate(max);
        *self != before
    }

    pub fn save(&self) {
        if let Some(f) = file()
            && let Err(err) = f
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|_| std::fs::write(&f, self.to_text()))
        {
            log::warn!("{}: {err}", f.display());
        }
    }

    pub fn publish(&self, done: impl Fn(Vec<String>) + Send + 'static) {
        #[cfg(windows)]
        {
            use std::sync::Mutex;
            use std::sync::mpsc::{Sender, channel};
            type Job = (Vec<Entry>, Vec<Entry>, Box<dyn Fn(Vec<String>) + Send>);
            static WORKER: Mutex<Option<Sender<Job>>> = Mutex::new(None);
            let mut worker = WORKER.lock().unwrap();
            let tx = worker.get_or_insert_with(|| {
                let (tx, rx) = channel::<Job>();
                std::thread::spawn(move || {
                    while let Ok(mut job) = rx.recv() {
                        while let Ok(newer) = rx.try_recv() {
                            job = newer;
                        }
                        match shell::publish(&job.0, &job.1) {
                            Ok(removed) => (job.2)(removed),
                            Err(err) => log::warn!("jump list: {err}"),
                        }
                    }
                });
                tx
            });
            let _ = tx.send((self.files.clone(), self.dirs.clone(), Box::new(done)));
        }
        #[cfg(not(windows))]
        let _ = done;
    }

    pub fn forget(&mut self, removed: &[String]) -> bool {
        let before = self.files.len() + self.dirs.len();
        self.files.retain(|e| !removed.contains(&e.args(false)));
        self.dirs.retain(|e| !removed.contains(&e.args(true)));
        before != self.files.len() + self.dirs.len()
    }
}

#[cfg(windows)]
mod shell {
    use windows::Win32::Storage::EnhancedStorage::PKEY_Title;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemAlloc,
    };
    use windows::Win32::System::Variant::VT_LPWSTR;
    use windows::Win32::UI::Shell::Common::{IObjectArray, IObjectCollection};
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{
        DestinationList, EnumerableObjectCollection, ICustomDestinationList, IShellLinkW, ShellLink,
    };
    use windows::core::{HSTRING, Interface, PWSTR, Result};

    use super::Entry;

    fn lpwstr(s: &str) -> PROPVARIANT {
        let wide: Vec<u16> = s.encode_utf16().chain([0]).collect();
        let mut pv = PROPVARIANT::default();
        unsafe {
            let p = CoTaskMemAlloc(wide.len() * 2) as *mut u16;
            if p.is_null() {
                return pv;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
            (*pv.Anonymous.Anonymous).vt = VT_LPWSTR;
            (*pv.Anonymous.Anonymous).Anonymous.pwszVal = PWSTR(p);
        }
        pv
    }

    fn link(exe: &HSTRING, args: &str, title: &str, tip: &str) -> Result<IShellLinkW> {
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(exe)?;
            link.SetArguments(&HSTRING::from(args))?;
            link.SetDescription(&HSTRING::from(tip))?;
            link.SetIconLocation(exe, 0)?;
            let store: IPropertyStore = link.cast()?;
            store.SetValue(&PKEY_Title, &lpwstr(title))?;
            store.Commit()?;
            Ok(link)
        }
    }

    fn args_of(link: &IShellLinkW) -> Option<String> {
        let mut buf = [0u16; 2048];
        unsafe { link.GetArguments(&mut buf).ok()? };
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        Some(String::from_utf16_lossy(&buf[..len]))
    }

    pub fn publish(files: &[Entry], dirs: &[Entry]) -> Result<Vec<String>> {
        let exe = std::env::current_exe()
            .map_err(|e| windows::core::Error::new(windows::core::HRESULT(-1), e.to_string()))?;
        let exe = HSTRING::from(exe.as_os_str());
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let list: ICustomDestinationList =
                CoCreateInstance(&DestinationList, None, CLSCTX_INPROC_SERVER)?;
            let mut slots = 0u32;
            let removed: IObjectArray = list.BeginList(&mut slots)?;
            let removed: Vec<String> = (0..removed.GetCount()?)
                .filter_map(|i| removed.GetAt::<IShellLinkW>(i).ok())
                .filter_map(|l| args_of(&l))
                .collect();
            for (name, entries, dir) in [("Recent files", files, false), ("Folders", dirs, true)] {
                let items: IObjectCollection =
                    CoCreateInstance(&EnumerableObjectCollection, None, CLSCTX_INPROC_SERVER)?;
                for e in entries {
                    let args = e.args(dir);
                    if removed.contains(&args) {
                        continue;
                    }
                    let tip = if dir {
                        e.path.clone()
                    } else {
                        format!("{}\n{}", e.path, e.cwd)
                    };
                    items.AddObject(&link(&exe, &args, e.name(), &tip)?)?;
                }
                if items.GetCount()? > 0 {
                    list.AppendCategory(&HSTRING::from(name), &items.cast::<IObjectArray>()?)?;
                }
            }
            list.CommitList()?;
            Ok(removed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> Entry {
        Entry {
            path: path.into(),
            cwd: "/home/me/proj".into(),
            distro: "arch".into(),
        }
    }

    #[test]
    fn most_recent_first_no_duplicates() {
        let mut r = Recent::default();
        assert!(r.add(RecentEvent::File(entry("/home/me/proj/a.rs"))));
        assert!(r.add(RecentEvent::File(entry("/home/me/proj/b.rs"))));
        assert!(r.add(RecentEvent::File(entry("/home/me/proj/a.rs"))));
        assert!(!r.add(RecentEvent::File(entry("/home/me/proj/a.rs"))));
        let paths: Vec<_> = r.files.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["/home/me/proj/a.rs", "/home/me/proj/b.rs"]);
        for i in 0..20 {
            r.add(RecentEvent::File(entry(&format!("/f{i}"))));
        }
        assert_eq!(r.files.len(), MAX_FILES);
    }

    #[test]
    fn round_trip_and_args() {
        let mut r = Recent::default();
        r.add(RecentEvent::File(entry("/home/me/my proj/a.rs")));
        r.add(RecentEvent::Dir(entry("/home/me/proj")));
        assert_eq!(Recent::parse(&r.to_text()), r);
        assert_eq!(
            r.files[0].args(false),
            "--distro \"arch\" --cwd \"/home/me/proj\" \"/home/me/my proj/a.rs\""
        );
        assert_eq!(r.dirs[0].name(), "proj");
        assert!(r.forget(&[r.dirs[0].args(true)]));
        assert!(r.dirs.is_empty());
    }
}
