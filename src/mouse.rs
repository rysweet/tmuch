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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_up_clears_drag() {
        let config = crate::config::Config::default();
        let mut app = App::new(config);
        app.drag_state = Some(DragState {
            split_path: vec![0],
            direction: SplitDirection::Vertical,
            parent_area: Rect::new(0, 0, 80, 24),
        });
        handle_mouse_up(&mut app);
        assert!(app.drag_state.is_none());
    }

    #[test]
    fn test_mouse_drag_no_drag_state() {
        let config = crate::config::Config::default();
        let mut app = App::new(config);
        // Should not panic when no drag state
        handle_mouse_drag(&mut app, 40, 12);
    }

    #[test]
    fn test_mouse_down_no_panes() {
        let config = crate::config::Config::default();
        let mut app = App::new(config);
        let main_area = Rect::new(0, 0, 80, 24);
        // Should not panic when no panes
        handle_mouse_down(&mut app, 10, 10, main_area);
    }

    #[test]
    fn test_mouse_drag_vertical_ratio() {
        let config = crate::config::Config::default();
        let mut app = App::new(config);
        app.drag_state = Some(DragState {
            split_path: vec![],
            direction: SplitDirection::Vertical,
            parent_area: Rect::new(0, 0, 100, 24),
        });
        handle_mouse_drag(&mut app, 70, 12);
        // The ratio should have been attempted (even if no layout exists)
    }

    #[test]
    fn test_mouse_drag_horizontal_ratio() {
        let config = crate::config::Config::default();
        let mut app = App::new(config);
        app.drag_state = Some(DragState {
            split_path: vec![],
            direction: SplitDirection::Horizontal,
            parent_area: Rect::new(0, 0, 80, 100),
        });
        handle_mouse_drag(&mut app, 40, 50);
    }

    #[test]
    fn test_mouse_drag_zero_width() {
        let config = crate::config::Config::default();
        let mut app = App::new(config);
        app.drag_state = Some(DragState {
            split_path: vec![],
            direction: SplitDirection::Vertical,
            parent_area: Rect::new(0, 0, 0, 24),
        });
        // Should early return, not panic
        handle_mouse_drag(&mut app, 0, 12);
    }

    #[test]
    fn test_mouse_drag_zero_height() {
        let config = crate::config::Config::default();
        let mut app = App::new(config);
        app.drag_state = Some(DragState {
            split_path: vec![],
            direction: SplitDirection::Horizontal,
            parent_area: Rect::new(0, 0, 80, 0),
        });
        handle_mouse_drag(&mut app, 40, 0);
    }
}
