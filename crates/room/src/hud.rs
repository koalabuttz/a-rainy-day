use crate::art::Frame;
use crate::{Dialogue, Interaction, Topic};

type Color = [u8; 4];

const DARK: Color = [31, 25, 25, 255];
const BORDER: Color = [153, 114, 73, 255];
const TEXT: Color = [249, 233, 197, 255];
const GOLD: Color = [236, 192, 116, 255];
const HIGHLIGHT: Color = [77, 61, 44, 255];

#[derive(Clone, Copy)]
pub enum HudControls<'a> {
    Vita,
    Ready,
    Warming,
    Recording,
    Full,
    Connecting,
    Transcribing,
    Review(&'a str),
    Error(&'a str),
}

pub(super) fn draw(
    frame: &mut Frame,
    left: i32,
    width: i32,
    dialogue: Option<Dialogue>,
    interaction: Option<Interaction>,
    microphone: HudControls<'_>,
) {
    let vita = matches!(microphone, HudControls::Vita);
    let Some(dialogue) = dialogue else {
        if let Some(interaction) = interaction {
            let label = match (vita, interaction) {
                (false, Interaction::Talk) => "A  TALK",
                (false, Interaction::Listen) => "A  LISTEN",
                (true, Interaction::Talk) => "X  TALK",
                (true, Interaction::Listen) => "X  LISTEN",
            };
            let w = label.len() as i32 * 6 + 12;
            let x = left + (width - w) / 2;
            panel(frame, x, 155, w, 17);
            text(frame, x + 6, 160, label, GOLD);
        }
        return;
    };

    let x = left + 6;
    let w = width - 12;
    panel(frame, x, 69, w, 105);
    let columns = (w as usize - 14) / 6;
    if let HudControls::Review(transcript) = microphone {
        text(frame, x + 7, 77, "You said", GOLD);
        wrapped(frame, x + 7, 89, columns, transcript);
        let topic = dialogue
            .match_text(transcript)
            .map_or("Try a room, food, or the staff.", |index| {
                dialogue.choices[index].label
            });
        text(frame, x + 7, 147, topic, GOLD);
        text(frame, x + 7, 164, "A SAY THIS   B DISCARD", GOLD);
        return;
    }
    text(frame, x + 7, 77, dialogue.speaker, GOLD);
    wrapped(frame, x + 7, 89, columns, dialogue.line);

    for (index, choice) in dialogue.choices.iter().enumerate() {
        let y = 122 + index as i32 * 10;
        let color = if index == dialogue.selected {
            frame.rect(x + 5, y - 1, w - 10, 9, HIGHLIGHT);
            text(frame, x + 7, y, ">", GOLD);
            GOLD
        } else {
            TEXT
        };
        text(frame, x + 17, y, choice.label, color);
    }

    let footer = if dialogue.choices[0].topic == Topic::Restart {
        if vita { "X START OVER" } else { "A START OVER" }
    } else if dialogue.choices.len() < 2 {
        if vita {
            "X CONTINUE   O BACK"
        } else {
            "A CONTINUE   B BACK"
        }
    } else {
        match microphone {
            HudControls::Vita => "X SELECT   O BACK",
            HudControls::Ready => "A SELECT   B BACK   HOLD X TO TALK",
            HudControls::Warming => "MIC STARTING. KEEP HOLDING X.",
            HudControls::Recording => "SPEAK NOW. RELEASE X WHEN DONE.",
            HudControls::Full => "CLIP READY. RELEASE X TO SEND.",
            HudControls::Connecting => "CONNECTING TO COMPANION. B CANCEL",
            HudControls::Transcribing => "TRANSCRIBING. B CANCEL",
            HudControls::Review(_) => unreachable!(),
            HudControls::Error(message) => message,
        }
    };
    text(frame, x + 7, 164, footer, GOLD);
}

fn panel(frame: &mut Frame, x: i32, y: i32, w: i32, h: i32) {
    frame.rect(x, y, w, h, BORDER);
    frame.rect(x + 1, y + 1, w - 2, h - 2, DARK);
}

fn wrapped(frame: &mut Frame, x: i32, mut y: i32, columns: usize, line: &str) {
    let mut rest = line;
    while rest.len() > columns {
        let end = rest.as_bytes()[..columns + 1]
            .iter()
            .rposition(|&byte| byte == b' ')
            .unwrap_or(columns);
        text(frame, x, y, &rest[..end], TEXT);
        rest = rest[end..].trim_start();
        y += 10;
    }
    text(frame, x, y, rest, TEXT);
}

fn text(frame: &mut Frame, mut x: i32, y: i32, label: &str, color: Color) {
    for byte in label.bytes() {
        let glyph = match byte.to_ascii_uppercase() {
            ch @ b'A'..=b'Z' => GLYPHS[(ch - b'A') as usize],
            b'.' => [0, 0, 0, 0, 0, 0, 4],
            b',' => [0, 0, 0, 0, 0, 4, 8],
            b'\'' => [4, 4, 8, 0, 0, 0, 0],
            b'!' => [4, 4, 4, 4, 4, 0, 4],
            b'?' => [14, 17, 1, 6, 4, 0, 4],
            b'>' => [0, 8, 4, 2, 4, 8, 0],
            _ => [0; 7],
        };
        for (row, bits) in glyph.into_iter().enumerate() {
            for column in 0..5 {
                if bits & (16 >> column) != 0 {
                    frame.blend_pixel(x + column, y + row as i32, &color);
                }
            }
        }
        x += 6;
    }
}

// Five columns and seven rows. One bit per pixel in each row.
const GLYPHS: [[u8; 7]; 26] = [
    [14, 17, 17, 31, 17, 17, 17], // A
    [30, 17, 17, 30, 17, 17, 30], // B
    [14, 17, 16, 16, 16, 17, 14], // C
    [30, 17, 17, 17, 17, 17, 30], // D
    [31, 16, 16, 30, 16, 16, 31], // E
    [31, 16, 16, 30, 16, 16, 16], // F
    [14, 17, 16, 23, 17, 17, 14], // G
    [17, 17, 17, 31, 17, 17, 17], // H
    [14, 4, 4, 4, 4, 4, 14],      // I
    [7, 2, 2, 2, 2, 18, 12],      // J
    [17, 18, 20, 24, 20, 18, 17], // K
    [16, 16, 16, 16, 16, 16, 31], // L
    [17, 27, 21, 21, 17, 17, 17], // M
    [17, 25, 21, 19, 17, 17, 17], // N
    [14, 17, 17, 17, 17, 17, 14], // O
    [30, 17, 17, 30, 16, 16, 16], // P
    [14, 17, 17, 17, 21, 18, 13], // Q
    [30, 17, 17, 30, 20, 18, 17], // R
    [15, 16, 16, 14, 1, 1, 30],   // S
    [31, 4, 4, 4, 4, 4, 4],       // T
    [17, 17, 17, 17, 17, 17, 14], // U
    [17, 17, 17, 17, 17, 10, 4],  // V
    [17, 17, 17, 21, 21, 21, 10], // W
    [17, 17, 10, 4, 10, 17, 17],  // X
    [17, 17, 10, 4, 4, 4, 4],     // Y
    [31, 1, 2, 4, 8, 16, 31],     // Z
];
