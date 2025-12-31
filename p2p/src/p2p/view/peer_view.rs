use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::p2p::model;

pub(super) fn ui(
    frame: &mut Frame,
    state: &model::peer_view_model::PeerViewModel,
    peer_area: Rect,
) {
    // 显示已连接节点
    let conns_title = Span::styled("Peers", Style::default().fg(Color::Green));
    let connected_nodes = state
        .peers
        .iter()
        .map(|peer_id| Line::from(Span::raw(peer_id.to_string())))
        .collect::<Vec<Line>>();

    let nodes_paragraph = Paragraph::new(connected_nodes)
        .block(Block::default().borders(Borders::ALL).title(conns_title));
    frame.render_widget(nodes_paragraph, peer_area);
}
