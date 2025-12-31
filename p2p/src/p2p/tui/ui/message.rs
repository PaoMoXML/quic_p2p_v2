use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind},
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, Wrap},
};

use crate::p2p::{
    model::{self, AutoScrollableViewModel, ViewModelFmt},
    tui::app::App,
    view::adjust_scroll_pos,
};

pub fn ui(
    frame: &mut Frame,
    state: &model::message_view_model::MessageViewModel,
    message_area: Rect,
) {
    adjust_scroll_pos(state, &message_area);
    // 主内容区域
    let message_block = Block::default()
        .borders(Borders::ALL)
        .title("Messages")
        .title_top(
            Line::from(Span::styled(
                format!("You are: {}", state.get_myself_id()),
                Style::default().fg(Color::Cyan),
            ))
            .right_aligned(),
        );

    // 显示内容
    let mut list_items = Vec::<Line>::new();
    for msg in state.get_inner_msgs() {
        let (fmt_msgs, relay_route) = msg.formart();
        let last_index = fmt_msgs.len() - 1;
        for (index, fmt_msg) in fmt_msgs.into_iter().enumerate() {
            let mut line = Line::default();
            // 第一行加入是谁说的
            if index == 0 {
                line.push_span({
                    match msg.get_user() {
                        model::message_view_model::UserType::User(peer_id) => {
                            if peer_id == state.get_myself_id() {
                                Span::styled(&peer_id[..5], Style::default().fg(Color::Cyan))
                            } else {
                                Span::styled(&peer_id[..5], Style::default().fg(Color::Yellow))
                            }
                        }
                        model::message_view_model::UserType::System => {
                            Span::styled("SYSTEM", Style::default().fg(Color::LightGreen))
                        }
                    }
                });
            }

            // 内容
            line.push_span(Span::styled(fmt_msg, Style::default().fg(Color::Gray)));

            // 转发内容
            {
                if index == last_index {
                    if let Some(relay_route) = relay_route {
                        line.push_span(Span::styled(
                            relay_route,
                            Style::default().fg(Color::Green),
                        ));
                    }
                    list_items.push(line);
                    break;
                }
            }

            list_items.push(line);
        }
    }

    state.set_vertical_scroll_content_len(list_items.len());

    let list = Paragraph::new(list_items)
        .gray()
        .block(message_block)
        // 滚动条
        .scroll((
            state
                .get_vertical_scroll()
                .load(std::sync::atomic::Ordering::Relaxed) as u16,
            0,
        ))
        // 换行
        .wrap(Wrap { trim: false });
    frame.render_widget(list, message_area);

    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        message_area,
        &mut state.get_vertical_scroll_state().lock().unwrap(),
    );
}

pub fn key_event(app: &mut App, key: &KeyEvent) {
    let model = &mut app.view_model.message_vm;

    if key.kind == KeyEventKind::Press {
        match key.code {
            KeyCode::Up => {
                model.vertical_scroll_up();
            }
            KeyCode::Down => {
                model.vertical_scroll_down();
            }
            _ => {}
        }
    }
}
