#![no_std]

extern crate alloc;

macro_rules! embedded_art {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/", $name, ".rgba"))
    };
}

mod art;
mod dialogue;
mod hud;

use art::{Fixtures, Frame, Image, Traveler};
use dialogue::CheckIn;
pub use dialogue::{Choice, Dialogue, Topic};
pub use hud::HudControls;

pub type ArtError = &'static str;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 176;
pub const TICKS_PER_SECOND: u32 = 60;
pub const UP: u8 = 1;
pub const DOWN: u8 = 2;
pub const LEFT: u8 = 4;
pub const RIGHT: u8 = 8;
pub const CONFIRM: u8 = 16;
pub const BACK: u8 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Interaction {
    Talk = 1,
    Listen,
}

const TILE: i32 = 16;
const COLUMNS: i32 = WIDTH as i32 / TILE;
const ROWS: i32 = HEIGHT as i32 / TILE;
const SPAWN: Point = Point { x: 160, y: 153 };
const INNKEEPER: Point = Point { x: 160, y: 46 };
const FROG: Point = Point { x: 239, y: 119 };
const COUNTER: Rect = Rect::new(8 * TILE, 3 * TILE, 4 * TILE, TILE);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Clone, Copy)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Rect {
    const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    fn overlaps(self, other: Self) -> bool {
        self.x < other.x + other.w
            && self.x + self.w > other.x
            && self.y < other.y + other.h
            && self.y + self.h > other.y
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Facing {
    Up,
    Down,
    Left,
    Right,
}

pub struct Room {
    traveler: Point,
    facing: Facing,
    stride: usize,
    check_in: CheckIn,
    previous_input: u8,
    traveler_art: Traveler,
    frog: Image,
    innkeeper: Image,
    fixtures: Fixtures,
    floor: Frame,
    frame: Frame,
}

impl Room {
    pub fn new() -> Result<Self, ArtError> {
        let fixtures = Fixtures::load()?;
        let mut room = Self {
            traveler: SPAWN,
            facing: Facing::Up,
            stride: 0,
            check_in: CheckIn::default(),
            previous_input: 0,
            traveler_art: Traveler::load()?,
            frog: Image::load(embedded_art!("frog"))?.sprite(13)?,
            innkeeper: Image::load(embedded_art!("innkeeper"))?.sprite(24)?,
            floor: art::room_floor(&fixtures)?,
            fixtures,
            frame: Frame::new(),
        };
        room.render();
        Ok(room)
    }

    pub fn tick(&mut self, input: u8) {
        if self.step(input) {
            self.render();
        }
    }

    /// Update one simulation tick and report whether the visual state changed.
    pub fn step(&mut self, input: u8) -> bool {
        let pressed = input & !self.previous_input;
        self.previous_input = input;
        if let Some(dialogue) = self.dialogue() {
            let before = self.check_in;
            if pressed & BACK != 0 {
                self.check_in.close();
            } else if pressed & CONFIRM != 0 {
                self.choose(dialogue.selected);
            } else if pressed & (UP | DOWN) != 0 {
                self.check_in
                    .move_selection(dialogue.choices.len(), pressed & DOWN != 0);
            }
            return before != self.check_in;
        }

        if pressed & CONFIRM != 0 && self.open_interaction() {
            return true;
        }

        let horizontal = i32::from(input & RIGHT != 0) - i32::from(input & LEFT != 0);
        let vertical = i32::from(input & DOWN != 0) - i32::from(input & UP != 0);
        // Keep cardinal movement at one pixel per tick, including when two keys are held.
        let (dx, dy) = if horizontal != 0 {
            (horizontal, 0)
        } else {
            (0, vertical)
        };
        let previous = (self.traveler, self.facing, self.stride);
        self.facing = match (dx, dy) {
            (1, _) => Facing::Right,
            (-1, _) => Facing::Left,
            (_, 1) => Facing::Down,
            (_, -1) => Facing::Up,
            _ => self.facing,
        };
        let next = Point {
            x: self.traveler.x + dx,
            y: self.traveler.y + dy,
        };
        if next != self.traveler && !blocked(next) {
            self.traveler = next;
            self.stride = self.traveler_art.advance(self.stride);
        } else {
            self.stride = 0;
        }
        previous != (self.traveler, self.facing, self.stride)
    }

    pub fn pixels(&self) -> &[u8] {
        self.frame.pixels()
    }

    pub fn dialogue(&self) -> Option<Dialogue> {
        self.check_in.view()
    }

    pub fn interaction(&self) -> Option<Interaction> {
        if self.dialogue().is_some() {
            return None;
        }
        let Point { x, y } = self.traveler;
        if (COUNTER.x..=COUNTER.x + COUNTER.w).contains(&x)
            && (COUNTER.y + COUNTER.h..=COUNTER.y + COUNTER.h + 18).contains(&y)
        {
            Some(Interaction::Talk)
        } else if !self.check_in.heard_staff
            && (COUNTER.x + COUNTER.w + 9..=COUNTER.x + COUNTER.w + 27).contains(&x)
            && (COUNTER.y + 4..=COUNTER.y + COUNTER.h + 12).contains(&y)
        {
            Some(Interaction::Listen)
        } else {
            None
        }
    }

    pub fn open_interaction(&mut self) -> bool {
        let Some(interaction) = self.interaction() else {
            return false;
        };
        self.check_in.open(interaction == Interaction::Listen);
        self.stride = 0;
        true
    }

    pub fn choose(&mut self, index: usize) -> bool {
        let Some(choice) = self.dialogue().and_then(|view| view.choices.get(index)) else {
            return false;
        };
        if self.check_in.choose(choice.topic) {
            self.traveler = SPAWN;
            self.facing = Facing::Up;
            self.stride = 0;
        }
        true
    }

    pub fn submit_text(&mut self, text: &str) -> bool {
        self.dialogue()
            .and_then(|dialogue| dialogue.match_text(text))
            .is_some_and(|index| self.choose(index))
    }

    pub fn render_hud(&mut self, viewport_width: usize, microphone: HudControls<'_>) {
        let camera = self.camera_x(viewport_width) as i32;
        let dialogue = self.dialogue();
        let interaction = self.interaction();
        hud::draw(
            &mut self.frame,
            camera,
            viewport_width as i32,
            dialogue,
            interaction,
            microphone,
        );
    }

    /// Center a smaller view on the traveler, stopping at the edges of the room.
    pub fn camera_x(&self, viewport_width: usize) -> usize {
        (self.traveler.x as usize)
            .saturating_sub(viewport_width / 2)
            .min(WIDTH.saturating_sub(viewport_width))
    }

    pub fn render(&mut self) {
        self.frame.copy_from(&self.floor);
        // Draw objects in foot position order so the player can pass behind them.
        let mut entities = [
            (COUNTER.y + COUNTER.h, 0),
            (INNKEEPER.y, 1),
            (FROG.y, 2),
            (self.traveler.y, 3),
        ];
        entities.sort_unstable();
        for (_, entity) in entities {
            match entity {
                0 => self.fixtures.draw_counter(&mut self.frame),
                1 => self.innkeeper.draw_at_feet(&mut self.frame, INNKEEPER),
                2 => self.frog.draw_at_feet(&mut self.frame, FROG),
                _ => {
                    self.traveler_art
                        .draw(&mut self.frame, self.traveler, self.facing, self.stride)
                }
            }
        }
    }
}

fn blocked(feet: Point) -> bool {
    let body = Rect::new(feet.x - 4, feet.y - 4, 8, 6);
    let left_tile = body.x.div_euclid(TILE);
    let top_tile = body.y.div_euclid(TILE);
    let right_tile = (body.x + body.w - 1).div_euclid(TILE);
    let bottom_tile = (body.y + body.h - 1).div_euclid(TILE);
    left_tile < 1
        || top_tile < 1
        || right_tile >= COLUMNS - 1
        || bottom_tile >= ROWS - 1
        || body.overlaps(COUNTER)
        || body.overlaps(Rect::new(INNKEEPER.x - 6, INNKEEPER.y - 6, 12, 6))
        || body.overlaps(Rect::new(FROG.x - 5, FROG.y - 5, 10, 5))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk(room: &mut Room, input: u8, ticks: usize) {
        for _ in 0..ticks {
            room.tick(input);
        }
    }

    #[test]
    fn camera_centers_a_ds_view_then_clamps_to_room_edges() {
        let mut room = Room::new().unwrap();
        assert_eq!(room.camera_x(256), 32);
        walk(&mut room, LEFT, 200);
        assert_eq!(room.camera_x(256), 0);
        walk(&mut room, RIGHT, 320);
        assert_eq!(room.camera_x(256), 64);
        assert_eq!(room.camera_x(WIDTH), 0);
    }

    #[test]
    fn check_in_remembers_the_room_offer_and_overheard_clue() {
        let mut room = Room::new().unwrap();
        assert!(!room.open_interaction());
        for _ in 0..90 {
            room.step(UP);
        }
        assert_eq!(room.interaction(), Some(Interaction::Talk));
        room.step(CONFIRM);
        assert!(!room.step(CONFIRM));
        assert!(room.dialogue().unwrap().line.starts_with("Come in"));
        assert!(!room.submit_text("What time is it?"));
        assert!(room.submit_text("Do you have an empty bed for tonight?"));
        assert_eq!(room.dialogue().unwrap().choices[0].topic, Topic::Rest);
        assert!(room.submit_text("Tell me about the staff."));
        assert!(room.dialogue().unwrap().line.contains("ordinary staff"));

        room.step(BACK);
        for _ in 0..46 {
            room.step(RIGHT);
        }
        assert_eq!(room.interaction(), Some(Interaction::Listen));
        assert!(room.open_interaction());
        assert!(room.dialogue().unwrap().line.contains("air holes"));
        assert!(room.choose(0));
        assert!(room.interaction().is_none());
        for _ in 0..46 {
            room.step(LEFT);
        }
        assert!(room.open_interaction());
        assert!(room.submit_text("What did I hear about the sacks?"));
        assert!(room.dialogue().unwrap().line.contains("grain"));
        assert!(room.submit_text("I'll take the room."));
        assert_eq!(room.dialogue().unwrap().choices[0].topic, Topic::Restart);
        assert!(room.choose(0));
        assert_eq!(room.traveler, SPAWN);
        assert!(room.dialogue().is_none());
    }

    #[test]
    fn can_reach_counter_then_walk_around_it() {
        let mut room = Room::new().unwrap();
        walk(&mut room, UP, 150);
        assert_eq!(room.traveler, Point { x: SPAWN.x, y: 68 });
        walk(&mut room, LEFT, 45);
        walk(&mut room, UP, 33);
        assert_eq!(room.traveler, Point { x: 115, y: 35 });
        walk(&mut room, RIGHT, 45);
        walk(&mut room, DOWN, 30);
        assert_eq!(room.traveler, Point { x: 160, y: 38 });
    }

    #[test]
    fn walls_frog_and_opposite_inputs_stop_movement() {
        let mut room = Room::new().unwrap();
        walk(&mut room, RIGHT, 300);
        assert_eq!(room.traveler.x, 300);
        walk(&mut room, LEFT, 61);
        walk(&mut room, UP, 80);
        assert_eq!(room.traveler, Point { x: 239, y: 123 });
        let stopped = room.traveler;
        walk(&mut room, UP | DOWN, 10);
        assert_eq!(room.traveler, stopped);
        assert_eq!(room.pixels().len(), WIDTH * HEIGHT * 4);
    }
}
