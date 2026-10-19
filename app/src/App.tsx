import { invoke } from "@tauri-apps/api/core";
import {
  Activity,
  AlertTriangle,
  Check,
  CircleHelp,
  ChevronDown,
  LoaderCircle,
  Monitor,
  Power,
  RefreshCw,
  Settings2,
  Wifi,
  Zap,
} from "lucide-react";
import { useEffect, useState } from "react";

type Device = {
  name: string;
  address: string;
  online: boolean;
  self: boolean;
};

type Dashboard = {
  backendState: string;
  connected: boolean;
  devices: Device[];
};

type Profile = {
  targetDevice: string;
  appName: string;
  moonlightPath: string;
  homeServerHost: string;
  homeServerUser: string;
  hostname: string;
  sshKeyPath: string;
  wakeCommand: string;
  resolution: string;
  fps: number;
  bitrate: number;
  wakeTimeoutSeconds: number;
};

const defaultDashboard: Dashboard = {
  backendState: "Loading",
  connected: false,
  devices: [],
};

const defaultProfile: Profile = {
  targetDevice: "",
  appName: "Desktop",
  moonlightPath: "moonlight",
  homeServerHost: "",
  homeServerUser: "",
  hostname: "",
  sshKeyPath: "",
  wakeCommand: "",
  resolution: "1080p",
  fps: 60,
  bitrate: 30000,
  wakeTimeoutSeconds: 120,
};

function App() {
  const [dashboard, setDashboard] = useState(defaultDashboard);
  const [profile, setProfile] = useState(defaultProfile);
  const [page, setPage] = useState<"home" | "settings">("home");
  const [devicePickerOpen, setDevicePickerOpen] = useState(false);
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  async function refresh() {
    try {
      const result = await invoke<Dashboard>("get_dashboard");
      setDashboard(result);
      setError("");
    } catch (cause) {
      setError(String(cause));
    }
  }

  useEffect(() => {
    void refresh();
    void invoke<Profile>("get_profile")
      .then(setProfile)
      .catch((cause) => setError(String(cause)));
    const timer = window.setInterval(() => void refresh(), 8000);
    return () => window.clearInterval(timer);
  }, []);

  async function runAction(key: string, action: () => Promise<void>) {
    setBusy(key);
    setError("");
    setNotice("");
    try {
      await action();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy("");
    }
  }

  function toggleConnection() {
    void runAction("connection", async () => {
      const result = await invoke<Dashboard>("set_connection", {
        enabled: !dashboard.connected,
      });
      setDashboard(result);
    });
  }

  function saveProfile(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    void runAction("save", async () => {
      const saved = await invoke<Profile>("save_profile", { profile });
      setProfile(saved);
      setNotice("Settings saved");
    });
  }

  const target = dashboard.devices.find(
    (device) => device.name.toLowerCase() === profile.targetDevice.toLowerCase(),
  );
  const wakeTargetLabel = profile.hostname.trim() || "host";

  return (
    <main className="shell">
      <header className="topbar">
        <div className="brand-mark"><Activity size={19} strokeWidth={2.2} /></div>
        <div className="brand-copy">
          <strong>MOONSCALE</strong>
          <span>TAILSCALE CONTROL</span>
        </div>
        <div className="topbar-spacer" />
        <span className={`connection-indicator ${dashboard.connected ? "is-online" : ""}`}>
          <span className="indicator-dot" />
          {dashboard.connected ? "Connected" : dashboard.backendState}
        </span>
        <button className="icon-button" title="Refresh devices" aria-label="Refresh devices" onClick={() => void refresh()}>
          <RefreshCw size={16} />
        </button>
        <button
          className={`icon-button ${page === "settings" ? "selected" : ""}`}
          title="Settings"
          aria-label="Settings"
          onClick={() => setPage(page === "settings" ? "home" : "settings")}
        >
          <Settings2 size={17} />
        </button>
      </header>

      <section className="workspace">
        <div className="page-heading">
          <div>
            <p className="eyebrow">REMOTE PLAY</p>
            <h1>{page === "home" ? "Your devices" : "Settings"}</h1>
          </div>
          {page === "home" && (
            <button
              className={`connection-button ${dashboard.connected ? "disconnect" : "connect"}`}
              disabled={Boolean(busy) || dashboard.backendState === "Loading"}
              onClick={toggleConnection}
            >
              {busy === "connection" ? <LoaderCircle className="spin" size={16} /> : <Power size={16} />}
              {dashboard.connected ? "Disconnect" : "Connect"}
            </button>
          )}
        </div>

        {error && (
          <div className="message error-message" role="alert">
            <AlertTriangle size={17} />
            <span>{error}</span>
            <button className="text-button" onClick={() => setError("")}>Dismiss</button>
          </div>
        )}
        {notice && <div className="message success-message"><Check size={16} />{notice}</div>}

        {page === "home" ? (
          <div className="dashboard-grid">
            <section className="device-section">
              <div className="section-heading">
                <div>
                  <h2>Tailnet devices</h2>
                  <p>{dashboard.connected ? `${dashboard.devices.length} devices on your network` : "Tailscale connection status"}</p>
                </div>
                <span className={`section-count ${dashboard.connected ? "" : "section-count-offline"}`}>
                  {dashboard.backendState === "Loading" ? "CHECKING" : dashboard.connected ? `${dashboard.devices.filter((device) => device.online).length} ONLINE` : "OFFLINE"}
                </span>
              </div>
              <div className="device-list">
                {!dashboard.connected && dashboard.backendState !== "Loading" ? (
                  <div className="offline-state" role="status">
                    <AlertTriangle size={52} strokeWidth={1.6} aria-hidden="true" />
                    <strong>Tailscale is offline</strong>
                    <span>Connect to your tailnet to see your devices.</span>
                  </div>
                ) : dashboard.devices.map((device) => (
                  <div className={`device-row ${device.name === target?.name ? "device-selected" : ""}`} key={`${device.name}-${device.address}`}>
                    <div className={`device-icon ${device.online ? "device-icon-online" : ""}`}>
                      <Monitor size={17} />
                    </div>
                    <div className="device-info">
                      <strong>{device.name}{device.self ? <span className="self-tag">THIS DEVICE</span> : null}</strong>
                      <span>{device.address || "No Tailscale IP"}</span>
                    </div>
                    <span className={`device-state ${device.online ? "online" : "offline"}`}>
                      <span className="indicator-dot" />{device.online ? "Online" : "Offline"}
                    </span>
                  </div>
                ))}
                {dashboard.devices.length === 0 && (dashboard.connected || dashboard.backendState === "Loading") && (
                  <div className="empty-state">
                    {dashboard.connected ? <Wifi size={22} /> : <LoaderCircle className="spin" size={22} />}
                    <strong>{dashboard.connected ? "No devices found" : "Checking Tailscale…"}</strong>
                    {dashboard.connected && <span>Devices on your tailnet will appear here.</span>}
                  </div>
                )}
              </div>
            </section>

            <aside className="stream-section">
              <div className="section-heading stream-heading">
                <div>
                  <h2>Stream target</h2>
                  <p>Moonlight · {profile.resolution} · {profile.fps} FPS</p>
                </div>
                <span className="target-glyph"><Monitor size={19} /></span>
              </div>
              <div className="target-summary">
                <span className="eyebrow">HOST PC</span>
                <strong>{target?.name || profile.targetDevice || "Not configured"}</strong>
                <span className={target?.online ? "target-online" : "target-offline"}>
                  <span className="indicator-dot" />
                  {target?.online ? "Ready on tailnet" : target ? "Offline · can be woken" : "Set up your target in Settings"}
                </span>
              </div>
              <div className="stream-actions">
                <button
                  className="stream-button"
                  disabled={Boolean(busy) || !dashboard.connected || !profile.targetDevice}
                  onClick={() => void runAction("stream", async () => {
                    await invoke("wake_and_stream");
                    setNotice("Moonlight launched");
                    await refresh();
                  })}
                >
                  {busy === "stream" ? <LoaderCircle className="spin" size={17} /> : <Activity size={17} />}
                  {busy === "stream" ? "Waking host…" : "Start streaming"}
                </button>
                <button
                  className="wake-button"
                  disabled={Boolean(busy) || !profile.homeServerHost || !profile.wakeCommand}
                  onClick={() => void runAction("wake", async () => {
                    await invoke("wake_device");
                    setNotice("Wake command sent to your home server");
                  })}
                >
                  {busy === "wake" ? <LoaderCircle className="spin" size={16} /> : <Zap size={16} />}
                  {busy === "wake" ? `Waking ${wakeTargetLabel}…` : `Wake ${wakeTargetLabel}`}
                </button>
              </div>
              <div className="stream-footnote">
                <CircleHelp size={14} />
                <span>Moonlight must be paired with this PC once.</span>
              </div>
            </aside>
          </div>
        ) : (
          <form className="settings-form" onSubmit={saveProfile}>
            <section className="settings-section">
              <div className="section-heading">
                <div><h2>Streaming PC</h2><p>Moonlight connection and stream quality</p></div>
              </div>
              <div className="form-grid">
                <div className="device-picker-field">
                  <span className="field-label" id="tailnet-device-label">Tailnet device</span>
                  <button
                    className="device-picker-trigger"
                    type="button"
                    aria-labelledby="tailnet-device-label"
                    aria-haspopup="listbox"
                    aria-expanded={devicePickerOpen}
                    aria-controls="tailnet-device-options"
                    onClick={() => setDevicePickerOpen(!devicePickerOpen)}
                    onKeyDown={(event) => {
                      if (event.key === "Escape") setDevicePickerOpen(false);
                    }}
                  >
                    <span className="device-picker-current">
                      {target ? (
                        <>
                          <strong>{target.name}</strong>
                          <span>{target.address || "No Tailscale IP"}</span>
                        </>
                      ) : profile.targetDevice ? (
                        <>
                          <strong>{profile.targetDevice}</strong>
                          <span>Device not currently available</span>
                        </>
                      ) : (
                        <span>Select a device</span>
                      )}
                    </span>
                    <ChevronDown size={16} aria-hidden="true" />
                  </button>
                  {devicePickerOpen && (
                    <div className="device-picker-options" id="tailnet-device-options" role="listbox" aria-label="Tailnet devices">
                      {dashboard.devices.map((device) => (
                        <button
                          className="device-picker-option"
                          type="button"
                          role="option"
                          aria-selected={device.name === profile.targetDevice}
                          key={`${device.name}-${device.address}`}
                          onClick={() => {
                            setProfile({ ...profile, targetDevice: device.name });
                            setDevicePickerOpen(false);
                          }}
                        >
                          <span className="device-picker-current">
                            <strong>{device.name}</strong>
                            <span>{device.address || "No Tailscale IP"}</span>
                          </span>
                        </button>
                      ))}
                      {dashboard.devices.length === 0 && (
                        <span className="device-picker-empty">No Tailscale devices available</span>
                      )}
                    </div>
                  )}
                </div>
                <label>Moonlight executable<input value={profile.moonlightPath} onChange={(event) => setProfile({ ...profile, moonlightPath: event.target.value })} placeholder="moonlight" /></label>
                <label>App name<input value={profile.appName} onChange={(event) => setProfile({ ...profile, appName: event.target.value })} placeholder="Desktop" /></label>
                <label>Resolution<select value={profile.resolution} onChange={(event) => setProfile({ ...profile, resolution: event.target.value })}><option>720p</option><option>1080p</option><option>1440p</option><option>4K</option></select></label>
                <label>Frame rate<select value={profile.fps} onChange={(event) => setProfile({ ...profile, fps: Number(event.target.value) })}><option value={30}>30 FPS</option><option value={60}>60 FPS</option><option value={90}>90 FPS</option><option value={120}>120 FPS</option></select></label>
                <label>Bitrate<input type="number" min={1000} max={150000} step={1000} value={profile.bitrate} onChange={(event) => setProfile({ ...profile, bitrate: Number(event.target.value) })} /></label>
              </div>
            </section>
            <section className="settings-section">
              <div className="section-heading">
                <div><h2>Wake-on-LAN</h2><p>Run your existing wake command over SSH</p></div>
              </div>
              <div className="form-grid">
                <label>Home server<input value={profile.homeServerHost} onChange={(event) => setProfile({ ...profile, homeServerHost: event.target.value })} placeholder="homeserver or IP" /></label>
                <label>SSH user<input value={profile.homeServerUser} onChange={(event) => setProfile({ ...profile, homeServerUser: event.target.value })} placeholder="username" /></label>
                <label>Hostname<input value={profile.hostname} onChange={(event) => setProfile({ ...profile, hostname: event.target.value })} placeholder="gaming-pc" /></label>
                <label>SSH private key path <span className="optional-label">optional</span><input value={profile.sshKeyPath} onChange={(event) => setProfile({ ...profile, sshKeyPath: event.target.value })} placeholder="~/.ssh/id_ed25519" /></label>
                <label>Wake command on server<input value={profile.wakeCommand} onChange={(event) => setProfile({ ...profile, wakeCommand: event.target.value })} placeholder="wakeonlan AA:BB:CC:DD:EE:FF" /></label>
                <label>Wait for PC (seconds)<input type="number" min={15} max={600} value={profile.wakeTimeoutSeconds} onChange={(event) => setProfile({ ...profile, wakeTimeoutSeconds: Number(event.target.value) })} /></label>
              </div>
            </section>
            <div className="settings-footer">
              <span>Profile stored on this device only.</span>
              <button className="connection-button connect" type="submit" disabled={Boolean(busy)}>
                {busy === "save" ? <LoaderCircle className="spin" size={16} /> : <Check size={16} />}
                Save settings
              </button>
            </div>
          </form>
        )}
      </section>

      <footer className="statusbar">
        <span className="statusbar-left"><span className={`indicator-dot ${dashboard.connected ? "dot-online" : ""}`} />Tailscale {dashboard.backendState}</span>
        <span className="statusbar-right"><span className="statusbar-brand">MOONSCALE</span><span>Linux · 0.1.0</span></span>
      </footer>
    </main>
  );
}

export default App;