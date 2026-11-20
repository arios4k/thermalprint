//! One desktop process owns the watcher, including while its window is hidden.
#[cfg(not(target_os = "linux"))]
use anyhow::bail;
use anyhow::{Context, Result};
use fs2::FileExt;
use std::{
    fs::{File, OpenOptions},
    path::PathBuf,
    time::{Duration, Instant},
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
    acquire_in(
        directory()?,
        &Launch::from_args(std::env::args_os().skip(1)),
    )
}

#[derive(Default, Debug, PartialEq)]
pub struct Launch {
    pub background: bool,
    pub file: Option<PathBuf>,
}
impl Launch {
    pub fn from_args(args: impl IntoIterator<Item = std::ffi::OsString>) -> Self {
        let mut launch = Self::default();
        for arg in args {
            if arg == "--background" {
                launch.background = true;
            } else if launch.file.is_none() {
                launch.file = Some(PathBuf::from(arg));
            }
        }
        launch
    }
    pub fn background_only(&self) -> bool {
        self.background && self.file.is_none()
    }
}

fn acquire_in(dir: PathBuf, launch: &Launch) -> Result<Option<File>> {
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
        Err(e) if e.raw_os_error() == fs2::lock_contended_error().raw_os_error() => {
            // A repeated login activation must not reveal the running background app.
            if launch.background_only() {
                return Ok(None);
            }
            let mut request = tempfile::NamedTempFile::new_in(&dir)?;
            serde_json::to_writer(request.as_file_mut(), &launch.file)?;
            request.persist(dir.join("open-request.json"))?;
            Ok(None)
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, PartialEq)]
pub enum StartupAction {
    Wait,
    Hide,
    Show,
}

/// A login launch waits for the desktop tray, without overriding an explicit Open.
pub struct Startup {
    pending: bool,
    started: Instant,
}
impl Startup {
    pub fn new(background: bool) -> Self {
        Self {
            pending: background,
            started: Instant::now(),
        }
    }
    pub fn cancel(&mut self) {
        self.pending = false;
    }
    pub fn waiting(&self) -> bool {
        self.pending
    }
    pub fn poll(&mut self, tray_ready: bool, first_frame_finished: bool) -> StartupAction {
        self.poll_at(self.started.elapsed(), tray_ready, first_frame_finished)
    }
    fn poll_at(
        &mut self,
        elapsed: Duration,
        tray_ready: bool,
        first_frame_finished: bool,
    ) -> StartupAction {
        if !self.pending {
            return StartupAction::Wait;
        }
        // eframe shows its first painted frame; a constructor-time hide can be lost.
        if tray_ready && first_frame_finished {
            self.pending = false;
            StartupAction::Hide
        } else if elapsed >= Duration::from_secs(30) {
            self.pending = false;
            StartupAction::Show
        } else {
            StartupAction::Wait
        }
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
    let connection = zbus::blocking::connection::Builder::session()
        .and_then(|builder| builder.method_timeout(Duration::from_secs(1)).build());
    let Ok(connection) = connection else {
        return false;
    };
    tray_available_on(&connection)
}

#[cfg(target_os = "linux")]
fn tray_available_on(connection: &zbus::blocking::Connection) -> bool {
    let proxy = (|| -> zbus::Result<zbus::blocking::Proxy<'_>> {
        zbus::blocking::proxy::Builder::new(connection)
            .destination("org.kde.StatusNotifierWatcher")?
            .path("/StatusNotifierWatcher")?
            .interface("org.kde.StatusNotifierWatcher")?
            // Strict Snap permits Properties.Get here, not the GetAll used by caching.
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
    })();
    let Ok(proxy) = proxy else {
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
    pub fn status(&self, paused: bool, enabled: bool) -> Result<()> {
        self.pause.set_text(crate::i18n::tr(if paused {
            "Resume auto-print"
        } else {
            "Pause auto-print"
        }));
        self.icon.set_tooltip(Some(if !enabled {
            "ThermalPrint · automatic printing off"
        } else if paused {
            "ThermalPrint · automatic printing paused"
        } else {
            "ThermalPrint · watching for labels"
        }))?;
        Ok(())
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
        let launch = Launch::default();
        let owner = acquire_in(dir.path().to_owned(), &launch).unwrap().unwrap();
        assert!(
            acquire_in(dir.path().to_owned(), &launch)
                .unwrap()
                .is_none()
        );
        assert!(dir.path().join("open-request.json").is_file());
        drop(owner);
        // Another parallel test can fork while the lock is held. Its child briefly
        // shares the open description until exec closes CLOEXEC descriptors.
        let released = (0..100).any(|_| {
            let acquired = acquire_in(dir.path().to_owned(), &launch)
                .unwrap()
                .is_some();
            if !acquired {
                std::thread::sleep(Duration::from_millis(10));
            }
            acquired
        });
        assert!(released, "The owner lock was not released after closing it");
        assert!(!dir.path().join("open-request.json").exists());
    }

    #[test]
    fn repeated_background_activation_does_not_open_the_window() {
        let dir = tempfile::tempdir().unwrap();
        let background = Launch {
            background: true,
            file: None,
        };
        let _owner = acquire_in(dir.path().to_owned(), &background)
            .unwrap()
            .unwrap();
        assert!(
            acquire_in(dir.path().to_owned(), &background)
                .unwrap()
                .is_none()
        );
        assert!(!dir.path().join("open-request.json").exists());
        let label = Launch {
            background: true,
            file: Some("test.thermalprint".into()),
        };
        assert!(acquire_in(dir.path().to_owned(), &label).unwrap().is_none());
        let saved: Option<PathBuf> =
            serde_json::from_slice(&std::fs::read(dir.path().join("open-request.json")).unwrap())
                .unwrap();
        assert_eq!(saved, label.file);
    }

    #[test]
    fn startup_waits_for_the_first_frame_and_a_ready_tray() {
        let mut startup = Startup::new(true);
        assert_eq!(
            startup.poll_at(Duration::ZERO, true, false),
            StartupAction::Wait
        );
        assert_eq!(
            startup.poll_at(Duration::from_secs(1), false, true),
            StartupAction::Wait
        );
        assert_eq!(
            startup.poll_at(Duration::from_secs(8), true, true),
            StartupAction::Hide
        );
        assert_eq!(
            startup.poll_at(Duration::from_secs(9), true, true),
            StartupAction::Wait
        );
    }

    #[test]
    fn unavailable_tray_falls_back_and_late_recovery_does_not_hide_a_visible_app() {
        let mut startup = Startup::new(true);
        assert_eq!(
            startup.poll_at(Duration::from_secs(30), false, true),
            StartupAction::Show
        );
        assert_eq!(
            startup.poll_at(Duration::from_secs(31), true, true),
            StartupAction::Wait
        );
        let mut opened = Startup::new(true);
        opened.cancel();
        assert_eq!(
            opened.poll_at(Duration::from_secs(1), true, true),
            StartupAction::Wait
        );
        assert_eq!(
            Startup::new(false).poll_at(Duration::from_secs(1), true, true),
            StartupAction::Wait
        );
    }

    #[test]
    fn launch_arguments_distinguish_background_startup_from_opening_a_label() {
        let parse = |args: &[&str]| Launch::from_args(args.iter().map(std::ffi::OsString::from));
        assert!(parse(&["--background"]).background_only());
        assert!(!parse(&[]).background_only());
        assert_eq!(
            parse(&["label.thermalprint", "--background"]),
            parse(&["--background", "label.thermalprint"])
        );
        assert!(!parse(&["--background", "label.thermalprint"]).background_only());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_late_tray_host_is_detected_when_get_all_is_forbidden() {
        use std::io::BufRead;
        use std::process::{Command, Stdio};

        struct Bus(std::process::Child);
        impl Drop for Bus {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        struct Watcher;
        #[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
        impl Watcher {
            #[zbus(property)]
            fn is_status_notifier_host_registered(&self) -> bool {
                true
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("bus.conf");
        std::fs::write(
            &config,
            format!(
                r#"<busconfig>
<type>session</type><listen>unix:tmpdir={}</listen><auth>EXTERNAL</auth>
<policy context="default"><allow own="*"/><allow send_destination="*"/>
<allow receive_sender="*"/>
<deny send_interface="org.freedesktop.DBus.Properties" send_member="GetAll"/>
</policy></busconfig>"#,
                dir.path().display()
            ),
        )
        .unwrap();
        let child = Command::new("dbus-daemon")
            .arg("--nofork")
            .arg("--config-file")
            .arg(&config)
            .arg("--print-address=1")
            .stdout(Stdio::piped())
            .spawn();
        let mut bus = match child {
            Ok(child) => Bus(child),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => panic!("Cannot start the private test bus: {error}"),
        };
        let mut address = String::new();
        std::io::BufReader::new(bus.0.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let address = address.trim();
        assert!(!address.is_empty());
        let client = zbus::blocking::connection::Builder::address(address)
            .unwrap()
            .build()
            .unwrap();
        assert!(!tray_available_on(&client));
        let _watcher = zbus::blocking::connection::Builder::address(address)
            .unwrap()
            .name("org.kde.StatusNotifierWatcher")
            .unwrap()
            .serve_at("/StatusNotifierWatcher", Watcher)
            .unwrap()
            .build()
            .unwrap();
        let properties = zbus::blocking::Proxy::new(
            &client,
            "org.kde.StatusNotifierWatcher",
            "/StatusNotifierWatcher",
            "org.freedesktop.DBus.Properties",
        )
        .unwrap();
        let rejected = properties
            .call_method("GetAll", &("org.kde.StatusNotifierWatcher",))
            .unwrap_err();
        assert!(
            matches!(&rejected, zbus::Error::MethodError(name, ..)
            if name.as_str() == "org.freedesktop.DBus.Error.AccessDenied"),
            "{rejected}"
        );
        assert!(tray_available_on(&client));
    }
}
