mod input;
mod journal;
mod code_editor;
mod syntax_ext;

pub use input::InputEditor;
pub use journal::JournalEditor;
pub use code_editor::{InputCodeEditor, SenderType};

// Common editor functionality
pub trait Editor {
    fn update(&mut self);
    fn render(&self);
    fn handle_input(&mut self, input: &str) -> bool;
}
