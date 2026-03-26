use ratatui::layout::Rect;

pub type PaneId = u32;

/// Reference to a split node found during hit-testing.
#[derive(Debug, Clone)]
pub struct SplitRef {
    /// Path from the root to the split node (0 = first child, 1 = second child).
    pub path: Vec<usize>,
    /// Direction of the split.
    pub direction: SplitDirection,
    /// Area of the parent split node.
    pub area: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub enum LayoutNode {
    Leaf(PaneId),
    Split {
        direction: SplitDirection,
        ratio: f32,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

impl LayoutNode {
    /// Resolve the tree into (PaneId, Rect) pairs.
    pub fn resolve(&self, area: Rect) -> Vec<(PaneId, Rect)> {
        let mut result = Vec::new();
        self.resolve_inner(area, &mut result);
        result
    }

    fn resolve_inner(&self, area: Rect, out: &mut Vec<(PaneId, Rect)>) {
        match self {
            LayoutNode::Leaf(id) => {
                out.push((*id, area));
            }
            LayoutNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (a, b) = split_rect(area, *direction, *ratio);
                first.resolve_inner(a, out);
                second.resolve_inner(b, out);
            }
        }
    }

    /// Build an auto-grid tree from a list of pane IDs.
    /// Produces the same visual layout as the old compute(): sqrt(n) columns,
    /// rows fill left to right, last row spans remaining width.
    pub fn auto_grid(ids: &[PaneId]) -> Option<Self> {
        if ids.is_empty() {
            return None;
        }
        if ids.len() == 1 {
            return Some(LayoutNode::Leaf(ids[0]));
        }

        let n = ids.len();
        let cols = (n as f64).sqrt().ceil() as usize;
        let rows = ((n as f64) / (cols as f64)).ceil() as usize;

        // Build each row as a horizontal split of columns
        let mut row_nodes: Vec<LayoutNode> = Vec::new();
        let mut idx = 0;
        for _row in 0..rows {
            let remaining = n - idx;
            let row_cols = if remaining < cols { remaining } else { cols };
            let row_ids: Vec<PaneId> = ids[idx..idx + row_cols].to_vec();
            idx += row_cols;

            let row_node = build_equal_split(&row_ids, SplitDirection::Vertical);
            row_nodes.push(row_node);
        }

        // Stack rows vertically
        Some(build_equal_split_nodes(
            &row_nodes,
            SplitDirection::Horizontal,
        ))
    }

    /// Collect leaf IDs in left-to-right, top-to-bottom order.
    pub fn leaf_ids(&self) -> Vec<PaneId> {
        let mut ids = Vec::new();
        self.collect_leaf_ids(&mut ids);
        ids
    }

    fn collect_leaf_ids(&self, out: &mut Vec<PaneId>) {
        match self {
            LayoutNode::Leaf(id) => out.push(*id),
            LayoutNode::Split { first, second, .. } => {
                first.collect_leaf_ids(out);
                second.collect_leaf_ids(out);
            }
        }
    }

    /// Find and remove a leaf by ID. Returns whether it was found and removed.
    /// If the leaf is found, the sibling takes the parent's place.
    pub fn remove(&mut self, id: PaneId) -> bool {
        match self {
            LayoutNode::Leaf(leaf_id) => {
                // Can't remove ourselves at top level — caller handles this
                *leaf_id == id
            }
            LayoutNode::Split { first, second, .. } => {
                // Check if first child is the target leaf
                if let LayoutNode::Leaf(leaf_id) = first.as_ref() {
                    if *leaf_id == id {
                        // Replace self with second
                        *self = *second.clone();
                        return true;
                    }
                }
                // Check if second child is the target leaf
                if let LayoutNode::Leaf(leaf_id) = second.as_ref() {
                    if *leaf_id == id {
                        // Replace self with first
                        *self = *first.clone();
                        return true;
                    }
                }
                // Recurse
                first.remove(id) || second.remove(id)
            }
        }
    }

    /// Split a leaf into two, placing the new_id alongside target in the given direction.
    pub fn split_leaf(&mut self, target: PaneId, new_id: PaneId, dir: SplitDirection) -> bool {
        match self {
            LayoutNode::Leaf(id) => {
                if *id == target {
                    *self = LayoutNode::Split {
                        direction: dir,
                        ratio: 0.5,
                        first: Box::new(LayoutNode::Leaf(target)),
                        second: Box::new(LayoutNode::Leaf(new_id)),
                    };
                    true
                } else {
                    false
                }
            }
            LayoutNode::Split { first, second, .. } => {
                first.split_leaf(target, new_id, dir) || second.split_leaf(target, new_id, dir)
            }
        }
    }

    /// Find the split whose boundary is near (x, y), within tolerance cells.
    /// Returns a SplitRef if found.
    pub fn find_split_at(&self, x: u16, y: u16, area: Rect, tolerance: u16) -> Option<SplitRef> {
        self.find_split_at_inner(x, y, area, tolerance, &mut Vec::new())
    }

    fn find_split_at_inner(
        &self,
        x: u16,
        y: u16,
        area: Rect,
        tolerance: u16,
        path: &mut Vec<usize>,
    ) -> Option<SplitRef> {
        match self {
            LayoutNode::Leaf(_) => None,
            LayoutNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let (a, b) = split_rect(area, *direction, *ratio);

                // Check if the position is near the boundary
                let near_boundary = match direction {
                    SplitDirection::Vertical => {
                        // Boundary is at a.x + a.width (= b.x)
                        let boundary_x = a.x + a.width;
                        x >= boundary_x.saturating_sub(tolerance)
                            && x <= boundary_x + tolerance
                            && y >= area.y
                            && y < area.y + area.height
                    }
                    SplitDirection::Horizontal => {
                        // Boundary is at a.y + a.height (= b.y)
                        let boundary_y = a.y + a.height;
                        y >= boundary_y.saturating_sub(tolerance)
                            && y <= boundary_y + tolerance
                            && x >= area.x
                            && x < area.x + area.width
                    }
                };

                if near_boundary {
                    return Some(SplitRef {
                        path: path.clone(),
                        direction: *direction,
                        area,
                    });
                }

                // Recurse into children
                path.push(0);
                if let Some(result) = first.find_split_at_inner(x, y, a, tolerance, path) {
                    return Some(result);
                }
                path.pop();

                path.push(1);
                if let Some(result) = second.find_split_at_inner(x, y, b, tolerance, path) {
                    return Some(result);
                }
                path.pop();

                None
            }
        }
    }

    /// Update a split's ratio at the given path.
    pub fn set_ratio_at(&mut self, path: &[usize], ratio: f32) {
        if path.is_empty() {
            // Apply ratio to this node
            if let LayoutNode::Split {
                ratio: ref mut r, ..
            } = self
            {
                *r = ratio;
            }
            return;
        }

        if let LayoutNode::Split {
            ref mut first,
            ref mut second,
            ..
        } = self
        {
            match path[0] {
                0 => first.set_ratio_at(&path[1..], ratio),
                1 => second.set_ratio_at(&path[1..], ratio),
                _ => {}
            }
        }
    }

    /// Swap two leaf IDs in the tree.
    pub fn swap_leaves(&mut self, a: PaneId, b: PaneId) {
        match self {
            LayoutNode::Leaf(id) => {
                if *id == a {
                    *id = b;
                } else if *id == b {
                    *id = a;
                }
            }
            LayoutNode::Split { first, second, .. } => {
                first.swap_leaves(a, b);
                second.swap_leaves(a, b);
            }
        }
    }
}

/// Build a balanced binary tree of equal splits from leaf IDs.
fn build_equal_split(ids: &[PaneId], dir: SplitDirection) -> LayoutNode {
    assert!(!ids.is_empty());
    if ids.len() == 1 {
        return LayoutNode::Leaf(ids[0]);
    }
    let mid = ids.len() / 2;
    let first = build_equal_split(&ids[..mid], dir);
    let second = build_equal_split(&ids[mid..], dir);
    let ratio = mid as f32 / ids.len() as f32;
    LayoutNode::Split {
        direction: dir,
        ratio,
        first: Box::new(first),
        second: Box::new(second),
    }
}

/// Build a balanced binary tree of equal splits from existing nodes.
fn build_equal_split_nodes(nodes: &[LayoutNode], dir: SplitDirection) -> LayoutNode {
    assert!(!nodes.is_empty());
    if nodes.len() == 1 {
        return nodes[0].clone();
    }
    let mid = nodes.len() / 2;
    let first = build_equal_split_nodes(&nodes[..mid], dir);
    let second = build_equal_split_nodes(&nodes[mid..], dir);
    let ratio = mid as f32 / nodes.len() as f32;
    LayoutNode::Split {
        direction: dir,
        ratio,
        first: Box::new(first),
        second: Box::new(second),
    }
}

/// Split a rect into two sub-rects based on direction and ratio.
fn split_rect(area: Rect, dir: SplitDirection, ratio: f32) -> (Rect, Rect) {
    match dir {
        SplitDirection::Horizontal => {
            let first_h = (area.height as f32 * ratio).round() as u16;
            let first_h = first_h.min(area.height);
            let second_h = area.height.saturating_sub(first_h);
            (
                Rect::new(area.x, area.y, area.width, first_h),
                Rect::new(area.x, area.y + first_h, area.width, second_h),
            )
        }
        SplitDirection::Vertical => {
            let first_w = (area.width as f32 * ratio).round() as u16;
            let first_w = first_w.min(area.width);
            let second_w = area.width.saturating_sub(first_w);
            (
                Rect::new(area.x, area.y, first_w, area.height),
                Rect::new(area.x + first_w, area.y, second_w, area.height),
            )
        }
    }
}

/// Compatibility wrapper: compute a grid layout for `n` panes within `area`.
/// Returns one Rect per pane (same order as old API).
#[allow(dead_code)]
pub fn compute(n: usize, area: Rect) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    let ids: Vec<PaneId> = (0..n as PaneId).collect();
    let tree = LayoutNode::auto_grid(&ids).unwrap();
    tree.resolve(area).into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_panes() {
        assert!(compute(0, Rect::new(0, 0, 80, 24)).is_empty());
    }

    #[test]
    fn test_one_pane() {
        let area = Rect::new(0, 0, 80, 24);
        let rects = compute(1, area);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0], area);
    }

    #[test]
    fn test_two_panes() {
        let rects = compute(2, Rect::new(0, 0, 80, 24));
        assert_eq!(rects.len(), 2);
        // 2 panes -> 2 cols, 1 row
        assert_eq!(rects[0].width, 40);
        assert_eq!(rects[1].width, 40);
    }

    #[test]
    fn test_four_panes() {
        let rects = compute(4, Rect::new(0, 0, 80, 24));
        assert_eq!(rects.len(), 4);
        // 4 panes -> 2x2 grid
    }

    #[test]
    fn test_three_panes_last_row_spans_full_width() {
        let rects = compute(3, Rect::new(0, 0, 80, 24));
        assert_eq!(rects.len(), 3);
        // 3 panes -> 2x2 grid, last row has 1 pane spanning full width
        assert_eq!(rects[0].width, 40);
        assert_eq!(rects[1].width, 40);
        assert_eq!(rects[2].width, 80);
        assert_eq!(rects[2].x, 0);
    }

    #[test]
    fn test_correct_count() {
        for n in 1..=20 {
            let rects = compute(n, Rect::new(0, 0, 120, 40));
            assert_eq!(rects.len(), n);
        }
    }

    #[test]
    fn test_auto_grid_single() {
        let tree = LayoutNode::auto_grid(&[42]).unwrap();
        let resolved = tree.resolve(Rect::new(0, 0, 80, 24));
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].0, 42);
    }

    #[test]
    fn test_remove_leaf() {
        let mut tree = LayoutNode::auto_grid(&[1, 2, 3]).unwrap();
        assert!(tree.remove(2));
        let ids = tree.leaf_ids();
        assert!(!ids.contains(&2));
        assert!(ids.contains(&1));
        assert!(ids.contains(&3));
    }

    #[test]
    fn test_split_leaf() {
        let mut tree = LayoutNode::auto_grid(&[1]).unwrap();
        assert!(tree.split_leaf(1, 2, SplitDirection::Vertical));
        let ids = tree.leaf_ids();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn test_swap_leaves() {
        let mut tree = LayoutNode::auto_grid(&[1, 2, 3]).unwrap();
        tree.swap_leaves(1, 3);
        let ids = tree.leaf_ids();
        // 1 and 3 should be swapped in position
        let pos_1 = ids.iter().position(|&id| id == 1).unwrap();
        let pos_3 = ids.iter().position(|&id| id == 3).unwrap();
        assert!(pos_3 < pos_1); // 3 is now where 1 was
    }
}
