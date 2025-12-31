use chrono::Local;

use super::message_view_model::{Message, UserType};

#[derive(Clone, Debug, Default)]
pub enum InputMode {
    #[default]
    Normal,
    Editing,
}

#[derive(Clone, Debug, Default)]
pub struct InputViewModel {
    myself_id: String,
    pub input: String,
    pub input_mode: InputMode,
    pub character_index: usize,
}

impl InputViewModel {
    pub fn new(myself_id: String) -> Self {
        Self {
            myself_id,
            ..Default::default()
        }
    }
    pub fn get_myself_id(&self) -> &str {
        &self.myself_id
    }
    pub fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    pub fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    pub fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    /// Returns the byte index based on the character position.
    ///
    /// Since each character in a string can be contain multiple bytes, it's necessary to calculate
    /// the byte index based on the index of the character.
    pub fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    pub fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.input.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    pub fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    pub fn reset_cursor(&mut self) {
        self.character_index = 0;
    }

    pub fn submit_message(&mut self) -> Message {
        let msg = Message::new(
            Local::now(),
            self.input.clone(),
            UserType::User(self.myself_id.clone()),
            None,
        );
        self.input.clear();
        self.reset_cursor();
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::{InputMode, InputViewModel};

    #[test]
    fn test_width() {
        let cn = "你好".to_string();
        let en = "nh".to_string();

        let cn_width = unicode_width::UnicodeWidthStr::width(&cn[..]);
        let en_width = unicode_width::UnicodeWidthStr::width(&en[..]);

        println!("cn_width: {cn_width}");
        println!("en_width: {en_width}");
    }

    #[test]
    fn test_ivm() {
        let mut ivm = InputViewModel {
            myself_id: "11".into(),
            input: "".into(),
            input_mode: InputMode::Editing,
            character_index: 0,
            // last_edit_char: None,
        };

        ivm.enter_char('你');
        ivm.enter_char('h');
        ivm.enter_char('好');
        // ivm.delete_char();
        dbg!(ivm.character_index);
        dbg!(ivm.byte_index());
        let x: Vec<char> = ivm
            .input
            .chars()
            .skip(ivm.character_index.saturating_sub(2))
            .take(1)
            .collect();
        dbg!(ivm.input.chars().collect::<Vec<char>>());
        dbg!(x);

        // 英文1个字节,中文3个字节
        // 英文1个宽度,中文2个宽度

        dbg!(ivm);
    }
}
