#[cfg(target_os = "windows")]
use std::io::{self, Cursor};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::str::FromStr;

use bitcoin_hashes::{sha256, Hash};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use flate2::read::GzDecoder;
use iced::{Command, Subscription};
use liana::{
    config::{BitcoindConfig, BitcoindRpcAuth},
    miniscript::bitcoin::Network,
};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use tar::Archive;
use tracing::info;

use jsonrpc::{client::Client, simple_http::SimpleHttpTransport};

use liana_ui::{component::form, widget::*};

use crate::{
    bitcoind::{
        self, bitcoind_network_dir, internal_bitcoind_datadir, internal_bitcoind_directory,
        Bitcoind, ConfigField, InternalBitcoindConfig, InternalBitcoindConfigError,
        InternalBitcoindNetworkConfig, RpcAuth, RpcAuthType, RpcAuthValues,
        StartInternalBitcoindError, VERSION,
    },
    download,
    hw::HardwareWallets,
    installer::{
        context::Context,
        message::{self, Message},
        step::Step,
        view, Error,
    },
};

// The approach for tracking download progress is taken from
// https://github.com/iced-rs/iced/blob/master/examples/download_progress/src/main.rs.
#[derive(Debug)]
struct Download {
    id: usize,
    state: DownloadState,
}

#[derive(Debug)]
pub enum DownloadState {
    Idle,
    Downloading { progress: f32 },
    Finished(Vec<u8>),
    Errored(download::DownloadError),
}

impl Download {
    pub fn new(id: usize) -> Self {
        Download {
            id,
            state: DownloadState::Idle,
        }
    }

    pub fn start(&mut self) {
        match self.state {
            DownloadState::Idle { .. }
            | DownloadState::Finished { .. }
            | DownloadState::Errored { .. } => {
                self.state = DownloadState::Downloading { progress: 0.0 };
            }
            _ => {}
        }
    }

    pub fn progress(&mut self, new_progress: download::Progress) {
        if let DownloadState::Downloading { progress } = &mut self.state {
            match new_progress {
                download::Progress::Started => {
                    *progress = 0.0;
                }
                download::Progress::Advanced(percentage) => {
                    *progress = percentage;
                }
                download::Progress::Finished(bytes) => {
                    self.state = DownloadState::Finished(bytes);
                }
                download::Progress::Errored(e) => {
                    self.state = DownloadState::Errored(e);
                }
            }
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        match self.state {
            DownloadState::Downloading { .. } => download::file(self.id, bitcoind::download_url())
                .map(|(_, progress)| {
                    Message::InternalBitcoind(message::InternalBitcoindMsg::DownloadProgressed(
                        progress,
                    ))
                }),
            _ => Subscription::none(),
        }
    }
}

/// Default prune value used by internal bitcoind.
pub const PRUNE_DEFAULT: u32 = 15_000;
/// Default ports used by bitcoind across all networks.
pub const BITCOIND_DEFAULT_PORTS: [u16; 8] = [8332, 8333, 18332, 18333, 18443, 18444, 38332, 38333];

#[derive(Debug)]
pub enum InstallState {
    InProgress,
    Finished,
    Errored(InstallBitcoindError),
}

/// Possible errors when installing bitcoind.
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum InstallBitcoindError {
    HashMismatch,
    UnpackingError(String),
}

impl std::fmt::Display for InstallBitcoindError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::HashMismatch => {
                write!(f, "Hashes do not match.")
            }
            Self::UnpackingError(e) => {
                write!(f, "Error unpacking: '{}'.", e)
            }
        }
    }
}

// The functions below for unpacking the bitcoin download and verifying its hash are based on
// https://github.com/RCasatta/bitcoind/blob/bada7ebb7197b89fd67e607f815ce1e43e76da7f/build.rs#L73.

/// Unpack the downloaded bytes in the specified directory.
fn unpack_bitcoind(install_dir: &PathBuf, bytes: &[u8]) -> Result<(), InstallBitcoindError> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let d = GzDecoder::new(bytes);

        let mut archive = Archive::new(d);
        for mut entry in archive
            .entries()
            .map_err(|e| InstallBitcoindError::UnpackingError(e.to_string()))?
            .flatten()
        {
            if let Ok(file) = entry.path() {
                if file.ends_with("bitcoind") {
                    if let Err(e) = entry.unpack_in(install_dir) {
                        return Err(InstallBitcoindError::UnpackingError(e.to_string()));
                    }
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| InstallBitcoindError::UnpackingError(e.to_string()))?;
        for i in 0..zip::ZipArchive::len(&archive) {
            let mut file = archive
                .by_index(i)
                .map_err(|e| InstallBitcoindError::UnpackingError(e.to_string()))?;
            let outpath = match file.enclosed_name() {
                Some(path) => path.to_owned(),
                None => continue,
            };
            if outpath.file_name().map(|s| s.to_str()) == Some(Some("bitcoind.exe")) {
                let mut exe_path = PathBuf::from(install_dir);
                for d in outpath.iter() {
                    exe_path.push(d);
                }
                let parent = exe_path.parent().expect("bitcoind.exe should have parent.");
                std::fs::create_dir_all(parent)
                    .map_err(|e| InstallBitcoindError::UnpackingError(e.to_string()))?;
                let mut outfile = std::fs::File::create(&exe_path)
                    .map_err(|e| InstallBitcoindError::UnpackingError(e.to_string()))?;
                io::copy(&mut file, &mut outfile)
                    .map_err(|e| InstallBitcoindError::UnpackingError(e.to_string()))?;
                break;
            }
        }
    }
    Ok(())
}

/// Verify the download hash against the expected value.
fn verify_hash(bytes: &[u8]) -> bool {
    let bytes_hash = sha256::Hash::hash(bytes);
    info!("Download hash: '{}'.", bytes_hash);
    let expected_hash = sha256::Hash::from_str(bitcoind::SHA256SUM).expect("This cannot fail.");
    expected_hash == bytes_hash
}

/// Install bitcoind by verifying the download hash and unpacking in the specified directory.
fn install_bitcoind(install_dir: &PathBuf, bytes: &[u8]) -> Result<(), InstallBitcoindError> {
    if !verify_hash(bytes) {
        return Err(InstallBitcoindError::HashMismatch);
    };
    unpack_bitcoind(install_dir, bytes)
}

/// RPC address for internal bitcoind.
fn internal_bitcoind_address(rpc_port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), rpc_port)
}

fn bitcoind_default_datadir() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    let configs_dir = dirs::home_dir();

    #[cfg(not(target_os = "linux"))]
    let configs_dir = dirs::config_dir();

    if let Some(mut path) = configs_dir {
        #[cfg(target_os = "linux")]
        path.push(".bitcoin");

        #[cfg(not(target_os = "linux"))]
        path.push("Bitcoin");

        return Some(path);
    }
    None
}

fn bitcoind_default_cookie_path(network: &Network) -> Option<String> {
    if let Some(mut path) = bitcoind_default_datadir() {
        if let Some(dir) = bitcoind_network_dir(network) {
            path.push(dir);
        }
        path.push(".cookie");
        return path.to_str().map(|s| s.to_string());
    }
    None
}

fn bitcoind_default_address(network: &Network) -> String {
    match network {
        Network::Bitcoin => "127.0.0.1:8332".to_string(),
        Network::Testnet => "127.0.0.1:18332".to_string(),
        Network::Regtest => "127.0.0.1:18443".to_string(),
        Network::Signet => "127.0.0.1:38332".to_string(),
        _ => "127.0.0.1:8332".to_string(),
    }
}

/// Get available port that is valid for use by internal bitcoind.
// Modified from https://github.com/RCasatta/bitcoind/blob/f047740d7d0af935ff7360cf77429c5f294cfd59/src/lib.rs#L435
pub fn get_available_port() -> Result<u16, Error> {
    // Perform multiple attempts to get a valid port.
    for _ in 0..10 {
        // Using 0 as port lets the system assign a port available.
        let t = TcpListener::bind(("127.0.0.1", 0))
            .map_err(|e| Error::CannotGetAvailablePort(e.to_string()))?;
        let port = t
            .local_addr()
            .map(|s| s.port())
            .map_err(|e| Error::CannotGetAvailablePort(e.to_string()))?;
        if port_is_valid(&port) {
            return Ok(port);
        }
    }
    Err(Error::CannotGetAvailablePort(
        "Exhausted attempts".to_string(),
    ))
}

/// Checks if port is valid for use by internal bitcoind.
pub fn port_is_valid(port: &u16) -> bool {
    !BITCOIND_DEFAULT_PORTS.contains(port)
}

pub struct SelectBitcoindTypeStep {
    use_external: bool,
}

impl Default for SelectBitcoindTypeStep {
    fn default() -> Self {
        Self::new()
    }
}

impl From<SelectBitcoindTypeStep> for Box<dyn Step> {
    fn from(s: SelectBitcoindTypeStep) -> Box<dyn Step> {
        Box::new(s)
    }
}

impl SelectBitcoindTypeStep {
    pub fn new() -> Self {
        Self { use_external: true }
    }
}

impl Step for SelectBitcoindTypeStep {
    fn update(&mut self, _hws: &mut HardwareWallets, message: Message) -> Command<Message> {
        if let Message::SelectBitcoindType(msg) = message {
            match msg {
                message::SelectBitcoindTypeMsg::UseExternal(selected) => {
                    self.use_external = selected;
                }
            };
            return Command::perform(async {}, |_| Message::Next);
        };
        Command::none()
    }

    fn apply(&mut self, ctx: &mut Context) -> bool {
        if !self.use_external {
            if ctx.internal_bitcoind_config.is_none() {
                ctx.bitcoind_config = None; // Ensures internal bitcoind can be restarted in case user has switched selection.
            }
        } else {
            ctx.internal_bitcoind_config = None;
        }
        ctx.bitcoind_is_external = self.use_external;
        true
    }

    fn view(&self, _hws: &HardwareWallets, progress: (usize, usize)) -> Element<Message> {
        view::select_bitcoind_type(progress)
    }
}

pub struct DefineBitcoind {
    rpc_auth_vals: RpcAuthValues,
    selected_auth_type: RpcAuthType,
    address: form::Value<String>,
    is_running: Option<Result<(), Error>>,
    modal_displayed: bool,

    // Internal cache to detect network change.
    network: Option<Network>,
}

impl DefineBitcoind {
    pub fn new() -> Self {
        Self {
            rpc_auth_vals: RpcAuthValues::default(),
            selected_auth_type: RpcAuthType::CookieFile,
            address: form::Value::default(),
            is_running: None,
            modal_displayed: false,
            network: None,
        }
    }

    pub fn ping(&self) -> Command<Message> {
        let address = self.address.value.to_owned();
        let selected_auth_type = self.selected_auth_type;
        let rpc_auth_vals = self.rpc_auth_vals.clone();
        Command::perform(
            async move {
                let builder = match selected_auth_type {
                    RpcAuthType::CookieFile => {
                        let cookie_path = rpc_auth_vals.cookie_path.value;
                        let cookie = std::fs::read_to_string(cookie_path).map_err(|e| {
                            Error::Bitcoind(format!("Failed to read cookie file: {}", e))
                        })?;
                        SimpleHttpTransport::builder().cookie_auth(cookie)
                    }
                    RpcAuthType::UserPass => {
                        let user = rpc_auth_vals.user.value;
                        let password = rpc_auth_vals.password.value;
                        SimpleHttpTransport::builder().auth(user, Some(password))
                    }
                };
                let client = Client::with_transport(
                    builder
                        .url(&address)?
                        .timeout(std::time::Duration::from_secs(3))
                        .build(),
                );
                client.send_request(client.build_request("echo", &[]))?;
                Ok(())
            },
            |res| Message::DefineBitcoind(message::DefineBitcoind::PingBitcoindResult(res)),
        )
    }
}

impl Step for DefineBitcoind {
    fn load_context(&mut self, ctx: &Context) {
        if self.rpc_auth_vals.cookie_path.value.is_empty()
            // if network changed then the values must be reset to default.
            || self.network != Some(ctx.bitcoin_config.network)
        {
            self.rpc_auth_vals.cookie_path.value =
                bitcoind_default_cookie_path(&ctx.bitcoin_config.network).unwrap_or_default()
        }
        if self.address.value.is_empty()
            // if network changed then the values must be reset to default.
            || self.network != Some(ctx.bitcoin_config.network)
        {
            self.address.value = bitcoind_default_address(&ctx.bitcoin_config.network);
        }

        self.network = Some(ctx.bitcoin_config.network);
    }
    fn update(&mut self, _hws: &mut HardwareWallets, message: Message) -> Command<Message> {
        if let Message::DefineBitcoind(msg) = message {
            match msg {
                message::DefineBitcoind::PingBitcoind => {
                    self.is_running = None;
                    return self.ping();
                }
                message::DefineBitcoind::PingAndCloseModal => {
                    self.is_running = None;
                    self.modal_displayed = false;
                    return self.ping();
                }
                message::DefineBitcoind::PingBitcoindResult(res) => self.is_running = Some(res),
                message::DefineBitcoind::ConfigFieldEdited(field, value) => match field {
                    ConfigField::Address => {
                        self.is_running = None;
                        self.address.value.clone_from(&value);
                        self.address.valid = false;
                        if let Some((ip, port)) = value.rsplit_once(':') {
                            let port = u16::from_str(port);
                            let (ipv4, ipv6) = (Ipv4Addr::from_str(ip), Ipv6Addr::from_str(ip));
                            if port.is_ok() && (ipv4.is_ok() || ipv6.is_ok()) {
                                self.address.valid = true;
                            }
                        }
                    }
                    ConfigField::CookieFilePath => {
                        self.is_running = None;
                        self.rpc_auth_vals.cookie_path.value = value;
                        self.rpc_auth_vals.cookie_path.valid = true;
                    }
                    ConfigField::User => {
                        self.is_running = None;
                        self.rpc_auth_vals.user.value = value;
                        self.rpc_auth_vals.user.valid = true;
                    }
                    ConfigField::Password => {
                        self.is_running = None;
                        self.rpc_auth_vals.password.value = value;
                        self.rpc_auth_vals.password.valid = true;
                    }
                },
                message::DefineBitcoind::RpcAuthTypeSelected(auth_type) => {
                    self.is_running = None;
                    self.selected_auth_type = auth_type;
                }
                message::DefineBitcoind::DisplayModal(display) => {
                    self.modal_displayed = display;
                    if !display {
                        // this message is produced by 'cancel' button so we reload default address
                        if let Some(network) = self.network {
                            self.address.value = bitcoind_default_address(&network);
                        }
                    }
                }
            };
        };
        Command::none()
    }

    fn apply(&mut self, ctx: &mut Context) -> bool {
        let addr = std::net::SocketAddr::from_str(&self.address.value);
        let rpc_auth = match self.selected_auth_type {
            RpcAuthType::CookieFile => {
                if let Ok(path) = PathBuf::from_str(&self.rpc_auth_vals.cookie_path.value) {
                    Some(BitcoindRpcAuth::CookieFile(path))
                } else {
                    self.rpc_auth_vals.cookie_path.valid = false;
                    None
                }
            }
            RpcAuthType::UserPass => Some(BitcoindRpcAuth::UserPass(
                self.rpc_auth_vals.user.value.clone(),
                self.rpc_auth_vals.password.value.clone(),
            )),
        };
        match (rpc_auth, addr) {
            (None, Ok(_)) => false,
            (_, Err(_)) => {
                self.address.valid = false;
                false
            }
            (Some(rpc_auth), Ok(addr)) => {
                ctx.bitcoind_config = Some(BitcoindConfig { rpc_auth, addr });
                true
            }
        }
    }

    fn view(&self, _hws: &HardwareWallets, progress: (usize, usize)) -> Element<Message> {
        view::define_bitcoin(
            progress,
            &self.address,
            &self.rpc_auth_vals,
            &self.selected_auth_type,
            self.is_running.as_ref(),
            self.modal_displayed,
        )
    }

    fn load(&self) -> Command<Message> {
        self.ping()
    }

    fn skip(&self, ctx: &Context) -> bool {
        !ctx.bitcoind_is_external
    }
}

impl Default for DefineBitcoind {
    fn default() -> Self {
        Self::new()
    }
}

impl From<DefineBitcoind> for Box<dyn Step> {
    fn from(s: DefineBitcoind) -> Box<dyn Step> {
        Box::new(s)
    }
}

pub struct InternalBitcoindStep {
    liana_datadir: PathBuf,
    bitcoind_datadir: PathBuf,
    network: Network,
    started: Option<Result<(), StartInternalBitcoindError>>,
    exe_path: Option<PathBuf>,
    bitcoind_config: Option<BitcoindConfig>,
    internal_bitcoind_config: Option<InternalBitcoindConfig>,
    error: Option<String>,
    exe_download: Option<Download>,
    install_state: Option<InstallState>,
    internal_bitcoind: Option<Bitcoind>,
}

impl From<InternalBitcoindStep> for Box<dyn Step> {
    fn from(s: InternalBitcoindStep) -> Box<dyn Step> {
        Box::new(s)
    }
}

impl InternalBitcoindStep {
    pub fn new(liana_datadir: &PathBuf) -> Self {
        Self {
            liana_datadir: liana_datadir.clone(),
            bitcoind_datadir: internal_bitcoind_datadir(liana_datadir),
            network: Network::Bitcoin,
            started: None,
            exe_path: None,
            bitcoind_config: None,
            internal_bitcoind_config: None,
            error: None,
            exe_download: None,
            install_state: None,
            internal_bitcoind: None,
        }
    }
}

impl Step for InternalBitcoindStep {
    fn load_context(&mut self, ctx: &Context) {
        if self.exe_path.is_none() {
            // Check if current managed bitcoind version is already installed.
            // For new installations, we ignore any previous managed bitcoind versions that might be installed.
            let exe_path = bitcoind::internal_bitcoind_exe_path(&ctx.data_dir, VERSION);
            if exe_path.exists() {
                self.exe_path = Some(exe_path)
            } else if self.exe_download.is_none() {
                self.exe_download = Some(Download::new(0));
            };
        }
        if self.network != ctx.bitcoin_config.network {
            self.internal_bitcoind_config = None;
            self.network = ctx.bitcoin_config.network;
        }
        if let Some(Ok(_)) = self.started {
            // This case can arise if a user switches from internal bitcoind to external and back to internal.
            if ctx.bitcoind_config.is_none() {
                self.started = None; // So that internal bitcoind will be restarted.
            }
        }
    }
    fn update(&mut self, _hws: &mut HardwareWallets, message: Message) -> Command<Message> {
        if let Message::InternalBitcoind(msg) = message {
            match msg {
                message::InternalBitcoindMsg::Previous => {
                    if let Some(bitcoind) = &self.internal_bitcoind {
                        bitcoind.stop();
                    }
                    self.internal_bitcoind = None;
                    if let Some(download) = self.exe_download.as_ref() {
                        // Clear exe_download if not Finished.
                        if let DownloadState::Finished { .. } = download.state {
                        } else {
                            self.exe_download = None;
                        }
                    }
                    self.started = None; // clear both Ok and Err
                    return Command::perform(async {}, |_| Message::Previous);
                }
                message::InternalBitcoindMsg::Reload => {
                    return self.load();
                }
                message::InternalBitcoindMsg::DefineConfig => {
                    let mut conf = match InternalBitcoindConfig::from_file(
                        &bitcoind::internal_bitcoind_config_path(&self.bitcoind_datadir),
                    ) {
                        Ok(conf) => conf,
                        Err(InternalBitcoindConfigError::FileNotFound) => {
                            InternalBitcoindConfig::new()
                        }
                        Err(e) => {
                            self.error = Some(e.to_string());
                            return Command::none();
                        }
                    };
                    let (rpc_port, p2p_port) = if let Some(network_conf) =
                        conf.networks.get(&self.network)
                    {
                        (network_conf.rpc_port, network_conf.p2p_port)
                    } else {
                        match (get_available_port(), get_available_port()) {
                            (Ok(rpc_port), Ok(p2p_port)) => {
                                // In case ports are the same, user will need to click button again for another attempt.
                                if rpc_port == p2p_port {
                                    self.error = Some(
                                        "Could not get distinct ports. Please try again."
                                            .to_string(),
                                    );
                                    return Command::none();
                                }
                                (rpc_port, p2p_port)
                            }
                            (Ok(_), Err(e)) | (Err(e), Ok(_)) => {
                                self.error = Some(format!("Could not get available port: {}.", e));
                                return Command::none();
                            }
                            (Err(e1), Err(e2)) => {
                                self.error =
                                    Some(format!("Could not get available ports: {}; {}.", e1, e2));
                                return Command::none();
                            }
                        }
                    };
                    let (rpc_auth, rpc_password) = match RpcAuth::new("liana") {
                        Ok((rpc_auth, password)) => (rpc_auth, password),
                        Err(e) => {
                            self.error = Some(e.to_string());
                            return Command::none();
                        }
                    };
                    let bitcoind_config = BitcoindConfig {
                        rpc_auth: BitcoindRpcAuth::UserPass(rpc_auth.user.clone(), rpc_password),
                        addr: internal_bitcoind_address(rpc_port),
                    };
                    let network_conf = InternalBitcoindNetworkConfig {
                        rpc_port,
                        p2p_port,
                        prune: PRUNE_DEFAULT,
                        // Overwrite any previous entry for this network as we would no longer know the RPC password.
                        rpc_auth: Some(rpc_auth),
                    };
                    conf.networks.insert(self.network, network_conf);
                    if let Err(e) = conf.to_file(&bitcoind::internal_bitcoind_config_path(
                        &self.bitcoind_datadir,
                    )) {
                        self.error = Some(e.to_string());
                        return Command::none();
                    };
                    self.error = None;
                    self.internal_bitcoind_config = Some(conf);
                    self.bitcoind_config = Some(bitcoind_config);
                    return Command::perform(async {}, |_| {
                        Message::InternalBitcoind(message::InternalBitcoindMsg::Reload)
                    });
                }
                message::InternalBitcoindMsg::Download => {
                    if let Some(download) = &mut self.exe_download {
                        if let DownloadState::Idle = download.state {
                            info!("Downloading bitcoind version {}...", &bitcoind::VERSION);
                            download.start();
                        }
                    }
                }
                message::InternalBitcoindMsg::DownloadProgressed(progress) => {
                    if let Some(download) = self.exe_download.as_mut() {
                        download.progress(progress);
                        if let DownloadState::Finished(_) = &download.state {
                            info!("Download of bitcoind complete.");
                            return Command::perform(async {}, |_| {
                                Message::InternalBitcoind(message::InternalBitcoindMsg::Install)
                            });
                        }
                    }
                }
                message::InternalBitcoindMsg::Install => {
                    if let Some(download) = &self.exe_download {
                        if let DownloadState::Finished(bytes) = &download.state {
                            info!("Installing bitcoind...");
                            self.install_state = Some(InstallState::InProgress);
                            match install_bitcoind(
                                &internal_bitcoind_directory(&self.liana_datadir),
                                bytes,
                            ) {
                                Ok(_) => {
                                    info!("Installation of bitcoind complete.");
                                    self.install_state = Some(InstallState::Finished);
                                    self.exe_path = Some(bitcoind::internal_bitcoind_exe_path(
                                        &self.liana_datadir,
                                        VERSION,
                                    ));
                                    return Command::perform(async {}, |_| {
                                        Message::InternalBitcoind(
                                            message::InternalBitcoindMsg::Start,
                                        )
                                    });
                                }
                                Err(e) => {
                                    info!("Installation of bitcoind failed.");
                                    self.install_state = Some(InstallState::Errored(e.clone()));
                                    self.error = Some(e.to_string());
                                    return Command::none();
                                }
                            };
                        }
                    }
                }
                message::InternalBitcoindMsg::Start => {
                    if let Err(e) = self.bitcoind_datadir.canonicalize() {
                        self.started = Some(Err(
                            StartInternalBitcoindError::CouldNotCanonicalizeDataDir(e.to_string()),
                        ));
                        return Command::none();
                    };
                    let bitcoind_config = self
                        .bitcoind_config
                        .as_ref()
                        .expect("already added")
                        .clone();
                    match Bitcoind::start(&self.network, bitcoind_config, &self.liana_datadir) {
                        Err(e) => {
                            self.started =
                                Some(Err(StartInternalBitcoindError::CommandError(e.to_string())));
                            return Command::none();
                        }
                        Ok(bitcoind) => {
                            self.error = None;
                            self.started = Some(Ok(()));
                            self.internal_bitcoind = Some(bitcoind);
                        }
                    };
                }
            };
        };
        Command::none()
    }

    fn subscription(&self, _hws: &HardwareWallets) -> Subscription<Message> {
        if let Some(download) = self.exe_download.as_ref() {
            return download.subscription();
        }
        Subscription::none()
    }

    fn load(&self) -> Command<Message> {
        if self.internal_bitcoind_config.is_none() {
            return Command::perform(async {}, |_| {
                Message::InternalBitcoind(message::InternalBitcoindMsg::DefineConfig)
            });
        }
        if let Some(download) = &self.exe_download {
            if let DownloadState::Idle = download.state {
                return Command::perform(async {}, |_| {
                    Message::InternalBitcoind(message::InternalBitcoindMsg::Download)
                });
            }
        }
        if self.started.is_none() {
            return Command::perform(async {}, |_| {
                Message::InternalBitcoind(message::InternalBitcoindMsg::Start)
            });
        }
        Command::none()
    }

    fn apply(&mut self, ctx: &mut Context) -> bool {
        // Any errors have been handled as part of `message::InternalBitcoindMsg::Start`
        if let Some(Ok(_)) = self.started {
            ctx.bitcoind_config.clone_from(&self.bitcoind_config);
            ctx.internal_bitcoind_config
                .clone_from(&self.internal_bitcoind_config);
            ctx.internal_bitcoind.clone_from(&self.internal_bitcoind);
            self.error = None;
            return true;
        }
        false
    }

    fn view(&self, _hws: &HardwareWallets, progress: (usize, usize)) -> Element<Message> {
        view::start_internal_bitcoind(
            progress,
            self.exe_path.as_ref(),
            self.started.as_ref(),
            self.error.as_ref(),
            self.exe_download.as_ref().map(|d| &d.state),
            self.install_state.as_ref(),
        )
    }

    fn stop(&self) {
        // In case the installer is closed before changes written to context, stop bitcoind.
        if let Some(bitcoind) = &self.internal_bitcoind {
            bitcoind.stop();
        }
    }

    fn skip(&self, ctx: &Context) -> bool {
        ctx.bitcoind_is_external
    }
}

#[cfg(test)]
mod tests {
    use super::verify_hash;

    #[test]
    fn hash() {
        let bytes = "this is not bitcoin".as_bytes().to_vec();
        assert!(!verify_hash(&bytes));
    }
}
