use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Stylize},
    widgets::{Block, BorderType, Paragraph, Widget},
};

use crate::p2p::tui::app::App;
pub mod input;
pub mod message;

impl App {
    pub fn ui(&self, frame: &mut Frame) {
        let vertical = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]);

        let [main_area, sub_area] = vertical.areas(frame.area());

        let main_vertical = Layout::vertical([
            Constraint::Min(10),   // 主内容区域
            Constraint::Length(3), // 输入框区域
        ]);

        let sub_vertical = Layout::vertical([
            Constraint::Percentage(60), // peers
            Constraint::Percentage(40), // logs
        ]);

        let [message_area, input_area] = main_vertical.areas(main_area);

        let [peer_area, log_area] = sub_vertical.areas(sub_area);

        crate::p2p::tui::ui::message::ui(frame, &self.view_model.message_vm, message_area);
        crate::p2p::tui::ui::input::ui(frame, &self.view_model.input_vm, input_area);

        // message_view::ui(frame, &state.message_vm, message_area);
        // input_view::ui(frame, &state.input_vm, input_area);
        // peer_view::ui(frame, &state.peer_vm, peer_area);
        // log_view::ui(frame, &state.log_vm, log_area);
    }
}

impl Widget for &App {
    /// Renders the user interface widgets.
    ///
    // This is where you add new widgets.
    // See the following resources:
    // - https://docs.rs/ratatui/latest/ratatui/widgets/index.html
    // - https://github.com/ratatui/ratatui/tree/master/examples
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered()
            .title("tui-demo")
            .title_alignment(Alignment::Center)
            .border_type(BorderType::Rounded);

        let text = format!(
            "This is a tui template.\n\
                Press `Esc`, `Ctrl-C` or `q` to stop running.\n\
                Press left and right to increment and decrement the counter respectively.",
        );

        let paragraph = Paragraph::new(text)
            .block(block)
            .fg(Color::Cyan)
            .bg(Color::Black)
            .centered();

        paragraph.render(area, buf);
    }
}
