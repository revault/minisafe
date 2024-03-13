use liana::miniscript::{
    bitcoin::{bip32::Fingerprint, Network},
    DescriptorPublicKey,
};
use std::path::PathBuf;

use super::Error;
use crate::{
    bitcoind::{Bitcoind, ConfigField, RpcAuthType},
    download::Progress,
    hw::HardwareWalletMessage,
};
use async_hwi::DeviceKind;

#[derive(Debug, Clone)]
pub enum Message {
    CreateWallet,
    ParticipateWallet,
    ImportWallet,
    UserActionDone(bool),
    Exit(PathBuf, Option<Bitcoind>),
    Clibpboard(String),
    Next,
    Skip,
    Previous,
    Install,
    Close,
    Reload,
    Select(usize),
    UseHotSigner,
    Installed(Result<PathBuf, Error>),
    Network(Network),
    SelectBitcoindType(SelectBitcoindTypeMsg),
    InternalBitcoind(InternalBitcoindMsg),
    DefineBitcoind(DefineBitcoind),
    DefineDescriptor(DefineDescriptor),
    ImportXpub(Fingerprint, Result<DescriptorPublicKey, Error>),
    HardwareWallets(HardwareWalletMessage),
    WalletRegistered(Result<(Fingerprint, Option<[u8; 32]>), Error>),
    MnemonicWord(usize, String),
    ImportMnemonic(bool),
    Back(PathBuf),
}

#[derive(Debug, Clone)]
pub enum DefineBitcoind {
    ConfigFieldEdited(ConfigField, String),
    RpcAuthTypeSelected(RpcAuthType),
    PingBitcoindResult(Result<(), Error>),
    PingBitcoind,
}

#[derive(Debug, Clone)]
pub enum SelectBitcoindTypeMsg {
    UseExternal(bool),
}

#[derive(Debug, Clone)]
pub enum InternalBitcoindMsg {
    Previous,
    Reload,
    DefineConfig,
    Download,
    DownloadProgressed(Progress),
    Install,
    Start,
}

#[derive(Debug, Clone)]
pub enum DefineDescriptor {
    ImportDescriptor(String),
    PrimaryPath(DefinePath),
    RecoveryPath(usize, DefinePath),
    AddRecoveryPath,
    KeyModal(ImportKeyModal),
    SequenceModal(SequenceModal),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum DefinePath {
    AddKey,
    Key(usize, DefineKey),
    ThresholdEdited(usize),
    SequenceEdited(u16),
    EditSequence,
}

#[derive(Debug, Clone)]
pub enum DefineKey {
    Delete,
    Edit,
    Clipboard(String),
    Edited(String, DescriptorPublicKey, Option<DeviceKind>),
}

#[derive(Debug, Clone)]
pub enum ImportKeyModal {
    HWXpubImported(Result<DescriptorPublicKey, Error>),
    XPubEdited(String),
    EditName,
    NameEdited(String),
    ConfirmXpub,
    SelectKey(usize),
}

#[derive(Debug, Clone)]
pub enum SequenceModal {
    SequenceEdited(String),
    ConfirmSequence,
}
