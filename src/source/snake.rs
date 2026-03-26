use super::{ContentSource, PaneSpec};
use anyhow::Result;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq)]
struct Pos {
    x: i16,
    y: i16,
}

/// A playable snake game widget.
pub struct SnakeSource {
    body: Vec<Pos>,
    food: Pos,
    direction: Direction,
    score: u32,
    game_over: bool,
    width: u16,
    height: u16,
    last_tick: Instant,
    tick_interval: Duration,
    rng_state: u64,
}

impl SnakeSource {
    pub fn new() -> Self {
        let mut s = Self {
            body: vec![Pos { x: 5, y: 5 }, Pos { x: 4, y: 5 }, Pos { x: 3, y: 5 }],
            food: Pos { x: 10, y: 10 },
            direction: Direction::Right,
            score: 0,
            game_over: false,
            width: 30,
            height: 15,
            last_tick: Instant::now(),
            tick_interval: Duration::from_millis(150),
            rng_state: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
        };
        s.spawn_food();
        s
    }

    fn pseudo_random(&mut self) -> u64 {
        // xorshift64
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        self.rng_state
    }

    fn spawn_food(&mut self) {
        loop {
            let x = (self.pseudo_random() % self.width as u64) as i16;
            let y = (self.pseudo_random() % self.height as u64) as i16;
            let pos = Pos { x, y };
            if !self.body.contains(&pos) {
                self.food = pos;
                return;
            }
        }
    }

    fn tick(&mut self) {
        if self.game_over {
            return;
        }

        let head = self.body[0];
        let new_head = match self.direction {
            Direction::Up => Pos {
                x: head.x,
                y: head.y - 1,
            },
            Direction::Down => Pos {
                x: head.x,
                y: head.y + 1,
            },
            Direction::Left => Pos {
                x: head.x - 1,
                y: head.y,
            },
            Direction::Right => Pos {
                x: head.x + 1,
                y: head.y,
            },
        };

        // Check wall collision
        if new_head.x < 0
            || new_head.y < 0
            || new_head.x >= self.width as i16
            || new_head.y >= self.height as i16
        {
            self.game_over = true;
            return;
        }

        // Check self collision
        if self.body.contains(&new_head) {
            self.game_over = true;
            return;
        }

        self.body.insert(0, new_head);

        // Check food
        if new_head == self.food {
            self.score += 1;
            self.spawn_food();
        } else {
            self.body.pop();
        }
    }

    fn restart(&mut self) {
        self.body = vec![Pos { x: 5, y: 5 }, Pos { x: 4, y: 5 }, Pos { x: 3, y: 5 }];
        self.direction = Direction::Right;
        self.score = 0;
        self.game_over = false;
        self.spawn_food();
    }
}

impl ContentSource for SnakeSource {
    fn capture(&mut self, width: u16, height: u16) -> Result<String> {
        // Update play area to fit the pane
        self.width = width.max(10);
        self.height = height.max(5);

        // Auto-tick
        if self.last_tick.elapsed() >= self.tick_interval {
            self.tick();
            self.last_tick = Instant::now();
        }

        // Fallback text for non-widget rendering
        if self.game_over {
            Ok(format!("GAME OVER - Score: {}", self.score))
        } else {
            Ok(format!("Snake - Score: {}", self.score))
        }
    }

    fn send_keys(&mut self, keys: &str) -> Result<()> {
        match keys {
            "Up" => {
                if self.direction != Direction::Down {
                    self.direction = Direction::Up;
                }
            }
            "Down" => {
                if self.direction != Direction::Up {
                    self.direction = Direction::Down;
                }
            }
            "Left" => {
                if self.direction != Direction::Right {
                    self.direction = Direction::Left;
                }
            }
            "Right" => {
                if self.direction != Direction::Left {
                    self.direction = Direction::Right;
                }
            }
            "Enter" => {
                if self.game_over {
                    self.restart();
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "snake"
    }

    fn source_label(&self) -> &str {
        "game"
    }

    fn is_interactive(&self) -> bool {
        true
    }

    fn to_spec(&self) -> PaneSpec {
        PaneSpec::Plugin {
            plugin_name: "snake".to_string(),
            config: toml::Value::Table(toml::map::Map::new()),
        }
    }

    fn has_custom_render(&self) -> bool {
        true
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height < 3 || area.width < 5 {
            return;
        }

        // Play area is the full area
        let play_w = area.width as i16;
        let play_h = area.height.saturating_sub(1) as i16; // leave 1 row for score

        // Draw border dots (top and bottom rows, left and right columns)
        let border_style = Style::default().fg(Color::DarkGray);
        for x in 0..play_w {
            let bx = area.x + x as u16;
            if bx < area.x + area.width {
                // top border
                if let Some(cell) = buf.cell_mut((bx, area.y)) {
                    cell.set_char('─').set_style(border_style);
                }
                // bottom border
                let by = area.y + play_h as u16;
                if by < area.y + area.height {
                    if let Some(cell) = buf.cell_mut((bx, by)) {
                        cell.set_char('─').set_style(border_style);
                    }
                }
            }
        }
        for y in 0..play_h {
            let by = area.y + y as u16;
            if by < area.y + area.height {
                // left border
                if let Some(cell) = buf.cell_mut((area.x, by)) {
                    cell.set_char('│').set_style(border_style);
                }
                // right border
                let bx = area.x + area.width.saturating_sub(1);
                if let Some(cell) = buf.cell_mut((bx, by)) {
                    cell.set_char('│').set_style(border_style);
                }
            }
        }

        // Draw food
        let fx = area.x + self.food.x as u16 + 1;
        let fy = area.y + self.food.y as u16 + 1;
        if fx < area.x + area.width && fy < area.y + area.height {
            if let Some(cell) = buf.cell_mut((fx, fy)) {
                cell.set_char('●')
                    .set_style(Style::default().fg(Color::Red));
            }
        }

        // Draw snake
        let snake_style = Style::default().fg(Color::Green);
        let head_style = Style::default()
            .fg(Color::LightGreen)
            .add_modifier(Modifier::BOLD);
        for (i, seg) in self.body.iter().enumerate() {
            let sx = area.x + seg.x as u16 + 1;
            let sy = area.y + seg.y as u16 + 1;
            if sx < area.x + area.width && sy < area.y + area.height {
                let style = if i == 0 { head_style } else { snake_style };
                if let Some(cell) = buf.cell_mut((sx, sy)) {
                    cell.set_char('█').set_style(style);
                }
            }
        }

        // Score in top-right
        let score_text = format!("Score: {}", self.score);
        let score_x = area
            .x
            .saturating_add(area.width.saturating_sub(score_text.len() as u16 + 1));
        for (i, ch) in score_text.chars().enumerate() {
            let cx = score_x + i as u16;
            if cx < area.x + area.width {
                if let Some(cell) = buf.cell_mut((cx, area.y)) {
                    cell.set_char(ch)
                        .set_style(Style::default().fg(Color::Yellow));
                }
            }
        }

        // Game over overlay
        if self.game_over {
            let msg = "GAME OVER - Press Enter to restart";
            let msg_line = Line::from(vec![Span::styled(
                msg,
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )]);
            let mid_y = area.y + area.height / 2;
            let para = Paragraph::new(msg_line).alignment(Alignment::Center);
            if mid_y < area.y + area.height {
                Widget::render(para, Rect::new(area.x, mid_y, area.width, 1), buf);
            }
        }
    }
}
