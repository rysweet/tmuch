/// Input mode for the command editor overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorInputMode {
    Browse,
    InputKey,
    InputCommand,
}

/// State for the command editor overlay.
pub struct CommandEditorState {
    pub entries: Vec<(char, String)>,
    pub selected: usize,
    pub input_mode: EditorInputMode,
    pub input_buffer: String,
    pub pending_key: Option<char>,
}

impl CommandEditorState {
    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1) % self.entries.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.entries.is_empty() {
            self.selected = if self.selected == 0 {
                self.entries.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn delete_selected(&mut self) -> Option<char> {
        if self.entries.is_empty() {
            return None;
        }
        let (key, _) = self.entries.remove(self.selected);
        if self.selected >= self.entries.len() && !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
        Some(key)
    }
}

/// State for the app launcher overlay.
pub struct AppLauncherState {
    pub apps: Vec<(&'static str, &'static str, &'static str)>, // (name, desc, usage)
    pub selected: usize,
}

impl AppLauncherState {
    pub fn new() -> Self {
        let registry = crate::source::registry::PluginRegistry::new();
        let apps: Vec<_> = registry
            .list()
            .iter()
            .map(|info| (info.name, info.description, info.usage))
            .collect();
        Self { apps, selected: 0 }
    }

    pub fn select_next(&mut self) {
        if !self.apps.is_empty() {
            self.selected = (self.selected + 1) % self.apps.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.apps.is_empty() {
            self.selected = if self.selected == 0 {
                self.apps.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn selected_usage(&self) -> Option<&'static str> {
        self.apps.get(self.selected).map(|(_, _, u)| *u)
    }
}

/// State for tracking border drag resize.
pub struct DragState {
    /// Path to the split node being dragged.
    pub split_path: Vec<usize>,
    /// Direction of the split being dragged.
    pub direction: crate::layout::SplitDirection,
    /// Area of the parent split node, used to compute ratios.
    pub parent_area: ratatui::layout::Rect,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_state_select_next() {
        let mut editor = CommandEditorState {
            entries: vec![('1', "cmd1".into()), ('2', "cmd2".into())],
            selected: 0,
            input_mode: EditorInputMode::Browse,
            input_buffer: String::new(),
            pending_key: None,
        };
        editor.select_next();
        assert_eq!(editor.selected, 1);
        editor.select_next();
        assert_eq!(editor.selected, 0); // wraps around
    }

    #[test]
    fn test_editor_state_select_prev() {
        let mut editor = CommandEditorState {
            entries: vec![('1', "cmd1".into()), ('2', "cmd2".into())],
            selected: 0,
            input_mode: EditorInputMode::Browse,
            input_buffer: String::new(),
            pending_key: None,
        };
        editor.select_prev();
        assert_eq!(editor.selected, 1); // wraps around
    }

    #[test]
    fn test_editor_state_delete_selected() {
        let mut editor = CommandEditorState {
            entries: vec![('1', "cmd1".into()), ('2', "cmd2".into())],
            selected: 0,
            input_mode: EditorInputMode::Browse,
            input_buffer: String::new(),
            pending_key: None,
        };
        let key = editor.delete_selected();
        assert_eq!(key, Some('1'));
        assert_eq!(editor.entries.len(), 1);
        assert_eq!(editor.entries[0].0, '2');
    }

    #[test]
    fn test_editor_state_delete_empty() {
        let mut editor = CommandEditorState {
            entries: vec![],
            selected: 0,
            input_mode: EditorInputMode::Browse,
            input_buffer: String::new(),
            pending_key: None,
        };
        assert_eq!(editor.delete_selected(), None);
    }

    #[test]
    fn test_launcher_state_navigation() {
        let mut launcher = AppLauncherState {
            apps: vec![("a", "desc a", "usage a"), ("b", "desc b", "usage b")],
            selected: 0,
        };
        launcher.select_next();
        assert_eq!(launcher.selected, 1);
        launcher.select_next();
        assert_eq!(launcher.selected, 0);
        launcher.select_prev();
        assert_eq!(launcher.selected, 1);
    }

    #[test]
    fn test_launcher_selected_usage() {
        let launcher = AppLauncherState {
            apps: vec![("a", "desc", "usage_a")],
            selected: 0,
        };
        assert_eq!(launcher.selected_usage(), Some("usage_a"));
    }

    #[test]
    fn test_launcher_empty() {
        let launcher = AppLauncherState {
            apps: vec![],
            selected: 0,
        };
        assert_eq!(launcher.selected_usage(), None);
    }

    #[test]
    fn test_launcher_select_prev_wraps_from_zero() {
        let mut launcher = AppLauncherState {
            apps: vec![("a", "d", "u"), ("b", "d", "u"), ("c", "d", "u")],
            selected: 0,
        };
        launcher.select_prev();
        assert_eq!(launcher.selected, 2);
    }

    #[test]
    fn test_launcher_select_next_wraps_at_end() {
        let mut launcher = AppLauncherState {
            apps: vec![("a", "d", "u"), ("b", "d", "u")],
            selected: 1,
        };
        launcher.select_next();
        assert_eq!(launcher.selected, 0);
    }
}
