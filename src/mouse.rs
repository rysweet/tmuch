use crate::app::App;
use crate::editor_state::DragState;
use crate::layout::SplitDirection;
use ratatui::layout::Rect;

/// Handle mouse-down for border drag detection.
pub fn handle_mouse_down(app: &mut App, col: u16, row: u16, main_area: Rect) {
    if let Some(layout) = app.pane_manager.layout() {
        if let Some(split_ref) = layout.find_split_at(col, row, main_area, 1) {
            app.drag_state = Some(DragState {
                split_path: split_ref.path,
                direction: split_ref.direction,
                parent_area: split_ref.area,
            });
            return;
        }
    }

    for (id, rect) in &app.pane_rects {
        if col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
        {
            app.pane_manager.focus_id(*id);
            break;
        }
    }
}

/// Handle mouse drag for border resize.
pub fn handle_mouse_drag(app: &mut App, col: u16, row: u16) {
    if let Some(ref drag) = app.drag_state {
        let path = drag.split_path.clone();
        let direction = drag.direction;
        let parent_area = drag.parent_area;

        let ratio = match direction {
            SplitDirection::Vertical => {
                if parent_area.width == 0 {
                    return;
                }
                let rel = col.saturating_sub(parent_area.x) as f32 / parent_area.width as f32;
                rel.clamp(0.1, 0.9)
            }
            SplitDirection::Horizontal => {
                if parent_area.height == 0 {
                    return;
                }
                let rel = row.saturating_sub(parent_area.y) as f32 / parent_area.height as f32;
                rel.clamp(0.1, 0.9)
            }
        };

        app.pane_manager.set_ratio_at(&path, ratio);
    }
}

/// Handle mouse up -- stop dragging.
pub fn handle_mouse_up(app: &mut App) {
    app.drag_state = None;
}
