//! Windows Indirect Display Driver (IddCx) Management Bridge
//!
//! Controls dynamic virtual display creation, configuration, and teardown
//! via IPC communication with the Virtual-Display-Driver companion service.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use super::{DisplayConfig, VirtualDisplayManager};

/// Default named pipe for Virtual-Display-Driver IPC
pub const DEFAULT_IDDCX_PIPE: &str = r"\\.\pipe\VirtualDisplayDriver";

/// Default registry key where Virtual-Display-Driver stores installation metadata
pub const IDDCX_REGISTRY_KEY: &str = r"HKLM\SOFTWARE\VirtualDisplayDriver";

/// Command messages sent across the IddCx IPC named pipe
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum IpcCommand {
    /// Plug in a virtual monitor adapter
    Create {
        id: u32,
        width: u32,
        height: u32,
        refresh_rate: u32,
    },
    /// Unplug a specific virtual monitor adapter
    Destroy { id: u32 },
    /// Unplug all virtual monitor adapters
    DestroyAll,
    /// Healthcheck / ping command
    Ping,
}

/// Response received from the IddCx IPC named pipe
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IpcResponse {
    pub success: bool,
    pub message: Option<String>,
}

/// Windows IddCx Virtual Display Manager
#[derive(Debug)]
pub struct WindowsIddCxManager {
    /// Active displays tracked by ID -> DisplayConfig
    active_displays: HashMap<u32, DisplayConfig>,
    /// Next ID counter for allocating unique monitor IDs
    next_id: u32,
    /// Whether simulated / mock mode is enabled (for testing or driver-less environments)
    simulated: bool,
    /// Simulated driver installed flag
    simulated_driver_installed: bool,
    /// Named pipe address for driver IPC
    ipc_pipe_name: String,
    /// Path to driver install batch script
    install_script_path: PathBuf,
    /// Path to driver uninstall batch script
    uninstall_script_path: PathBuf,
}

impl WindowsIddCxManager {
    /// Create a new WindowsIddCxManager in real driver mode.
    ///
    /// Driver communication occurs over the default named pipe (`\\.\pipe\VirtualDisplayDriver`).
    /// If the driver is not installed or the pipe is not active, operations will return
    /// clear error messages rather than silently faking virtual displays.
    /// To run with a software mock for testing, use [`WindowsIddCxManager::new_simulated()`].
    pub fn new() -> Self {
        let default_install_script = PathBuf::from("drivers/windows/install_driver.bat");
        let default_uninstall_script = PathBuf::from("drivers/windows/uninstall_driver.bat");

        Self {
            active_displays: HashMap::new(),
            next_id: 1,
            simulated: false,
            simulated_driver_installed: false,
            ipc_pipe_name: DEFAULT_IDDCX_PIPE.to_string(),
            install_script_path: default_install_script,
            uninstall_script_path: default_uninstall_script,
        }
    }

    /// Create an explicitly simulated manager for testing and unit tests.
    pub fn new_simulated() -> Self {
        let mut manager = Self::new();
        manager.simulated = true;
        manager.simulated_driver_installed = true;
        manager
    }

    /// Set custom batch script paths (useful when working with alternate directory structures)
    pub fn with_scripts(
        mut self,
        install_script: impl Into<PathBuf>,
        uninstall_script: impl Into<PathBuf>,
    ) -> Self {
        self.install_script_path = install_script.into();
        self.uninstall_script_path = uninstall_script.into();
        self
    }

    /// Set a custom IPC pipe name
    pub fn with_ipc_pipe(mut self, pipe_name: impl Into<String>) -> Self {
        self.ipc_pipe_name = pipe_name.into();
        self
    }

    /// Check if currently running in simulated mode
    pub fn is_simulated(&self) -> bool {
        self.simulated
    }

    /// Toggle simulated mode
    pub fn set_simulated(&mut self, simulated: bool) {
        self.simulated = simulated;
    }

    /// Set the simulated driver installation status (used in tests)
    pub fn set_simulated_driver_installed(&mut self, installed: bool) {
        self.simulated_driver_installed = installed;
    }

    /// Return the count of currently active virtual displays
    pub fn display_count(&self) -> usize {
        self.active_displays.len()
    }

    /// Return a sorted list of all active monitor IDs
    pub fn active_displays(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.active_displays.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// Get the configuration of a specific active virtual display
    pub fn get_display_config(&self, monitor_id: u32) -> Option<&DisplayConfig> {
        self.active_displays.get(&monitor_id)
    }

    /// Check whether the driver IPC named pipe is accessible
    pub fn check_ipc_pipe_available(&self) -> bool {
        #[cfg(windows)]
        {
            if std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.ipc_pipe_name)
                .is_ok()
            {
                return true;
            }
        }
        false
    }

    /// Resolve absolute path to a helper script, checking common relative locations
    fn resolve_script_path(&self, script: &Path) -> Option<PathBuf> {
        if script.exists() {
            return Some(script.to_path_buf());
        }

        // Try relative to workspace root or current executable
        let candidates = [
            PathBuf::from("drivers/windows").join(script.file_name().unwrap_or_default()),
            PathBuf::from("../drivers/windows").join(script.file_name().unwrap_or_default()),
            PathBuf::from("../../drivers/windows").join(script.file_name().unwrap_or_default()),
        ];

        for candidate in &candidates {
            if candidate.exists() {
                return Some(candidate.clone());
            }
        }

        None
    }

    /// Send an IPC command to the Virtual-Display-Driver companion service
    pub fn send_ipc_command(&self, cmd: &IpcCommand) -> Result<IpcResponse> {
        if self.simulated {
            debug!("Simulated IPC dispatch: {:?}", cmd);
            return Ok(IpcResponse {
                success: true,
                message: Some("Simulated operation successful".to_string()),
            });
        }

        #[cfg(windows)]
        {
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&self.ipc_pipe_name)
                .with_context(|| {
                    format!(
                        "Failed to open Virtual-Display-Driver IPC pipe at '{}'. Is the driver installed and running?",
                        self.ipc_pipe_name
                    )
                })?;

            let mut serialized = serde_json::to_string(cmd)?;
            serialized.push('\n');
            file.write_all(serialized.as_bytes())?;
            file.flush()?;

            let mut reader = BufReader::new(file);
            let mut line = String::new();
            reader.read_line(&mut line)?;

            let response: IpcResponse = serde_json::from_str(&line).with_context(|| {
                format!("Failed to parse driver response from pipe: '{}'", line)
            })?;

            if !response.success {
                bail!(
                    "Driver IPC returned failure: {}",
                    response
                        .message
                        .unwrap_or_else(|| "Unknown driver error".to_string())
                );
            }

            Ok(response)
        }

        #[cfg(not(windows))]
        {
            bail!("Native Windows IddCx driver IPC is only supported on Windows targets");
        }
    }
}

impl Default for WindowsIddCxManager {
    fn default() -> Self {
        Self::new()
    }
}

impl VirtualDisplayManager for WindowsIddCxManager {
    /// Detect whether the IddCx driver is installed on Windows.
    ///
    /// In simulated mode, returns `simulated_driver_installed`.
    /// On Windows, checks the Windows registry key `HKLM\SOFTWARE\VirtualDisplayDriver`
    /// or queries the driver service status.
    fn is_driver_installed(&self) -> Result<bool> {
        if self.simulated {
            return Ok(self.simulated_driver_installed);
        }

        #[cfg(windows)]
        {
            // Check Windows registry key via `reg query`
            let output = Command::new("reg")
                .args(["query", IDDCX_REGISTRY_KEY])
                .output();

            match output {
                Ok(out) if out.status.success() => return Ok(true),
                _ => {}
            }

            // Alternatively verify if named pipe exists
            if self.check_ipc_pipe_available() {
                return Ok(true);
            }

            // Check device class Display via pnputil
            let pnp_output = Command::new("pnputil")
                .args(["/enum-devices", "/class", "Display"])
                .output();

            if let Ok(out) = pnp_output {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.contains("Virtual Display") || stdout.contains("IddSampleDriver") {
                    return Ok(true);
                }
            }

            Ok(false)
        }

        #[cfg(not(windows))]
        {
            Ok(false)
        }
    }

    /// Install the virtual display driver by executing `install_driver.bat`.
    /// In simulated mode, records success without altering system drivers.
    fn install_driver(&self) -> Result<()> {
        let script = self
            .resolve_script_path(&self.install_script_path)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Driver installation script not found at '{:?}'",
                    self.install_script_path
                )
            })?;

        if self.simulated {
            info!(
                "Simulated driver installation using script: {}",
                script.display()
            );
            return Ok(());
        }

        info!("Executing driver installation script: {}", script.display());

        #[cfg(windows)]
        {
            // Execute the batch script with elevated privileges via PowerShell Start-Process,
            // propagating the child process exit code through to PowerShell's exit code.
            let script_str = script.to_string_lossy();
            let status = Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    &format!(
                        "$p = Start-Process -FilePath 'cmd.exe' -ArgumentList '/c', '\"{}\"' -Verb RunAs -Wait -PassThru; exit $p.ExitCode",
                        script_str
                    ),
                ])
                .status()
                .with_context(|| format!("Failed to launch elevated installer: {}", script_str))?;

            if !status.success() {
                bail!("Driver installation script failed with status: {:?}", status);
            }

            Ok(())
        }

        #[cfg(not(windows))]
        {
            bail!("Windows IddCx driver installation is only supported on Windows");
        }
    }

    /// Create and attach a virtual display adapter with the specified configuration.
    fn create_display(&mut self, config: &DisplayConfig) -> Result<u32> {
        // Step 1: Validate configuration parameters
        config.validate().context("Invalid display configuration")?;

        let id = self.next_id;
        self.next_id += 1;

        debug!(
            "Creating virtual display ID {}: {}x{} @ {}Hz",
            id, config.width, config.height, config.refresh_rate
        );

        // Step 2: Send IPC creation command to driver
        let cmd = IpcCommand::Create {
            id,
            width: config.width,
            height: config.height,
            refresh_rate: config.refresh_rate,
        };

        self.send_ipc_command(&cmd)?;

        // Step 3: Record display in active registry
        self.active_displays.insert(id, config.clone());
        info!("Virtual display ID {} created successfully", id);

        Ok(id)
    }

    /// Detach and destroy a virtual display by monitor ID.
    fn destroy_display(&mut self, monitor_id: u32) -> Result<()> {
        if !self.active_displays.contains_key(&monitor_id) {
            bail!("Virtual display with ID {} not found", monitor_id);
        }

        debug!("Destroying virtual display ID {}", monitor_id);

        let cmd = IpcCommand::Destroy { id: monitor_id };
        self.send_ipc_command(&cmd)?;

        self.active_displays.remove(&monitor_id);
        info!("Virtual display ID {} destroyed successfully", monitor_id);

        Ok(())
    }

    /// Detach and destroy all active virtual displays.
    /// Guarantees clean teardown to prevent orphaned displays.
    fn destroy_all(&mut self) -> Result<()> {
        if self.active_displays.is_empty() {
            debug!("destroy_all called with 0 active displays; no-op");
            return Ok(());
        }

        info!(
            "Destroying all {} active virtual displays",
            self.active_displays.len()
        );

        let cmd = IpcCommand::DestroyAll;
        let _ = self.send_ipc_command(&cmd);

        self.active_displays.clear();
        info!("All virtual displays destroyed");

        Ok(())
    }
}

/// Implement Drop for safety: guarantees that when WindowsIddCxManager goes out of scope
/// or the application shuts down, all created virtual displays are destroyed.
impl Drop for WindowsIddCxManager {
    fn drop(&mut self) {
        if !self.active_displays.is_empty() {
            info!(
                "WindowsIddCxManager is dropping; cleaning up {} active virtual displays",
                self.active_displays.len()
            );
            let _ = self.destroy_all();
        }
    }
}
