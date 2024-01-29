use std::collections::HashSet;
use std::convert::From;
use std::path::PathBuf;
use std::sync::Arc;

use iced::{Command, Subscription};

use liana::miniscript::bitcoin::{bip32::Fingerprint, Network};

use liana_ui::{
    component::{form, modal},
    widget::Element,
};

use crate::{
    app::{
        cache::Cache, error::Error, message::Message, settings, state::State, view, wallet::Wallet,
    },
    daemon::Daemon,
    hw::{HardwareWallet, HardwareWalletConfig, HardwareWallets},
};

pub struct WalletSettingsState {
    data_dir: PathBuf,
    warning: Option<Error>,
    descriptor: String,
    keys_aliases: Vec<(Fingerprint, form::Value<String>)>,
    wallet: Arc<Wallet>,
    modal: Option<RegisterWalletModal>,
    processing: bool,
    updated: bool,
    creation_date: u32,
}

impl WalletSettingsState {
    pub fn new(data_dir: PathBuf, wallet: Arc<Wallet>, creation_date: u32) -> Self {
        WalletSettingsState {
            data_dir,
            descriptor: wallet.main_descriptor.to_string(),
            keys_aliases: Self::keys_aliases(&wallet),
            wallet,
            warning: None,
            modal: None,
            processing: false,
            updated: false,
            creation_date,
        }
    }

    fn keys_aliases(wallet: &Wallet) -> Vec<(Fingerprint, form::Value<String>)> {
        let mut keys_aliases: Vec<(Fingerprint, form::Value<String>)> = wallet
            .keys_aliases
            .clone()
            .into_iter()
            .map(|(fg, name)| {
                (
                    fg,
                    form::Value {
                        value: name,
                        valid: true,
                    },
                )
            })
            .collect();

        for fingerprint in wallet.descriptor_keys().into_iter() {
            if wallet.keys_aliases.get(&fingerprint).is_none() {
                keys_aliases.push((fingerprint, form::Value::default()));
            }
        }

        keys_aliases.sort_by(|(fg1, _), (fg2, _)| fg1.cmp(fg2));
        keys_aliases
    }
}

impl State for WalletSettingsState {
    fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, view::Message> {
        let content = view::settings::wallet_settings(
            cache,
            self.warning.as_ref(),
            &self.descriptor,
            &self.keys_aliases,
            self.processing,
            self.updated,
            self.creation_date,
        );
        if let Some(m) = &self.modal {
            modal::Modal::new(content, m.view())
                .on_blur(Some(view::Message::Close))
                .into()
        } else {
            content
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        if let Some(modal) = &self.modal {
            modal.subscription()
        } else {
            Subscription::none()
        }
    }

    fn update(
        &mut self,
        daemon: Arc<dyn Daemon + Sync + Send>,
        cache: &Cache,
        message: Message,
    ) -> Command<Message> {
        match message {
            Message::Updated(res) => match res {
                Ok(()) => {
                    self.processing = false;
                    self.updated = true;
                    Command::perform(async {}, |_| Message::LoadWallet)
                }
                Err(e) => {
                    self.processing = false;
                    self.warning = Some(e);
                    Command::none()
                }
            },
            Message::WalletLoaded(res) => {
                match res {
                    Ok(wallet) => {
                        if let Some(modal) = &mut self.modal {
                            modal.wallet = wallet.clone();
                        }
                        self.keys_aliases = Self::keys_aliases(&wallet);
                        self.wallet = wallet;
                    }
                    Err(e) => self.warning = Some(e),
                };
                Command::none()
            }
            Message::View(view::Message::Settings(
                view::SettingsMessage::FingerprintAliasEdited(fg, value),
            )) => {
                if let Some((_, name)) = self
                    .keys_aliases
                    .iter_mut()
                    .find(|(fingerprint, _)| fg == *fingerprint)
                {
                    name.value = value;
                }
                Command::none()
            }
            Message::View(view::Message::Settings(view::SettingsMessage::Save)) => {
                self.modal = None;
                self.processing = true;
                self.updated = false;
                Command::perform(
                    update_keys_aliases(
                        self.data_dir.clone(),
                        cache.network,
                        self.wallet.clone(),
                        self.keys_aliases
                            .iter()
                            .map(|(fg, name)| (*fg, name.value.to_owned()))
                            .collect(),
                    ),
                    Message::Updated,
                )
            }
            Message::View(view::Message::Close) => {
                self.modal = None;
                Command::none()
            }
            Message::View(view::Message::Settings(view::SettingsMessage::RegisterWallet)) => {
                self.modal = Some(RegisterWalletModal::new(
                    self.data_dir.clone(),
                    self.wallet.clone(),
                    cache.network,
                ));
                Command::none()
            }
            _ => self
                .modal
                .as_mut()
                .map(|m| m.update(daemon, cache, message))
                .unwrap_or_else(Command::none),
        }
    }

    fn load(&self, daemon: Arc<dyn Daemon + Sync + Send>) -> Command<Message> {
        Command::perform(
            async move { daemon.get_info().map_err(|e| e.into()) },
            Message::Info,
        )
    }
}

impl From<WalletSettingsState> for Box<dyn State> {
    fn from(s: WalletSettingsState) -> Box<dyn State> {
        Box::new(s)
    }
}

pub struct RegisterWalletModal {
    data_dir: PathBuf,
    wallet: Arc<Wallet>,
    warning: Option<Error>,
    chosen_hw: Option<usize>,
    hws: HardwareWallets,
    registered: HashSet<Fingerprint>,
    processing: bool,
}

impl RegisterWalletModal {
    pub fn new(data_dir: PathBuf, wallet: Arc<Wallet>, network: Network) -> Self {
        let mut registered = HashSet::new();
        for hw in &wallet.hardware_wallets {
            registered.insert(hw.fingerprint);
        }
        Self {
            data_dir: data_dir.clone(),
            warning: None,
            chosen_hw: None,
            hws: HardwareWallets::new(data_dir, network).with_wallet(wallet.clone()),
            wallet,
            processing: false,
            registered,
        }
    }
}

impl RegisterWalletModal {
    fn view(&self) -> Element<view::Message> {
        view::settings::register_wallet_modal(
            self.warning.as_ref(),
            &self.hws.list,
            self.processing,
            self.chosen_hw,
            &self.registered,
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        self.hws.refresh().map(Message::HardwareWallets)
    }

    fn update(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        cache: &Cache,
        message: Message,
    ) -> Command<Message> {
        match message {
            Message::View(view::Message::Reload) => {
                self.chosen_hw = None;
                self.warning = None;
                Command::none()
            }
            Message::HardwareWallets(msg) => match self.hws.update(msg) {
                Ok(cmd) => cmd.map(Message::HardwareWallets),
                Err(e) => {
                    self.warning = Some(e.into());
                    Command::none()
                }
            },
            Message::WalletRegistered(res) => {
                self.processing = false;
                self.chosen_hw = None;
                match res {
                    Ok(fingerprint) => {
                        self.registered.insert(fingerprint);
                        return Command::perform(async {}, |_| Message::LoadWallet);
                    }
                    Err(e) => self.warning = Some(e),
                }
                Command::none()
            }
            Message::View(view::Message::SelectHardwareWallet(i)) => {
                if let Some(HardwareWallet::Supported {
                    fingerprint,
                    device,
                    ..
                }) = self.hws.list.get(i)
                {
                    self.chosen_hw = Some(i);
                    self.processing = true;
                    Command::perform(
                        register_wallet(
                            self.data_dir.clone(),
                            cache.network,
                            device.clone(),
                            *fingerprint,
                            self.wallet.clone(),
                        ),
                        Message::WalletRegistered,
                    )
                } else {
                    Command::none()
                }
            }
            _ => Command::none(),
        }
    }
}

async fn register_wallet(
    data_dir: PathBuf,
    network: Network,
    hw: std::sync::Arc<dyn async_hwi::HWI + Send + Sync>,
    fingerprint: Fingerprint,
    wallet: Arc<Wallet>,
) -> Result<Fingerprint, Error> {
    let hmac = hw
        .register_wallet(&wallet.name, &wallet.main_descriptor.to_string())
        .await
        .map_err(Error::from)?;

    if let Some(hmac) = hmac {
        let mut settings = settings::Settings::from_file(data_dir.clone(), network)?;
        let checksum = wallet.descriptor_checksum();
        if let Some(wallet_setting) = settings
            .wallets
            .iter_mut()
            .find(|w| w.descriptor_checksum == checksum)
        {
            let kind = hw.device_kind().to_string();
            if let Some(hw_config) = wallet_setting
                .hardware_wallets
                .iter_mut()
                .find(|cfg| cfg.kind == kind && cfg.fingerprint == fingerprint)
            {
                hw_config.token = hex::encode(hmac);
            } else {
                wallet_setting.hardware_wallets.push(HardwareWalletConfig {
                    kind,
                    token: hex::encode(hmac),
                    fingerprint,
                })
            }
        }

        settings.to_file(data_dir, network)?;
    }

    Ok(fingerprint)
}

async fn update_keys_aliases(
    data_dir: PathBuf,
    network: Network,
    wallet: Arc<Wallet>,
    keys_aliases: Vec<(Fingerprint, String)>,
) -> Result<(), Error> {
    let mut settings = settings::Settings::from_file(data_dir.clone(), network)?;
    let checksum = wallet.descriptor_checksum();
    if let Some(wallet_setting) = settings
        .wallets
        .iter_mut()
        .find(|w| w.descriptor_checksum == checksum)
    {
        wallet_setting.keys = keys_aliases
            .into_iter()
            .map(|(master_fingerprint, name)| settings::KeySetting {
                master_fingerprint,
                name,
            })
            .collect();
    }

    settings.to_file(data_dir, network)?;

    Ok(())
}
