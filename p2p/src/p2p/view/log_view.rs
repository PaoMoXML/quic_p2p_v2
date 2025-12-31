use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, Wrap},
};

use crate::p2p::model::{self, AutoScrollableViewModel};

pub fn ui(frame: &mut Frame, state: &model::log_view_model::LogViewModel, log_area: Rect) {
    super::adjust_scroll_pos(state, &log_area);
    // 显示已连接节点
    let conns_title = Span::styled("Logs", Style::default().fg(Color::Green));
    let logs = state
        .get_logs()
        .iter()
        .map(|log| Line::from(log.fmt_styled()))
        .collect::<Vec<Line>>();

    state.set_vertical_scroll_content_len(logs.len());

    let logs_paragraph = Paragraph::new(logs)
        .block(Block::default().borders(Borders::ALL).title(conns_title))
        .scroll((
            state
                .get_vertical_scroll()
                .load(std::sync::atomic::Ordering::Relaxed) as u16,
            0,
        ))
        .wrap(Wrap { trim: false });
    frame.render_widget(logs_paragraph, log_area);

    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        log_area,
        &mut state.get_vertical_scroll_state().lock().unwrap(),
    );
}
