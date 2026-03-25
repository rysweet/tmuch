pub struct Pane {
    pub session_name: String,
    pub content: String,
    /// If true, we created this session and should kill it on drop
    pub owned: bool,
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

    pub fn add(&mut self, session_name: String, owned: bool) {
        self.panes.push(Pane {
            session_name,
            content: String::new(),
            owned,
        });
        // Focus the newly added pane
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
