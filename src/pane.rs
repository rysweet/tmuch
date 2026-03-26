use crate::source::ContentSource;

pub struct Pane {
    pub source: Box<dyn ContentSource>,
    pub content: String,
}

impl Pane {
    pub fn new(source: Box<dyn ContentSource>) -> Self {
        Self {
            source,
            content: String::new(),
        }
    }

    pub fn name(&self) -> &str {
        self.source.name()
    }

    pub fn source_label(&self) -> &str {
        self.source.source_label()
    }

    pub fn is_interactive(&self) -> bool {
        self.source.is_interactive()
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        self.source.cleanup();
    }
}

pub struct PaneManager {
    panes: Vec<Pane>,
    focused: usize,
}

impl PaneManager {
    pub fn new() -> Self {
        Self {
            panes: Vec::new(),
            focused: 0,
        }
    }

    pub fn add(&mut self, source: Box<dyn ContentSource>) {
        self.panes.push(Pane::new(source));
        self.focused = self.panes.len() - 1;
    }

    pub fn remove_focused(&mut self) -> Option<Pane> {
        if self.panes.is_empty() {
            return None;
        }
        let pane = self.panes.remove(self.focused);
        if self.focused >= self.panes.len() && !self.panes.is_empty() {
            self.focused = self.panes.len() - 1;
        }
        Some(pane)
    }

    pub fn focused(&self) -> Option<&Pane> {
        self.panes.get(self.focused)
    }

    pub fn focused_mut(&mut self) -> Option<&mut Pane> {
        self.panes.get_mut(self.focused)
    }

    pub fn focused_index(&self) -> usize {
        self.focused
    }

    pub fn focus_next(&mut self) {
        if !self.panes.is_empty() {
            self.focused = (self.focused + 1) % self.panes.len();
        }
    }

    pub fn focus_prev(&mut self) {
        if !self.panes.is_empty() {
            self.focused = if self.focused == 0 {
                self.panes.len() - 1
            } else {
                self.focused - 1
            };
        }
    }

    pub fn focus_index(&mut self, idx: usize) {
        if idx < self.panes.len() {
            self.focused = idx;
        }
    }

    pub fn panes(&self) -> &[Pane] {
        &self.panes
    }

    pub fn panes_mut(&mut self) -> &mut [Pane] {
        &mut self.panes
    }

    pub fn count(&self) -> usize {
        self.panes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }
}
