//! The innkeeper's check-in conversation and the optional overheard exchange.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Topic {
    Room = 1,
    Food,
    Staff,
    Rest,
    Leave,
    Continue,
    Restart,
}

#[derive(Clone, Copy)]
pub struct Choice {
    pub topic: Topic,
    pub label: &'static str,
}

pub struct Dialogue {
    pub speaker: &'static str,
    pub line: &'static str,
    pub choices: &'static [Choice],
    pub selected: usize,
}

impl Dialogue {
    pub fn match_text(&self, text: &str) -> Option<usize> {
        // The first scene understands the same topics from spoken or typed phrases.
        for (index, choice) in self.choices.iter().enumerate() {
            let words: &[&str] = match choice.topic {
                Topic::Room => &["room", "bed", "stay", "sleep", "shelter", "rest"],
                Topic::Food => &["food", "soup", "eat", "hungry", "meal", "dinner", "supper"],
                Topic::Staff => &[
                    "staff", "supplies", "supply", "stock", "sacks", "sack", "heard", "hear",
                    "frogs", "special",
                ],
                Topic::Rest => &["room", "bed", "key", "stay", "sleep", "rest", "upstairs"],
                Topic::Leave => &["leave", "later", "goodbye"],
                Topic::Continue => &["continue", "look"],
                Topic::Restart => &["restart", "again"],
            };
            if text
                .split(|ch: char| !ch.is_ascii_alphanumeric())
                .any(|word| words.iter().any(|alias| word.eq_ignore_ascii_case(alias)))
            {
                return Some(index);
            }
        }
        None
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Scene {
    #[default]
    Closed,
    Welcome,
    Room,
    Food,
    Staff,
    Overheard,
    Rest,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub(super) struct CheckIn {
    scene: Scene,
    offered_room: bool,
    pub(super) heard_staff: bool,
    selected: usize,
}

const ROOM: Choice = Choice {
    topic: Topic::Room,
    label: "Ask for a room",
};
const FOOD: Choice = Choice {
    topic: Topic::Food,
    label: "Ask for food",
};
const STAFF: Choice = Choice {
    topic: Topic::Staff,
    label: "Ask about the staff",
};
const SUPPLIES: Choice = Choice {
    topic: Topic::Staff,
    label: "Ask about the supplies",
};
const REST: Choice = Choice {
    topic: Topic::Rest,
    label: "Take the room",
};
const LEAVE: Choice = Choice {
    topic: Topic::Leave,
    label: "Step away",
};

impl CheckIn {
    pub(super) fn view(self) -> Option<Dialogue> {
        let (speaker, line, choices): (&str, &str, &[Choice]) = match self.scene {
            Scene::Closed => return None,
            Scene::Overheard => (
                "Beside the counter",
                "We're out of the house special. Order two sacks this time. The kind with air holes.",
                &[Choice {
                    topic: Topic::Continue,
                    label: "Keep looking",
                }],
            ),
            Scene::Rest => (
                "A room for the night",
                "The bed is warm, and the rain sounds softer from here.",
                &[Choice {
                    topic: Topic::Restart,
                    label: "Start over",
                }],
            ),
            scene => {
                let line = match scene {
                    Scene::Welcome => {
                        "Come in, come in! I had a formal greeting ready. What can I get you?"
                    }
                    Scene::Room => {
                        "A dry room and a warm bed. I'll leave the key right here for you."
                    }
                    Scene::Food => {
                        "There's soup. Make yourself at home, just mind the far end of the counter."
                    }
                    Scene::Staff if self.heard_staff => {
                        "The sacks? For the house special. It needs to breathe. I mean, the grain does."
                    }
                    Scene::Staff => "A very ordinary staff. I mean excellent. An excellent staff.",
                    _ => unreachable!(),
                };
                let choices: &'static [Choice] = match (self.offered_room, self.heard_staff) {
                    (false, false) => &[ROOM, FOOD, STAFF, LEAVE],
                    (false, true) => &[ROOM, FOOD, SUPPLIES, LEAVE],
                    (true, false) => &[REST, FOOD, STAFF, LEAVE],
                    (true, true) => &[REST, FOOD, SUPPLIES, LEAVE],
                };
                ("Innkeeper", line, choices)
            }
        };
        Some(Dialogue {
            speaker,
            line,
            choices,
            selected: self.selected,
        })
    }

    pub(super) fn open(&mut self, overhear: bool) {
        self.scene = if overhear {
            Scene::Overheard
        } else {
            Scene::Welcome
        };
        self.heard_staff |= overhear;
        self.selected = 0;
    }

    pub(super) fn close(&mut self) {
        if self.scene != Scene::Rest {
            self.scene = Scene::Closed;
            self.selected = 0;
        }
    }

    pub(super) fn move_selection(&mut self, count: usize, down: bool) {
        let shift = if down { 1 } else { count - 1 };
        self.selected = (self.selected + shift) % count;
    }

    /// Returns true when the traveler should restart at the entrance.
    pub(super) fn choose(&mut self, topic: Topic) -> bool {
        self.selected = 0;
        self.scene = match topic {
            Topic::Room => {
                self.offered_room = true;
                Scene::Room
            }
            Topic::Food => Scene::Food,
            Topic::Staff => Scene::Staff,
            Topic::Rest => Scene::Rest,
            Topic::Continue | Topic::Leave | Topic::Restart => Scene::Closed,
        };
        if topic == Topic::Restart {
            *self = Self::default();
            return true;
        }
        false
    }
}
