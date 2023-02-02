use crate::{
    app::view::message::*,
    hw::HardwareWallet,
    ui::{
        color,
        component::{
            button, card,
            text::{text, Text},
        },
        icon,
        util::Collection,
    },
};
use iced::{
    widget::{Button, Column, Container, Row},
    Alignment, Element, Length,
};

pub fn hw_list_view<'a>(
    i: usize,
    hw: &HardwareWallet,
    chosen: bool,
    processing: bool,
    signed: bool,
) -> Element<'a, Message> {
    let mut bttn = Button::new(
        Row::new()
            .push(
                Column::new()
                    .push(text(format!("{}", hw.kind)).bold())
                    .push(text(format!("fingerprint: {}", hw.fingerprint)).small())
                    .spacing(5)
                    .width(Length::Fill),
            )
            .push_maybe(if chosen && processing {
                Some(
                    Column::new()
                        .push(text("Processing..."))
                        .push(text("Please check your device").small()),
                )
            } else {
                None
            })
            .push_maybe(if signed {
                Some(
                    Column::new().push(
                        Row::new()
                            .spacing(5)
                            .push(icon::circle_check_icon().style(color::SUCCESS))
                            .push(text("Signed").style(color::SUCCESS)),
                    ),
                )
            } else {
                None
            })
            .align_items(Alignment::Center)
            .width(Length::Fill),
    )
    .padding(10)
    .style(button::Style::Border.into())
    .width(Length::Fill);
    if !processing {
        bttn = bttn.on_press(Message::Spend(SpendTxMessage::SelectHardwareWallet(i)));
    }
    Container::new(bttn)
        .width(Length::Fill)
        .style(card::SimpleCardStyle)
        .into()
}
