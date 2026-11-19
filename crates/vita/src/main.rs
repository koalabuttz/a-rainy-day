#[cfg(not(target_os = "vita"))]
fn main() {
    eprintln!("Build this launcher with cargo-vita for PlayStation Vita.");
}

#[cfg(target_os = "vita")]
fn main() -> Result<(), String> {
    use rainy_day_room::{
        BACK, CONFIRM, DOWN, HEIGHT, HudControls, LEFT, RIGHT, Room, TICKS_PER_SECOND, UP, WIDTH,
    };
    use sdl2::{
        controller::Button, event::Event, pixels::Color, pixels::PixelFormatEnum, rect::Rect,
    };

    let mut room = Room::new().map_err(|error| error.to_string())?;
    let sdl = sdl2::init()?;
    let video = sdl.video()?;
    let timer = sdl.timer()?;
    let controllers = sdl.game_controller()?;
    let controller = controllers.open(0).map_err(|error| error.to_string())?;
    let mut events = sdl.event_pump()?;
    sdl2::hint::set("SDL_RENDER_SCALE_QUALITY", "nearest");

    let window = video
        .window("a rainy day", 960, 544)
        .build()
        .map_err(|error| error.to_string())?;
    let mut canvas = window
        .into_canvas()
        .present_vsync()
        .build()
        .map_err(|error| error.to_string())?;
    let textures = canvas.texture_creator();
    let mut texture = textures
        .create_texture_streaming(PixelFormatEnum::RGBA32, WIDTH as u32, HEIGHT as u32)
        .map_err(|error| error.to_string())?;
    let destination = Rect::new(0, 8, (WIDTH * 3) as u32, (HEIGHT * 3) as u32);

    // Count time in multiples of the simulation rate to avoid rounding ticks down.
    let frequency = timer.performance_frequency();
    let mut last_time = timer.performance_counter();
    let mut elapsed = 0;
    let mut dirty = true;
    loop {
        if events
            .poll_iter()
            .any(|event| matches!(event, Event::Quit { .. }))
            || controller.button(Button::Start)
        {
            return Ok(());
        }
        let now = timer.performance_counter();
        elapsed += ((now - last_time) * u64::from(TICKS_PER_SECOND)).min(frequency * 4);
        last_time = now;
        let input = [
            (Button::DPadUp, UP),
            (Button::DPadDown, DOWN),
            (Button::DPadLeft, LEFT),
            (Button::DPadRight, RIGHT),
            (Button::A, CONFIRM),
            (Button::B, BACK),
        ]
        .into_iter()
        .filter(|&(button, _)| controller.button(button))
        .fold(0, |mask, (_, direction)| mask | direction);
        while elapsed >= frequency {
            dirty |= room.step(input);
            elapsed -= frequency;
        }

        if dirty {
            room.render();
            room.render_hud(WIDTH, HudControls::Vita);
            texture
                .update(None, room.pixels(), WIDTH * 4)
                .map_err(|error| error.to_string())?;
            dirty = false;
        }
        canvas.set_draw_color(Color::RGB(0x19, 0x1f, 0x20));
        canvas.clear();
        canvas.copy(&texture, None, destination)?;
        canvas.present();
    }
}
