import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface DshStatus {
  running: boolean;
  pid: number | null;
  port: number;
  url: string;
  dshVersion: string | null;
  launcher: string | null;
}

function $(id: string): HTMLElement {
  return document.getElementById(id)!;
}

const dot = $("status-dot");
const statusText = $("status-text");
const metaUrl = $("meta-url");
const metaPid = $("meta-pid");
const metaVersion = $("meta-version");
const metaLauncher = $("meta-launcher");
const btnStart = $("btn-start") as HTMLButtonElement;
const btnStop = $("btn-stop") as HTMLButtonElement;
const btnOpen = $("btn-open") as HTMLButtonElement;
const chkExitStop = $("chk-exit-stop") as HTMLInputElement;
const logLine = $("log-line");

function setStatus(label: string, ok: boolean, busy: boolean) {
  statusText.textContent = label;
  dot.className = busy ? "dot busy" : ok ? "dot ok" : "dot off";
}

function render(s: DshStatus) {
  metaUrl.textContent = s.url;
  metaVersion.textContent = s.dshVersion ?? "未找到";
  metaLauncher.textContent = s.launcher ?? "未找到（请先安装 @deepseek-ai/dsh）";
  if (s.running) {
    setStatus("运行中", true, false);
    metaPid.textContent = s.pid != null ? String(s.pid) : "-";
    btnStart.disabled = true;
    btnStop.disabled = false;
    btnOpen.disabled = false;
  } else {
    setStatus("未运行", false, false);
    metaPid.textContent = "-";
    btnStart.disabled = false;
    btnStop.disabled = true;
    btnOpen.disabled = true;
  }
}

async function refresh() {
  try {
    const s = await invoke<DshStatus>("dsh_status");
    render(s);
  } catch (e) {
    setStatus("状态查询失败", false, false);
    logLine.textContent = String(e);
  }
}

async function runAction(action: string, busyLabel: string) {
  try {
    setStatus(busyLabel, true, true);
    const s = await invoke<DshStatus>(action);
    render(s);
  } catch (e) {
    setStatus("操作失败", false, false);
    logLine.textContent = String(e);
  }
}

btnStart.addEventListener("click", () => runAction("dsh_start", "启动中…"));
btnStop.addEventListener("click", () => runAction("dsh_stop", "停止中…"));
btnOpen.addEventListener("click", async () => {
  try {
    await invoke("open_harness");
  } catch (e) {
    logLine.textContent = String(e);
  }
});

chkExitStop.addEventListener("change", () => {
  invoke("set_exit_stops_dsh", { enabled: chkExitStop.checked });
});

listen("dsh-status", (event) => {
  render(event.payload as DshStatus);
});

refresh();
setInterval(refresh, 2000);
