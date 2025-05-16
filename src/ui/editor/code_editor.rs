use egui::Ui;

// (Placeholder - this implementation is not used in the final app)
pub struct InputCodeEditor;

// Types of message senders
#[derive(PartialEq, Clone, Copy)]
pub enum SenderType {
    User,
    Assistant,
    System,
    Tool,
}
