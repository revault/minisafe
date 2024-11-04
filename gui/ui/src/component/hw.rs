use crate::{color, component::text, icon, image, theme, widget::*};
use iced::{
    widget::{column, container, row, tooltip},
    Alignment, Length,
};
use std::borrow::Cow;
use std::fmt::Display;

pub fn locked_hardware_wallet<'a, T: 'a, K: Display>(
    kind: K,
    pairing_code: Option<impl Into<Cow<'a, str>>>,
) -> Container<'a, T> {
    Container::new(
        column(vec![
            Row::new()
                .spacing(5)
                .push(text::p1_bold(format!(
                    "Locked{}",
                    if pairing_code.is_some() {
                        ", check code:"
                    } else {
                        ""
                    }
                )))
                .push_maybe(pairing_code.map(|a| text::p1_bold(a)))
                .into(),
            Row::new()
                .spacing(5)
                .push(text::caption(kind.to_string()))
                .into(),
        ])
        .width(Length::Fill),
    )
    .padding(10)
}

pub fn supported_hardware_wallet<'a, T: 'a, K: Display, V: Display, F: Display>(
    kind: K,
    version: Option<V>,
    fingerprint: F,
    alias: Option<impl Into<Cow<'a, str>>>,
) -> Container<'a, T> {
    Container::new(
        column(vec![
            Row::new()
                .spacing(5)
                .push_maybe(alias.map(|a| text::p1_bold(a)))
                .push(text::p1_regular(format!("#{}", fingerprint)))
                .into(),
            Row::new()
                .spacing(5)
                .push(text::caption(kind.to_string()))
                .push_maybe(version.map(|v| text::caption(v.to_string())))
                .into(),
        ])
        .width(Length::Fill),
    )
    .padding(10)
}

pub fn warning_hardware_wallet<'a, T: 'a, K: Display, V: Display, F: Display>(
    kind: K,
    version: Option<V>,
    fingerprint: F,
    alias: Option<impl Into<Cow<'a, str>>>,
    warning: &'static str,
) -> Container<'a, T> {
    container(
        row(vec![
            column(vec![
                Row::new()
                    .spacing(5)
                    .push_maybe(alias.map(|a| text::p1_bold(a)))
                    .push(text::p1_regular(format!("#{}", fingerprint)))
                    .into(),
                Row::new()
                    .spacing(5)
                    .push(text::caption(kind.to_string()))
                    .push_maybe(version.map(|v| text::caption(v.to_string())))
                    .into(),
            ])
            .width(Length::Fill)
            .into(),
            column(vec![tooltip::Tooltip::new(
                icon::warning_icon(),
                warning,
                tooltip::Position::Bottom,
            )
            .style(theme::Container::Card(theme::Card::Simple))
            .into()])
            .into(),
        ])
        .align_items(Alignment::Center),
    )
    .padding(10)
}

pub fn unimplemented_method_hardware_wallet<'a, T: 'a, K: Display, V: Display, F: Display>(
    kind: K,
    version: Option<V>,
    fingerprint: F,
    message: &'static str,
) -> Container<'a, T> {
    container(
        tooltip::Tooltip::new(
            container(
                column(vec![
                    text::p1_regular(format!("#{}", fingerprint)).into(),
                    Row::new()
                        .spacing(5)
                        .push(text::caption(kind.to_string()))
                        .push_maybe(version.map(|v| text::caption(v.to_string())))
                        .into(),
                ])
                .width(Length::Fill),
            )
            .width(Length::Fill)
            .padding(10),
            message,
            tooltip::Position::Bottom,
        )
        .style(theme::Container::Card(theme::Card::Simple)),
    )
    .width(Length::Fill)
}

pub fn unrelated_hardware_wallet<'a, T: 'a, K: Display, V: Display, F: Display>(
    kind: K,
    version: Option<V>,
    fingerprint: F,
) -> Container<'a, T> {
    container(
        tooltip::Tooltip::new(
            container(
                column(vec![
                    text::p1_regular(format!("#{}", fingerprint)).into(),
                    Row::new()
                        .spacing(5)
                        .push(text::caption(kind.to_string()))
                        .push_maybe(version.map(|v| text::caption(v.to_string())))
                        .into(),
                ])
                .width(Length::Fill),
            )
            .width(Length::Fill)
            .padding(10),
            "This signer key does not match this wallet",
            tooltip::Position::Bottom,
        )
        .style(theme::Container::Card(theme::Card::Simple)),
    )
    .width(Length::Fill)
}

pub fn processing_hardware_wallet<'a, T: 'a, K: Display, V: Display, F: Display>(
    kind: K,
    version: Option<V>,
    fingerprint: F,
    alias: Option<impl Into<Cow<'a, str>>>,
) -> Container<'a, T> {
    container(
        row(vec![
            column(vec![
                Row::new()
                    .spacing(5)
                    .push_maybe(alias.map(|a| text::p1_bold(a)))
                    .push(text::p1_regular(format!("#{}", fingerprint)))
                    .into(),
                Row::new()
                    .spacing(5)
                    .push(text::caption(kind.to_string()))
                    .push_maybe(version.map(|v| text::caption(v.to_string())))
                    .into(),
            ])
            .width(Length::Fill)
            .into(),
            column(vec![
                text::p1_regular("Processing...").into(),
                text::p1_regular("Please check your device").into(),
            ])
            .into(),
        ])
        .align_items(Alignment::Center),
    )
    .padding(10)
}

pub fn selected_hardware_wallet<'a, T: 'a, K: Display, V: Display, F: Display>(
    kind: K,
    version: Option<V>,
    fingerprint: F,
    alias: Option<impl Into<Cow<'a, str>>>,
) -> Container<'a, T> {
    container(
        row(vec![
            column(vec![
                Row::new()
                    .spacing(5)
                    .push_maybe(alias.map(|a| text::p1_bold(a)))
                    .push(text::p1_regular(format!("#{}", fingerprint)))
                    .into(),
                Row::new()
                    .spacing(5)
                    .push(text::caption(kind.to_string()))
                    .push_maybe(version.map(|v| text::caption(v.to_string())))
                    .into(),
            ])
            .width(Length::Fill)
            .into(),
            image::success_mark_icon().width(Length::Fixed(50.0)).into(),
        ])
        .align_items(Alignment::Center),
    )
    .padding(10)
}

pub fn sign_success_hardware_wallet<'a, T: 'a, K: Display, V: Display, F: Display>(
    kind: K,
    version: Option<V>,
    fingerprint: F,
    alias: Option<impl Into<Cow<'a, str>>>,
) -> Container<'a, T> {
    container(
        row(vec![
            column(vec![
                Row::new()
                    .spacing(5)
                    .push_maybe(alias.map(|a| text::p1_bold(a)))
                    .push(text::p1_regular(format!("#{}", fingerprint)))
                    .into(),
                Row::new()
                    .spacing(5)
                    .push(text::caption(kind.to_string()))
                    .push_maybe(version.map(|v| text::caption(v.to_string())))
                    .into(),
            ])
            .width(Length::Fill)
            .into(),
            row(vec![
                text::p1_regular("Signed").style(color::GREEN).into(),
                image::success_mark_icon().width(Length::Fixed(50.0)).into(),
            ])
            .align_items(Alignment::Center)
            .spacing(5)
            .into(),
        ])
        .align_items(Alignment::Center),
    )
    .padding(10)
}

pub fn registration_success_hardware_wallet<'a, T: 'a, K: Display, V: Display, F: Display>(
    kind: K,
    version: Option<V>,
    fingerprint: F,
    alias: Option<impl Into<Cow<'a, str>>>,
) -> Container<'a, T> {
    container(
        row(vec![
            column(vec![
                Row::new()
                    .spacing(5)
                    .push_maybe(alias.map(|a| text::p1_bold(a)))
                    .push(text::p1_regular(format!("#{}", fingerprint)))
                    .into(),
                Row::new()
                    .spacing(5)
                    .push(text::caption(kind.to_string()))
                    .push_maybe(version.map(|v| text::caption(v.to_string())))
                    .into(),
            ])
            .width(Length::Fill)
            .into(),
            row(vec![
                text::p1_regular("Registered").style(color::GREEN).into(),
                image::success_mark_icon().width(Length::Fixed(50.0)).into(),
            ])
            .align_items(Alignment::Center)
            .spacing(5)
            .into(),
        ])
        .align_items(Alignment::Center),
    )
    .padding(10)
}

pub fn wrong_network_hardware_wallet<'a, T: 'a, K: Display, V: Display>(
    kind: K,
    version: Option<V>,
) -> Container<'a, T> {
    container(
        row(vec![
            column(vec![
                Row::new()
                    .spacing(5)
                    .push(text::p1_bold("Wrong network in the device settings"))
                    .into(),
                Row::new()
                    .spacing(5)
                    .push(text::caption(kind.to_string()))
                    .push_maybe(version.map(|v| text::caption(v.to_string())))
                    .into(),
            ])
            .width(Length::Fill)
            .into(),
            tooltip::Tooltip::new(
                icon::warning_icon(),
                "The wrong bitcoin application is open or the device was initialized with the wrong network",
                tooltip::Position::Bottom,
            )
            .style(theme::Container::Card(theme::Card::Simple))
            .into(),
        ])
        .align_items(Alignment::Center),
    )
    .padding(10)
}

pub fn unsupported_hardware_wallet<'a, T: 'a, K: Display, V: Display>(
    kind: K,
    version: Option<V>,
) -> Container<'a, T> {
    container(
        row(vec![
            column(vec![
                Row::new()
                    .spacing(5)
                    .push(text::p1_bold("Connection error"))
                    .into(),
                Row::new()
                    .spacing(5)
                    .push(text::caption(kind.to_string()))
                    .push_maybe(version.map(|v| text::caption(v.to_string())))
                    .into(),
            ])
            .width(Length::Fill)
            .into(),
            tooltip::Tooltip::new(
                icon::warning_icon(),
                "Make sure your device is unlocked and a supported Bitcoin application is opened.",
                tooltip::Position::Bottom,
            )
            .style(theme::Container::Card(theme::Card::Simple))
            .into(),
        ])
        .align_items(Alignment::Center),
    )
    .padding(10)
}

pub fn unsupported_version_hardware_wallet<'a, T: 'a, K: Display, V: Display, S: Display>(
    kind: K,
    version: Option<V>,
    requested_version: S,
) -> Container<'a, T> {
    container(
        row(vec![
            column(vec![
                text::p1_bold("Unsupported firmware version").into(),
                text::p1_regular(format!("Install version {} or later", requested_version)).into(),
                Row::new()
                    .spacing(5)
                    .push(text::caption(kind.to_string()))
                    .push_maybe(version.map(|v| text::caption(v.to_string())))
                    .into(),
            ])
            .width(Length::Fill)
            .into(),
            tooltip::Tooltip::new(
                icon::warning_icon(),
                "Please upgrade firmware",
                tooltip::Position::Bottom,
            )
            .style(theme::Container::Card(theme::Card::Simple))
            .into(),
        ])
        .align_items(Alignment::Center),
    )
    .padding(10)
}

pub fn sign_success_hot_signer<'a, T: 'a, F: Display>(
    fingerprint: F,
    alias: Option<impl Into<Cow<'a, str>>>,
) -> Container<'a, T> {
    container(
        row(vec![
            column(vec![
                Row::new()
                    .spacing(5)
                    .push_maybe(alias.map(|a| text::p1_bold(a)))
                    .push(text::p1_regular(format!("#{}", fingerprint)))
                    .into(),
                Row::new()
                    .spacing(5)
                    .push(text::caption("This computer"))
                    .into(),
            ])
            .width(Length::Fill)
            .into(),
            row(vec![
                text::p1_regular("Signed").style(color::GREEN).into(),
                image::success_mark_icon().width(Length::Fixed(50.0)).into(),
            ])
            .align_items(Alignment::Center)
            .spacing(5)
            .into(),
        ])
        .align_items(Alignment::Center),
    )
    .padding(10)
}

pub fn selected_hot_signer<'a, T: 'a, F: Display>(
    fingerprint: F,
    alias: Option<impl Into<Cow<'a, str>>>,
) -> Container<'a, T> {
    container(
        row(vec![
            column(vec![
                Row::new()
                    .spacing(5)
                    .push_maybe(alias.map(|a| text::p1_bold(a)))
                    .push(text::p1_regular(format!("#{}", fingerprint)))
                    .into(),
                Row::new()
                    .spacing(5)
                    .push(text::caption("This computer"))
                    .push(text::caption(
                        "(A derived key from a mnemonic stored locally)",
                    ))
                    .into(),
            ])
            .width(Length::Fill)
            .into(),
            image::success_mark_icon().width(Length::Fixed(50.0)).into(),
        ])
        .align_items(Alignment::Center),
    )
    .padding(10)
}

pub fn unselected_hot_signer<'a, T: 'a, F: Display>(
    fingerprint: F,
    alias: Option<impl Into<Cow<'a, str>>>,
) -> Container<'a, T> {
    Container::new(
        column(vec![
            Row::new()
                .spacing(5)
                .push_maybe(alias.map(|a| text::p1_bold(a)))
                .push(text::p1_regular(format!("#{}", fingerprint)))
                .into(),
            Row::new()
                .spacing(5)
                .push(text::caption("This computer"))
                .push(text::caption(
                    "(A derived key from a mnemonic stored locally)",
                ))
                .into(),
        ])
        .width(Length::Fill),
    )
    .padding(10)
}

pub fn hot_signer<'a, T: 'a, F: Display>(
    fingerprint: F,
    alias: Option<impl Into<Cow<'a, str>>>,
) -> Container<'a, T> {
    Container::new(
        column(vec![
            Row::new()
                .spacing(5)
                .push_maybe(alias.map(|a| text::p1_bold(a)))
                .push(text::p1_regular(format!("#{}", fingerprint)))
                .into(),
            Row::new()
                .spacing(5)
                .push(text::caption("This computer"))
                .into(),
        ])
        .width(Length::Fill),
    )
    .padding(10)
}
