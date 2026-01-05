use std::collections::HashSet;

use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::{Position, Rect},
};

use crate::p2p::{
    controller::LogicEvent,
    model::{self, input_view_model::InputMode},
    tui::{app::App, event::AppEvent},
};

use ratatui::{
    style::{Color, Style, Stylize},
    text::Span,
    widgets::{Block, Borders, Paragraph},
};

#[derive(Clone, Debug)]
pub enum InputEvent {
    /// 发送消息
    MsgSending(crate::p2p::model::message_view_model::Message),
    /// 接收消息
    MsgReceiving(crate::p2p::model::message_view_model::Message),
    PeersUpdating(HashSet<String>),
    /// 修改状态
    ChangeStatus,
}

impl From<LogicEvent> for InputEvent {
    fn from(val: LogicEvent) -> Self {
        match val {
            LogicEvent::PeerListUpdated(hash_set) => InputEvent::PeersUpdating(hash_set),
            LogicEvent::NewMessageReceived(message) => InputEvent::MsgReceiving(message),
            LogicEvent::Log(_level, _) => todo!(),
        }
    }
}

pub fn ui(frame: &mut Frame, state: &model::input_view_model::InputViewModel, input_area: Rect) {
    let help_msg = match state.input_mode {
        InputMode::Normal => {
            vec![
                "Press ".into(),
                Span::styled("q", Style::default().fg(Color::Red)).bold(),
                " to exit, ".into(),
                Span::styled("e", Style::default().fg(Color::Green)).bold(),
                " to start editing.".into(),
            ]
        }
        InputMode::Editing => {
            vec![
                "Press ".into(),
                Span::styled("Esc", Style::default().fg(Color::Red)).bold(),
                " to stop editing, ".into(),
                Span::styled("Enter", Style::default().fg(Color::Green)).bold(),
                " to send the message".into(),
            ]
        }
    };

    let paragraph_color = match state.input_mode {
        // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
        InputMode::Normal => Color::Gray,

        // Make the cursor visible and ask ratatui to put it at the specified coordinates after
        // rendering
        #[allow(clippy::cast_possible_truncation)]
        InputMode::Editing => {
            // 根据index动态计算光标移动宽度 -> 英文1个宽度,中文2个宽度
            let sub_char: String = state.input.chars().take(state.character_index).collect();
            frame.set_cursor_position(Position::new(
                // Draw the cursor at the current position in the input field.
                // This position is can be controlled via the left and right arrow key
                input_area.x + unicode_width::UnicodeWidthStr::width(&sub_char[..]) as u16 + 1,
                // Move one line down, from the border to the input line
                input_area.y + 1,
            ));
            Color::Blue
        }
    };

    let input_paragraph = Paragraph::new(state.input.clone())
        .style(paragraph_color)
        .block(Block::default().borders(Borders::ALL).title(help_msg));
    frame.render_widget(input_paragraph, input_area);
}

pub fn key_event(app: &mut App, key: &KeyEvent) {
    let model = &mut app.view_model.input_vm;
    match model.input_mode {
        InputMode::Normal => match key.code {
            KeyCode::Char('e') => {
                model.input_mode = InputMode::Editing;
                app.events.send(AppEvent::Input(InputEvent::ChangeStatus));
            }
            KeyCode::Esc | KeyCode::Char('q') => app.events.send(AppEvent::Quit),
            KeyCode::Char('c' | 'C') if key.modifiers == KeyModifiers::CONTROL => {
                app.events.send(AppEvent::Quit)
            }
            _ => {}
        },
        InputMode::Editing if key.kind == KeyEventKind::Press => match key.code {
            KeyCode::Enter => {
                let msg = model.submit_message();
                app.events
                    .send(AppEvent::Input(InputEvent::MsgSending(msg)));
            }
            KeyCode::Char(to_insert) => model.enter_char(to_insert),
            KeyCode::Backspace => model.delete_char(),
            KeyCode::Left => model.move_cursor_left(),
            KeyCode::Right => model.move_cursor_right(),
            KeyCode::Esc => model.input_mode = InputMode::Normal,
            _ => {}
        },
        InputMode::Editing => {}
    }
}
