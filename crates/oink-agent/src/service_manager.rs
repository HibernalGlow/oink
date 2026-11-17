//! Management for background LaunchAgent services and headless operation.
//!
//! On macOS, `oink-agent service` installs, removes, and inspects a user LaunchAgent
//! plist (`~/Library/LaunchAgents/com.hachiwii.oink-agent.plist`) running strictly
//! within the interactive GUI session (`Aqua`) for accessibility & pasteboard access.

use anyhow::Context;
use clap::Subcommand;
use std::path::{Path, PathBuf};

#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum ServiceCmd {
    /// Install user LaunchAgent on macOS (~/Library/LaunchAgents/com.hachiwii.oink-agent.plist)
    Install,
    /// Uninstall and unload user LaunchAgent on macOS
    Uninstall,
    /// Query background service & LaunchAgent status
    Status,
    /// Start the background service via launchctl
    Start,
    /// Stop the background service via launchctl
    Stop,
}

pub const LAUNCH_AGENT_LABEL: &str = "com.hachiwii.oink-agent";

#[cfg(target_os = "macos")]
pub fn plist_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME 环境变量未设置"))?;
    Ok(home
        .join("Library/LaunchAgents")
        .join(format!("{LAUNCH_AGENT_LABEL}.plist")))
}

#[cfg(target_os = "macos")]
pub fn generate_plist(exe_path: &Path, log_path: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LAUNCH_AGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>run</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>LimitLoadToSessionType</key>
    <string>Aqua</string>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
</dict>
</plist>
"#,
        exe_path.display(),
        log_path.display(),
        log_path.display()
    )
}

pub fn run(cmd: ServiceCmd) -> anyhow::Result<()> {
    match cmd {
        ServiceCmd::Install => install(),
        ServiceCmd::Uninstall => uninstall(),
        ServiceCmd::Status => status(),
        ServiceCmd::Start => start(),
        ServiceCmd::Stop => stop(),
    }
}

#[cfg(target_os = "macos")]
fn install() -> anyhow::Result<()> {
    let plist = plist_path()?;
    let exe = std::env::current_exe()
        .context("无法获取当前可执行文件路径")?
        .canonicalize()
        .context("无法解析当前可执行文件规范路径")?;
    let log_dir = oink_foundation::paths::log_dir();
    let log_file = log_dir.join("oink-service.log");

    if let Some(parent) = plist.parent() {
        std::fs::create_dir_all(parent).context("无法创建 LaunchAgents 目录")?;
    }
    std::fs::create_dir_all(&log_dir).context("无法创建日志目录")?;

    let content = generate_plist(&exe, &log_file);
    std::fs::write(&plist, content).context("无法写入 LaunchAgent plist 文件")?;

    // Unload existing service registration first if present
    let _ = std::process::Command::new("launchctl")
        .args(["unload", plist.to_str().unwrap()])
        .output();

    // Load and activate the agent
    let load_res = std::process::Command::new("launchctl")
        .args(["load", "-w", plist.to_str().unwrap()])
        .output();

    let success = match load_res {
        Ok(out) if out.status.success() => true,
        _ => {
            // Fallback to bootstrap for modern macOS
            let uid = unsafe { libc::getuid() };
            std::process::Command::new("launchctl")
                .args(["bootstrap", &format!("gui/{uid}"), plist.to_str().unwrap()])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
    };

    if success {
        println!("已成功安装并启动 macOS LaunchAgent 服务：{}", plist.display());
    } else {
        println!("LaunchAgent 配置文件已写入：{}", plist.display());
        println!("注意：launchctl 加载返回非零状态，可能需要在图形桌面会话中执行");
    }
    println!("程序路径：{} run", exe.display());
    println!("日志路径：{}", log_file.display());
    println!("会话类型：Aqua（交互式 GUI 会话，支持输入监听与剪贴板）");

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn install() -> anyhow::Result<()> {
    println!("在当前平台上，后台服务通过系统服务或直接运行管理：");
    #[cfg(windows)]
    println!("Windows 系统服务由安装包中的 OinkInput (oink-service.exe) 管理；开发模式可直接运行 `oink-agent run`。");
    #[cfg(not(windows))]
    println!("Linux 可使用 systemd 用户单元管理 `oink-agent run`。");
    Ok(())
}

#[cfg(target_os = "macos")]
fn uninstall() -> anyhow::Result<()> {
    let plist = plist_path()?;

    if plist.exists() {
        let _ = std::process::Command::new("launchctl")
            .args(["unload", plist.to_str().unwrap()])
            .output();
        let uid = unsafe { libc::getuid() };
        let _ = std::process::Command::new("launchctl")
            .args(["bootout", &format!("gui/{uid}/{LAUNCH_AGENT_LABEL}")])
            .output();

        std::fs::remove_file(&plist).context("无法删除 LaunchAgent plist 文件")?;
        println!("已移除 LaunchAgent 配置文件：{}", plist.display());
    } else {
        println!("LaunchAgent 配置文件不存在：{}", plist.display());
    }

    println!("已停止并卸载 {LAUNCH_AGENT_LABEL} 服务");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn uninstall() -> anyhow::Result<()> {
    println!("当前平台无需卸载 LaunchAgent。");
    Ok(())
}

#[cfg(target_os = "macos")]
fn status() -> anyhow::Result<()> {
    let plist = plist_path()?;
    let plist_installed = plist.exists();
    let launchctl_output = std::process::Command::new("launchctl")
        .args(["list", LAUNCH_AGENT_LABEL])
        .output()
        .ok();
    let loaded = launchctl_output.as_ref().map_or(false, |o| o.status.success());

    let data_dir = oink_foundation::paths::data_dir();
    let running = crate::instance::is_running(&data_dir).unwrap_or(false);
    let client_hello = crate::ipc::Client::connect(&data_dir).and_then(|c| c.hello()).ok();

    let status_json = serde_json::json!({
        "platform": "macos",
        "service_label": LAUNCH_AGENT_LABEL,
        "plist_path": plist.display().to_string(),
        "plist_installed": plist_installed,
        "launchctl_loaded": loaded,
        "backend_running": running,
        "service_info": client_hello,
    });

    println!("{}", serde_json::to_string_pretty(&status_json)?);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn status() -> anyhow::Result<()> {
    let data_dir = oink_foundation::paths::data_dir();
    let running = crate::instance::is_running(&data_dir).unwrap_or(false);
    let client_hello = crate::ipc::Client::connect(&data_dir).and_then(|c| c.hello()).ok();

    #[cfg(windows)]
    let windows_service = oink_winsvc::client::service_installation().unwrap_or(false);
    #[cfg(not(windows))]
    let windows_service = false;

    let status_json = serde_json::json!({
        "platform": std::env::consts::OS,
        "windows_system_service": windows_service,
        "backend_running": running,
        "service_info": client_hello,
    });

    println!("{}", serde_json::to_string_pretty(&status_json)?);
    Ok(())
}

#[cfg(target_os = "macos")]
fn start() -> anyhow::Result<()> {
    let plist = plist_path()?;
    anyhow::ensure!(
        plist.exists(),
        "LaunchAgent 未安装，请先执行 `oink-agent service install`"
    );

    let status = std::process::Command::new("launchctl")
        .args(["load", "-w", plist.to_str().unwrap()])
        .status()
        .context("执行 launchctl load 失败")?;

    if status.success() {
        println!("服务已通过 launchctl 启动");
    } else {
        let uid = unsafe { libc::getuid() };
        let _ = std::process::Command::new("launchctl")
            .args(["kickstart", "-k", &format!("gui/{uid}/{LAUNCH_AGENT_LABEL}")])
            .status();
        println!("已请求 launchctl kickstart 启动服务");
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn start() -> anyhow::Result<()> {
    anyhow::bail!("当前平台请使用 `oink-agent run` 启动服务");
}

#[cfg(target_os = "macos")]
fn stop() -> anyhow::Result<()> {
    let plist = plist_path()?;
    let _ = std::process::Command::new("launchctl")
        .args(["unload", plist.to_str().unwrap()])
        .output();
    let uid = unsafe { libc::getuid() };
    let _ = std::process::Command::new("launchctl")
        .args(["bootout", &format!("gui/{uid}/{LAUNCH_AGENT_LABEL}")])
        .output();
    println!("服务已停止");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn stop() -> anyhow::Result<()> {
    let dir = oink_foundation::paths::data_dir();
    crate::host::stop_service(&dir, std::time::Duration::from_secs(10))?;
    println!("服务已停止");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plist_content() {
        #[cfg(target_os = "macos")]
        {
            let exe = Path::new("/Applications/Oink.app/Contents/MacOS/oink-agent");
            let log = Path::new("/Users/test/Library/Logs/Oink/oink-service.log");
            let plist = generate_plist(exe, log);

            assert!(plist.contains(LAUNCH_AGENT_LABEL));
            assert!(plist.contains("<string>/Applications/Oink.app/Contents/MacOS/oink-agent</string>"));
            assert!(plist.contains("<string>run</string>"));
            assert!(plist.contains("<key>RunAtLoad</key>"));
            assert!(plist.contains("<key>KeepAlive</key>"));
            assert!(plist.contains("<key>LimitLoadToSessionType</key>\n    <string>Aqua</string>"));
            assert!(plist.contains("<string>/Users/test/Library/Logs/Oink/oink-service.log</string>"));
        }
    }

    #[test]
    fn test_service_cmd_variants() {
        assert_eq!(ServiceCmd::Install, ServiceCmd::Install);
        assert_eq!(ServiceCmd::Uninstall, ServiceCmd::Uninstall);
        assert_eq!(ServiceCmd::Status, ServiceCmd::Status);
        assert_eq!(ServiceCmd::Start, ServiceCmd::Start);
        assert_eq!(ServiceCmd::Stop, ServiceCmd::Stop);
    }
}
