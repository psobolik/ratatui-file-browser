/*
 * Copyright (c) 2023 Paul Sobolik
 * Created 2023-12-23
 */
use ratatui::widgets::ListState;

#[derive(Default)]
pub struct StatefulList<T> {
    pub(crate) state: ListState,
    items: Vec<T>,
}

#[allow(dead_code)]
impl<T> StatefulList<T> {
    pub fn with_items(items: Vec<T>) -> StatefulList<T> {
        StatefulList {
            state: ListState::default(),
            items,
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn lower_bound(&self) -> usize {
        0
    }
    pub fn upper_bound(&self) -> usize {
        let len = self.len();
        if len > 0 {
            len - 1
        } else {
            0
        }
    }

    pub fn offset(&self) -> usize {
        self.state.offset()
    }
    pub fn set_offset(&mut self, offset: usize) {
        self.set_selected(Some(offset));
        *self.state.offset_mut() = offset;
    }

    pub fn at_offset_first(&self) -> bool {
        self.state.offset() == self.lower_bound()
    }
    pub fn offset_first(&mut self) {
        self.set_offset(self.lower_bound())
    }

    pub fn at_offset_last(&mut self) -> bool {
        self.state.offset() == self.upper_bound()
    }
    pub fn offset_last(&mut self) {
        self.set_offset(self.upper_bound())
    }

    pub fn advance_offset(&mut self, distance: usize) {
        let new = self.offset() + distance;
        if new < self.upper_bound() {
            self.set_offset(new)
        } else {
            self.offset_last()
        }
    }

    pub fn retreat_offset(&mut self, distance: usize) {
        let offset = self.offset();
        if offset < distance {
            self.offset_first()
        } else {
            self.set_selected(Some(offset - distance));
        }
    }

    pub fn next_offset(&mut self) {
        self.advance_offset(1)
    }

    pub fn previous_offset(&mut self) {
        self.retreat_offset(1)
    }

    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }
    pub fn set_selected(&mut self, index: Option<usize>) {
        self.state.select(index);
    }

    pub fn iter(&self) -> std::slice::Iter<T> {
        self.items.iter()
    }

    pub fn first(&mut self) -> bool {
        if self.is_first() {
            return false;
        }
        self.set_selected(Some(self.lower_bound()));
        true
    }
    pub fn is_first(&self) -> bool {
        match self.selected() {
            Some(selected) => selected == self.lower_bound(),
            None => false,
        }
    }

    pub fn last(&mut self) -> bool {
        if self.is_last() {
            return false;
        }
        self.set_selected(Some(self.upper_bound()));
        true
    }
    pub fn is_last(&self) -> bool {
        match self.selected() {
            Some(selected) => selected == self.upper_bound(),
            None => false,
        }
    }

    pub fn advance(&mut self, distance: usize) -> bool {
        if self.is_last() {
            return false;
        }
        let selected = self.selected().unwrap_or(self.lower_bound());
        let new = selected + distance;
        if new < self.upper_bound() {
            self.set_selected(Some(new));
        } else {
            self.last();
        }
        true
    }

    pub fn retreat(&mut self, distance: usize) -> bool {
        if self.is_first() {
            return false;
        }
        let selected = self.selected().unwrap_or(self.lower_bound());
        if selected < distance {
            self.first();
        } else {
            self.set_selected(Some(selected - distance));
        }
        true
    }

    pub fn next(&mut self) -> bool {
        self.advance(1)
    }

    pub fn previous(&mut self) -> bool {
        self.retreat(1)
    }

    pub fn unselect(&mut self) {
        self.set_selected(None);
    }
}

impl<T> std::ops::Index<usize> for StatefulList<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.items[index]
    }
}
