use alloc::{boxed::Box, vec, vec::Vec};

use crate::{ArtError, COLUMNS, COUNTER, Facing, HEIGHT, Point, ROWS, Rect, TILE, WIDTH};

type Color = [u8; 4];

const fn rgb(hex: u32) -> Color {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255]
}

pub(super) struct Frame {
    pixels: Box<[u8]>,
}

impl Frame {
    pub(super) fn new() -> Self {
        Self {
            pixels: vec![0; WIDTH * HEIGHT * 4].into_boxed_slice(),
        }
    }

    pub(super) fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub(super) fn copy_from(&mut self, other: &Self) {
        self.pixels.copy_from_slice(&other.pixels);
    }

    pub(super) fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x0 = x.clamp(0, WIDTH as i32) as usize;
        let x1 = (x + w).clamp(0, WIDTH as i32) as usize;
        for row in y.max(0)..(y + h).min(HEIGHT as i32) {
            let start = row as usize * WIDTH * 4;
            for pixel in self.pixels[start + x0 * 4..start + x1 * 4].chunks_exact_mut(4) {
                pixel.copy_from_slice(&color);
            }
        }
    }

    pub(super) fn blend_pixel(&mut self, x: i32, y: i32, source: &[u8]) {
        let (x, y) = (x as usize, y as usize);
        if x >= WIDTH || y >= HEIGHT {
            return;
        }
        if source[3] == 0 {
            return;
        }
        let offset = (y * WIDTH + x) * 4;
        let destination = &mut self.pixels[offset..offset + 4];
        if source[3] == 255 {
            destination.copy_from_slice(source);
            return;
        }
        let alpha = u16::from(source[3]);
        for i in 0..3 {
            destination[i] =
                ((u16::from(source[i]) * alpha + u16::from(destination[i]) * (255 - alpha) + 127)
                    / 255) as u8;
        }
    }
}

pub(super) struct Image {
    width: usize,
    height: usize,
    pixels: Vec<Color>,
}

impl Image {
    pub(super) fn load(bytes: &[u8]) -> Result<Self, ArtError> {
        let (header, rgba) = bytes.split_at_checked(4).ok_or("Missing art header")?;
        let width = usize::from(u16::from_le_bytes([header[0], header[1]]));
        let height = usize::from(u16::from_le_bytes([header[2], header[3]]));
        if width == 0
            || height == 0
            || width
                .checked_mul(height)
                .and_then(|size| size.checked_mul(4))
                != Some(rgba.len())
        {
            return Err("Invalid embedded art size");
        }
        Ok(Self {
            width,
            height,
            pixels: rgba
                .chunks_exact(4)
                .map(|p| [p[0], p[1], p[2], p[3]])
                .collect(),
        })
    }

    pub(super) fn sprite(self, max_height: usize) -> Result<Self, ArtError> {
        self.fit(usize::MAX, max_height)
    }

    fn fit(self, max_width: usize, max_height: usize) -> Result<Self, ArtError> {
        let (mut left, mut top, mut right, mut bottom) = (self.width, self.height, 0, 0);
        for (i, pixel) in self.pixels.iter().enumerate() {
            if pixel[3] > 0 {
                let (x, y) = (i % self.width, i / self.width);
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
            }
        }
        if right == 0 {
            return Err("Sprite is empty");
        }
        let (source_w, source_h) = (right - left, bottom - top);
        let height = source_h
            .min(max_height)
            .min(max_width.saturating_mul(source_h) / source_w)
            .max(1);
        let width = ((source_w * height + source_h / 2) / source_h).max(1);
        Ok(self
            .region(Rect::new(
                left as i32,
                top as i32,
                source_w as i32,
                source_h as i32,
            ))?
            .scaled(width, height))
    }

    fn scaled(&self, width: usize, height: usize) -> Self {
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            let sy = (2 * y + 1) * self.height / (2 * height);
            for x in 0..width {
                let sx = (2 * x + 1) * self.width / (2 * width);
                pixels.push(self.pixels[sy * self.width + sx]);
            }
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    fn draw(&self, frame: &mut Frame, x: i32, y: i32) {
        for (row, pixels) in self.pixels.chunks_exact(self.width).enumerate() {
            for (column, pixel) in pixels.iter().enumerate() {
                frame.blend_pixel(x + column as i32, y + row as i32, pixel);
            }
        }
    }

    fn region(&self, rect: Rect) -> Result<Self, ArtError> {
        let (x, y, w, h) = (
            rect.x as usize,
            rect.y as usize,
            rect.w as usize,
            rect.h as usize,
        );
        if w == 0 || h == 0 || x.saturating_add(w) > self.width || y.saturating_add(h) > self.height
        {
            return Err("Art region is out of bounds");
        }
        let mut pixels = Vec::with_capacity(w * h);
        for row in y..y + h {
            let start = row * self.width + x;
            pixels.extend_from_slice(&self.pixels[start..start + w]);
        }
        Ok(Self {
            width: w,
            height: h,
            pixels,
        })
    }

    pub(super) fn draw_at_feet(&self, frame: &mut Frame, feet: Point) {
        let left = feet.x - self.width as i32 / 2;
        frame.rect(
            left + 1,
            feet.y - 2,
            self.width as i32 - 2,
            3,
            rgb(0x543827),
        );
        self.draw(frame, left, feet.y - self.height as i32);
    }

    fn tile(mut self, light: u16) -> Result<Self, ArtError> {
        if (self.width, self.height) != (TILE as usize, TILE as usize)
            || self.pixels.iter().any(|pixel| pixel[3] != 255)
        {
            return Err("Room tiles must be opaque 16×16 PNGs");
        }
        for color in &mut self.pixels {
            for channel in &mut color[..3] {
                *channel = ((u16::from(*channel) * light + 127) / 255) as u8;
            }
        }
        Ok(self)
    }

    fn trim(&self, frame: &mut Frame, rect: Rect, vertical: bool) {
        let depth = if vertical { rect.w } else { rect.h };
        for y in 0..rect.h {
            for x in 0..rect.w {
                let (along, across) = if vertical { (y, x) } else { (x, y) };
                let src = (across * TILE / depth * TILE + along % TILE) as usize;
                frame.blend_pixel(rect.x + x, rect.y + y, &self.pixels[src]);
            }
        }
    }
}

pub(super) struct Fixtures {
    cap: Image,
    counter: Image,
    threshold: Image,
}

impl Fixtures {
    pub(super) fn load() -> Result<Self, ArtError> {
        Ok(Self {
            cap: Image::load(embedded_art!("wall-cap"))?.tile(225)?,
            counter: Image::load(embedded_art!("counter"))?.fit(COUNTER.w as usize, 24)?,
            threshold: Image::load(embedded_art!("threshold"))?.fit(44, 13)?,
        })
    }

    pub(super) fn draw_counter(&self, frame: &mut Frame) {
        self.counter.draw(
            frame,
            COUNTER.x + (COUNTER.w - self.counter.width as i32) / 2,
            COUNTER.y + COUNTER.h + 2 - self.counter.height as i32,
        );
    }

    fn draw_front_wall(&self, frame: &mut Frame) {
        for (x, w) in [(5, 133), (182, 133)] {
            self.cap.trim(frame, Rect::new(x, 160, w, 13), false);
        }
        self.threshold
            .draw(frame, (WIDTH as i32 - self.threshold.width as i32) / 2, 160);
    }
}

pub(super) fn room_floor(fixtures: &Fixtures) -> Result<Frame, ArtError> {
    let floor = Image::load(embedded_art!("floor"))?.tile(193)?;
    let wall = Image::load(embedded_art!("wall"))?.tile(235)?;
    // Keep the complete sash and discard the loose pixels outside the frame.
    let window = Image::load(embedded_art!("window"))?.region(Rect::new(9, 7, 30, 18))?;
    let rug = Image::load(embedded_art!("rug"))?.fit(88, 26)?;
    let mut frame = Frame::new();
    frame.rect(0, 0, WIDTH as i32, HEIGHT as i32, rgb(0x252523));
    for row in 1..ROWS - 1 {
        for column in 1..COLUMNS - 1 {
            floor.draw(&mut frame, column * TILE, row * TILE);
        }
    }

    // Repeat the straight wall section between the posts, then place the windows.
    frame.rect(8, 2, 304, 22, rgb(0x3f2d28));
    for column in 1..COLUMNS - 1 {
        wall.draw(&mut frame, column * TILE, 5);
    }
    frame.rect(12, 22, 296, 2, rgb(0x47312c));
    for x in [14, 98, 122, 194, 218, 303] {
        fixtures.cap.trim(&mut frame, Rect::new(x, 3, 3, 19), true);
    }
    for x in [43, 246] {
        window.draw(&mut frame, x, 3);
    }
    for x in [5, 305] {
        fixtures
            .cap
            .trim(&mut frame, Rect::new(x, 9, 10, 151), true);
    }
    rug.draw(&mut frame, (WIDTH as i32 - rug.width as i32) / 2, 122);
    // Collision keeps entity feet above this wall, so it can be drawn once.
    fixtures.draw_front_wall(&mut frame);
    Ok(frame)
}

pub(super) struct Traveler {
    frames: Vec<Image>,
    columns: usize,
    baselines: [i32; 4],
}

impl Traveler {
    const CELL: usize = 36;
    const DRAW_SIZE: usize = 30;
    const TICKS_PER_FRAME: usize = 6;

    pub(super) fn load() -> Result<Self, ArtError> {
        let atlas = Image::load(embedded_art!("traveler"))?;
        let columns = atlas.width / Self::CELL;
        if columns < 2 || atlas.width % Self::CELL != 0 || atlas.height != 4 * Self::CELL {
            return Err("Invalid traveler frame grid");
        }
        let mut frames = Vec::with_capacity(columns * 4);
        let mut baselines = [0; 4];
        for (row, baseline) in baselines.iter_mut().enumerate() {
            for column in 0..columns {
                let cell = Self::CELL as i32;
                let region = Rect::new(column as i32 * cell, row as i32 * cell, cell, cell);
                let image = atlas
                    .region(region)?
                    .scaled(Self::DRAW_SIZE, Self::DRAW_SIZE);
                let bottom = image
                    .pixels
                    .iter()
                    .rposition(|pixel| pixel[3] > 0)
                    .ok_or("Empty traveler frame")?;
                *baseline = (*baseline).max((bottom / Self::DRAW_SIZE + 1) as i32);
                frames.push(image);
            }
        }
        Ok(Self {
            frames,
            columns,
            baselines,
        })
    }

    pub(super) fn advance(&self, stride: usize) -> usize {
        stride % ((self.columns - 1) * Self::TICKS_PER_FRAME) + 1
    }

    pub(super) fn draw(&self, frame: &mut Frame, feet: Point, facing: Facing, stride: usize) {
        let row = match facing {
            Facing::Up => 0,
            Facing::Down => 1,
            Facing::Left => 2,
            Facing::Right => 3,
        };
        let column = if stride == 0 {
            0
        } else {
            1 + (stride - 1) / Self::TICKS_PER_FRAME
        };
        frame.rect(feet.x - 5, feet.y - 1, 10, 2, rgb(0x523a2b));
        self.frames[row * self.columns + column].draw(
            frame,
            feet.x - Self::DRAW_SIZE as i32 / 2,
            feet.y - self.baselines[row],
        );
    }
}
