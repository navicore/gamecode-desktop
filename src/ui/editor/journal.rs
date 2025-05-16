use super::Editor;

// Journal editor component for the middle pane
pub struct JournalEditor {
    // TODO: Integration with Lapce editor
    messages: Vec<String>,
}

impl JournalEditor {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }
    
    pub fn add_message(&mut self, message: String) {
        self.messages.push(message);
        // TODO: Update editor content
    }
    
    pub fn scroll_to_bottom(&mut self) {
        // TODO: Scroll view to most recent message
    }
}

impl Editor for JournalEditor {
    fn update(&mut self) {
        // TODO: Update editor state
    }
    
    fn render(&self) {
        // TODO: Render editor content
    }
    
    fn handle_input(&mut self, input: &str) -> bool {
        // TODO: Handle input events (mostly for scrolling/selection)
        println!("Journal editor received: {}", input);
        true
    }
}
