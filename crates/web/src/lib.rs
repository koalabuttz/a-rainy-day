use std::cell::RefCell;
use std::ptr;

use rainy_day_room::{HEIGHT, Room, TICKS_PER_SECOND, WIDTH};

thread_local! {
    static ROOM: RefCell<Option<Room>> = const { RefCell::new(None) };
}

#[unsafe(no_mangle)]
pub extern "C" fn room_init() -> *const u8 {
    let Ok(room) = Room::new() else {
        return ptr::null();
    };
    let pixels = room.pixels().as_ptr();
    ROOM.set(Some(room));
    pixels
}

#[unsafe(no_mangle)]
pub extern "C" fn room_tick(input: u8) {
    ROOM.with_borrow_mut(|room| {
        if let Some(room) = room {
            room.tick(input);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn room_width() -> usize {
    WIDTH
}

#[unsafe(no_mangle)]
pub extern "C" fn room_height() -> usize {
    HEIGHT
}

#[unsafe(no_mangle)]
pub extern "C" fn room_ticks_per_second() -> u32 {
    TICKS_PER_SECOND
}

const TEXT_CAPACITY: usize = 192;

thread_local! {
    static TEXT_INPUT: RefCell<[u8; TEXT_CAPACITY]> = const { RefCell::new([0; TEXT_CAPACITY]) };
}

fn dialogue_text(slot: usize) -> &'static str {
    ROOM.with_borrow(|room| {
        let Some(dialogue) = room.as_ref().and_then(Room::dialogue) else {
            return "";
        };
        match slot {
            0 => dialogue.speaker,
            1 => dialogue.line,
            _ => dialogue
                .choices
                .get(slot - 2)
                .map_or("", |choice| choice.label),
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn room_text(slot: usize) -> *const u8 {
    dialogue_text(slot).as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn room_text_len(slot: usize) -> usize {
    dialogue_text(slot).len()
}

#[unsafe(no_mangle)]
pub extern "C" fn room_interaction() -> u8 {
    ROOM.with_borrow(|room| {
        room.as_ref()
            .and_then(Room::interaction)
            .map_or(0, |target| target as u8)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn room_select(index: usize) -> bool {
    update(|room| room.choose(index))
}

#[unsafe(no_mangle)]
pub extern "C" fn room_open_interaction() {
    update(Room::open_interaction);
}

#[unsafe(no_mangle)]
pub extern "C" fn room_input_buffer() -> *mut u8 {
    TEXT_INPUT.with_borrow_mut(|input| input.as_mut_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn room_input_capacity() -> usize {
    TEXT_CAPACITY
}

#[unsafe(no_mangle)]
pub extern "C" fn room_submit_text(length: usize) -> bool {
    TEXT_INPUT.with_borrow(|input| {
        let Some(text) = input
            .get(..length)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
        else {
            return false;
        };
        update(|room| room.submit_text(text))
    })
}

fn update(action: impl FnOnce(&mut Room) -> bool) -> bool {
    ROOM.with_borrow_mut(|room| {
        if let Some(room) = room.as_mut()
            && action(room)
        {
            room.render();
            return true;
        }
        false
    })
}
