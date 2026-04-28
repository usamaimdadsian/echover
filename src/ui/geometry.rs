#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, point: (f32, f32)) -> bool {
        let (px, py) = point;
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    pub fn split_horizontal(&self, left_width: f32) -> (Self, Self) {
        let left_width = left_width.clamp(0.0, self.width);
        (
            Self::new(self.x, self.y, left_width, self.height),
            Self::new(
                self.x + left_width,
                self.y,
                self.width - left_width,
                self.height,
            ),
        )
    }

    #[allow(dead_code)]
    pub fn split_vertical(&self, top_height: f32) -> (Self, Self) {
        let top_height = top_height.clamp(0.0, self.height);
        (
            Self::new(self.x, self.y, self.width, top_height),
            Self::new(
                self.x,
                self.y + top_height,
                self.width,
                self.height - top_height,
            ),
        )
    }

    pub fn inset(&self, padding: f32) -> Self {
        let width = (self.width - padding * 2.0).max(0.0);
        let height = (self.height - padding * 2.0).max(0.0);
        Self::new(self.x + padding, self.y + padding, width, height)
    }
}
