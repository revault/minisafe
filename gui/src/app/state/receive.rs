use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use iced::{widget::qr_code, Command, Subscription};
use liana::miniscript::bitcoin::{
    bip32::{ChildNumber, Fingerprint},
    Address, Network,
};
use liana_ui::{component::modal, widget::*};

use crate::{
    app::{
        cache::Cache,
        error::Error,
        menu::Menu,
        message::Message,
        state::{label::LabelsEdited, State},
        view,
        wallet::Wallet,
    },
    hw::{HardwareWallet, HardwareWallets},
};

use crate::daemon::{
    model::{LabelItem, Labelled},
    Daemon,
};

#[derive(Debug, Default)]
pub struct Addresses {
    list: Vec<Address>,
    derivation_indexes: Vec<ChildNumber>,
    labels: HashMap<String, String>,
}

impl Addresses {
    fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

impl Labelled for Addresses {
    fn labelled(&self) -> Vec<LabelItem> {
        self.list
            .iter()
            .map(|a| LabelItem::Address(a.clone()))
            .collect()
    }
    fn labels(&mut self) -> &mut HashMap<String, String> {
        &mut self.labels
    }
}

pub struct ReceivePanel {
    data_dir: PathBuf,
    wallet: Arc<Wallet>,
    addresses: Addresses,
    labels_edited: LabelsEdited,
    qr_code: Option<qr_code::State>,
    modal: Option<VerifyAddressModal>,
    warning: Option<Error>,
}

impl ReceivePanel {
    pub fn new(data_dir: PathBuf, wallet: Arc<Wallet>) -> Self {
        Self {
            data_dir,
            wallet,
            addresses: Addresses::default(),
            labels_edited: LabelsEdited::default(),
            qr_code: None,
            modal: None,
            warning: None,
        }
    }
}

impl State for ReceivePanel {
    fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, view::Message> {
        let content = view::dashboard(
            &Menu::Receive,
            cache,
            self.warning.as_ref(),
            view::receive::receive(
                &self.addresses.list,
                self.qr_code.as_ref(),
                &self.addresses.labels,
                self.labels_edited.cache(),
            ),
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
            Message::View(view::Message::Label(_, _)) | Message::LabelsUpdated(_) => {
                match self.labels_edited.update(
                    daemon,
                    message,
                    std::iter::once(&mut self.addresses).map(|a| a as &mut dyn Labelled),
                ) {
                    Ok(cmd) => cmd,
                    Err(e) => {
                        self.warning = Some(e);
                        Command::none()
                    }
                }
            }
            Message::ReceiveAddress(res) => {
                match res {
                    Ok((address, derivation_index)) => {
                        self.warning = None;
                        self.qr_code = qr_code::State::new(format!(
                            "bitcoin:{}?index={}",
                            address, derivation_index
                        ))
                        .ok();
                        self.addresses.list.push(address);
                        self.addresses.derivation_indexes.push(derivation_index);
                    }
                    Err(e) => self.warning = Some(e),
                }
                Command::none()
            }
            Message::View(view::Message::Close) => {
                self.modal = None;
                Command::none()
            }
            Message::View(view::Message::Select(i)) => {
                self.modal = Some(VerifyAddressModal::new(
                    self.data_dir.clone(),
                    self.wallet.clone(),
                    cache.network,
                    self.addresses.list.get(i).expect("Must be present").clone(),
                    *self
                        .addresses
                        .derivation_indexes
                        .get(i)
                        .expect("Must be present"),
                ));
                Command::none()
            }
            Message::View(view::Message::Next) => {
                let daemon = daemon.clone();
                Command::perform(
                    async move {
                        daemon
                            .get_new_address()
                            .map(|res| (res.address, res.derivation_index))
                            .map_err(|e| e.into())
                    },
                    Message::ReceiveAddress,
                )
            }
            _ => self
                .modal
                .as_mut()
                .map(|m| m.update(daemon, cache, message))
                .unwrap_or_else(Command::none),
        }
    }

    fn reload(&mut self, daemon: Arc<dyn Daemon + Sync + Send>) -> Command<Message> {
        // Fill at least with one address, user will then use the generate button.
        if self.addresses.is_empty() {
            let daemon = daemon.clone();
            Command::perform(
                async move {
                    daemon
                        .get_new_address()
                        .map(|res| (res.address, res.derivation_index))
                        .map_err(|e| e.into())
                },
                Message::ReceiveAddress,
            )
        } else {
            Command::none()
        }
    }
}

impl From<ReceivePanel> for Box<dyn State> {
    fn from(s: ReceivePanel) -> Box<dyn State> {
        Box::new(s)
    }
}

pub struct VerifyAddressModal {
    warning: Option<Error>,
    chosen_hws: HashSet<Fingerprint>,
    hws: HardwareWallets,
    address: Address,
    derivation_index: ChildNumber,
}

impl VerifyAddressModal {
    pub fn new(
        data_dir: PathBuf,
        wallet: Arc<Wallet>,
        network: Network,
        address: Address,
        derivation_index: ChildNumber,
    ) -> Self {
        Self {
            warning: None,
            chosen_hws: HashSet::new(),
            hws: HardwareWallets::new(data_dir, network).with_wallet(wallet),
            address,
            derivation_index,
        }
    }
}

impl VerifyAddressModal {
    fn view(&self) -> Element<view::Message> {
        view::receive::verify_address_modal(
            self.warning.as_ref(),
            &self.hws.list,
            &self.chosen_hws,
            &self.address,
            &self.derivation_index,
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        self.hws.refresh().map(Message::HardwareWallets)
    }

    fn update(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        _cache: &Cache,
        message: Message,
    ) -> Command<Message> {
        match message {
            Message::HardwareWallets(msg) => match self.hws.update(msg) {
                Ok(cmd) => cmd.map(Message::HardwareWallets),
                Err(e) => {
                    self.warning = Some(e.into());
                    Command::none()
                }
            },
            Message::Verified(fg, res) => {
                self.chosen_hws.remove(&fg);
                if let Err(e) = res {
                    self.warning = Some(e);
                }
                Command::none()
            }
            Message::View(view::Message::SelectHardwareWallet(i)) => {
                if let Some(HardwareWallet::Supported {
                    device,
                    fingerprint,
                    ..
                }) = self.hws.list.get(i)
                {
                    self.warning = None;
                    self.chosen_hws.insert(*fingerprint);
                    let fg = *fingerprint;
                    Command::perform(
                        verify_address(device.clone(), self.derivation_index),
                        move |res| Message::Verified(fg, res),
                    )
                } else {
                    Command::none()
                }
            }
            _ => Command::none(),
        }
    }
}

async fn verify_address(
    hw: std::sync::Arc<dyn async_hwi::HWI + Send + Sync>,
    index: ChildNumber,
) -> Result<(), Error> {
    hw.display_address(&async_hwi::AddressScript::Miniscript {
        change: false,
        index: index.into(),
    })
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::cache::Cache,
        daemon::{
            client::{Lianad, Request},
            model::*,
        },
        utils::{mock::Daemon, sandbox::Sandbox},
    };

    use liana::{descriptors::LianaDescriptor, miniscript::bitcoin::Address};
    use serde_json::json;
    use std::str::FromStr;

    const DESC: &str = "wsh(or_d(multi(2,[ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N/<0;1>/*,[de6eb005/48'/1'/0'/2']tpubDFGuYfS2JwiUSEXiQuNGdT3R7WTDhbaE6jbUhgYSSdhmfQcSx7ZntMPPv7nrkvAqjpj3jX9wbhSGMeKVao4qAzhbNyBi7iQmv5xxQk6H6jz/<0;1>/*),and_v(v:pkh([ffd63c8d/48'/1'/0'/2']tpubDExA3EC3iAsPxPhFn4j6gMiVup6V2eH3qKyk69RcTc9TTNRfFYVPad8bJD5FCHVQxyBT4izKsvr7Btd2R4xmQ1hZkvsqGBaeE82J71uTK4N/<2;3>/*),older(3))))#p9ax3xxp";

    #[tokio::test]
    async fn test_receive_panel() {
        let addr =
            Address::from_str("tb1qkldgvljmjpxrjq2ev5qxe8dvhn0dph9q85pwtfkjeanmwdue2akqj4twxj")
                .unwrap()
                .assume_checked();
        let daemon = Daemon::new(vec![(
            Some(json!({"method": "getnewaddress", "params": Option::<Request>::None})),
            Ok(json!(GetAddressResult::new(
                addr.clone(),
                ChildNumber::from_normal_idx(0).unwrap()
            ))),
        )]);

        let sandbox: Sandbox<ReceivePanel> = Sandbox::new(ReceivePanel::new(
            PathBuf::new(),
            Arc::new(Wallet::new(LianaDescriptor::from_str(DESC).unwrap(), 0)),
        ));
        let client = Arc::new(Lianad::new(daemon.run()));
        let sandbox = sandbox.load(client, &Cache::default()).await;

        let panel = sandbox.state();
        assert_eq!(panel.addresses.list, vec![addr]);
    }
}
