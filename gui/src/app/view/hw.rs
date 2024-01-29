use iced::Length;

use liana_ui::{component::hw, theme, widget::*};

use crate::{
    app::view::message::*,
    hw::{HardwareWallet, UnsupportedReason},
};
use async_hwi::DeviceKind;

pub fn hw_list_view(
    i: usize,
    hw: &HardwareWallet,
    signed: bool,
    signing: bool,
) -> Element<Message> {
    let mut bttn = Button::new(match hw {
        HardwareWallet::Supported {
            kind,
            version,
            fingerprint,
            alias,
            registered,
            ..
        } => {
            if signing {
                hw::processing_hardware_wallet(kind, version.as_ref(), fingerprint, alias.as_ref())
            } else if signed {
                hw::sign_success_hardware_wallet(
                    kind,
                    version.as_ref(),
                    fingerprint,
                    alias.as_ref(),
                )
            } else if *registered == Some(false) {
                hw::unregistered_hardware_wallet(
                    kind,
                    version.as_ref(),
                    fingerprint,
                    alias.as_ref(),
                )
            } else {
                hw::supported_hardware_wallet(kind, version.as_ref(), fingerprint, alias.as_ref())
            }
        }
        HardwareWallet::Unsupported {
            version,
            kind,
            reason,
            ..
        } => match reason {
            UnsupportedReason::NotPartOfWallet(fg) => {
                hw::unrelated_hardware_wallet(&kind.to_string(), version.as_ref(), fg)
            }
            _ => hw::unsupported_hardware_wallet(&kind.to_string(), version.as_ref()),
        },
        HardwareWallet::Locked {
            kind, pairing_code, ..
        } => hw::locked_hardware_wallet(kind, pairing_code.as_ref()),
    })
    .style(theme::Button::Border)
    .width(Length::Fill);
    if !signing {
        if let HardwareWallet::Supported { registered, .. } = hw {
            if *registered != Some(false) {
                bttn = bttn.on_press(Message::SelectHardwareWallet(i));
            }
        }
    }
    Container::new(bttn)
        .width(Length::Fill)
        .style(theme::Container::Card(theme::Card::Simple))
        .into()
}

pub fn hw_list_view_for_registration(
    i: usize,
    hw: &HardwareWallet,
    chosen: bool,
    processing: bool,
    registered: bool,
) -> Element<Message> {
    let mut bttn = Button::new(match hw {
        HardwareWallet::Supported {
            kind,
            version,
            fingerprint,
            alias,
            ..
        } => {
            if chosen && processing {
                hw::processing_hardware_wallet(kind, version.as_ref(), fingerprint, alias.as_ref())
            } else if registered {
                hw::registration_success_hardware_wallet(
                    kind,
                    version.as_ref(),
                    fingerprint,
                    alias.as_ref(),
                )
            } else {
                hw::supported_hardware_wallet(kind, version.as_ref(), fingerprint, alias.as_ref())
            }
        }
        HardwareWallet::Unsupported {
            version,
            kind,
            reason,
            ..
        } => match reason {
            UnsupportedReason::NotPartOfWallet(fg) => {
                hw::unrelated_hardware_wallet(&kind.to_string(), version.as_ref(), fg)
            }
            _ => hw::unsupported_hardware_wallet(&kind.to_string(), version.as_ref()),
        },
        HardwareWallet::Locked {
            kind, pairing_code, ..
        } => hw::locked_hardware_wallet(kind, pairing_code.as_ref()),
    })
    .style(theme::Button::Border)
    .width(Length::Fill);
    if !processing && hw.is_supported() {
        bttn = bttn.on_press(Message::SelectHardwareWallet(i));
    }
    Container::new(bttn)
        .width(Length::Fill)
        .style(theme::Container::Card(theme::Card::Simple))
        .into()
}

pub fn hw_list_view_verify_address(
    i: usize,
    hw: &HardwareWallet,
    chosen: bool,
) -> Element<Message> {
    let (content, selectable) = match hw {
        HardwareWallet::Supported {
            kind,
            version,
            fingerprint,
            alias,
            ..
        } => {
            if chosen {
                (
                    hw::processing_hardware_wallet(
                        kind,
                        version.as_ref(),
                        fingerprint,
                        alias.as_ref(),
                    ),
                    false,
                )
            } else {
                match kind {
                    DeviceKind::Specter | DeviceKind::SpecterSimulator => {
                        (hw::unimplemented_method_hardware_wallet(
                            &kind.to_string(),
                            version.as_ref(),
                            fingerprint,
                            "Liana cannot request the device to display the address. \n The verification must be done manually with the device control."
                        ), false)
                    }
                    _ => (hw::supported_hardware_wallet(
                        kind,
                        version.as_ref(),
                        fingerprint,
                        alias.as_ref(),
                    ), true),
                }
            }
        }
        HardwareWallet::Unsupported {
            version,
            kind,
            reason,
            ..
        } => (
            match reason {
                UnsupportedReason::NotPartOfWallet(fg) => {
                    hw::unrelated_hardware_wallet(&kind.to_string(), version.as_ref(), fg)
                }
                _ => hw::unsupported_hardware_wallet(&kind.to_string(), version.as_ref()),
            },
            false,
        ),
        HardwareWallet::Locked {
            kind, pairing_code, ..
        } => (
            hw::locked_hardware_wallet(kind, pairing_code.as_ref()),
            false,
        ),
    };
    let mut bttn = Button::new(content)
        .style(theme::Button::Border)
        .width(Length::Fill);
    if selectable && hw.is_supported() {
        bttn = bttn.on_press(Message::SelectHardwareWallet(i));
    }
    Container::new(bttn)
        .width(Length::Fill)
        .style(theme::Container::Card(theme::Card::Simple))
        .into()
}
