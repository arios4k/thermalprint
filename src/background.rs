//! One desktop process owns the watcher, including while its window is hidden.
#[cfg(not(target_os = "linux"))]
use anyhow::bail;
use anyhow::{Context, Result};
use fs2::FileExt;
use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
};
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

pub fn directory() -> Result<PathBuf> {
    directories::ProjectDirs::from("org", "ThermalPrint", "ThermalPrint")
        .map(|d| d.config_dir().to_owned())
        .context("No application settings folder")
}

/// Keep the lock alive for the process lifetime. A second launch wakes the owner.
pub fn acquire() -> Result<Option<File>> {
    acquire_in(directory()?)
}
fn acquire_in(dir: PathBuf) -> Result<Option<File>> {
    std::fs::create_dir_all(&dir)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("instance.lock"))?;
    match lock.try_lock_exclusive() {
        Ok(()) => {
            let _ = std::fs::remove_file(dir.join("open-request.json"));
            Ok(Some(lock))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            let path = std::env::args_os()
                .skip(1)
                .find(|a| a != "--background")
                .map(PathBuf::from);
            let mut request = tempfile::NamedTempFile::new_in(&dir)?;
            serde_json::to_writer(request.as_file_mut(), &path)?;
            request.persist(dir.join("open-request.json"))?;
            Ok(None)
        }
        Err(e) => Err(e.into()),
    }
}
pub fn open_request() -> Option<Option<PathBuf>> {
    let file = directory().ok()?.join("open-request.json");
    let bytes = std::fs::read(&file).ok()?;
    let request = serde_json::from_slice(&bytes).ok()?;
    let _ = std::fs::remove_file(file);
    Some(request)
}

#[cfg(target_os = "linux")]
pub fn tray_available() -> bool {
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };
    let Ok(proxy) = zbus::blocking::Proxy::new(
        &connection,
        "org.kde.StatusNotifierWatcher",
        "/StatusNotifierWatcher",
        "org.kde.StatusNotifierWatcher",
    ) else {
        return false;
    };
    proxy
        .get_property::<bool>("IsStatusNotifierHostRegistered")
        .unwrap_or(false)
}
#[cfg(not(target_os = "linux"))]
pub fn tray_available() -> bool {
    true
}

pub enum Action {
    Open,
    Pause,
    Quit,
}
pub struct Tray {
    icon: TrayIcon,
    pause: MenuItem,
    open: MenuItem,
    quit: MenuItem,
}
impl Tray {
    pub fn new() -> Result<Self> {
        if !tray_available() {
            anyhow::bail!(
                "The desktop does not provide a status tray. Enable a StatusNotifier/AppIndicator extension."
            );
        }
        let menu = Menu::new();
        let open = MenuItem::new(crate::i18n::tr("Open ThermalPrint"), true, None);
        let pause = MenuItem::new(crate::i18n::tr("Pause auto-print"), true, None);
        let quit = MenuItem::new(crate::i18n::tr("Quit"), true, None);
        menu.append_items(&[&open, &pause, &quit])?;
        let image =
            image::load_from_memory(include_bytes!("../data/org.thermalprint.ThermalPrint.png"))?
                .to_rgba8();
        let icon =
            tray_icon::Icon::from_rgba(image.as_raw().clone(), image.width(), image.height())?;
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_tooltip("ThermalPrint")
            .build()?;
        Ok(Self {
            icon,
            pause,
            open,
            quit,
        })
    }
    pub fn action(&self) -> Option<Action> {
        let event = MenuEvent::receiver().try_recv().ok()?;
        if event.id == *self.open.id() {
            Some(Action::Open)
        } else if event.id == *self.pause.id() {
            Some(Action::Pause)
        } else if event.id == *self.quit.id() {
            Some(Action::Quit)
        } else {
            None
        }
    }
    pub fn status(&self, paused: bool, enabled: bool) {
        self.pause.set_text(crate::i18n::tr(if paused {
            "Resume auto-print"
        } else {
            "Pause auto-print"
        }));
        let _ = self.icon.set_tooltip(Some(if !enabled {
            "ThermalPrint · automatic printing off"
        } else if paused {
            "ThermalPrint · automatic printing paused"
        } else {
            "ThermalPrint · watching for labels"
        }));
    }
}

#[cfg(target_os = "linux")]
pub fn set_autostart(enabled: bool) -> Result<()> {
    let base = std::env::var_os("SNAP_USER_DATA")
        .map(PathBuf::from)
        .map(|p| p.join(".config"))
        .or_else(|| directories::BaseDirs::new().map(|d| d.config_dir().to_owned()))
        .context("No desktop configuration folder")?
        .join("autostart");
    let path = base.join("thermalprint.desktop");
    if !enabled {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        return Ok(());
    }
    let command = if std::env::var_os("SNAP").is_some() {
        "thermalprint".to_owned()
    } else {
        format!(
            "\"{}\"",
            std::env::current_exe()?
                .to_string_lossy()
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('$', "\\$")
                .replace(char::from(96), "\\`")
                .replace('%', "%%")
        )
    };
    std::fs::create_dir_all(&base)?;
    let mut out = tempfile::NamedTempFile::new_in(&base)?;
    use std::io::Write;
    write!(
        out,
        "[Desktop Entry]\nType=Application\nName=ThermalPrint\nExec={command} --background\nTerminal=false\n"
    )?;
    out.persist(path)?;
    Ok(())
}
#[cfg(windows)]
pub fn set_autostart(enabled: bool) -> Result<()> {
    use windows::ApplicationModel::{Package, StartupTask, StartupTaskState};
    if Package::Current().is_ok() {
        let task = StartupTask::GetAsync(&windows::core::HSTRING::from("ThermalPrintBackground"))?
            .join()?;
        if !enabled {
            task.Disable()?;
            return Ok(());
        }
        let state = task.RequestEnableAsync()?.join()?;
        if state != StartupTaskState::Enabled {
            bail!(
                "Startup is disabled in Windows. Enable ThermalPrint in Settings → Apps → Startup."
            );
        }
        return Ok(());
    }
    use windows_sys::Win32::System::Registry::*;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let subkey = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let name = wide("ThermalPrint");
    let command = wide(&format!(
        "\"{}\" --background",
        std::env::current_exe()?.display()
    ));
    let mut key = std::ptr::null_mut();
    unsafe {
        let code = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        if code != 0 {
            bail!("Cannot open startup settings: {code}");
        }
        let result = if enabled {
            RegSetValueExW(
                key,
                name.as_ptr(),
                0,
                REG_SZ,
                command.as_ptr().cast(),
                (command.len() * 2) as u32,
            )
        } else {
            RegDeleteValueW(key, name.as_ptr())
        };
        RegCloseKey(key);
        if result != 0 && !(result == 2 && !enabled) {
            bail!("Cannot update startup settings: {result}");
        }
    }
    Ok(())
}
#[cfg(not(any(windows, target_os = "linux")))]
pub fn set_autostart(_: bool) -> Result<()> {
    bail!("Startup is supported on Windows and Linux");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_one_process_can_own_the_watch_folder() {
        let dir = tempfile::tempdir().unwrap();
        let owner = acquire_in(dir.path().to_owned()).unwrap().unwrap();
        assert!(acquire_in(dir.path().to_owned()).unwrap().is_none());
        assert!(dir.path().join("open-request.json").is_file());
        drop(owner);
        assert!(acquire_in(dir.path().to_owned()).unwrap().is_some());
        assert!(!dir.path().join("open-request.json").exists());
    }
}
