pub mod cache;
pub mod config;
pub mod menu;
pub mod message;
pub mod state;
pub mod view;
pub mod wallet;

mod error;

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use iced::{clipboard, time, Command, Element, Subscription};
use iced_native::{window, Event};

pub use liana::config::Config as DaemonConfig;

pub use config::Config;
pub use message::Message;

use state::{CoinsPanel, CreateSpendPanel, Home, ReceivePanel, RecoveryPanel, SpendPanel, State};

use crate::{
    app::{cache::Cache, error::Error, menu::Menu, wallet::Wallet},
    daemon::Daemon,
};

pub struct App {
    should_exit: bool,
    state: Box<dyn State>,
    cache: Cache,
    config: Config,
    wallet: Wallet,
    daemon: Arc<dyn Daemon + Sync + Send>,
}

impl App {
    pub fn new(
        cache: Cache,
        wallet: Wallet,
        config: Config,
        daemon: Arc<dyn Daemon + Sync + Send>,
    ) -> (App, Command<Message>) {
        let state: Box<dyn State> = Home::new(wallet.clone(), &cache.coins).into();
        let cmd = state.load(daemon.clone());
        (
            Self {
                should_exit: false,
                state,
                cache,
                config,
                daemon,
                wallet,
            },
            cmd,
        )
    }

    fn load_state(&mut self, menu: &Menu) -> Command<Message> {
        self.state = match menu {
            menu::Menu::Settings => state::SettingsState::new(
                self.daemon.config().cloned(),
                &self.cache,
                self.daemon.is_external(),
            )
            .into(),
            menu::Menu::Home => Home::new(self.wallet.clone(), &self.cache.coins).into(),
            menu::Menu::Coins => CoinsPanel::new(
                &self.cache.coins,
                self.wallet.main_descriptor.timelock_value(),
            )
            .into(),
            menu::Menu::Recovery => RecoveryPanel::new(
                self.wallet.clone(),
                self.config.clone(),
                &self.cache.coins,
                self.wallet.main_descriptor.timelock_value(),
                self.cache.blockheight as u32,
            )
            .into(),
            menu::Menu::Receive => ReceivePanel::default().into(),
            menu::Menu::Spend => SpendPanel::new(
                self.wallet.clone(),
                self.config.clone(),
                &self.cache.spend_txs,
            )
            .into(),
            menu::Menu::CreateSpendTx => CreateSpendPanel::new(
                self.wallet.clone(),
                self.config.clone(),
                &self.cache.coins,
                self.cache.blockheight as u32,
            )
            .into(),
        };
        self.state.load(self.daemon.clone())
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            iced_native::subscription::events().map(Message::Event),
            time::every(Duration::from_secs(5)).map(|_| Message::Tick),
            self.state.subscription(),
        ])
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn stop(&mut self) {
        log::info!("Close requested");
        if !self.daemon.is_external() {
            log::info!("Stopping internal daemon...");
            if let Some(d) = Arc::get_mut(&mut self.daemon) {
                d.stop().expect("Daemon is internal");
                log::info!("Internal daemon stopped");
                self.should_exit = true;
            }
        } else {
            self.should_exit = true;
        }
    }

    pub fn update(&mut self, message: Message) -> Command<Message> {
        // Update cache when values are passing by.
        // State will handle the error case.
        match &message {
            Message::Coins(Ok(coins)) => {
                self.cache.coins = coins.clone();
            }
            Message::SpendTxs(Ok(txs)) => {
                self.cache.spend_txs = txs.clone();
            }
            Message::Info(Ok(info)) => {
                self.cache.blockheight = info.block_height;
                self.cache.rescan_progress = info.rescan_progress;
            }
            Message::StartRescan(Ok(())) => {
                self.cache.rescan_progress = Some(0.0);
            }
            _ => {}
        };

        match message {
            Message::Tick => {
                let daemon = self.daemon.clone();
                Command::perform(
                    async move { daemon.get_info().map_err(|e| e.into()) },
                    Message::Info,
                )
            }
            Message::LoadDaemonConfig(cfg) => {
                let res = self.load_daemon_config(*cfg);
                self.update(Message::DaemonConfigLoaded(res))
            }
            Message::View(view::Message::Menu(menu)) => self.load_state(&menu),
            Message::View(view::Message::Clipboard(text)) => clipboard::write(text),
            Message::Event(Event::Window(window::Event::CloseRequested)) => {
                self.stop();
                Command::none()
            }
            _ => self.state.update(self.daemon.clone(), &self.cache, message),
        }
    }

    pub fn load_daemon_config(&mut self, cfg: DaemonConfig) -> Result<(), Error> {
        loop {
            if let Some(daemon) = Arc::get_mut(&mut self.daemon) {
                daemon.load_config(cfg)?;
                break;
            }
        }

        if let Some(path) = &self.config.daemon_config_path {
            let mut daemon_config_file = OpenOptions::new()
                .write(true)
                .open(path)
                .map_err(|e| Error::Config(e.to_string()))?;

            let content =
                toml::to_string(&self.daemon.config()).map_err(|e| Error::Config(e.to_string()))?;

            daemon_config_file
                .write_all(content.as_bytes())
                .map_err(|e| {
                    log::warn!("failed to write to file: {:?}", e);
                    Error::Config(e.to_string())
                })?;
        }

        Ok(())
    }

    pub fn view(&self) -> Element<Message> {
        self.state.view(&self.cache).map(Message::View)
    }
}
