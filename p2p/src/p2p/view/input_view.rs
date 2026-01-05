use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Color, Style, Stylize},
    text::Span,
    widgets::{Block, Borders, Paragraph},
};

use crate::p2p::model::{self, input_view_model::InputMode};

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
