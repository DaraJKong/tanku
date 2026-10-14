//! The reference players: the hands that hold a player's tank when no person does.
//!
//! The trials measure the enemies against them, and a playtest can bring them along as
//! teammates. The three fight the same way and differ in one thing, which is what they do about
//! what flies at them:
//!
//! | | Novice | Regular | Expert |
//! |---|---|---|---|
//! | Sees a bullet coming | never | after 0.30 s | after 0.20 s |
//! | Follows a bullet off a wall | no | no | yes |
//! | Shield | never | when the bullet can't be dodged | the same |
//!
//! With the time that the standard tank takes to get out of the way, the regular player
//! reacts in about the 0.55 s that the specification counts for a player who keeps moving.
//! How they fight is the doing of the pilot of the server.

use serde::{Deserialize, Serialize};

/// One of the reference players.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Reference {
    Novice,
    #[default]
    Regular,
    Expert,
}

/// What a reference player does about what flies at the tank.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Skill {
    /// For how long a bullet must be in view before the player acts on it, in seconds. A
    /// player who has none never dodges.
    pub notice: Option<f32>,
    /// Whether the player sees where a bullet goes after a bounce before it bounces. One who
    /// doesn't takes a bullet that has bounced for a new one.
    pub follows: bool,
    /// Whether the player raises the shield against what can't be dodged.
    pub shields: bool,
}

impl Reference {
    pub const ALL: [Self; 3] = [Self::Novice, Self::Regular, Self::Expert];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Novice => "novice",
            Self::Regular => "regular",
            Self::Expert => "expert",
        }
    }

    pub fn of_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|player| player.name() == name)
    }

    pub const fn skill(self) -> Skill {
        match self {
            Self::Novice => Skill {
                notice: None,
                follows: false,
                shields: false,
            },
            Self::Regular => Skill {
                notice: Some(0.30),
                follows: false,
                shields: true,
            },
            Self::Expert => Skill {
                notice: Some(0.20),
                follows: true,
                shields: true,
            },
        }
    }
}

/// The teammates that the game drives beside a player in a playtest: so many of the last
/// seats of the run, all in the hands of one of the reference players.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Teammates {
    pub count: u8,
    pub player: Reference,
}

impl Teammates {
    /// No teammate: every seat is a person's.
    pub const NONE: Self = Self {
        count: 0,
        player: Reference::Regular,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_players_are_told_by_their_names() {
        for player in Reference::ALL {
            assert_eq!(Reference::of_name(player.name()), Some(player));
        }
        assert_eq!(Reference::of_name("master"), None);
        assert_eq!(Reference::Novice.skill().notice, None);
    }
}
