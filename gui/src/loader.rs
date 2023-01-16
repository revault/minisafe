use std::convert::From;
use std::path::PathBuf;
use std::sync::Arc;

use iced::{
    widget::{Column, Container, ProgressBar, Row},
    Element,
};
use iced::{Alignment, Command, Length, Subscription};
use iced_native::{window, Event};
use log::{debug, info};

use liana::{
    config::{Config, ConfigError},
    StartupError,
};

use crate::{
    app::config::Config as GUIConfig,
    daemon::{client, embedded::EmbeddedDaemon, model::*, Daemon, DaemonError},
    ui::{
        component::{button, notification, text::*},
        icon,
        util::Collection,
    },
};

type Lianad = client::Lianad<client::jsonrpc::JsonRPCClient>;

pub struct Loader {
    pub datadir_path: Option<PathBuf>,
    pub gui_config: GUIConfig,

    should_exit: bool,
    step: Step,
}

pub enum Step {
    Connecting,
    StartingDaemon,
    Syncing {
        daemon: Arc<dyn Daemon + Sync + Send>,
        progress: f64,
    },
    Error(Box<Error>),
}

#[derive(Debug)]
pub enum Message {
    View(ViewMessage),
    Event(iced_native::Event),
    Syncing(Result<GetInfoResult, DaemonError>),
    Synced(
        GetInfoResult,
        Vec<Coin>,
        Vec<SpendTx>,
        Arc<dyn Daemon + Sync + Send>,
    ),
    Started(Result<Arc<dyn Daemon + Sync + Send>, Error>),
    Connected(Result<Arc<dyn Daemon + Sync + Send>, Error>),
}

impl Loader {
    pub fn new(datadir_path: Option<PathBuf>, gui_config: GUIConfig) -> (Self, Command<Message>) {
        (
            Loader {
                datadir_path,
                gui_config: gui_config.clone(),
                step: Step::Connecting,
                should_exit: false,
            },
            if let Some(path) = gui_config.daemon_config_path {
                Command::perform(start_daemon(path), Message::Started)
            } else if let Some(socket_path) = gui_config.daemon_rpc_path {
                Command::perform(connect(socket_path), Message::Connected)
            } else {
                Command::none()
            },
        )
    }

    fn on_start(&mut self, res: Result<Arc<dyn Daemon + Sync + Send>, Error>) -> Command<Message> {
        match res {
            Ok(daemon) => {
                self.step = Step::Syncing {
                    daemon: daemon.clone(),
                    progress: 0.0,
                };
                Command::perform(sync(daemon, false), Message::Syncing)
            }
            Err(e) => {
                self.step = Step::Error(Box::new(e));
                Command::none()
            }
        }
    }

    fn on_sync(&mut self, res: Result<GetInfoResult, DaemonError>) -> Command<Message> {
        match &mut self.step {
            Step::Syncing {
                daemon, progress, ..
            } => {
                match res {
                    Ok(info) => {
                        if (info.sync - 1.0_f64).abs() < f64::EPSILON {
                            let daemon = daemon.clone();
                            return Command::perform(
                                async move {
                                    let coins = daemon
                                        .list_coins()
                                        .map(|res| res.coins)
                                        .unwrap_or_else(|_| Vec::new());
                                    let spend_txs = daemon
                                        .list_spend_transactions()
                                        .unwrap_or_else(|_| Vec::new());
                                    (info, coins, spend_txs, daemon)
                                },
                                |res| Message::Synced(res.0, res.1, res.2, res.3),
                            );
                        } else {
                            *progress = info.sync
                        }
                    }
                    Err(e) => {
                        self.step = Step::Error(Box::new(e.into()));
                        return Command::none();
                    }
                };
                Command::perform(sync(daemon.clone(), true), Message::Syncing)
            }
            _ => Command::none(),
        }
    }

    pub fn stop(&mut self) {
        log::info!("Close requested");
        if let Step::Syncing { daemon, .. } = &mut self.step {
            if !daemon.is_external() {
                log::info!("Stopping internal daemon...");
                if let Some(d) = Arc::get_mut(daemon) {
                    d.stop().expect("Daemon is internal");
                    log::info!("Internal daemon stopped");
                    self.should_exit = true;
                }
            } else {
                self.should_exit = true;
            }
        } else {
            self.should_exit = true;
        }
    }

    pub fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::View(ViewMessage::Retry) => {
                let (loader, cmd) = Self::new(self.datadir_path.clone(), self.gui_config.clone());
                *self = loader;
                cmd
            }
            Message::Started(res) => self.on_start(res),
            Message::Connected(res) => self.on_start(res),
            Message::Syncing(res) => self.on_sync(res),
            Message::Event(Event::Window(window::Event::CloseRequested)) => {
                self.stop();
                Command::none()
            }
            _ => Command::none(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iced_native::subscription::events().map(Message::Event)
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn view(&self) -> Element<Message> {
        view(self.datadir_path.as_ref(), &self.step).map(Message::View)
    }
}

#[derive(Clone, Debug)]
pub enum ViewMessage {
    Retry,
    SwitchNetwork,
}

pub fn view<'a>(datadir_path: Option<&'a PathBuf>, step: &'a Step) -> Element<'a, ViewMessage> {
    match &step {
        Step::StartingDaemon => cover(
            None,
            Column::new()
                .width(Length::Fill)
                .push(ProgressBar::new(0.0..=1.0, 0.0).width(Length::Fill))
                .push(text("Starting daemon...")),
        ),
        Step::Connecting => cover(
            None,
            Column::new()
                .width(Length::Fill)
                .push(ProgressBar::new(0.0..=1.0, 0.0).width(Length::Fill))
                .push(text("Connecting to daemon...")),
        ),
        Step::Syncing { progress, .. } => cover(
            None,
            Column::new()
                .width(Length::Fill)
                .push(ProgressBar::new(0.0..=1.0, *progress as f32).width(Length::Fill))
                .push(text("Syncing the wallet with the blockchain...")),
        ),
        Step::Error(error) => cover(
            if matches!(error.as_ref(), Error::Daemon(DaemonError::Transport(_, _))) {
                Some(("Error while connecting to the external daemon", error))
            } else {
                Some(("Error while starting the internal daemon", error))
            },
            Column::new()
                .spacing(20)
                .width(Length::Fill)
                .align_items(Alignment::Center)
                .push(icon::plug_icon().size(100).width(Length::Units(300)))
                .push(
                    if matches!(
                        error.as_ref(),
                        Error::Daemon(DaemonError::Start(StartupError::Bitcoind(_)))
                    ) {
                        text("Liana failed to start, please check if bitcoind is running")
                    } else {
                        text("Liana failed to start")
                    },
                )
                .push(
                    Row::new()
                        .spacing(10)
                        .push_maybe(datadir_path.map(|_| {
                            button::border(None, "Use another Bitcoin network")
                                .on_press(ViewMessage::SwitchNetwork)
                        }))
                        .push(
                            button::primary(None, "Retry")
                                .width(Length::Units(200))
                                .on_press(ViewMessage::Retry),
                        ),
                ),
        ),
    }
}

pub fn cover<'a, T: 'a + Clone, C: Into<Element<'a, T>>>(
    warn: Option<(&'static str, &Error)>,
    content: C,
) -> Element<'a, T> {
    Column::new()
        .push_maybe(warn.map(|w| notification::warning(w.0.to_string(), w.1.to_string())))
        .push(
            Container::new(content)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .center_x()
                .center_y()
                .padding(50),
        )
        .into()
}

async fn connect(socket_path: PathBuf) -> Result<Arc<dyn Daemon + Sync + Send>, Error> {
    info!(
        "Searching for connect to external daemon at {}",
        socket_path.to_string_lossy(),
    );
    let client = client::jsonrpc::JsonRPCClient::new(socket_path);
    let daemon = Lianad::new(client);

    daemon.get_info()?;
    info!("Connected to external daemon");

    Ok(Arc::new(daemon))
}

// Daemon can start only if a config path is given.
pub async fn start_daemon(config_path: PathBuf) -> Result<Arc<dyn Daemon + Sync + Send>, Error> {
    debug!("starting liana daemon");

    let config = Config::from_file(Some(config_path)).map_err(Error::Config)?;

    let mut daemon = EmbeddedDaemon::new(config);
    daemon.start()?;

    Ok(Arc::new(daemon))
}

async fn sync(
    daemon: Arc<dyn Daemon + Sync + Send>,
    sleep: bool,
) -> Result<GetInfoResult, DaemonError> {
    if sleep {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    daemon.get_info()
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Error {
    Config(ConfigError),
    Daemon(DaemonError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Config(e) => write!(f, "Config error: {}", e),
            Self::Daemon(e) => write!(f, "Liana daemon error: {}", e),
        }
    }
}

impl From<ConfigError> for Error {
    fn from(error: ConfigError) -> Self {
        Error::Config(error)
    }
}

impl From<DaemonError> for Error {
    fn from(error: DaemonError) -> Self {
        Error::Daemon(error)
    }
}
