use crate::layout::{LayoutNode, PaneId, SplitDirection};
use crate::source::ContentSource;
use ratatui::layout::Rect;
use std::collections::HashMap;

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
    panes: HashMap<PaneId, Pane>,
    layout: Option<LayoutNode>,
    focused: PaneId,
    next_id: PaneId,
    manual_layout: bool,
    pub maximized: Option<PaneId>,
}

impl PaneManager {
    pub fn new() -> Self {
        Self {
            panes: HashMap::new(),
            layout: None,
            focused: 0,
            next_id: 0,
            manual_layout: false,
            maximized: None,
        }
    }

    /// Add a pane with the given source, returning its PaneId.
    pub fn add(&mut self, source: Box<dyn ContentSource>) -> PaneId {
        let id = self.next_id;
        self.next_id += 1;
        self.panes.insert(id, Pane::new(source));
        self.focused = id;

        if !self.manual_layout {
            self.rebuild_auto_grid();
        } else {
            // In manual layout mode, add as a leaf at the root level
            // by splitting the focused pane or adding to root
            if let Some(ref mut layout) = self.layout {
                // Just append by splitting the last leaf
                let ids = layout.leaf_ids();
                if let Some(&last) = ids.last() {
                    layout.split_leaf(last, id, SplitDirection::Vertical);
                }
            } else {
                self.layout = Some(LayoutNode::Leaf(id));
            }
        }

        id
    }

    /// Remove a pane by ID.
    pub fn remove(&mut self, id: PaneId) {
        // Before removing, figure out what to focus next if we're removing the focused pane
        let new_focus = if self.focused == id {
            let ids = self.pane_ids_in_order();
            let pos = ids.iter().position(|&x| x == id).unwrap_or(0);
            if ids.len() <= 1 {
                None
            } else if pos >= ids.len() - 1 {
                // Was last in order, focus the previous
                Some(ids[pos - 1])
            } else {
                // Focus the next one (which slides into this position)
                Some(ids[pos + 1])
            }
        } else {
            Some(self.focused)
        };

        self.panes.remove(&id);

        if let Some(ref mut layout) = self.layout {
            let leaf_ids = layout.leaf_ids();
            if leaf_ids.len() <= 1 {
                // Removing the only leaf
                self.layout = None;
            } else {
                layout.remove(id);
            }
        }

        // Clear maximized if we removed the maximized pane
        if self.maximized == Some(id) {
            self.maximized = None;
        }

        // Apply new focus
        self.focused = new_focus.unwrap_or(0);

        // Rebuild auto grid if not manual
        if !self.manual_layout {
            self.rebuild_auto_grid();
        }
    }

    /// Remove the currently focused pane.
    pub fn remove_focused(&mut self) -> Option<PaneId> {
        if self.panes.is_empty() {
            return None;
        }
        let id = self.focused;
        self.remove(id);
        Some(id)
    }

    pub fn focused(&self) -> Option<&Pane> {
        self.panes.get(&self.focused)
    }

    pub fn focused_mut(&mut self) -> Option<&mut Pane> {
        self.panes.get_mut(&self.focused)
    }

    pub fn focused_id(&self) -> PaneId {
        self.focused
    }

    pub fn focus_next(&mut self) {
        let ids = self.pane_ids_in_order();
        if ids.is_empty() {
            return;
        }
        let pos = ids.iter().position(|&id| id == self.focused).unwrap_or(0);
        self.focused = ids[(pos + 1) % ids.len()];
    }

    pub fn focus_prev(&mut self) {
        let ids = self.pane_ids_in_order();
        if ids.is_empty() {
            return;
        }
        let pos = ids.iter().position(|&id| id == self.focused).unwrap_or(0);
        self.focused = ids[if pos == 0 { ids.len() - 1 } else { pos - 1 }];
    }

    pub fn focus_id(&mut self, id: PaneId) {
        if self.panes.contains_key(&id) {
            self.focused = id;
        }
    }

    /// Get pane IDs in layout traversal order.
    pub fn pane_ids_in_order(&self) -> Vec<PaneId> {
        if let Some(ref layout) = self.layout {
            layout.leaf_ids()
        } else {
            let mut ids: Vec<PaneId> = self.panes.keys().copied().collect();
            ids.sort();
            ids
        }
    }

    /// Resolve layout into (PaneId, Rect) pairs for rendering.
    pub fn resolve_layout(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        if let Some(ref layout) = self.layout {
            layout.resolve(area)
        } else {
            Vec::new()
        }
    }

    pub fn count(&self) -> usize {
        self.panes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    /// Iterate over (PaneId, &Pane) in layout order.
    pub fn panes(&self) -> Vec<(PaneId, &Pane)> {
        let ids = self.pane_ids_in_order();
        ids.into_iter()
            .filter_map(|id| self.panes.get(&id).map(|p| (id, p)))
            .collect()
    }

    /// Get a pane by ID.
    pub fn get(&self, id: PaneId) -> Option<&Pane> {
        self.panes.get(&id)
    }

    /// Get a mutable pane by ID.
    pub fn get_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.get_mut(&id)
    }

    /// Split the focused pane in the given direction.
    /// Returns the new pane ID slot (caller must create the pane and insert it).
    pub fn split_focused(
        &mut self,
        dir: SplitDirection,
        source: Box<dyn ContentSource>,
    ) -> Option<PaneId> {
        if self.panes.is_empty() {
            return None;
        }

        let new_id = self.next_id;
        self.next_id += 1;
        self.panes.insert(new_id, Pane::new(source));

        self.manual_layout = true;

        if let Some(ref mut layout) = self.layout {
            layout.split_leaf(self.focused, new_id, dir);
        } else {
            self.layout = Some(LayoutNode::Split {
                direction: dir,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf(self.focused)),
                second: Box::new(LayoutNode::Leaf(new_id)),
            });
        }

        self.focused = new_id;
        Some(new_id)
    }

    /// Toggle maximize for the focused pane.
    pub fn toggle_maximize(&mut self) {
        if self.maximized.is_some() {
            self.maximized = None;
        } else if self.panes.contains_key(&self.focused) {
            self.maximized = Some(self.focused);
        }
    }

    /// Swap the focused pane with the next one in layout order.
    pub fn swap_focused_with_next(&mut self) {
        let ids = self.pane_ids_in_order();
        if ids.len() < 2 {
            return;
        }
        let pos = ids.iter().position(|&id| id == self.focused).unwrap_or(0);
        let next_pos = (pos + 1) % ids.len();
        let a = ids[pos];
        let b = ids[next_pos];

        if let Some(ref mut layout) = self.layout {
            layout.swap_leaves(a, b);
        }
    }

    /// Get a reference to the layout tree (for split detection).
    pub fn layout(&self) -> Option<&LayoutNode> {
        self.layout.as_ref()
    }

    /// Update a split's ratio at the given path.
    pub fn set_ratio_at(&mut self, path: &[usize], ratio: f32) {
        if let Some(ref mut layout) = self.layout {
            layout.set_ratio_at(path, ratio);
        }
    }

    /// Rebuild the auto-grid layout from current pane IDs.
    fn rebuild_auto_grid(&mut self) {
        let ids = {
            let mut ids: Vec<PaneId> = self.panes.keys().copied().collect();
            ids.sort();
            ids
        };
        self.layout = LayoutNode::auto_grid(&ids);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{ContentSource, PaneSpec};
    use anyhow::Result;

    struct MockSource {
        name: String,
    }

    impl ContentSource for MockSource {
        fn capture(&mut self, _w: u16, _h: u16) -> Result<String> {
            Ok("mock".into())
        }
        fn send_keys(&mut self, _keys: &str) -> Result<()> {
            Ok(())
        }
        fn name(&self) -> &str {
            &self.name
        }
        fn source_label(&self) -> &str {
            "mock"
        }
        fn is_interactive(&self) -> bool {
            false
        }
        fn to_spec(&self) -> PaneSpec {
            PaneSpec::Command {
                command: "mock".into(),
                interval_ms: 1000,
            }
        }
    }

    fn mock(name: &str) -> Box<dyn ContentSource> {
        Box::new(MockSource {
            name: name.to_string(),
        })
    }

    #[test]
    fn test_add_pane() {
        let mut mgr = PaneManager::new();
        let id = mgr.add(mock("a"));
        assert_eq!(mgr.count(), 1);
        assert_eq!(mgr.focused_id(), id);
    }

    #[test]
    fn test_remove_focused() {
        let mut mgr = PaneManager::new();
        mgr.add(mock("a"));
        mgr.add(mock("b"));
        assert_eq!(mgr.count(), 2);
        mgr.remove_focused();
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn test_focus_next_cycle() {
        let mut mgr = PaneManager::new();
        let id0 = mgr.add(mock("a"));
        let id1 = mgr.add(mock("b"));
        // After adding b, focus is on b (id1)
        mgr.focus_id(id0);
        assert_eq!(mgr.focused_id(), id0);
        mgr.focus_next();
        assert_eq!(mgr.focused_id(), id1);
        mgr.focus_next();
        assert_eq!(mgr.focused_id(), id0); // wraps
    }

    #[test]
    fn test_focus_prev_cycle() {
        let mut mgr = PaneManager::new();
        let id0 = mgr.add(mock("a"));
        let id1 = mgr.add(mock("b"));
        mgr.focus_id(id0);
        mgr.focus_prev();
        assert_eq!(mgr.focused_id(), id1); // wraps
    }

    #[test]
    fn test_empty_focus() {
        let mut mgr = PaneManager::new();
        mgr.focus_next(); // should not panic
        mgr.focus_prev(); // should not panic
        assert!(mgr.focused().is_none());
    }

    #[test]
    fn test_maximize_toggle() {
        let mut mgr = PaneManager::new();
        let id = mgr.add(mock("a"));
        assert!(mgr.maximized.is_none());
        mgr.toggle_maximize();
        assert_eq!(mgr.maximized, Some(id));
        mgr.toggle_maximize();
        assert!(mgr.maximized.is_none());
    }

    #[test]
    fn test_pane_name_and_label() {
        let mut mgr = PaneManager::new();
        let id = mgr.add(mock("test-pane"));
        let pane = mgr.get(id).unwrap();
        assert_eq!(pane.name(), "test-pane");
        assert_eq!(pane.source_label(), "mock");
        assert!(!pane.is_interactive());
    }

    #[test]
    fn test_get_mut() {
        let mut mgr = PaneManager::new();
        let id = mgr.add(mock("a"));
        assert!(mgr.get_mut(id).is_some());
        assert!(mgr.get_mut(999).is_none());
    }

    #[test]
    fn test_focus_id_invalid() {
        let mut mgr = PaneManager::new();
        let id = mgr.add(mock("a"));
        mgr.focus_id(999); // should not change focus
        assert_eq!(mgr.focused_id(), id);
    }

    #[test]
    fn test_pane_ids_in_order() {
        let mut mgr = PaneManager::new();
        mgr.add(mock("a"));
        mgr.add(mock("b"));
        mgr.add(mock("c"));
        let ids = mgr.pane_ids_in_order();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn test_remove_clears_maximized() {
        let mut mgr = PaneManager::new();
        let id = mgr.add(mock("a"));
        mgr.toggle_maximize();
        assert_eq!(mgr.maximized, Some(id));
        mgr.remove(id);
        assert!(mgr.maximized.is_none());
    }

    #[test]
    fn test_panes_iter() {
        let mut mgr = PaneManager::new();
        mgr.add(mock("a"));
        mgr.add(mock("b"));
        let panes = mgr.panes();
        assert_eq!(panes.len(), 2);
    }

    #[test]
    fn test_split_focused() {
        let mut mgr = PaneManager::new();
        mgr.add(mock("a"));
        let new_id = mgr
            .split_focused(SplitDirection::Vertical, mock("b"))
            .unwrap();
        assert_eq!(mgr.count(), 2);
        assert_eq!(mgr.focused_id(), new_id);
    }

    #[test]
    fn test_swap_focused_with_next() {
        let mut mgr = PaneManager::new();
        let id0 = mgr.add(mock("a"));
        mgr.add(mock("b"));
        mgr.focus_id(id0);
        mgr.swap_focused_with_next(); // should not panic
        assert_eq!(mgr.count(), 2);
    }
}
