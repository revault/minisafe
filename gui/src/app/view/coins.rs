use crate::{
    app::{
        cache::Cache,
        view::{message::Message, util::*},
    },
    daemon::model::{remaining_sequence, Coin},
    ui::{
        color,
        component::{badge, button, card, separation, text::*},
        icon,
        util::Collection,
    },
};
use iced::{
    widget::{Button, Column, Container, Row},
    Alignment, Element, Length,
};

pub fn coins_view<'a>(
    cache: &Cache,
    coins: &'a [Coin],
    timelock: u32,
    selected: &[usize],
) -> Element<'a, Message> {
    Column::new()
        .push(
            Container::new(
                Row::new()
                    .push(text(format!(" {}", coins.len())))
                    .push(text(" coins")),
            )
            .width(Length::Fill),
        )
        .push(
            Column::new()
                .spacing(10)
                .push(coins.iter().enumerate().fold(
                    Column::new().spacing(10),
                    |col, (i, coin)| {
                        col.push(coin_list_view(
                            coin,
                            timelock,
                            cache.blockheight as u32,
                            i,
                            selected.contains(&i),
                        ))
                    },
                )),
        )
        .align_items(Alignment::Center)
        .spacing(20)
        .into()
}

#[allow(clippy::collapsible_else_if)]
fn coin_list_view(
    coin: &Coin,
    timelock: u32,
    blockheight: u32,
    index: usize,
    collapsed: bool,
) -> Container<Message> {
    Container::new(
        Column::new()
            .push(
                Button::new(
                    Row::new()
                        .push(
                            Row::new()
                                .push(badge::coin())
                                .push_maybe(if coin.spend_info.is_some() {
                                    Some(
                                        Container::new(text("  Spent  ").small())
                                            .padding(3)
                                            .style(badge::PillStyle::Success),
                                    )
                                } else {
                                    let seq = remaining_sequence(coin, blockheight, timelock);
                                    if seq == 0 {
                                        Some(Container::new(
                                            Row::new()
                                                .spacing(5)
                                                .push(text(" 0").small().style(color::ALERT))
                                                .push(
                                                    icon::hourglass_done_icon()
                                                        .small()
                                                        .style(color::ALERT),
                                                )
                                                .align_items(Alignment::Center),
                                        ))
                                    } else if seq < timelock * 10 / 100 {
                                        Some(Container::new(
                                            Row::new()
                                                .spacing(5)
                                                .push(
                                                    text(format!(" {}", seq))
                                                        .small()
                                                        .style(color::WARNING),
                                                )
                                                .push(
                                                    icon::hourglass_icon()
                                                        .small()
                                                        .style(color::WARNING),
                                                )
                                                .align_items(Alignment::Center),
                                        ))
                                    } else {
                                        Some(Container::new(
                                            Row::new()
                                                .spacing(5)
                                                .push(text(format!(" {}", seq)).small())
                                                .push(icon::hourglass_icon().small())
                                                .align_items(Alignment::Center),
                                        ))
                                    }
                                })
                                .push_maybe(if coin.block_height.is_none() {
                                    Some(
                                        Container::new(text("  Unconfirmed  ").small())
                                            .padding(3)
                                            .style(badge::PillStyle::Simple),
                                    )
                                } else {
                                    None
                                })
                                .spacing(10)
                                .align_items(Alignment::Center)
                                .width(Length::Fill),
                        )
                        .push(amount(&coin.amount))
                        .align_items(Alignment::Center)
                        .spacing(20),
                )
                .style(button::Style::TransparentBorder.into())
                .padding(10)
                .on_press(Message::Select(index)),
            )
            .push_maybe(if collapsed {
                Some(
                    Column::new()
                        .spacing(10)
                        .push(separation().width(Length::Fill))
                        .push(
                            Column::new()
                                .padding(10)
                                .spacing(5)
                                .push_maybe(if coin.spend_info.is_none() {
                                    if let Some(b) = coin.block_height {
                                        if blockheight > b as u32 + timelock {
                                            Some(Container::new(
                                                text("The recovery path is available")
                                                    .bold()
                                                    .small()
                                                    .style(color::ALERT),
                                            ))
                                        } else {
                                            Some(Container::new(
                                                text(format!("The recovery path will be available in {} blocks", b as u32 + timelock - blockheight))
                                                .bold()
                                                .small(),
                                            ))
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                })
                                .push(
                                    Column::new()
                                        .push(
                                            Row::new()
                                                .align_items(Alignment::Center)
                                                .push(text("Outpoint:").small().bold())
                                                .push(Row::new().align_items(Alignment::Center)
                                                    .push(text(format!("{}", coin.outpoint)).small())
                                                    .push(Button::new(icon::clipboard_icon())
                                                        .on_press(Message::Clipboard(coin.outpoint.to_string()))
                                                        .style(button::Style::TransparentBorder.into())
                                                    ))
                                                .spacing(5),
                                        )
                                        .push_maybe(coin.block_height.map(|b| {
                                            Row::new()
                                                .push(text("Block height:").small().bold())
                                                .push(text(format!("{}", b)).small())
                                                .spacing(5)
                                        })),
                                )
                                .push_maybe(coin.spend_info.map(|info| {
                                    Column::new()
                                        .push(
                                            Row::new()
                                                .push(text("Spend txid:").small().bold())
                                                .push(text(format!("{}", info.txid)).small())
                                                .spacing(5),
                                        )
                                        .push(if let Some(height) = info.height {
                                            Row::new()
                                                .push(text("Spend block height:").small().bold())
                                                .push(text(format!("{}", height)).small())
                                                .spacing(5)
                                        } else {
                                            Row::new().push(text("Not in a block").bold().small())
                                        })
                                        .spacing(5)
                                })),
                        ),
                )
            } else {
                None
            }),
    )
    .style(card::SimpleCardStyle)
}
