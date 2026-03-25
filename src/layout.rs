use ratatui::layout::Rect;

/// Compute a grid layout for `n` panes within `area`.
/// Returns one Rect per pane.
pub fn compute(n: usize, area: Rect) -> Vec<Rect> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![area];
    }

    let cols = (n as f64).sqrt().ceil() as u16;
    let rows = ((n as f64) / (cols as f64)).ceil() as u16;

    let col_width = area.width / cols;
    let row_height = area.height / rows;

    let mut rects = Vec::with_capacity(n);
    let mut idx = 0;

    for row in 0..rows {
        for col in 0..cols {
            if idx >= n {
                break;
            }
            // Last column/row gets remaining pixels
            let x = area.x + col * col_width;
            let y = area.y + row * row_height;
            let w = if col == cols - 1 {
                area.width - col * col_width
            } else {
                col_width
            };
            let h = if row == rows - 1 {
                area.height - row * row_height
            } else {
                row_height
            };
            rects.push(Rect::new(x, y, w, h));
            idx += 1;
        }
    }

    rects
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
    fn test_correct_count() {
        for n in 1..=20 {
            let rects = compute(n, Rect::new(0, 0, 120, 40));
            assert_eq!(rects.len(), n);
        }
    }
}
