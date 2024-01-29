use liana::{
    config::{BitcoindConfig, BitcoindRpcAuth},
    miniscript::bitcoin::{self, Network},
};
use liana_ui::component::form;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time;

use tracing::{info, warn};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Current and previous managed bitcoind versions, in order of descending version.
pub const VERSIONS: [&str; 3] = ["26.0", "25.1", "25.0"];

/// Current managed bitcoind version for new installations.
pub const VERSION: &str = VERSIONS[0];

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub const SHA256SUM: &str = "6e9864d0f59d5b7e8769ee867dd4b1f91602584b5736796e37d292e5c34d885a";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const SHA256SUM: &str = "23e5ab226d9e01ffaadef5ffabe8868d0db23db952b90b0593652993680bb8ab";

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
pub const SHA256SUM: &str = "8d0e909280012d91d08f0321c53a3ceea064682ca635098910b33e4e94c82ed1";

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub fn download_filename() -> String {
    format!("bitcoin-{}-x86_64-apple-darwin.tar.gz", &VERSION)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn download_filename() -> String {
    format!("bitcoin-{}-x86_64-linux-gnu.tar.gz", &VERSION)
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn download_filename() -> String {
    format!("bitcoin-{}-win64.zip", &VERSION)
}

pub fn download_url() -> String {
    format!(
        "https://bitcoincore.org/bin/bitcoin-core-{}/{}",
        &VERSION,
        download_filename()
    )
}

pub fn internal_bitcoind_directory(liana_datadir: &PathBuf) -> PathBuf {
    let mut datadir = PathBuf::from(liana_datadir);
    datadir.push("bitcoind");
    datadir
}

/// Data directory used by internal bitcoind.
pub fn internal_bitcoind_datadir(liana_datadir: &PathBuf) -> PathBuf {
    let mut datadir = internal_bitcoind_directory(liana_datadir);
    datadir.push("datadir");
    datadir
}

/// Internal bitcoind executable path.
pub fn internal_bitcoind_exe_path(liana_datadir: &PathBuf, bitcoind_version: &str) -> PathBuf {
    internal_bitcoind_directory(liana_datadir)
        .join(format!("bitcoin-{}", bitcoind_version))
        .join("bin")
        .join(if cfg!(target_os = "windows") {
            "bitcoind.exe"
        } else {
            "bitcoind"
        })
}

/// Path of the `bitcoin.conf` file used by internal bitcoind.
pub fn internal_bitcoind_config_path(bitcoind_datadir: &PathBuf) -> PathBuf {
    let mut config_path = PathBuf::from(bitcoind_datadir);
    config_path.push("bitcoin.conf");
    config_path
}

/// Path of the cookie file used by internal bitcoind on a given network.
pub fn internal_bitcoind_cookie_path(bitcoind_datadir: &Path, network: &Network) -> PathBuf {
    let mut cookie_path = bitcoind_datadir.to_path_buf();
    if let Some(dir) = bitcoind_network_dir(network) {
        cookie_path.push(dir);
    }
    cookie_path.push(".cookie");
    cookie_path
}

/// Path of the cookie file used by internal bitcoind on a given network.
pub fn internal_bitcoind_debug_log_path(lianad_datadir: &PathBuf, network: Network) -> PathBuf {
    let mut debug_log_path = internal_bitcoind_datadir(lianad_datadir);
    if let Some(dir) = bitcoind_network_dir(&network) {
        debug_log_path.push(dir);
    }
    debug_log_path.push("debug.log");
    debug_log_path
}

pub fn bitcoind_network_dir(network: &Network) -> Option<String> {
    let dir = match network {
        Network::Bitcoin => {
            return None;
        }
        Network::Testnet => "testnet3",
        Network::Regtest => "regtest",
        Network::Signet => "signet",
        _ => panic!("Directory required for this network is unknown."),
    };
    Some(dir.to_string())
}

/// Possible errors when starting bitcoind.
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum StartInternalBitcoindError {
    CommandError(String),
    CouldNotCanonicalizeExePath(String),
    CouldNotCanonicalizeDataDir(String),
    CouldNotCanonicalizeCookiePath(String),
    CookieFileNotFound(String),
    BitcoinDError(String),
    ExecutableNotFound,
}

impl std::fmt::Display for StartInternalBitcoindError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::CommandError(e) => {
                write!(f, "Command to start bitcoind returned an error: {}", e)
            }
            Self::CouldNotCanonicalizeExePath(e) => {
                write!(f, "Failed to canonicalize executable path: {}", e)
            }
            Self::CouldNotCanonicalizeDataDir(e) => {
                write!(f, "Failed to canonicalize datadir: {}", e)
            }
            Self::CouldNotCanonicalizeCookiePath(e) => {
                write!(f, "Failed to canonicalize cookie path: {}", e)
            }
            Self::CookieFileNotFound(path) => {
                write!(
                    f,
                    "Cookie file was not found at the expected path: {}",
                    path
                )
            }
            Self::BitcoinDError(e) => write!(f, "bitcoind connection check failed: {}", e),
            Self::ExecutableNotFound => write!(f, "bitcoind executable not found."),
        }
    }
}
#[derive(Debug, Clone)]
pub struct Bitcoind {
    _process: Arc<std::process::Child>,
    pub config: BitcoindConfig,
}

impl Bitcoind {
    /// Start internal bitcoind for the given network.
    pub fn start(
        network: &bitcoin::Network,
        mut config: BitcoindConfig,
        liana_datadir: &PathBuf,
    ) -> Result<Self, StartInternalBitcoindError> {
        let bitcoind_datadir = internal_bitcoind_datadir(liana_datadir);
        // Find most recent bitcoind version available.
        let bitcoind_exe_path = VERSIONS
            .iter()
            .filter_map(|v| {
                let path = internal_bitcoind_exe_path(liana_datadir, v);
                if path.exists() {
                    Some(path)
                } else {
                    None
                }
            })
            .next()
            .ok_or(StartInternalBitcoindError::ExecutableNotFound)?;
        info!(
            "Found bitcoind executable at '{}'.",
            bitcoind_exe_path.to_string_lossy()
        );
        let datadir_path_str = bitcoind_datadir
            .canonicalize()
            .map_err(|e| StartInternalBitcoindError::CouldNotCanonicalizeDataDir(e.to_string()))?
            .to_str()
            .ok_or_else(|| {
                StartInternalBitcoindError::CouldNotCanonicalizeDataDir(
                    "Couldn't convert path to str.".to_string(),
                )
            })?
            .to_string();

        // See https://github.com/rust-lang/rust/issues/42869.
        #[cfg(target_os = "windows")]
        let datadir_path_str = datadir_path_str.replace("\\\\?\\", "").replace("\\\\?", "");

        let args = vec![
            format!("-chain={}", network.to_core_arg()),
            format!("-datadir={}", datadir_path_str),
        ];
        let mut command = std::process::Command::new(bitcoind_exe_path);

        #[cfg(target_os = "windows")]
        let command = command.creation_flags(CREATE_NO_WINDOW);

        let mut process = command
            .args(&args)
            // FIXME: can we pipe stderr to our logging system somehow?
            .stdout(std::process::Stdio::null())
            .spawn()
            .map_err(|e| StartInternalBitcoindError::CommandError(e.to_string()))?;

        // We've started bitcoind in the background, however it may fail to start for whatever
        // reason. And we need its JSONRPC interface to be available to continue. Thus wait for it
        // to have created the cookie file, regularly checking it did not fail to start.
        let cookie_path = internal_bitcoind_cookie_path(&bitcoind_datadir, network);
        loop {
            match process.try_wait() {
                Ok(None) => {}
                Err(e) => log::error!("Error while trying to wait for bitcoind: {}", e),
                Ok(Some(status)) => {
                    log::error!("Bitcoind exited with status '{}'", status);
                    return Err(StartInternalBitcoindError::CookieFileNotFound(
                        cookie_path.to_string_lossy().into_owned(),
                    ));
                }
            }
            if cookie_path.exists() {
                log::info!("Bitcoind seems to have successfully started.");
                break;
            }
            log::info!("Waiting for bitcoind to start.");
            thread::sleep(time::Duration::from_millis(500));
        }

        config.rpc_auth = BitcoindRpcAuth::CookieFile(cookie_path.canonicalize().map_err(|e| {
            StartInternalBitcoindError::CouldNotCanonicalizeCookiePath(e.to_string())
        })?);

        liana::BitcoinD::new(&config, "internal_bitcoind_start".to_string())
            .map_err(|e| StartInternalBitcoindError::BitcoinDError(e.to_string()))?;

        Ok(Self {
            config,
            _process: Arc::new(process),
        })
    }

    /// Stop (internal) bitcoind.
    pub fn stop(&self) {
        stop_bitcoind(&self.config);
    }
}

pub fn stop_bitcoind(config: &BitcoindConfig) -> bool {
    match liana::BitcoinD::new(config, "internal_bitcoind_stop".to_string()) {
        Ok(bitcoind) => {
            info!("Stopping internal bitcoind...");
            bitcoind.stop();
            info!("Stopped liana managed bitcoind");
            true
        }
        Err(e) => {
            warn!("Could not create interface to internal bitcoind: '{}'.", e);
            false
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RpcAuthType {
    CookieFile,
    UserPass,
}

impl fmt::Display for RpcAuthType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RpcAuthType::CookieFile => write!(f, "Cookie file path"),
            RpcAuthType::UserPass => write!(f, "User and password"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RpcAuthValues {
    pub cookie_path: form::Value<String>,
    pub user: form::Value<String>,
    pub password: form::Value<String>,
}
