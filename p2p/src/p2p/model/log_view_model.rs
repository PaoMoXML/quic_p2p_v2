use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize},
};

use chrono::{DateTime, Local};
use ratatui::{
    style::{Color, Style, Stylize},
    text::Span,
    widgets::ScrollbarState,
};

use super::{AutoScrollableViewModel, ViewModelFmt};

#[derive(Debug, Clone, Default)]
pub struct LogViewModel {
    logs: Vec<Log>,
    vertical_scroll_state: Arc<Mutex<ScrollbarState>>,
    vertical_scroll: Arc<AtomicUsize>,
    vertical_scroll_at_end: Arc<AtomicBool>,
}

impl LogViewModel {
    pub fn get_logs(&self) -> &Vec<Log> {
        &self.logs
    }

    pub fn add_log(&mut self, log: Log) {
        self.logs.push(log);
    }

    pub fn set_vertical_scroll_content_len(&self, len: usize) {
        let mut state = self.vertical_scroll_state.lock().unwrap();
        *state = state.content_length(len);
    }

    pub fn get_vertical_scroll_state(&self) -> Arc<Mutex<ScrollbarState>> {
        self.vertical_scroll_state.clone()
    }
}

impl AutoScrollableViewModel for LogViewModel {
    type Msg = Log;
    fn get_inner_msgs(&self) -> &Vec<Self::Msg> {
        &self.logs
    }

    fn get_vertical_scroll(&self) -> Arc<AtomicUsize> {
        self.vertical_scroll.clone()
    }

    fn vertical_scroll_to_end(&self) {
        self.vertical_scroll_at_end
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn get_vertical_scroll_at_end(&self) -> Arc<AtomicBool> {
        self.vertical_scroll_at_end.clone()
    }

    fn reset_vertical_scroll_at_end(&self) {
        self.vertical_scroll_at_end
            .store(false, std::sync::atomic::Ordering::Relaxed);
    }

    fn vertical_scroll_to(&self, pos: usize) {
        self.vertical_scroll
            .store(pos, std::sync::atomic::Ordering::Relaxed);
        let mut state = self.vertical_scroll_state.lock().unwrap();
        *state = state.position(pos);
    }
}

#[derive(Debug, Clone)]
pub struct Log {
    time: DateTime<Local>,
    level: log::Level,
    content: String,
}

impl Log {
    pub fn new(level: log::Level, content: String) -> Self {
        Self {
            time: Local::now(),
            level,
            content,
        }
    }

    pub fn fmt_styled<'a>(&self) -> Vec<Span<'a>> {
        vec![
            "[".into(),
            self.time
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
                .into(),
            " ".into(),
            {
                let color = match self.level {
                    log::Level::Error => Color::Red,
                    log::Level::Warn => Color::Yellow,
                    log::Level::Info => Color::Green,
                    log::Level::Debug => Color::Blue,
                    log::Level::Trace => Color::Cyan,
                };
                Span::styled(self.level.to_string(), Style::default().fg(color)).bold()
            },
            "]".into(),
            " ".into(),
            self.content.to_string().into(),
        ]
    }
}
impl ViewModelFmt for Log {
    fn formart(&self) -> (Vec<String>, Option<String>) {
        (
            vec![format!(
                "[{} {}] {}",
                self.time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                self.level,
                self.content
            )],
            None,
        )
    }
}
