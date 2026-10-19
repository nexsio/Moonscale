use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env, fs,
    net::IpAddr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use tauri::{AppHandle, Manager};
use tokio::time::{sleep, timeout};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Profile {
    target_device: String,
    app_name: String,
    moonlight_path: String,
    home_server_host: String,
    home_server_user: String,
    #[serde(default)]
    hostname: String,
    ssh_key_path: String,
    wake_command: String,
    resolution: String,
    fps: u16,
    bitrate: u32,
    wake_timeout_seconds: u64,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            target_device: String::new(),
            app_name: "Desktop".into(),
            moonlight_path: "moonlight".into(),
            home_server_host: String::new(),
            home_server_user: String::new(),
            hostname: String::new(),
            ssh_key_path: String::new(),
            wake_command: String::new(),
            resolution: "1080p".into(),
            fps: 60,
            bitrate: 30_000,
            wake_timeout_seconds: 120,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Device {
    name: String,
    address: String,
    online: bool,
    #[serde(rename = "self")]
    self_device: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Dashboard {
    backend_state: String,
    connected: bool,
    devices: Vec<Device>,
}

#[async_trait]
trait NetworkAccess {
    async fn status(&self) -> Result<Dashboard, String>;
    async fn set_enabled(&self, enabled: bool) -> Result<(), String>;
}

struct TailscaleCli;

#[async_trait]
impl NetworkAccess for TailscaleCli {
    async fn status(&self) -> Result<Dashboard, String> {
        let output = run_process("tailscale", &["status", "--json"], 10).await?;
        if !output.status.success() {
            return Err(command_error("tailscale status", &output.stderr));
        }
        parse_dashboard(&output.stdout)
    }

    async fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        let action = if enabled { "up" } else { "down" };
        let output = run_process("pkexec", &["tailscale", action], 60).await?;
        if !output.status.success() {
            return Err(command_error("Tailscale connection change", &output.stderr));
        }
        Ok(())
    }
}

#[async_trait]
trait WakeService {
    async fn wake(&self, profile: &Profile) -> Result<(), String>;
}

struct SshWakeService;

#[async_trait]
impl WakeService for SshWakeService {
    async fn wake(&self, profile: &Profile) -> Result<(), String> {
        validate_host(&profile.home_server_host)?;
        validate_user(&profile.home_server_user)?;
        if profile.wake_command.trim().is_empty() {
            return Err("Set the Wake-on-LAN command in Settings first.".into());
        }

        let askpass = find_ssh_askpass().ok_or_else(|| {
            "No graphical SSH passphrase prompt was found. On Fedora KDE, install ksshaskpass with `sudo dnf install ksshaskpass`.".to_string()
        })?;

        let mut args = vec![
            "-o".into(),
            "BatchMode=no".into(),
            "-o".into(),
            "ConnectTimeout=8".into(),
            "-o".into(),
            "IdentityAgent=none".into(),
            "-o".into(),
            "IdentitiesOnly=yes".into(),
            "-o".into(),
            "PreferredAuthentications=publickey".into(),
            "-o".into(),
            "PasswordAuthentication=no".into(),
            "-o".into(),
            "KbdInteractiveAuthentication=no".into(),
            "-o".into(),
            "StrictHostKeyChecking=yes".into(),
        ];
        if !profile.ssh_key_path.trim().is_empty() {
            args.extend([
                "-i".into(),
                expand_home(&profile.ssh_key_path)
                    .to_string_lossy()
                    .into_owned(),
            ]);
        }
        args.push(format!(
            "{}@{}",
            profile.home_server_user, profile.home_server_host
        ));
        args.push(format!(
            "sh -lc {}",
            shell_quote(profile.wake_command.trim())
        ));
        let mut command = tokio::process::Command::new("ssh");
        command
            .args(&args)
            .kill_on_drop(true)
            .env("SSH_ASKPASS", askpass)
            .env("SSH_ASKPASS_REQUIRE", "force");
        let output = timeout(Duration::from_secs(120), command.output())
            .await
            .map_err(|_| "SSH wake command did not finish within 120 seconds.".to_string())?
            .map_err(|error| format!("Could not run ssh: {error}"))?;
        if !output.status.success() {
            return Err(command_error("SSH wake command", &output.stderr));
        }
        Ok(())
    }
}

fn find_ssh_askpass() -> Option<PathBuf> {
    if let Some(configured) = env::var_os("SSH_ASKPASS").map(PathBuf::from) {
        if is_executable(&configured) {
            return Some(configured);
        }
    }

    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .flat_map(|directory| {
            ["ksshaskpass", "ssh-askpass", "gnome-ssh-askpass"]
                .map(|program| directory.join(program))
        })
        .find(|candidate| is_executable(candidate))
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[async_trait]
trait StreamingService {
    async fn launch(&self, profile: &Profile, target_host: &str) -> Result<(), String>;
}

struct MoonlightCli;

const MOONLIGHT_FLATPAK_ID: &str = "com.moonlight_stream.Moonlight";

#[async_trait]
impl StreamingService for MoonlightCli {
    async fn launch(&self, profile: &Profile, target_host: &str) -> Result<(), String> {
        let resolution = match profile.resolution.as_str() {
            "720p" => "-720",
            "1080p" => "-1080",
            "1440p" => "-1440",
            "4K" => "-4K",
            _ => return Err("Choose a supported streaming resolution in Settings.".into()),
        };

        let args = vec![
            "stream".into(),
            target_host.into(),
            "-app".into(),
            if profile.app_name.trim().is_empty() { "Desktop" } else { profile.app_name.trim() }.into(),
            resolution.into(),
            "-fps".into(),
            profile.fps.clamp(30, 120).to_string(),
            "-bitrate".into(),
            profile.bitrate.clamp(1_000, 150_000).to_string(),
            "-fullscreen".into(),
        ];

        let mut command = Command::new(expand_home(&profile.moonlight_path));
        command.args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        match command.spawn() {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && profile.moonlight_path.trim() == "moonlight" => {
                launch_moonlight_flatpak(&args)
            }
            Err(error) => Err(format!("Could not launch Moonlight: {error}")),
        }
    }
}

fn launch_moonlight_flatpak(args: &[String]) -> Result<(), String> {
    let flatpak_available = Command::new("flatpak")
        .args(["info", MOONLIGHT_FLATPAK_ID])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());

    if flatpak_available {
        let mut command = Command::new("flatpak");
        command
            .args(["run", "--command=moonlight", MOONLIGHT_FLATPAK_ID])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        return command
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not launch the Moonlight Flatpak: {error}"));
    }

    let host_flatpak_available = Command::new("flatpak-spawn")
        .args(["--host", "flatpak", "info", MOONLIGHT_FLATPAK_ID])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());

    if host_flatpak_available {
        let mut command = Command::new("flatpak-spawn");
        command
            .args(["--host", "flatpak", "run", "--command=moonlight", MOONLIGHT_FLATPAK_ID])
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        return command
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("Could not launch Moonlight on the host: {error}"));
    }

    Err("Moonlight was not found in PATH, and its Flatpak installation is unavailable. Install Moonlight or set its executable path in Settings.".into())
}

fn profile_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app.path().app_config_dir().map_err(|error| error.to_string())?;
    fs::create_dir_all(&directory).map_err(|error| format!("Could not create app settings directory: {error}"))?;
    Ok(directory.join("profile.json"))
}

fn read_profile(app: &AppHandle) -> Result<Profile, String> {
    let path = profile_path(app)?;
    match fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str(&contents).map_err(|error| format!("Could not read saved profile: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Profile::default()),
        Err(error) => Err(format!("Could not read saved profile: {error}")),
    }
}

fn parse_dashboard(bytes: &[u8]) -> Result<Dashboard, String> {
    let status: Value = serde_json::from_slice(bytes).map_err(|error| format!("Could not parse Tailscale status: {error}"))?;
    let backend_state = status["BackendState"].as_str().unwrap_or("Unknown").to_string();
    let mut devices = Vec::new();

    if let Some(self_device) = status.get("Self") {
        devices.push(parse_device(self_device, true, true));
    }
    if let Some(peers) = status["Peer"].as_object() {
        devices.extend(peers.values().map(|peer| parse_device(peer, false, false)));
    }
    devices.sort_by(|first, second| second.online.cmp(&first.online).then_with(|| first.name.cmp(&second.name)));

    Ok(Dashboard {
        connected: backend_state == "Running",
        backend_state,
        devices,
    })
}

fn parse_device(value: &Value, self_device: bool, always_online: bool) -> Device {
    let name = value["HostName"]
        .as_str()
        .or_else(|| value["DNSName"].as_str())
        .unwrap_or("Unknown device")
        .trim_end_matches('.')
        .to_string();
    let address = value["TailscaleIPs"]
        .as_array()
        .map(|ips| ips.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    Device {
        name,
        address,
        online: always_online || value["Online"].as_bool().unwrap_or(false),
        self_device,
    }
}

async fn run_process(program: &str, args: &[&str], seconds: u64) -> Result<std::process::Output, String> {
    run_process_owned(program, &args.iter().map(|arg| (*arg).to_string()).collect::<Vec<_>>(), seconds).await
}

async fn run_process_owned(program: &str, args: &[String], seconds: u64) -> Result<std::process::Output, String> {
    let mut command = tokio::process::Command::new(program);
    command.args(args).kill_on_drop(true);
    timeout(Duration::from_secs(seconds), command.output())
        .await
        .map_err(|_| format!("{program} did not finish within {seconds} seconds."))?
        .map_err(|error| format!("Could not run {program}: {error}"))
}

fn command_error(command: &str, stderr: &[u8]) -> String {
    let detail = String::from_utf8_lossy(stderr).trim().to_string();
    if detail.is_empty() { format!("{command} failed.") } else { detail }
}

fn validate_host(host: &str) -> Result<(), String> {
    if host.is_empty() || !host.chars().all(|character| character.is_ascii_alphanumeric() || ".-:".contains(character)) {
        return Err("Use a hostname or IP address without spaces for the home server.".into());
    }
    Ok(())
}

fn validate_user(user: &str) -> Result<(), String> {
    if user.is_empty() || !user.chars().all(|character| character.is_ascii_alphanumeric() || "_-".contains(character)) {
        return Err("Use a valid SSH username.".into());
    }
    Ok(())
}

fn shell_quote(command: &str) -> String {
    format!("'{}'", command.replace('\'', "'\\''"))
}

fn expand_home(value: &str) -> PathBuf {
    if value == "~" {
        return std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(value));
    }
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(value)
}

#[tauri::command]
async fn get_dashboard() -> Result<Dashboard, String> {
    TailscaleCli.status().await
}

#[tauri::command]
async fn set_connection(enabled: bool) -> Result<Dashboard, String> {
    TailscaleCli.set_enabled(enabled).await?;
    sleep(Duration::from_millis(700)).await;
    TailscaleCli.status().await
}

#[tauri::command]
fn get_profile(app: AppHandle) -> Result<Profile, String> {
    read_profile(&app)
}

#[tauri::command]
fn save_profile(app: AppHandle, profile: Profile) -> Result<Profile, String> {
    let path = profile_path(&app)?;
    let contents = serde_json::to_vec_pretty(&profile).map_err(|error| error.to_string())?;
    fs::write(path, contents).map_err(|error| format!("Could not save profile: {error}"))?;
    Ok(profile)
}

#[tauri::command]
async fn wake_device(app: AppHandle) -> Result<(), String> {
    SshWakeService.wake(&read_profile(&app)?).await
}

#[tauri::command]
async fn wake_and_stream(app: AppHandle) -> Result<(), String> {
    let profile = read_profile(&app)?;
    if profile.target_device.trim().is_empty() {
        return Err("Select a Tailnet device in Settings first.".into());
    }
    let initial = TailscaleCli.status().await?;
    if !initial.connected {
        return Err("Connect Tailscale before starting a stream.".into());
    }

    let target_name = profile.target_device.trim();
    let mut target = initial
        .devices
        .into_iter()
        .find(|device| device.name.eq_ignore_ascii_case(target_name))
        .ok_or_else(|| "The selected Tailnet device was not found. Refresh devices and select it again.".to_string())?;

    if !target.online {
        SshWakeService.wake(&profile).await?;
        let wait_seconds = profile.wake_timeout_seconds.clamp(15, 600);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(wait_seconds);
        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(format!("The target did not come online within {wait_seconds} seconds."));
            }
            sleep(Duration::from_secs(5)).await;
            if let Ok(status) = TailscaleCli.status().await {
                if let Some(device) = status.devices.into_iter().find(|device| {
                    device.online && device.name.eq_ignore_ascii_case(target_name)
                }) {
                    target = device;
                    break;
                }
            }
        }
    }

    let target_host = target
        .address
        .split(',')
        .map(str::trim)
        .find(|address| address.parse::<IpAddr>().is_ok())
        .ok_or_else(|| "The selected Tailnet device has no usable Tailscale IP address.".to_string())?;
    MoonlightCli.launch(&profile, target_host).await
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_dashboard,
            set_connection,
            get_profile,
            save_profile,
            wake_device,
            wake_and_stream
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tailscale Stream");
}