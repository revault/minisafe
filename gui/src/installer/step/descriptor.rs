use std::collections::{BTreeMap, HashMap, HashSet};
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use iced::{Command, Subscription};
use liana::miniscript::bitcoin::bip32::Xpub;
use liana::{
    descriptors::{LianaDescriptor, LianaPolicy, PathInfo},
    miniscript::{
        bitcoin::{
            bip32::{DerivationPath, Fingerprint},
            Network,
        },
        descriptor::{
            DerivPaths, DescriptorMultiXKey, DescriptorPublicKey, DescriptorXKey, Wildcard,
        },
    },
};

use liana_ui::{
    component::{form, modal::Modal},
    widget::Element,
};

use async_hwi::{DeviceKind, Version};

use crate::hw::hw_subscriptions;
use crate::{
    app::{settings::KeySetting, wallet::wallet_name},
    hw,
    hw::{HardwareWallet, HardwareWallets},
    installer::{
        message::{self, Message},
        step::{Context, Step},
        view, Error,
    },
    signer::Signer,
};

pub trait DescriptorEditModal {
    fn processing(&self) -> bool {
        false
    }
    fn update(&mut self, _hws: &mut HardwareWallets, _message: Message) -> Command<Message> {
        Command::none()
    }
    fn view<'a>(&'a self, _hws: &'a HardwareWallets) -> Element<'a, Message>;
    fn subscription(&self, _hws: &HardwareWallets) -> Subscription<Message> {
        Subscription::none()
    }
}

pub struct RecoveryPath {
    keys: Vec<Option<Fingerprint>>,
    threshold: usize,
    sequence: u16,
    duplicate_sequence: bool,
}

impl RecoveryPath {
    pub fn new() -> Self {
        Self {
            keys: vec![None],
            threshold: 1,
            sequence: u16::MAX,
            duplicate_sequence: false,
        }
    }

    fn valid(&self) -> bool {
        !self.keys.is_empty() && !self.keys.iter().any(|k| k.is_none()) && !self.duplicate_sequence
    }

    fn view(
        &self,
        aliases: &HashMap<Fingerprint, String>,
        duplicate_name: &HashSet<Fingerprint>,
        incompatible_with_tapminiscript: &HashSet<Fingerprint>,
    ) -> Element<message::DefinePath> {
        view::recovery_path_view(
            self.sequence,
            self.duplicate_sequence,
            self.threshold,
            self.keys
                .iter()
                .enumerate()
                .map(|(i, key)| {
                    if let Some(key) = key {
                        view::defined_descriptor_key(
                            aliases.get(key).unwrap().to_string(),
                            duplicate_name.contains(key),
                            incompatible_with_tapminiscript.contains(key),
                        )
                    } else {
                        view::undefined_descriptor_key()
                    }
                    .map(move |msg| message::DefinePath::Key(i, msg))
                })
                .collect(),
        )
    }
}

struct Setup {
    keys: Vec<Key>,
    duplicate_name: HashSet<Fingerprint>,
    incompatible_with_tapminiscript: HashSet<Fingerprint>,
    spending_keys: Vec<Option<Fingerprint>>,
    spending_threshold: usize,
    recovery_paths: Vec<RecoveryPath>,
}

impl Setup {
    fn new() -> Self {
        Self {
            keys: Vec::new(),
            duplicate_name: HashSet::new(),
            incompatible_with_tapminiscript: HashSet::new(),
            spending_keys: vec![None],
            spending_threshold: 1,
            recovery_paths: vec![RecoveryPath::new()],
        }
    }

    fn valid(&self) -> bool {
        !self.spending_keys.is_empty()
            && !self.spending_keys.iter().any(|k| k.is_none())
            && !self.recovery_paths.iter().any(|path| !path.valid())
            && self.duplicate_name.is_empty()
            && self.incompatible_with_tapminiscript.is_empty()
    }

    // Mark as duplicate every defined key that have the same name but not the same fingerprint.
    // And every undefined_key that have a same name than an other key.
    fn check_for_duplicate(&mut self) {
        self.duplicate_name = HashSet::new();
        for a in &self.keys {
            for b in &self.keys {
                if a.name == b.name && a.fingerprint != b.fingerprint {
                    self.duplicate_name.insert(a.fingerprint);
                    self.duplicate_name.insert(b.fingerprint);
                }
            }
        }

        let mut all_sequence = HashSet::new();
        let mut duplicate_sequence = HashSet::new();
        for path in &mut self.recovery_paths {
            if all_sequence.contains(&path.sequence) {
                duplicate_sequence.insert(path.sequence);
            } else {
                all_sequence.insert(path.sequence);
            }
        }

        for path in &mut self.recovery_paths {
            path.duplicate_sequence = duplicate_sequence.contains(&path.sequence);
        }
    }

    fn check_for_tapminiscript_support(&mut self, must_support_taproot: bool) {
        self.incompatible_with_tapminiscript = HashSet::new();
        if must_support_taproot {
            for key in &self.keys {
                // check if key is used by a path
                if !self
                    .spending_keys
                    .iter()
                    .chain(self.recovery_paths.iter().flat_map(|path| &path.keys))
                    .any(|k| *k == Some(key.fingerprint))
                {
                    continue;
                }

                // device_kind is none only for HotSigner which is compatible.
                if let Some(device_kind) = key.device_kind.as_ref() {
                    if !hw::is_compatible_with_tapminiscript(
                        device_kind,
                        key.device_version.as_ref(),
                    ) {
                        self.incompatible_with_tapminiscript.insert(key.fingerprint);
                    }
                }
            }
        }
    }

    fn keys_aliases(&self) -> HashMap<Fingerprint, String> {
        let mut map = HashMap::new();
        for key in &self.keys {
            map.insert(key.key.master_fingerprint(), key.name.clone());
        }
        map
    }
}

pub struct DefineDescriptor {
    setup: Setup,

    network: Network,
    use_taproot: bool,

    modal: Option<Box<dyn DescriptorEditModal>>,
    signer: Arc<Mutex<Signer>>,

    error: Option<String>,
}

impl DefineDescriptor {
    pub fn new(network: Network, signer: Arc<Mutex<Signer>>) -> Self {
        Self {
            network,
            use_taproot: false,
            setup: Setup::new(),
            modal: None,
            signer,
            error: None,
        }
    }

    fn valid(&self) -> bool {
        self.setup.valid()
    }
    fn setup_mut(&mut self) -> &mut Setup {
        &mut self.setup
    }

    fn check_setup(&mut self) {
        self.setup_mut().check_for_duplicate();
        let use_taproot = self.use_taproot;
        self.setup_mut()
            .check_for_tapminiscript_support(use_taproot);
    }
}

impl Step for DefineDescriptor {
    // form value is set as valid each time it is edited.
    // Verification of the values is happening when the user click on Next button.
    fn update(&mut self, hws: &mut HardwareWallets, message: Message) -> Command<Message> {
        self.error = None;
        match message {
            Message::Close => {
                self.modal = None;
            }
            Message::CreateTaprootDescriptor(use_taproot) => {
                self.use_taproot = use_taproot;
                self.check_setup();
            }
            Message::DefineDescriptor(message::DefineDescriptor::AddRecoveryPath) => {
                self.setup_mut().recovery_paths.push(RecoveryPath::new());
            }
            Message::DefineDescriptor(message::DefineDescriptor::PrimaryPath(msg)) => match msg {
                message::DefinePath::ThresholdEdited(value) => {
                    self.setup_mut().spending_threshold = value;
                }
                message::DefinePath::AddKey => {
                    self.setup_mut().spending_keys.push(None);
                    self.setup_mut().spending_threshold += 1;
                }
                message::DefinePath::Key(i, msg) => match msg {
                    message::DefineKey::Clipboard(key) => {
                        return Command::perform(async move { key }, Message::Clibpboard);
                    }
                    message::DefineKey::Edited(name, imported_key, kind, version) => {
                        let fingerprint = imported_key.master_fingerprint();
                        hws.set_alias(fingerprint, name.clone());
                        if let Some(key) = self
                            .setup_mut()
                            .keys
                            .iter_mut()
                            .find(|k| k.fingerprint == fingerprint)
                        {
                            key.name = name;
                        } else {
                            self.setup_mut().keys.push(Key {
                                fingerprint,
                                name,
                                key: imported_key,
                                device_kind: kind,
                                device_version: version,
                            });
                        }

                        self.setup_mut().spending_keys[i] = Some(fingerprint);

                        self.modal = None;
                        self.check_setup();
                    }
                    message::DefineKey::Edit => {
                        let use_taproot = self.use_taproot;
                        let network = self.network;
                        let setup = self.setup_mut();
                        let modal = EditXpubModal::new(
                            use_taproot,
                            HashSet::from_iter(setup.spending_keys.iter().filter_map(|key| {
                                if key.is_some() && key != &setup.spending_keys[i] {
                                    *key
                                } else {
                                    None
                                }
                            })),
                            self.setup_mut().spending_keys[i],
                            None,
                            i,
                            network,
                            self.signer.clone(),
                            self.setup_mut()
                                .keys
                                .iter()
                                .filter(|k| check_key_network(&k.key, network))
                                .cloned()
                                .collect(),
                        );
                        let cmd = modal.load();
                        self.modal = Some(Box::new(modal));
                        return cmd;
                    }
                    message::DefineKey::Delete => {
                        self.setup_mut().spending_keys.remove(i);
                        if self.setup_mut().spending_threshold
                            > self.setup_mut().spending_keys.len()
                        {
                            self.setup_mut().spending_threshold -= 1;
                        }
                        self.check_setup();
                    }
                },
                _ => {}
            },
            Message::DefineDescriptor(message::DefineDescriptor::RecoveryPath(i, msg)) => match msg
            {
                message::DefinePath::ThresholdEdited(value) => {
                    if let Some(path) = self.setup_mut().recovery_paths.get_mut(i) {
                        path.threshold = value;
                    }
                }
                message::DefinePath::SequenceEdited(seq) => {
                    self.modal = None;
                    if let Some(path) = self.setup_mut().recovery_paths.get_mut(i) {
                        path.sequence = seq;
                    }
                    self.setup_mut().check_for_duplicate();
                }
                message::DefinePath::EditSequence => {
                    if let Some(path) = self.setup_mut().recovery_paths.get(i) {
                        self.modal = Some(Box::new(EditSequenceModal::new(i, path.sequence)));
                    }
                }
                message::DefinePath::AddKey => {
                    if let Some(path) = self.setup_mut().recovery_paths.get_mut(i) {
                        path.keys.push(None);
                        path.threshold += 1;
                    }
                }
                message::DefinePath::Key(j, msg) => match msg {
                    message::DefineKey::Clipboard(key) => {
                        return Command::perform(async move { key }, Message::Clibpboard);
                    }
                    message::DefineKey::Edited(name, imported_key, kind, version) => {
                        let fingerprint = imported_key.master_fingerprint();
                        hws.set_alias(fingerprint, name.clone());
                        if let Some(key) = self
                            .setup_mut()
                            .keys
                            .iter_mut()
                            .find(|k| k.fingerprint == fingerprint)
                        {
                            key.name = name;
                        } else {
                            self.setup_mut().keys.push(Key {
                                fingerprint,
                                name,
                                key: imported_key,
                                device_kind: kind,
                                device_version: version,
                            });
                        }

                        self.setup_mut().recovery_paths[i].keys[j] = Some(fingerprint);

                        self.modal = None;
                        self.check_setup();
                    }
                    message::DefineKey::Edit => {
                        let use_taproot = self.use_taproot;
                        let setup = self.setup_mut();
                        let modal = EditXpubModal::new(
                            use_taproot,
                            HashSet::from_iter(setup.recovery_paths[i].keys.iter().filter_map(
                                |key| {
                                    if key.is_some() && key != &setup.recovery_paths[i].keys[j] {
                                        *key
                                    } else {
                                        None
                                    }
                                },
                            )),
                            setup.recovery_paths[i].keys[j],
                            Some(i),
                            j,
                            self.network,
                            self.signer.clone(),
                            self.setup.keys.clone(),
                        );
                        let cmd = modal.load();
                        self.modal = Some(Box::new(modal));
                        return cmd;
                    }
                    message::DefineKey::Delete => {
                        if let Some(path) = self.setup_mut().recovery_paths.get_mut(i) {
                            path.keys.remove(j);
                            if path.threshold > path.keys.len() {
                                path.threshold -= 1;
                            }
                        }
                        if self
                            .setup_mut()
                            .recovery_paths
                            .get(i)
                            .map(|path| path.keys.is_empty())
                            .unwrap_or(false)
                        {
                            self.setup_mut().recovery_paths.remove(i);
                        }
                        self.check_setup();
                    }
                },
            },
            _ => {
                if let Some(modal) = &mut self.modal {
                    return modal.update(hws, message);
                }
            }
        };
        Command::none()
    }

    fn subscription(&self, hws: &HardwareWallets) -> Subscription<Message> {
        if let Some(modal) = &self.modal {
            modal.subscription(hws)
        } else {
            Subscription::none()
        }
    }

    fn apply(&mut self, ctx: &mut Context) -> bool {
        ctx.bitcoin_config.network = self.network;
        ctx.keys = Vec::new();
        let mut hw_is_used = false;
        let mut spending_keys: Vec<DescriptorPublicKey> = Vec::new();
        let mut key_derivation_index = HashMap::<Fingerprint, usize>::new();
        for spending_key in self.setup.spending_keys.iter().clone() {
            let fingerprint = spending_key.expect("Must be present at this step");
            let key = self
                .setup
                .keys
                .iter()
                .find(|key| key.key.master_fingerprint() == fingerprint)
                .expect("Must be present at this step");
            if let DescriptorPublicKey::XPub(xpub) = &key.key {
                if let Some((master_fingerprint, _)) = xpub.origin {
                    ctx.keys.push(KeySetting {
                        master_fingerprint,
                        name: key.name.clone(),
                    });
                    if key.device_kind.is_some() {
                        hw_is_used = true;
                    }
                }
                let derivation_index = key_derivation_index.get(&fingerprint).unwrap_or(&0);
                spending_keys.push(DescriptorPublicKey::MultiXPub(new_multixkey_from_xpub(
                    xpub.clone(),
                    *derivation_index,
                )));
                key_derivation_index.insert(fingerprint, derivation_index + 1);
            }
        }

        let mut recovery_paths = BTreeMap::new();

        for path in &self.setup.recovery_paths {
            let mut recovery_keys: Vec<DescriptorPublicKey> = Vec::new();
            for recovery_key in path.keys.iter().clone() {
                let fingerprint = recovery_key.expect("Must be present at this step");
                let key = self
                    .setup
                    .keys
                    .iter()
                    .find(|key| key.key.master_fingerprint() == fingerprint)
                    .expect("Must be present at this step");
                if let DescriptorPublicKey::XPub(xpub) = &key.key {
                    if let Some((master_fingerprint, _)) = xpub.origin {
                        ctx.keys.push(KeySetting {
                            master_fingerprint,
                            name: key.name.clone(),
                        });
                        if key.device_kind.is_some() {
                            hw_is_used = true;
                        }
                    }

                    let derivation_index = key_derivation_index.get(&fingerprint).unwrap_or(&0);
                    recovery_keys.push(DescriptorPublicKey::MultiXPub(new_multixkey_from_xpub(
                        xpub.clone(),
                        *derivation_index,
                    )));
                    key_derivation_index.insert(fingerprint, derivation_index + 1);
                }
            }

            let recovery_keys = if recovery_keys.len() == 1 {
                PathInfo::Single(recovery_keys[0].clone())
            } else {
                PathInfo::Multi(path.threshold, recovery_keys)
            };

            recovery_paths.insert(path.sequence, recovery_keys);
        }

        if spending_keys.is_empty() {
            return false;
        }

        let spending_keys = if spending_keys.len() == 1 {
            PathInfo::Single(spending_keys[0].clone())
        } else {
            PathInfo::Multi(self.setup.spending_threshold, spending_keys)
        };

        let policy = match if self.use_taproot {
            LianaPolicy::new(spending_keys, recovery_paths)
        } else {
            LianaPolicy::new_legacy(spending_keys, recovery_paths)
        } {
            Ok(policy) => policy,
            Err(e) => {
                self.error = Some(e.to_string());
                return false;
            }
        };

        ctx.descriptor = Some(LianaDescriptor::new(policy));
        ctx.hw_is_used = hw_is_used;
        true
    }

    fn view<'a>(
        &'a self,
        hws: &'a HardwareWallets,
        progress: (usize, usize),
    ) -> Element<'a, Message> {
        let aliases = self.setup.keys_aliases();
        let content = view::define_descriptor(
            progress,
            self.use_taproot,
            self.setup
                .spending_keys
                .iter()
                .enumerate()
                .map(|(i, key)| {
                    if let Some(key) = key {
                        view::defined_descriptor_key(
                            aliases.get(key).unwrap().to_string(),
                            self.setup.duplicate_name.contains(key),
                            self.setup.incompatible_with_tapminiscript.contains(key),
                        )
                    } else {
                        view::undefined_descriptor_key()
                    }
                    .map(move |msg| {
                        Message::DefineDescriptor(message::DefineDescriptor::PrimaryPath(
                            message::DefinePath::Key(i, msg),
                        ))
                    })
                })
                .collect(),
            self.setup.spending_threshold,
            self.setup
                .recovery_paths
                .iter()
                .enumerate()
                .map(|(i, path)| {
                    path.view(
                        &aliases,
                        &self.setup.duplicate_name,
                        &self.setup.incompatible_with_tapminiscript,
                    )
                    .map(move |msg| {
                        Message::DefineDescriptor(message::DefineDescriptor::RecoveryPath(i, msg))
                    })
                })
                .collect(),
            self.valid(),
            self.error.as_ref(),
        );
        if let Some(modal) = &self.modal {
            Modal::new(content, modal.view(hws))
                .on_blur(if modal.processing() {
                    None
                } else {
                    Some(Message::Close)
                })
                .into()
        } else {
            content
        }
    }
}

fn new_multixkey_from_xpub(
    xpub: DescriptorXKey<Xpub>,
    derivation_index: usize,
) -> DescriptorMultiXKey<Xpub> {
    DescriptorMultiXKey {
        origin: xpub.origin,
        xkey: xpub.xkey,
        derivation_paths: DerivPaths::new(vec![
            DerivationPath::from_str(&format!("m/{}", 2 * derivation_index)).unwrap(),
            DerivationPath::from_str(&format!("m/{}", 2 * derivation_index + 1)).unwrap(),
        ])
        .unwrap(),
        wildcard: Wildcard::Unhardened,
    }
}

#[derive(Clone)]
pub struct Key {
    pub device_kind: Option<DeviceKind>,
    pub device_version: Option<Version>,
    pub name: String,
    pub fingerprint: Fingerprint,
    pub key: DescriptorPublicKey,
}

fn check_key_network(key: &DescriptorPublicKey, network: Network) -> bool {
    match key {
        DescriptorPublicKey::XPub(key) => {
            if network == Network::Bitcoin {
                key.xkey.network == Network::Bitcoin
            } else {
                key.xkey.network == Network::Testnet
            }
        }
        DescriptorPublicKey::MultiXPub(key) => {
            if network == Network::Bitcoin {
                key.xkey.network == Network::Bitcoin
            } else {
                key.xkey.network == Network::Testnet
            }
        }
        _ => true,
    }
}

impl From<DefineDescriptor> for Box<dyn Step> {
    fn from(s: DefineDescriptor) -> Box<dyn Step> {
        Box::new(s)
    }
}

pub struct EditSequenceModal {
    path_index: usize,
    sequence: form::Value<String>,
}

impl EditSequenceModal {
    pub fn new(path_index: usize, sequence: u16) -> Self {
        Self {
            path_index,
            sequence: form::Value {
                value: sequence.to_string(),
                valid: true,
            },
        }
    }
}

impl DescriptorEditModal for EditSequenceModal {
    fn processing(&self) -> bool {
        false
    }

    fn update(&mut self, _hws: &mut HardwareWallets, message: Message) -> Command<Message> {
        if let Message::DefineDescriptor(message::DefineDescriptor::SequenceModal(msg)) = message {
            match msg {
                message::SequenceModal::SequenceEdited(seq) => {
                    if let Ok(s) = u16::from_str(&seq) {
                        self.sequence.valid = s != 0
                    } else {
                        self.sequence.valid = false;
                    }
                    self.sequence.value = seq;
                }
                message::SequenceModal::ConfirmSequence => {
                    if self.sequence.valid {
                        if let Ok(sequence) = u16::from_str(&self.sequence.value) {
                            let path_index = self.path_index;
                            return Command::perform(
                                async move { (path_index, sequence) },
                                |(path_index, sequence)| {
                                    message::DefineDescriptor::RecoveryPath(
                                        path_index,
                                        message::DefinePath::SequenceEdited(sequence),
                                    )
                                },
                            )
                            .map(Message::DefineDescriptor);
                        }
                    }
                }
            }
        }
        Command::none()
    }

    fn view(&self, _hws: &HardwareWallets) -> Element<Message> {
        view::edit_sequence_modal(&self.sequence)
    }
}

pub struct EditXpubModal {
    device_must_support_tapminiscript: bool,
    /// None if path is primary path
    path_index: Option<usize>,
    key_index: usize,
    network: Network,
    error: Option<Error>,
    processing: bool,
    upgrading: bool,

    form_name: form::Value<String>,
    form_xpub: form::Value<String>,
    edit_name: bool,

    other_path_keys: HashSet<Fingerprint>,
    duplicate_master_fg: bool,

    keys: Vec<Key>,
    hot_signer: Arc<Mutex<Signer>>,
    hot_signer_fingerprint: Fingerprint,
    chosen_signer: Option<(Fingerprint, Option<DeviceKind>, Option<Version>)>,
}

impl EditXpubModal {
    #[allow(clippy::too_many_arguments)]
    fn new(
        device_must_support_tapminiscript: bool,
        other_path_keys: HashSet<Fingerprint>,
        key: Option<Fingerprint>,
        path_index: Option<usize>,
        key_index: usize,
        network: Network,
        hot_signer: Arc<Mutex<Signer>>,
        keys: Vec<Key>,
    ) -> Self {
        let hot_signer_fingerprint = hot_signer.lock().unwrap().fingerprint();
        Self {
            device_must_support_tapminiscript,
            other_path_keys,
            form_name: form::Value {
                valid: true,
                value: key
                    .map(|fg| {
                        keys.iter()
                            .find(|k| k.fingerprint == fg)
                            .expect("must be stored")
                            .name
                            .clone()
                    })
                    .unwrap_or_default(),
            },
            form_xpub: form::Value {
                valid: true,
                value: key
                    .map(|fg| {
                        keys.iter()
                            .find(|k| k.fingerprint == fg)
                            .expect("must be stored")
                            .key
                            .to_string()
                    })
                    .unwrap_or_default(),
            },
            keys,
            path_index,
            key_index,
            processing: false,
            error: None,
            network,
            edit_name: false,
            chosen_signer: key.map(|k| (k, None, None)),
            hot_signer_fingerprint,
            hot_signer,
            duplicate_master_fg: false,
            upgrading: false,
        }
    }
    fn load(&self) -> Command<Message> {
        Command::none()
    }
}

impl DescriptorEditModal for EditXpubModal {
    fn processing(&self) -> bool {
        self.processing || self.upgrading
    }

    fn update(&mut self, hws: &mut HardwareWallets, message: Message) -> Command<Message> {
        // Reset these fields.
        // the fonction will setup them again if something is wrong
        self.duplicate_master_fg = false;
        self.error = None;
        match message {
            Message::Select(i) => {
                if let Some(HardwareWallet::Supported {
                    device,
                    fingerprint,
                    kind,
                    version,
                    ..
                }) = hws.list.get(i)
                {
                    self.chosen_signer = Some((*fingerprint, Some(*kind), version.clone()));
                    self.processing = true;
                    return Command::perform(
                        get_extended_pubkey(device.clone(), *fingerprint, self.network),
                        |res| {
                            Message::DefineDescriptor(message::DefineDescriptor::KeyModal(
                                message::ImportKeyModal::HWXpubImported(res),
                            ))
                        },
                    );
                }
            }
            Message::Reload => {
                return self.load();
            }
            Message::UseHotSigner => {
                let fingerprint = self.hot_signer.lock().unwrap().fingerprint();
                self.chosen_signer = Some((fingerprint, None, None));
                self.form_xpub.valid = true;
                if let Some(alias) = self
                    .keys
                    .iter()
                    .find(|key| key.fingerprint == fingerprint)
                    .map(|k| k.name.clone())
                {
                    self.form_name.valid = true;
                    self.form_name.value = alias;
                    self.edit_name = false;
                } else {
                    self.edit_name = true;
                    self.form_name.value = String::new();
                }
                let derivation_path = default_derivation_path(self.network);
                self.form_xpub.value = format!(
                    "[{}{}]{}",
                    fingerprint,
                    derivation_path.to_string().trim_start_matches('m'),
                    self.hot_signer
                        .lock()
                        .unwrap()
                        .get_extended_pubkey(&derivation_path)
                );
            }
            Message::DefineDescriptor(message::DefineDescriptor::KeyModal(msg)) => match msg {
                message::ImportKeyModal::HWXpubImported(res) => {
                    self.processing = false;
                    match res {
                        Ok(key) => {
                            if let Some(alias) = self
                                .keys
                                .iter()
                                .find(|k| k.fingerprint == key.master_fingerprint())
                                .map(|k| k.name.clone())
                            {
                                self.form_name.valid = true;
                                self.form_name.value = alias;
                                self.edit_name = false;
                            } else {
                                self.edit_name = true;
                                self.form_name.value = String::new();
                            }
                            self.form_xpub.valid = check_key_network(&key, self.network);
                            self.form_xpub.value = key.to_string();
                        }
                        Err(e) => {
                            self.chosen_signer = None;
                            self.error = Some(e);
                        }
                    }
                }
                message::ImportKeyModal::EditName => {
                    self.edit_name = true;
                }
                message::ImportKeyModal::NameEdited(name) => {
                    self.form_name.valid = true;
                    self.form_name.value = name;
                }
                message::ImportKeyModal::XPubEdited(s) => {
                    if let Ok(DescriptorPublicKey::XPub(key)) = DescriptorPublicKey::from_str(&s) {
                        self.chosen_signer = None;
                        if !key.derivation_path.is_master() {
                            self.form_xpub.valid = false;
                        } else if let Some((fingerprint, _)) = key.origin {
                            self.form_xpub.valid = if self.network == Network::Bitcoin {
                                key.xkey.network == Network::Bitcoin
                            } else {
                                key.xkey.network == Network::Testnet
                            };
                            if let Some(alias) = self
                                .keys
                                .iter()
                                .find(|k| k.fingerprint == fingerprint)
                                .map(|k| k.name.clone())
                            {
                                self.form_name.valid = true;
                                self.form_name.value = alias;
                                self.edit_name = false;
                            } else {
                                self.edit_name = true;
                            }
                        } else {
                            self.form_xpub.valid = false;
                        }
                    } else {
                        self.form_xpub.valid = false;
                    }
                    self.form_xpub.value = s;
                }
                message::ImportKeyModal::ConfirmXpub => {
                    if let Ok(key) = DescriptorPublicKey::from_str(&self.form_xpub.value) {
                        let key_index = self.key_index;
                        let name = self.form_name.value.clone();
                        let (device_kind, device_version) =
                            if let Some((_, kind, version)) = &self.chosen_signer {
                                (*kind, version.clone())
                            } else {
                                (None, None)
                            };
                        if self.other_path_keys.contains(&key.master_fingerprint()) {
                            self.duplicate_master_fg = true;
                        } else if let Some(path_index) = self.path_index {
                            return Command::perform(
                                async move { (path_index, key_index, key) },
                                move |(path_index, key_index, key)| {
                                    message::DefineDescriptor::RecoveryPath(
                                        path_index,
                                        message::DefinePath::Key(
                                            key_index,
                                            message::DefineKey::Edited(
                                                name,
                                                key,
                                                device_kind,
                                                device_version,
                                            ),
                                        ),
                                    )
                                },
                            )
                            .map(Message::DefineDescriptor);
                        } else {
                            return Command::perform(
                                async move { (key_index, key) },
                                move |(key_index, key)| {
                                    message::DefineDescriptor::PrimaryPath(
                                        message::DefinePath::Key(
                                            key_index,
                                            message::DefineKey::Edited(
                                                name,
                                                key,
                                                device_kind,
                                                device_version,
                                            ),
                                        ),
                                    )
                                },
                            )
                            .map(Message::DefineDescriptor);
                        }
                    }
                }
                message::ImportKeyModal::SelectKey(i) => {
                    if let Some(key) = self.keys.get(i) {
                        self.chosen_signer =
                            Some((key.fingerprint, key.device_kind, key.device_version.clone()));
                        self.form_xpub.value = key.key.to_string();
                        self.form_xpub.valid = true;
                        self.form_name.value.clone_from(&key.name);
                        self.form_name.valid = true;
                    }
                }
            },
            Message::LockModal(upgrading) => self.upgrading = upgrading,
            _ => {}
        };
        Command::none()
    }

    fn subscription(&self, hws: &HardwareWallets) -> Subscription<Message> {
        hw_subscriptions(hws, Some(self.device_must_support_tapminiscript))
            .map(Message::HardwareWallets)
    }

    fn view<'a>(&'a self, hws: &'a HardwareWallets) -> Element<'a, Message> {
        let chosen_signer = self.chosen_signer.as_ref().map(|s| s.0);
        let upgrading = hws.list.iter().any(|hw| hw.is_upgrade_in_progress());
        view::edit_key_modal(
            self.network,
            hws.list
                .iter()
                .enumerate()
                .filter_map(|(i, hw)| {
                    if self
                        .keys
                        .iter()
                        .any(|k| Some(k.fingerprint) == hw.fingerprint())
                    {
                        None
                    } else {
                        Some(view::hw_list_view(
                            i,
                            hw,
                            hw.fingerprint() == chosen_signer,
                            self.processing,
                            !self.processing
                                && hw.fingerprint() == chosen_signer
                                && self.form_xpub.valid
                                && !self.form_xpub.value.is_empty(),
                            self.network,
                            upgrading,
                        ))
                    }
                })
                .collect(),
            self.keys
                .iter()
                .enumerate()
                .filter_map(|(i, key)| {
                    if key.fingerprint == self.hot_signer_fingerprint {
                        None
                    } else {
                        Some(view::key_list_view(
                            i,
                            &key.name,
                            &key.fingerprint,
                            key.device_kind.as_ref(),
                            key.device_version.as_ref(),
                            Some(key.fingerprint) == chosen_signer,
                            self.device_must_support_tapminiscript,
                        ))
                    }
                })
                .collect(),
            self.error.as_ref(),
            self.chosen_signer.as_ref().map(|s| s.0),
            &self.hot_signer_fingerprint,
            self.keys.iter().find_map(|k| {
                if k.fingerprint == self.hot_signer_fingerprint {
                    Some(&k.name)
                } else {
                    None
                }
            }),
            &self.form_xpub,
            &self.form_name,
            self.edit_name,
            self.duplicate_master_fg,
        )
    }
}

pub fn default_derivation_path(network: Network) -> DerivationPath {
    DerivationPath::from_str({
        if network == Network::Bitcoin {
            "m/48'/0'/0'/2'"
        } else {
            "m/48'/1'/0'/2'"
        }
    })
    .unwrap()
}

/// LIANA_STANDARD_PATH: m/48'/0'/0'/2';
/// LIANA_TESTNET_STANDARD_PATH: m/48'/1'/0'/2';
pub async fn get_extended_pubkey(
    hw: std::sync::Arc<dyn async_hwi::HWI + Send + Sync>,
    fingerprint: Fingerprint,
    network: Network,
) -> Result<DescriptorPublicKey, Error> {
    let derivation_path = default_derivation_path(network);
    let xkey = hw
        .get_extended_pubkey(&derivation_path)
        .await
        .map_err(Error::from)?;
    Ok(DescriptorPublicKey::XPub(DescriptorXKey {
        origin: Some((fingerprint, derivation_path)),
        derivation_path: DerivationPath::master(),
        wildcard: Wildcard::None,
        xkey,
    }))
}

pub struct ImportDescriptor {
    network: Network,
    imported_descriptor: form::Value<String>,
    wrong_network: bool,
    error: Option<String>,
}

impl ImportDescriptor {
    pub fn new(network: Network) -> Self {
        Self {
            network,
            imported_descriptor: form::Value::default(),
            wrong_network: false,
            error: None,
        }
    }

    fn check_descriptor(&mut self, network: Network) -> Option<LianaDescriptor> {
        if !self.imported_descriptor.value.is_empty() {
            if let Ok(desc) = LianaDescriptor::from_str(&self.imported_descriptor.value) {
                if network == Network::Bitcoin {
                    self.imported_descriptor.valid = desc.all_xpubs_net_is(network);
                } else {
                    self.imported_descriptor.valid = desc.all_xpubs_net_is(Network::Testnet);
                }
                if self.imported_descriptor.valid {
                    self.wrong_network = false;
                    Some(desc)
                } else {
                    self.wrong_network = true;
                    None
                }
            } else {
                self.imported_descriptor.valid = false;
                self.wrong_network = false;
                None
            }
        } else {
            self.wrong_network = false;
            self.imported_descriptor.valid = true;
            None
        }
    }
}

impl Step for ImportDescriptor {
    // form value is set as valid each time it is edited.
    // Verification of the values is happening when the user click on Next button.
    fn update(&mut self, _hws: &mut HardwareWallets, message: Message) -> Command<Message> {
        if let Message::DefineDescriptor(message::DefineDescriptor::ImportDescriptor(desc)) =
            message
        {
            self.imported_descriptor.value = desc;
            self.check_descriptor(self.network);
        }
        Command::none()
    }

    fn apply(&mut self, ctx: &mut Context) -> bool {
        ctx.bitcoin_config.network = self.network;
        // Set to true in order to force the registration process to be shown to user.
        ctx.hw_is_used = true;
        // descriptor forms for import or creation cannot be both empty or filled.
        if let Some(desc) = self.check_descriptor(self.network) {
            ctx.descriptor = Some(desc);
            true
        } else {
            false
        }
    }

    fn view(&self, _hws: &HardwareWallets, progress: (usize, usize)) -> Element<Message> {
        view::import_descriptor(
            progress,
            &self.imported_descriptor,
            self.wrong_network,
            self.error.as_ref(),
        )
    }
}

impl From<ImportDescriptor> for Box<dyn Step> {
    fn from(s: ImportDescriptor) -> Box<dyn Step> {
        Box::new(s)
    }
}

pub struct RegisterDescriptor {
    descriptor: Option<LianaDescriptor>,
    processing: bool,
    chosen_hw: Option<usize>,
    hmacs: Vec<(Fingerprint, DeviceKind, Option<[u8; 32]>)>,
    registered: HashSet<Fingerprint>,
    error: Option<Error>,
    done: bool,
    /// Whether this step is part of the descriptor creation process. This is used to detect when
    /// it's instead shown as part of the descriptor *import* process, where we can't detect
    /// whether a signing device is used, to explicit this step is not required if the user isn't
    /// using a signing device.
    created_desc: bool,
    upgrading: bool,
}

impl RegisterDescriptor {
    fn new(created_desc: bool) -> Self {
        Self {
            created_desc,
            descriptor: Default::default(),
            processing: Default::default(),
            chosen_hw: Default::default(),
            hmacs: Default::default(),
            registered: Default::default(),
            error: Default::default(),
            done: Default::default(),
            upgrading: false,
        }
    }

    pub fn new_create_wallet() -> Self {
        Self::new(true)
    }

    pub fn new_import_wallet() -> Self {
        Self::new(false)
    }
}

impl Step for RegisterDescriptor {
    fn load_context(&mut self, ctx: &Context) {
        // we reset device registered set if the descriptor have changed.
        if self.descriptor != ctx.descriptor {
            self.registered = Default::default();
            self.done = false;
        }
        self.descriptor.clone_from(&ctx.descriptor);
        let mut map = HashMap::new();
        for key in ctx.keys.iter().filter(|k| !k.name.is_empty()) {
            map.insert(key.master_fingerprint, key.name.clone());
        }
    }
    fn update(&mut self, hws: &mut HardwareWallets, message: Message) -> Command<Message> {
        match message {
            Message::Select(i) => {
                if let Some(HardwareWallet::Supported {
                    device,
                    fingerprint,
                    ..
                }) = hws.list.get(i)
                {
                    if !self.registered.contains(fingerprint) {
                        let descriptor = self.descriptor.as_ref().unwrap();
                        let name = wallet_name(descriptor);
                        self.chosen_hw = Some(i);
                        self.processing = true;
                        self.error = None;
                        return Command::perform(
                            register_wallet(
                                device.clone(),
                                *fingerprint,
                                name,
                                descriptor.to_string(),
                            ),
                            Message::WalletRegistered,
                        );
                    }
                }
            }
            Message::WalletRegistered(res) => {
                self.processing = false;
                self.chosen_hw = None;
                match res {
                    Ok((fingerprint, hmac)) => {
                        if let Some(hw_h) = hws
                            .list
                            .iter()
                            .find(|hw_h| hw_h.fingerprint() == Some(fingerprint))
                        {
                            self.registered.insert(fingerprint);
                            self.hmacs.push((fingerprint, *hw_h.kind(), hmac));
                        }
                    }
                    Err(e) => {
                        if !matches!(e, Error::HardwareWallet(async_hwi::Error::UserRefused)) {
                            self.error = Some(e)
                        }
                    }
                }
            }
            Message::Reload => {
                return self.load();
            }
            Message::UserActionDone(done) => {
                self.done = done;
            }
            Message::LockModal(upgrading) => self.upgrading = upgrading,
            _ => {}
        };
        Command::none()
    }
    fn skip(&self, ctx: &Context) -> bool {
        !ctx.hw_is_used
    }
    fn apply(&mut self, ctx: &mut Context) -> bool {
        for (fingerprint, kind, token) in &self.hmacs {
            ctx.hws.push((*kind, *fingerprint, *token));
        }
        true
    }
    fn subscription(&self, hws: &HardwareWallets) -> Subscription<Message> {
        hw_subscriptions(hws, self.descriptor.as_ref().map(|d| d.is_taproot()))
            .map(Message::HardwareWallets)
    }
    fn load(&self) -> Command<Message> {
        Command::none()
    }
    fn view<'a>(
        &'a self,
        hws: &'a HardwareWallets,
        progress: (usize, usize),
    ) -> Element<'a, Message> {
        let desc = self.descriptor.as_ref().unwrap();
        view::register_descriptor(
            progress,
            desc.to_string(),
            &hws.list,
            &self.registered,
            self.error.as_ref(),
            self.processing,
            self.chosen_hw,
            self.done,
            self.created_desc,
            self.upgrading,
        )
    }
}

async fn register_wallet(
    hw: std::sync::Arc<dyn async_hwi::HWI + Send + Sync>,
    fingerprint: Fingerprint,
    name: String,
    descriptor: String,
) -> Result<(Fingerprint, Option<[u8; 32]>), Error> {
    let hmac = hw
        .register_wallet(&name, &descriptor)
        .await
        .map_err(Error::from)?;
    Ok((fingerprint, hmac))
}

impl From<RegisterDescriptor> for Box<dyn Step> {
    fn from(s: RegisterDescriptor) -> Box<dyn Step> {
        Box::new(s)
    }
}

#[derive(Default)]
pub struct BackupDescriptor {
    done: bool,
    descriptor: Option<LianaDescriptor>,
}

impl Step for BackupDescriptor {
    fn update(&mut self, _hws: &mut HardwareWallets, message: Message) -> Command<Message> {
        if let Message::UserActionDone(done) = message {
            self.done = done;
        }
        Command::none()
    }
    fn load_context(&mut self, ctx: &Context) {
        if self.descriptor != ctx.descriptor {
            self.descriptor.clone_from(&ctx.descriptor);
            self.done = false;
        }
    }
    fn view(&self, _hws: &HardwareWallets, progress: (usize, usize)) -> Element<Message> {
        let desc = self.descriptor.as_ref().unwrap();
        view::backup_descriptor(progress, desc.to_string(), self.done)
    }
}

impl From<BackupDescriptor> for Box<dyn Step> {
    fn from(s: BackupDescriptor) -> Box<dyn Step> {
        Box::new(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_runtime::command::Action;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    pub struct Sandbox<S: Step> {
        step: Arc<Mutex<S>>,
    }

    impl<S: Step + 'static> Sandbox<S> {
        pub fn new(step: S) -> Self {
            Self {
                step: Arc::new(Mutex::new(step)),
            }
        }

        pub fn check<F: FnOnce(&mut S)>(&self, check: F) {
            let mut step = self.step.lock().unwrap();
            check(&mut step)
        }

        pub async fn update(&self, message: Message) {
            let mut hws = HardwareWallets::new(PathBuf::from_str("/").unwrap(), Network::Bitcoin);
            let cmd = self.step.lock().unwrap().update(&mut hws, message);
            for action in cmd.actions() {
                if let Action::Future(f) = action {
                    let msg = f.await;
                    let _cmd = self.step.lock().unwrap().update(&mut hws, msg);
                }
            }
        }
        pub async fn load(&self, ctx: &Context) {
            self.step.lock().unwrap().load_context(ctx);
        }
    }

    #[tokio::test]
    async fn test_define_descriptor_use_hotkey() {
        let mut ctx = Context::new(Network::Signet, PathBuf::from_str("/").unwrap());
        let sandbox: Sandbox<DefineDescriptor> = Sandbox::new(DefineDescriptor::new(
            Network::Bitcoin,
            Arc::new(Mutex::new(Signer::generate(Network::Bitcoin).unwrap())),
        ));

        // Edit primary key
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::PrimaryPath(message::DefinePath::Key(
                    0,
                    message::DefineKey::Edit,
                )),
            ))
            .await;
        sandbox.check(|step| assert!(step.modal.is_some()));
        sandbox.update(Message::UseHotSigner).await;
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::KeyModal(message::ImportKeyModal::NameEdited(
                    "hot signer key".to_string(),
                )),
            ))
            .await;
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::KeyModal(message::ImportKeyModal::ConfirmXpub),
            ))
            .await;
        sandbox.check(|step| assert!(step.modal.is_none()));

        // Edit sequence
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::RecoveryPath(
                    0,
                    message::DefinePath::SequenceEdited(1000),
                ),
            ))
            .await;

        // Edit recovery key
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::RecoveryPath(
                    0,
                    message::DefinePath::Key(0, message::DefineKey::Edit),
                ),
            ))
            .await;
        sandbox.check(|step| assert!(step.modal.is_some()));
        sandbox.update(Message::DefineDescriptor(
                message::DefineDescriptor::KeyModal(
                    message::ImportKeyModal::XPubEdited("[f5acc2fd/48'/1'/0'/2']tpubDFAqEGNyad35aBCKUAXbQGDjdVhNueno5ZZVEn3sQbW5ci457gLR7HyTmHBg93oourBssgUxuWz1jX5uhc1qaqFo9VsybY1J5FuedLfm4dK".to_string()),
                )
        )).await;
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::KeyModal(message::ImportKeyModal::NameEdited(
                    "External recovery key".to_string(),
                )),
            ))
            .await;
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::KeyModal(message::ImportKeyModal::ConfirmXpub),
            ))
            .await;
        sandbox.check(|step| {
            assert!(step.modal.is_none());
            assert!((step).apply(&mut ctx));
            assert!(ctx
                .descriptor
                .as_ref()
                .unwrap()
                .to_string()
                .contains(&step.signer.lock().unwrap().fingerprint().to_string()));
        });
    }

    #[tokio::test]
    async fn test_define_descriptor_stores_if_hw_is_used() {
        let mut ctx = Context::new(Network::Testnet, PathBuf::from_str("/").unwrap());
        let sandbox: Sandbox<DefineDescriptor> = Sandbox::new(DefineDescriptor::new(
            Network::Testnet,
            Arc::new(Mutex::new(Signer::generate(Network::Testnet).unwrap())),
        ));
        sandbox.load(&ctx).await;

        let specter_key = message::DefinePath::Key(
            0,
            message::DefineKey::Edited(
                "My Specter key".to_string(),
                DescriptorPublicKey::from_str("[4df3f0e3/84'/0'/0']tpubDDRs9DnRUiJc4hq92PSJKhfzQBgHJUrDo7T2i48smsDfLsQcm3Vh7JhuGqJv8zozVkNFin8YPgpmn2NWNmpRaE3GW2pSxbmAzYf2juy7LeW").unwrap(),
                Some(DeviceKind::Specter),
                None,
            ),
        );

        // Use Specter device for primary key
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::PrimaryPath(specter_key.clone()),
            ))
            .await;

        // Edit recovery key
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::RecoveryPath(
                    0,
                    message::DefinePath::Key(0, message::DefineKey::Edit),
                ),
            ))
            .await;
        sandbox.check(|step| assert!(step.modal.is_some()));
        sandbox.update(Message::DefineDescriptor(
                message::DefineDescriptor::KeyModal(
                    message::ImportKeyModal::XPubEdited("[f5acc2fd/48'/1'/0'/2']tpubDFAqEGNyad35aBCKUAXbQGDjdVhNueno5ZZVEn3sQbW5ci457gLR7HyTmHBg93oourBssgUxuWz1jX5uhc1qaqFo9VsybY1J5FuedLfm4dK".to_string()),
                )
        )).await;
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::KeyModal(message::ImportKeyModal::NameEdited(
                    "External recovery key".to_string(),
                )),
            ))
            .await;
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::KeyModal(message::ImportKeyModal::ConfirmXpub),
            ))
            .await;
        sandbox.check(|step| {
            assert!(step.modal.is_none());
            assert!((step).apply(&mut ctx));
            assert!(ctx.hw_is_used);
        });

        // Now edit primary key to use hot signer instead of Specter device
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::PrimaryPath(message::DefinePath::Key(
                    0,
                    message::DefineKey::Edit,
                )),
            ))
            .await;
        sandbox.check(|step| assert!(step.modal.is_some()));
        sandbox.update(Message::UseHotSigner).await;
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::KeyModal(message::ImportKeyModal::NameEdited(
                    "hot signer key".to_string(),
                )),
            ))
            .await;
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::KeyModal(message::ImportKeyModal::ConfirmXpub),
            ))
            .await;
        sandbox.check(|step| {
            assert!(step.modal.is_none());
            assert!((step).apply(&mut ctx));
            assert!(!ctx.hw_is_used);
        });

        // Now edit the recovery key to use Specter device
        sandbox
            .update(Message::DefineDescriptor(
                message::DefineDescriptor::RecoveryPath(0, specter_key.clone()),
            ))
            .await;
        sandbox.check(|step| {
            assert!((step).apply(&mut ctx));
            assert!(ctx.hw_is_used);
        });
    }
}
