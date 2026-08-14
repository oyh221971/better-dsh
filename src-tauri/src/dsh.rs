use serde::Serialize;
use std::{
    net::{TcpStream, ToSocketAddrs},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::Duration,
};

pub const DEFAULT_PORT: u16 = 3080;
const START_TIMEOUT_TICKS: u32 = 60; // 60 * 500ms = 30s

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DshStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: u16,
    pub url: String,
    pub dsh_version: Option<String>,
    pub launcher: Option<String>,
}

pub struct DshManager {
    port: u16,
    data_dir: PathBuf,
    child: Mutex<Option<Child>>,
    version_cache: Mutex<Option<String>>,
}

impl DshManager {
    pub fn new(port: u16, data_dir: PathBuf) -> Self {
        Self {
            port,
            data_dir,
            child: Mutex::new(None),
            version_cache: Mutex::new(None),
        }
    }

    #[allow(dead_code)]
    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    fn pidfile_path(&self) -> PathBuf {
        self.data_dir.join("dsh.pid")
    }

    pub fn status(&self) -> DshStatus {
        let running = is_port_listening(self.port);
        let pid = if running { listener_pid(self.port) } else { None };
        let launcher = Self::resolve_dsh().map(|p| p.display().to_string());
        let mut cache = self.version_cache.lock().unwrap();
        if cache.is_none() {
            *cache = query_version();
        }
        DshStatus {
            running,
            pid,
            port: self.port,
            url: self.url(),
            dsh_version: cache.clone(),
            launcher,
        }
    }

    pub fn resolve_dsh() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("DSH_LAUNCHER") {
            let pb = PathBuf::from(&p);
            if pb.exists() {
                return Some(pb);
            }
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                for candidate in ["dsh.cmd", "dsh.exe", "dsh"] {
                    let bundled = dir.join("dsh-runtime").join(candidate);
                    if bundled.exists() {
                        return Some(bundled);
                    }
                    let sidecar = dir.join(candidate);
                    if sidecar.exists() {
                        return Some(sidecar);
                    }
                }
            }
        }
        for name in ["dsh.cmd", "dsh.exe", "dsh"] {
            if let Some(found) = find_on_path(name) {
                return Some(found);
            }
        }
        None
    }

    pub fn start(&self) -> Result<DshStatus, String> {
        if is_port_listening(self.port) {
            return Ok(self.status());
        }
        let dsh = Self::resolve_dsh()
            .ok_or_else(|| "找不到 dsh 可执行文件，请先安装 @deepseek-ai/dsh 或设置 DSH_LAUNCHER".to_string())?;

        std::fs::create_dir_all(self.logs_dir()).map_err(|e| e.to_string())?;
        let stdout_path = self.logs_dir().join("dsh-web.stdout.log");
        let stderr_path = self.logs_dir().join("dsh-web.stderr.log");
        let stdout = std::fs::File::create(&stdout_path).map_err(|e| e.to_string())?;
        let stderr = std::fs::File::create(&stderr_path).map_err(|e| e.to_string())?;

        let mut cmd = dsh_command(&dsh);
        cmd.arg("web")
            .arg("--port")
            .arg(self.port.to_string());
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::from(stdout));
        cmd.stderr(Stdio::from(stderr));

        let child = cmd.spawn().map_err(|e| format!("启动 dsh 失败: {e}"))?;
        *self.child.lock().unwrap() = Some(child);

        let mut ok = false;
        for _ in 0..START_TIMEOUT_TICKS {
            if is_port_listening(self.port) {
                ok = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        if !ok {
            return Err(format!(
                "dsh 启动超时（{} 秒内未监听端口 {}），请查看日志：{}",
                START_TIMEOUT_TICKS * 500 / 1000,
                self.port,
                stderr_path.display()
            ));
        }
        let pid = listener_pid(self.port)
            .or_else(|| self.child.lock().unwrap().as_ref().map(|c| c.id()));
        if let Some(pid) = pid {
            let _ = std::fs::write(self.pidfile_path(), pid.to_string());
        }
        Ok(self.status())
    }

    pub fn stop(&self) -> Result<DshStatus, String> {
        if let Some(pid) = listener_pid(self.port) {
            let _ = kill_tree(pid);
        }
        if let Some(mut child) = self.child.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Ok(raw) = std::fs::read_to_string(self.pidfile_path()) {
            if let Ok(pid) = raw.trim().parse::<u32>() {
                let _ = kill_tree(pid);
            }
        }
        let _ = std::fs::remove_file(self.pidfile_path());
        for _ in 0..15 {
            if !is_port_listening(self.port) {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        Ok(self.status())
    }
}

fn is_port_listening(port: u16) -> bool {
    let addr = format!("127.0.0.1:{port}");
    let mut addrs = match addr.to_socket_addrs() {
        Ok(a) => a,
        Err(_) => return false,
    };
    if let Some(sa) = addrs.next() {
        if TcpStream::connect_timeout(&sa, Duration::from_millis(300)).is_ok() {
            return true;
        }
    }
    false
}

fn listener_pid(port: u16) -> Option<u32> {
    let mut cmd = Command::new("netstat");
    console_hidden(&mut cmd);
    let out = cmd.args(["-ano", "-p", "tcp"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let needle = format!(":{port}");
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 5 && fields[0].eq_ignore_ascii_case("tcp") {
            let local = fields[1];
            let state = fields[3];
            if state.eq_ignore_ascii_case("listening") && local.ends_with(&needle) {
                if let Ok(pid) = fields[4].parse::<u32>() {
                    return Some(pid);
                }
            }
        }
    }
    None
}

fn kill_tree(pid: u32) -> Result<(), String> {
    let mut cmd = Command::new("taskkill");
    console_hidden(&mut cmd);
    let out = cmd
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let full = dir.join(name);
        if full.is_file() {
            return Some(full);
        }
    }
    None
}

/// Build a Command that launches the dsh CLI without creating a console
/// window. When the launcher is an npm `.cmd` shim, run `node lib/bin.js`
/// directly so no `cmd.exe` appears in the process chain.
fn dsh_command(dsh: &Path) -> Command {
    let mut cmd = match dsh_to_node(dsh) {
        Some((node, bin)) => {
            let mut c = Command::new(node);
            c.arg(bin);
            c
        }
        None => {
            let mut c = Command::new("cmd");
            c.arg("/c").arg(dsh);
            c
        }
    };
    console_hidden(&mut cmd);
    cmd
}

/// If `dsh` is a standard npm `.cmd` shim, resolve it to
/// `node <dir>\node_modules\@deepseek-ai\dsh\lib\bin.js`.
fn dsh_to_node(dsh: &Path) -> Option<(PathBuf, String)> {
    if dsh.extension().and_then(|e| e.to_str()) != Some("cmd") {
        return None;
    }
    let dir = dsh.parent()?;
    let bin = dir
        .join("node_modules")
        .join("@deepseek-ai")
        .join("dsh")
        .join("lib")
        .join("bin.js");
    if !bin.is_file() {
        return None;
    }
    let node = if dir.join("node.exe").is_file() {
        dir.join("node.exe")
    } else {
        PathBuf::from("node")
    };
    Some((node, bin.to_string_lossy().into_owned()))
}

/// Keep console-subsystem children (cmd, node, netstat, taskkill) from
/// flashing a window when spawned from the windowless Tauri process.
fn console_hidden(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

fn query_version() -> Option<String> {
    let dsh = DshManager::resolve_dsh()?;
    let out = dsh_command(&dsh).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
