use crate::ui::{color, component::text::text, icon};
use iced::{
    widget::{self, Container, Row, Tooltip},
    Element,
};

pub fn simple<'a, T: 'a, C: Into<Element<'a, T>>>(content: C) -> widget::Container<'a, T> {
    Container::new(content).padding(15).style(SimpleCardStyle)
}

pub struct SimpleCardStyle;
impl widget::container::StyleSheet for SimpleCardStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> widget::container::Appearance {
        widget::container::Appearance {
            border_radius: 10.0,
            border_color: color::BORDER_GREY,
            border_width: 1.0,
            background: color::FOREGROUND.into(),
            ..widget::container::Appearance::default()
        }
    }
}

impl From<SimpleCardStyle> for Box<dyn widget::container::StyleSheet<Style = iced::Theme>> {
    fn from(s: SimpleCardStyle) -> Box<dyn widget::container::StyleSheet<Style = iced::Theme>> {
        Box::new(s)
    }
}

impl From<SimpleCardStyle> for iced::theme::Container {
    fn from(i: SimpleCardStyle) -> iced::theme::Container {
        iced::theme::Container::Custom(i.into())
    }
}

pub fn invalid<'a, T: 'a, C: Into<Element<'a, T>>>(content: C) -> widget::Container<'a, T> {
    Container::new(content).padding(15).style(InvalidCardStyle)
}

pub struct InvalidCardStyle;
impl widget::container::StyleSheet for InvalidCardStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> widget::container::Appearance {
        widget::container::Appearance {
            border_radius: 10.0,
            border_color: color::ALERT,
            border_width: 1.0,
            background: color::FOREGROUND.into(),
            ..widget::container::Appearance::default()
        }
    }
}

impl From<InvalidCardStyle> for Box<dyn widget::container::StyleSheet<Style = iced::Theme>> {
    fn from(s: InvalidCardStyle) -> Box<dyn widget::container::StyleSheet<Style = iced::Theme>> {
        Box::new(s)
    }
}

impl From<InvalidCardStyle> for iced::theme::Container {
    fn from(i: InvalidCardStyle) -> iced::theme::Container {
        iced::theme::Container::Custom(i.into())
    }
}

/// display an error card with the message and the error in a tooltip.
pub fn warning<'a, T: 'a>(message: String) -> widget::Container<'a, T> {
    Container::new(
        Row::new()
            .spacing(20)
            .align_items(iced::Alignment::Center)
            .push(icon::warning_octagon_icon().style(color::WARNING))
            .push(text(message).style(color::WARNING)),
    )
    .padding(15)
    .style(WarningCardStyle)
}

pub struct WarningCardStyle;
impl widget::container::StyleSheet for WarningCardStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> widget::container::Appearance {
        widget::container::Appearance {
            border_radius: 10.0,
            border_color: color::WARNING,
            border_width: 1.5,
            background: color::FOREGROUND.into(),
            ..widget::container::Appearance::default()
        }
    }
}

impl From<WarningCardStyle> for Box<dyn widget::container::StyleSheet<Style = iced::Theme>> {
    fn from(s: WarningCardStyle) -> Box<dyn widget::container::StyleSheet<Style = iced::Theme>> {
        Box::new(s)
    }
}

impl From<WarningCardStyle> for iced::theme::Container {
    fn from(i: WarningCardStyle) -> iced::theme::Container {
        iced::theme::Container::Custom(i.into())
    }
}

/// display an error card with the message and the error in a tooltip.
pub fn error<'a, T: 'a>(message: &'static str, error: String) -> widget::Container<'a, T> {
    Container::new(
        Tooltip::new(
            Row::new()
                .spacing(20)
                .align_items(iced::Alignment::Center)
                .push(icon::warning_icon().style(color::ALERT))
                .push(text(message).style(color::ALERT)),
            error,
            widget::tooltip::Position::Bottom,
        )
        .style(ErrorCardStyle),
    )
    .padding(15)
    .style(ErrorCardStyle)
}

pub struct ErrorCardStyle;
impl widget::container::StyleSheet for ErrorCardStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> widget::container::Appearance {
        widget::container::Appearance {
            border_radius: 10.0,
            border_color: color::ALERT,
            border_width: 1.5,
            background: color::FOREGROUND.into(),
            ..widget::container::Appearance::default()
        }
    }
}

impl From<ErrorCardStyle> for Box<dyn widget::container::StyleSheet<Style = iced::Theme>> {
    fn from(s: ErrorCardStyle) -> Box<dyn widget::container::StyleSheet<Style = iced::Theme>> {
        Box::new(s)
    }
}

impl From<ErrorCardStyle> for iced::theme::Container {
    fn from(i: ErrorCardStyle) -> iced::theme::Container {
        iced::theme::Container::Custom(i.into())
    }
}
