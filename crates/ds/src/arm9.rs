use alloc::boxed::Box;
use core::alloc::{GlobalAlloc, Layout};
use core::ffi::{CStr, c_char};
use core::panic::PanicInfo;
use core::ptr;

use rainy_day_room::{HEIGHT, HudControls, Room, WIDTH};

const SCREEN_WIDTH: usize = 256;
const SCREEN_HEIGHT: usize = 192;
const TOP_PADDING: usize = (SCREEN_HEIGHT - HEIGHT) / 2;
const _: () = assert!(WIDTH >= SCREEN_WIDTH && HEIGHT <= SCREEN_HEIGHT);

unsafe extern "C" {
    fn rainy_ds_alloc(size: usize, alignment: usize) -> *mut u8;
    fn rainy_ds_free(memory: *mut u8);
    fn rainy_ds_halt() -> !;
}

struct DsAllocator;

unsafe impl GlobalAlloc for DsAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { rainy_ds_alloc(layout.size(), layout.align()) }
    }

    unsafe fn dealloc(&self, memory: *mut u8, _layout: Layout) {
        unsafe { rainy_ds_free(memory) }
    }
}

#[global_allocator]
static ALLOCATOR: DsAllocator = DsAllocator;

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    unsafe { rainy_ds_halt() }
}

#[unsafe(no_mangle)]
extern "C" fn rainy_ds_room_new() -> *mut Room {
    Room::new().map_or(ptr::null_mut(), |room| Box::into_raw(Box::new(room)))
}

#[unsafe(no_mangle)]
unsafe extern "C" fn rainy_ds_room_step(room: *mut Room, input: u8) -> bool {
    unsafe { &mut *room }.step(input)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn rainy_ds_room_can_speak(room: *const Room) -> bool {
    unsafe { &*room }
        .dialogue()
        .is_some_and(|view| view.choices.len() > 1)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn rainy_ds_room_submit(room: *mut Room, transcript: *const c_char) -> bool {
    let spoken = unsafe { CStr::from_ptr(transcript) }.to_str().unwrap_or("");
    unsafe { &mut *room }.submit_text(spoken)
}

#[unsafe(no_mangle)]
unsafe extern "C" fn rainy_ds_room_draw(
    room: *mut Room,
    vram: *mut u32,
    voice: u8,
    message: *const c_char,
) {
    // The launcher passes its inactive 256x192 LCD framebuffer, two pixels per word.
    let room = unsafe { &mut *room };
    room.render();
    let microphone = match voice {
        0 => HudControls::Ready,
        1 => HudControls::Warming,
        2 => HudControls::Recording,
        3 => HudControls::Full,
        4 => HudControls::Connecting,
        5 => HudControls::Transcribing,
        6 | 7 => {
            let message = unsafe { CStr::from_ptr(message) }.to_str().unwrap_or("");
            if voice == 6 {
                HudControls::Review(message)
            } else {
                HudControls::Error(message)
            }
        }
        _ => HudControls::Ready,
    };
    room.render_hud(SCREEN_WIDTH, microphone);
    let camera_x = room.camera_x(SCREEN_WIDTH);
    let (prefix, pixels, suffix) = unsafe { room.pixels().align_to::<u32>() };
    // The DS allocator uses malloc alignment and the RGBA frame has four bytes per pixel.
    assert!(prefix.is_empty() && suffix.is_empty());

    for (y, row) in pixels.chunks_exact(WIDTH).enumerate() {
        let visible = &row[camera_x..camera_x + SCREEN_WIDTH];
        let output = (TOP_PADDING + y) * SCREEN_WIDTH / 2;
        for (x, pair) in visible.chunks_exact(2).enumerate() {
            let colors = rgb555(pair[0]) | (rgb555(pair[1]) << 16);
            unsafe { vram.add(output + x).write_volatile(colors) };
        }
    }
}

fn rgb555(rgba: u32) -> u32 {
    let r = (rgba >> 3) & 0x001f;
    let g = (rgba >> 6) & 0x03e0;
    let b = (rgba >> 9) & 0x7c00;
    0x8000 | b | g | r
}
