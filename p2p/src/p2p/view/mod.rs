use ratatui::{Terminal, backend::CrosstermBackend, layout::Rect};
use std::{io, sync::Arc};
use tokio::sync::mpsc;

use super::{
    model::{AutoScrollableViewModel, ViewModelFmt},
};

mod input_view;
mod log_view;
mod message_view;
mod peer_view;

/// 计算应该滚动到什么位置
pub fn adjust_scroll_pos(state: &impl AutoScrollableViewModel, message_area: &Rect) {
    if message_area.height == 0 || message_area.width == 0 {
        return;
    }

    let curr_pos = state
        .get_vertical_scroll()
        .load(std::sync::atomic::Ordering::Relaxed);
    let mut new_pos = 0;

    if !state.get_inner_msgs().is_empty() {
        let height = (message_area.height as usize).saturating_sub(2);
        let width = (message_area.width as usize).saturating_sub(2);
        let line_width = calculate_message_lines(width, state.get_inner_msgs());

        if line_width > height && curr_pos < line_width - height {
            new_pos = line_width - height;
        }

        let at_end = state
            .get_vertical_scroll_at_end()
            .load(std::sync::atomic::Ordering::Relaxed);

        // println!("width: {width}, height: {height}");

        // println!("new_pos: {new_pos}, curr_pos: {curr_pos}, at_end: {at_end}, msg_len: {}", state.get_inner_msgs().len());

        if new_pos > 0 && at_end {
            state.vertical_scroll_to(new_pos);
        }
    }
}

// todo 需要优化，ui会根据空格自动将单词进行换行显示
/// 计算消息占用了几行
fn calculate_message_lines(width: usize, message_list: &Vec<impl ViewModelFmt>) -> usize {
    let mut lines: usize = 0;
    for message in message_list {
        let (contents, ext_content) = message.formart();
        let last_index = contents.len() - 1;
        for (index, content) in contents.into_iter().enumerate() {
            let mut content_with: usize = 0;
            // 空的也算一行（因为可能出现\n\n的情况）
            if content.is_empty() {
                content_with += 1;
            }
            content_with += unicode_width::UnicodeWidthStr::width(&content[..]);

            // 转发路径用
            {
                if last_index == index {
                    if let Some(ext_content) = ext_content {
                        content_with += unicode_width::UnicodeWidthStr::width(&ext_content[..]);
                    }
                    let result = content_with as f64 / width as f64;
                    let ceil_result = result.ceil();
                    let result_usize = ceil_result as usize;

                    lines += result_usize;
                    break;
                }
            }

            let result = content_with as f64 / width as f64;
            let ceil_result = result.ceil();
            let result_usize = ceil_result as usize;

            lines += result_usize
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use chrono::Local;

    use crate::p2p::model::{
        ViewModelFmt,
        message_view_model::{Message, UserType},
    };

    use super::calculate_message_lines;

    #[test]
    fn test_calculate_message_lines() {
        let msgs = vec![Message::new(
            Local::now(),
            "你好\n你好2\n你好3\n你好4\n你好5\n\n".to_string(),
            UserType::User("xmlaaaaaaaaa".to_string()),
            None,
        )];
        for ele in &msgs {
            println!("{:?}", ele.formart().0);
        }
        let lines = calculate_message_lines(40, &msgs);
        println!("lines: {lines}");
        assert_eq!(lines, 7);
    }
}
