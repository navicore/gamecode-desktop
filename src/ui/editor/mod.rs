mod code_editor;
mod input;
mod journal;
mod syntax_ext;

pub use code_editor::{InputCodeEditor, SenderType};
pub use input::InputEditor;
pub use journal::JournalEditor;

// Common editor functionality
pub trait Editor {
    fn update(&mut self);
    fn render(&self);
    fn handle_input(&mut self, input: &str) -> bool;
}
