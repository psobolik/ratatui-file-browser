use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::Frame;

pub(crate) trait Component {
    fn set_area(&mut self, area: Rect);
    fn has_focus(&self) -> bool;
    fn set_focus(&mut self, focus: bool);
    fn hit_test(&self, x: u16, y: u16) -> bool;
    async fn handle_mouse_event(&mut self, mouse_event: MouseEvent) -> Result<(), std::io::Error>;
    async fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<(), std::io::Error>;
    fn render(&mut self, area: Rect, frame: &mut Frame<'_>) -> Result<(), std::io::Error>;
}
