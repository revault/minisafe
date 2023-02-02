pub mod badge;
pub mod button;
pub mod card;
pub mod collapse;
pub mod container;
pub mod form;
pub mod modal;
pub mod notification;
pub mod text;
pub mod tooltip;

pub use tooltip::tooltip;

use crate::ui::color;
use iced::{
    widget::{Column, Container, Text},
    Length,
};

pub fn separation<'a, T: 'a>() -> Container<'a, T> {
    Container::new(Column::new().push(Text::new(" ")))
        .style(SepStyle)
        .height(Length::Units(1))
}

pub struct SepStyle;
impl iced::widget::container::StyleSheet for SepStyle {
    type Style = iced::Theme;
    fn appearance(&self, _style: &Self::Style) -> iced::widget::container::Appearance {
        iced::widget::container::Appearance {
            background: color::BORDER_GREY.into(),
            ..iced::widget::container::Appearance::default()
        }
    }
}

impl From<SepStyle> for Box<dyn iced::widget::container::StyleSheet<Style = iced::Theme>> {
    fn from(s: SepStyle) -> Box<dyn iced::widget::container::StyleSheet<Style = iced::Theme>> {
        Box::new(s)
    }
}

impl From<SepStyle> for iced::theme::Container {
    fn from(i: SepStyle) -> iced::theme::Container {
        iced::theme::Container::Custom(i.into())
    }
}
