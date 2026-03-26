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

    /// Iterate over (PaneId, &mut Pane) in layout order.
    #[allow(dead_code)]
    pub fn panes_mut(&mut self) -> Vec<(PaneId, &mut Pane)> {
        let ids = self.pane_ids_in_order();
        // We need to collect mutable refs safely
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(pane) = self.panes.get_mut(&id) {
                // SAFETY: Each id is unique, so we get disjoint mutable borrows.
                // We use unsafe to bypass the borrow checker limitation with HashMap.
                let pane_ptr = pane as *mut Pane;
                result.push((id, unsafe { &mut *pane_ptr }));
            }
        }
        result
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
