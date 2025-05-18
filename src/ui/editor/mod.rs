mod code_editor;
mod input;
mod journal;
mod syntax_ext;

// Common editor functionality
pub trait Editor {
    fn update(&mut self);
    fn render(&self);
    fn handle_input(&mut self, input: &str) -> bool;
}
