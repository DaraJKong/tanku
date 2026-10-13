//! The notation of `game.balance`, the file in which the balance is written by hand, and the
//! reader that turns its text into a [`Balance`].
//!
//! The crate reads the file while it is compiled: a file that can't be read stops the build,
//! and the error says on which line. The reader is a plain function, so a program can also
//! read a balance while it runs.
//!
//! The file is text, with one record per line and its words apart by spaces. A line that
//! starts with `#` is a comment. A number is written right before its unit, as in `1.5s`,
//! `8m`, `5m/s`, `45deg` or `150%`, and two numbers with `..` between them are the least and
//! the most of something. `docs/BALANCE.md` describes the records.

use std::{f32::consts::PI, fmt};

use super::{
    Arena, Balance, Budget, Chassis, ClassRule, Enemy, Experience, Group, Gun, Hard, LEVELS, Mine,
    RISES, Repair, Room, Shield, Solo, Split, Stats, Step, Steps, Terrain, Unlock, WEIGHTS,
};
use crate::stats::{Ammo, Class, Stat};

const HEADING: &str = "the file starts with `tanku-balance 1`";
const TWICE: &str = "the file has this line twice";
const UNKNOWN: &str = "a line starts with the name of a record: `stat`, `player`, `gun` and so on";
const ORDER: &str = "the smaller of the two comes first";

/// Why a file can't be read, and on which of its lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fault {
    pub line: u32,
    pub what: &'static str,
    /// What the fault is about, where that has a name: a record, a stat or a class.
    pub about: &'static str,
}

impl Fault {
    /// Stops the build with the name of the file, the line and what is wrong with it.
    pub const fn stop(self, file: &str) -> ! {
        let mut text = [0; 400];
        let mut len = append(&mut text, 0, file.as_bytes());
        len = append(&mut text, len, b", line ");
        let mut digits = 1;
        while self.line / digits >= 10 {
            digits *= 10;
        }
        while digits > 0 {
            len = append(&mut text, len, &[b'0' + (self.line / digits % 10) as u8]);
            digits /= 10;
        }
        len = append(&mut text, len, b": ");
        len = append(&mut text, len, self.what.as_bytes());
        if !self.about.is_empty() {
            len = append(&mut text, len, b" ");
            len = append(&mut text, len, self.about.as_bytes());
        }
        match std::str::from_utf8(text.split_at(len).0) {
            Ok(message) => panic!("{}", message),
            Err(_) => panic!("the balance can't be read"),
        }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.what)?;
        if !self.about.is_empty() {
            write!(formatter, " {}", self.about)?;
        }
        Ok(())
    }
}

/// Writes some bytes into a text from a place on, as far as they fit, and tells where they
/// end.
const fn append(text: &mut [u8], mut len: usize, bytes: &[u8]) -> usize {
    let mut index = 0;
    while index < bytes.len() && len < text.len() {
        text[len] = bytes[index];
        len += 1;
        index += 1;
    }
    len
}

const fn same(text: &[u8], other: &[u8]) -> bool {
    if text.len() != other.len() {
        return false;
    }
    let mut index = 0;
    while index < text.len() {
        if text[index] != other[index] {
            return false;
        }
        index += 1;
    }
    true
}

/// The words of one line, one after the other.
#[derive(Clone, Copy)]
struct Words<'a> {
    text: &'a [u8],
    at: usize,
    end: usize,
}

impl<'a> Words<'a> {
    const fn next(&mut self) -> Option<&'a [u8]> {
        while self.at < self.end && is_space(self.text[self.at]) {
            self.at += 1;
        }
        if self.at == self.end {
            return None;
        }
        let start = self.at;
        while self.at < self.end && !is_space(self.text[self.at]) {
            self.at += 1;
        }
        Some(self.text.split_at(self.at).0.split_at(start).1)
    }

    /// The next word, without taking it.
    const fn ahead(&self) -> Option<&'a [u8]> {
        let mut ahead = *self;
        ahead.next()
    }

    /// Whether the line has no word left.
    const fn is_over(&self) -> bool {
        self.ahead().is_none()
    }
}

const fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r')
}

/// The value of a result, or its error as the answer of the function.
macro_rules! tried {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(wrong) => return Err(wrong),
        }
    };
}

/// A word of a record and the value after it, with what to say when either is missing.
macro_rules! after {
    ($words:expr, $key:literal, $read:ident, $unit:literal, $like:literal) => {{
        const WRONG: &str = concat!("`", $key, "` is expected here, with ", $like);
        tried!(key($words, $key.as_bytes(), WRONG));
        tried!($read($words, $unit.as_bytes(), WRONG))
    }};
}

macro_rules! time {
    ($words:expr, $key:literal) => {
        after!($words, $key, measure, "s", "a time, as in `1.5s`")
    };
}

macro_rules! length {
    ($words:expr, $key:literal) => {
        after!($words, $key, measure, "m", "a length, as in `8m`")
    };
}

macro_rules! meters {
    ($words:expr, $key:literal) => {
        after!(
            $words,
            $key,
            count,
            "m",
            "a whole number of meters, as in `15m`"
        ) as i32
    };
}

macro_rules! reach {
    ($words:expr, $key:literal) => {
        after!(
            $words,
            $key,
            span,
            "m",
            "the nearest and the furthest, as in `8m..14m`"
        )
    };
}

macro_rules! angle {
    ($words:expr, $key:literal) => {
        after!($words, $key, measure, "deg", "an angle, as in `45deg`")
    };
}

macro_rules! share {
    ($words:expr, $key:literal) => {
        after!($words, $key, count, "%", "a share, as in `150%`")
    };
}

macro_rules! amount {
    ($words:expr, $key:literal) => {
        after!($words, $key, count, "", "a whole number")
    };
}

/// A whole number from its digits, if the word is one of seven digits at most.
const fn whole(word: &[u8]) -> Option<u32> {
    if word.is_empty() || word.len() > 7 {
        return None;
    }
    let (mut value, mut index) = (0, 0);
    while index < word.len() {
        if !word[index].is_ascii_digit() {
            return None;
        }
        value = 10 * value + (word[index] - b'0') as u32;
        index += 1;
    }
    Some(value)
}

/// A number that may have decimals, as in `7.5`: its digits over a power of ten, which is
/// the value nearest to what is written.
const fn decimal(word: &[u8]) -> Option<f32> {
    let (mut digits, mut tens, mut seen, mut point) = (0u32, 1u32, 0, false);
    let mut index = 0;
    while index < word.len() {
        let byte = word[index];
        if byte == b'.' {
            // A point stands between two digits, and a number has one.
            if point || index == 0 || index + 1 == word.len() {
                return None;
            }
            point = true;
        } else if byte.is_ascii_digit() && seen < 7 {
            digits = 10 * digits + (byte - b'0') as u32;
            seen += 1;
            if point {
                tens *= 10;
            }
        } else {
            return None;
        }
        index += 1;
    }
    if seen == 0 {
        return None;
    }
    Some(digits as f32 / tens as f32)
}

/// What stands in front of a unit, if the word ends with that unit. A word without a unit is
/// all number.
const fn before<'a>(word: &'a [u8], unit: &[u8]) -> Option<&'a [u8]> {
    if word.len() <= unit.len() {
        return None;
    }
    let (number, end) = word.split_at(word.len() - unit.len());
    if same(end, unit) { Some(number) } else { None }
}

/// The two halves of a word around what parts them.
const fn halves<'a>(word: &'a [u8], between: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
    let mut index = 0;
    while index + between.len() <= word.len() {
        let (first, rest) = word.split_at(index);
        let (middle, second) = rest.split_at(between.len());
        if same(middle, between) {
            return Some((first, second));
        }
        index += 1;
    }
    None
}

/// Which of some names a word is.
const fn named(word: Option<&[u8]>, names: &[&str]) -> Option<usize> {
    let Some(word) = word else {
        return None;
    };
    let mut index = 0;
    while index < names.len() {
        if same(word, names[index].as_bytes()) {
            return Some(index);
        }
        index += 1;
    }
    None
}

const fn stat_named(word: Option<&[u8]>) -> Option<Stat> {
    let Some(word) = word else {
        return None;
    };
    let mut index = 0;
    while index < Stat::ALL.len() {
        if same(word, Stat::ALL[index].name().as_bytes()) {
            return Some(Stat::ALL[index]);
        }
        index += 1;
    }
    None
}

/// The next word, which must be this one.
const fn key(words: &mut Words, name: &[u8], wrong: &'static str) -> Result<(), &'static str> {
    match words.next() {
        Some(word) if same(word, name) => Ok(()),
        _ => Err(wrong),
    }
}

/// The next word as a number in a unit.
const fn measure(words: &mut Words, unit: &[u8], wrong: &'static str) -> Result<f32, &'static str> {
    let Some(word) = words.next() else {
        return Err(wrong);
    };
    let Some(number) = before(word, unit) else {
        return Err(wrong);
    };
    match decimal(number) {
        Some(value) => Ok(value),
        None => Err(wrong),
    }
}

/// The next word as a whole number in a unit.
const fn count(words: &mut Words, unit: &[u8], wrong: &'static str) -> Result<u32, &'static str> {
    let Some(word) = words.next() else {
        return Err(wrong);
    };
    let Some(number) = before(word, unit) else {
        return Err(wrong);
    };
    match whole(number) {
        Some(value) => Ok(value),
        None => Err(wrong),
    }
}

/// The next word as the least and the most of something, in a unit.
const fn span(
    words: &mut Words,
    unit: &[u8],
    wrong: &'static str,
) -> Result<(f32, f32), &'static str> {
    let Some(word) = words.next() else {
        return Err(wrong);
    };
    let Some((least, most)) = halves(word, b"..") else {
        return Err(wrong);
    };
    let (Some(least), Some(most)) = (before(least, unit), before(most, unit)) else {
        return Err(wrong);
    };
    match (decimal(least), decimal(most)) {
        (Some(least), Some(most)) if least <= most => Ok((least, most)),
        (Some(_), Some(_)) => Err(ORDER),
        _ => Err(wrong),
    }
}

/// The same in whole numbers.
const fn range(
    words: &mut Words,
    unit: &[u8],
    wrong: &'static str,
) -> Result<(i32, i32), &'static str> {
    let Some(word) = words.next() else {
        return Err(wrong);
    };
    let Some((least, most)) = halves(word, b"..") else {
        return Err(wrong);
    };
    let (Some(least), Some(most)) = (before(least, unit), before(most, unit)) else {
        return Err(wrong);
    };
    match (whole(least), whole(most)) {
        (Some(least), Some(most)) if least <= most => Ok((least as i32, most as i32)),
        (Some(_), Some(_)) => Err(ORDER),
        _ => Err(wrong),
    }
}

/// An angle in radians. A half turn over what it takes of them, so that the angles that
/// divide a half turn are as exact as they can be.
const fn radians(degrees: f32) -> f32 {
    PI / (180.0 / degrees)
}

/// The records that the file has one of.
macro_rules! singles {
    ($($record:ident $name:literal),* $(,)?) => {
        #[derive(Clone, Copy)]
        enum Single {
            $($record),*
        }

        impl Single {
            const ALL: &[Single] = &[$(Single::$record),*];
            const NAMES: &[&str] = &[$($name),*];
        }
    };
}

singles!(
    Player "player",
    Garage "garage",
    Solo "solo",
    Tank "tank",
    Gun "gun",
    Mine "mine",
    Blast "blast",
    Shield "shield",
    Split "split",
    Mark "mark",
    Repair "repair",
    Budget "budget",
    Experience "experience",
    Team "team",
    Hard "hard",
    Group "group",
    Corridor "corridor",
    Arena "arena",
    Room "room",
    Unlock "unlock",
);

/// What grows by steps, what the terrain says and what the brains say, each on a line of
/// its own.
const STEPS: [&str; 4] = ["tanks", "brain", "ballistics", "muzzle"];
const TERRAIN: [&str; 6] = ["harshest", "width", "cover", "crates", "angled", "spawn"];
const ENEMY: [&str; 3] = ["keep", "mines", "ace"];

/// The records that a reader has met.
struct Found {
    heading: bool,
    singles: [bool; Single::ALL.len()],
    stats: [bool; Stat::COUNT],
    steps: [bool; STEPS.len()],
    classes: [bool; Class::ALL.len()],
    terrain: [bool; TERRAIN.len()],
    enemy: [bool; ENEMY.len()],
    /// The line of the standard tank, for the fault that only shows once the prices are
    /// read.
    player: u32,
}

const NO_STEP: Step = Step {
    first: 0,
    sectors: [0; RISES],
    rises: 0,
};

const NO_CLASS: ClassRule = ClassRule {
    weight: 0,
    favorites: [Stat::Engine; 2],
    favored: 0,
    ricochet: 0,
    ballistics: 0,
};

/// A balance before its file is read.
const EMPTY: Balance = Balance {
    stats: Stats {
        gives: [[0.0; LEVELS]; Stat::COUNT],
        costs: [[0; LEVELS]; Stat::COUNT],
    },
    standard: [0; Stat::COUNT],
    garage: 0,
    solo: Solo {
        lives: 0,
        respawn: 0.0,
    },
    tank: Chassis {
        turret: 0.0,
        speed_up: 0.0,
        pivot: 0.0,
        hold_fire: 0.0,
    },
    gun: Gun {
        cooldown: 0.0,
        bullet: 0.0,
    },
    mine: Mine {
        arm: 0.0,
        fuse: 0.0,
        warn: 0.0,
        trigger: 0.0,
        every: 0.0,
    },
    blast: 0.0,
    shield: Shield {
        lasts: 0.0,
        radius: 0.0,
    },
    split: Split {
        after: 0.0,
        angle: 0.0,
    },
    mark: 0.0,
    repair: Repair {
        within: 0.0,
        takes: 0.0,
        safe: 0.0,
    },
    budget: Budget {
        first: 0,
        cap: 0,
        every: 1,
    },
    experience: Experience {
        every: 1,
        reaction: (0.0, 0.0),
        aim: (0.0, 0.0),
    },
    steps: Steps {
        tanks: NO_STEP,
        brain: NO_STEP,
        ballistics: NO_STEP,
        muzzle: NO_STEP,
        others: [None; Stat::COUNT],
    },
    team: 0,
    hard: Hard {
        budget: 0,
        ahead: 0,
    },
    group: Group {
        mean: (0, 0),
        least: 0,
        leader: 0,
        weights: [0; WEIGHTS],
        drawn: 0,
        favorite: 0,
    },
    classes: [NO_CLASS; Class::ALL.len()],
    terrain: Terrain {
        harshest: 2,
        width: (0, 0),
        spread: 0,
        cover: (0, 0),
        crates: ((0, 0), (0, 0)),
        angled: (0, 0),
        spawn: (0, 0),
    },
    corridor: (0, 0),
    arena: Arena {
        every: (0, 0),
        tanks: 0,
        waves: 0,
        wait: 0.0,
        mark: 0.0,
        clear: 0.0,
        life: 0,
        reach: 0.0,
    },
    room: Room {
        at_once: 0,
        lull: 0.0,
        life_every: 0,
        rise: 0.0,
        apart: 0.0,
        cover: 0,
        width: (0, 0),
        depth: (0, 0),
    },
    enemy: Enemy {
        keep: (0.0, 0.0),
        hold: 0.0,
        foresight: 0.0,
        last_moment: 0.0,
        room: 0.0,
        parry: 0.0,
        mine_reflex: 0.0,
        mine_range: 0.0,
        mine_apart: 0.0,
        charge: 0.0,
        bait: (0.0, 0.0),
        press: (0.0, 0.0),
        until: 0,
        retreat: (0.0, 0.0),
        crossfire: 0.0,
    },
    unlock: Unlock {
        arena: 0,
        seed: 0,
        free: 0,
        hard: 0,
        impossible: 0,
    },
};

/// Reads a balance from the text of its file.
pub const fn read(text: &str) -> Result<Balance, Fault> {
    let text = text.as_bytes();
    let mut balance = EMPTY;
    let mut found = Found {
        heading: false,
        singles: [false; Single::ALL.len()],
        stats: [false; Stat::COUNT],
        steps: [false; STEPS.len()],
        classes: [false; Class::ALL.len()],
        terrain: [false; TERRAIN.len()],
        enemy: [false; ENEMY.len()],
        player: 0,
    };
    let (mut at, mut line) = (0, 0);
    while at < text.len() {
        let mut end = at;
        while end < text.len() && text[end] != b'\n' {
            end += 1;
        }
        let mut words = Words { text, at, end };
        at = end + 1;
        line += 1;
        let Some(kind) = words.next() else {
            continue;
        };
        if kind[0] == b'#' {
            continue;
        }
        if let Err(what) = record(&mut balance, &mut found, kind, &mut words, line) {
            let about = "";
            return Err(Fault { line, what, about });
        }
        if !words.is_over() {
            let (what, about) = ("this line has a word too many", "");
            return Err(Fault { line, what, about });
        }
    }
    if let Some((what, about)) = missing(&found) {
        return Err(Fault { line, what, about });
    }
    if let Err(what) = affordable(&balance) {
        let (line, about) = (found.player, "");
        return Err(Fault { line, what, about });
    }
    // A tank draws its class by the weights of the classes.
    let (mut weight, mut index) = (0, 0);
    while index < balance.classes.len() {
        weight += balance.classes[index].weight;
        index += 1;
    }
    if weight == 0 {
        let (what, about) = ("one class at least has a weight that isn't 0", "");
        return Err(Fault { line, what, about });
    }
    Ok(balance)
}

/// What tells the words of one file from those of another. Comments and the room between
/// two words are no part of it.
pub const fn fingerprint(text: &str) -> u64 {
    const PRIME: u64 = 0x0000_0100_0000_01B3;
    let text = text.as_bytes();
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
    let mut at = 0;
    while at < text.len() {
        let mut end = at;
        while end < text.len() && text[end] != b'\n' {
            end += 1;
        }
        let mut words = Words { text, at, end };
        at = end + 1;
        let mut first = true;
        while let Some(word) = words.next() {
            if first && word[0] == b'#' {
                break;
            }
            first = false;
            let mut index = 0;
            while index < word.len() {
                hash = (hash ^ word[index] as u64).wrapping_mul(PRIME);
                index += 1;
            }
            hash = (hash ^ b' ' as u64).wrapping_mul(PRIME);
        }
        if !first {
            hash = (hash ^ b'\n' as u64).wrapping_mul(PRIME);
        }
    }
    hash
}

/// The first record that a file lacks, and what it is of.
const fn missing(found: &Found) -> Option<(&'static str, &'static str)> {
    if !found.heading {
        return Some((HEADING, ""));
    }
    let mut index = 0;
    while index < Stat::COUNT {
        if !found.stats[index] {
            return Some(("the file has no `stat` line for", Stat::ALL[index].name()));
        }
        index += 1;
    }
    index = 0;
    while index < found.singles.len() {
        if !found.singles[index] {
            return Some(("the file has no line for", Single::NAMES[index]));
        }
        index += 1;
    }
    index = 0;
    while index < STEPS.len() {
        if !found.steps[index] {
            return Some(("the file has no `step` line for", STEPS[index]));
        }
        index += 1;
    }
    index = 0;
    while index < Class::ALL.len() {
        if !found.classes[index] {
            return Some(("the file has no `class` line for", Class::ALL[index].name()));
        }
        index += 1;
    }
    index = 0;
    while index < TERRAIN.len() {
        if !found.terrain[index] {
            return Some(("the file has no `terrain` line for", TERRAIN[index]));
        }
        index += 1;
    }
    index = 0;
    while index < ENEMY.len() {
        if !found.enemy[index] {
            return Some(("the file has no `enemy` line for", ENEMY[index]));
        }
        index += 1;
    }
    None
}

/// Whether the points of the garage pay for the standard tank, which is one that a player
/// may build.
const fn affordable(balance: &Balance) -> Result<(), &'static str> {
    let (mut cost, mut index) = (0, 0);
    while index < Stat::HARDWARE.len() {
        let stat = Stat::HARDWARE[index] as usize;
        cost += balance.stats.costs[stat][balance.standard[stat] as usize] as u32;
        index += 1;
    }
    if cost > balance.garage as u32 {
        return Err("the standard tank costs more points than the garage has");
    }
    Ok(())
}

const fn record(
    balance: &mut Balance,
    found: &mut Found,
    kind: &[u8],
    words: &mut Words,
    line: u32,
) -> Result<(), &'static str> {
    if same(kind, b"tanku-balance") {
        if found.heading {
            return Err(TWICE);
        }
        if tried!(count(words, b"", HEADING)) != 1 {
            return Err("this reader knows version 1 of the file, and no other");
        }
        found.heading = true;
        return Ok(());
    }
    if !found.heading {
        return Err(HEADING);
    }
    if same(kind, b"stat") {
        return stat(balance, found, words);
    }
    if same(kind, b"step") {
        return step(balance, found, words);
    }
    if same(kind, b"class") {
        return class(balance, found, words);
    }
    if same(kind, b"terrain") {
        return terrain(&mut balance.terrain, found, words);
    }
    if same(kind, b"enemy") {
        return enemy(&mut balance.enemy, found, words);
    }
    let Some(index) = named(Some(kind), Single::NAMES) else {
        return Err(UNKNOWN);
    };
    if found.singles[index] {
        return Err(TWICE);
    }
    found.singles[index] = true;
    match Single::ALL[index] {
        Single::Player => {
            found.player = line;
            balance.standard = tried!(player(words));
        }
        Single::Garage => {
            const WRONG: &str = "the points of the garage are a whole number, up to 9999";
            let points = tried!(count(words, b"", WRONG));
            if points > 9999 {
                return Err(WRONG);
            }
            balance.garage = points as u16;
        }
        Single::Solo => {
            let (lives, respawn) = (amount!(words, "lives"), time!(words, "respawn"));
            if lives == 0 {
                return Err("a single player has one life at least");
            }
            balance.solo = Solo { lives, respawn };
        }
        Single::Tank => {
            balance.tank = Chassis {
                turret: radians(after!(
                    words,
                    "turret",
                    measure,
                    "deg/s",
                    "a turn, as in `720deg/s`"
                )),
                speed_up: time!(words, "speed-up"),
                pivot: radians(angle!(words, "pivot")),
                hold_fire: time!(words, "hold-fire"),
            };
        }
        Single::Gun => {
            let gun = Gun {
                cooldown: time!(words, "cooldown"),
                bullet: length!(words, "bullet"),
            };
            if gun.bullet <= 0.0 {
                return Err("a bullet has a size");
            }
            balance.gun = gun;
        }
        Single::Mine => {
            let mine = Mine {
                arm: time!(words, "arm"),
                fuse: time!(words, "fuse"),
                warn: time!(words, "warn"),
                trigger: length!(words, "trigger"),
                every: time!(words, "every"),
            };
            if mine.warn > mine.fuse {
                return Err("a mine warns for the last moments of its fuse, and no longer");
            }
            balance.mine = mine;
        }
        Single::Blast => {
            balance.blast = tried!(measure(words, b"m", "a blast reaches a length, as in `3m`"));
        }
        Single::Shield => {
            balance.shield = Shield {
                lasts: time!(words, "lasts"),
                radius: length!(words, "radius"),
            };
        }
        Single::Split => {
            balance.split = Split {
                after: time!(words, "after"),
                angle: radians(angle!(words, "angle")),
            };
        }
        Single::Mark => {
            const WRONG: &str = "what a bullet is about to do is marked for a time, as in `1.5s`";
            balance.mark = tried!(measure(words, b"s", WRONG));
        }
        Single::Repair => {
            balance.repair = Repair {
                within: length!(words, "within"),
                takes: time!(words, "takes"),
                safe: time!(words, "safe"),
            };
        }
        Single::Budget => balance.budget = tried!(budget(words)),
        Single::Experience => balance.experience = tried!(experience(words)),
        Single::Team => {
            const WRONG: &str =
                "each player after the first adds a share of the tanks, as in `+50%`";
            balance.team = tried!(more(words, b"%", WRONG));
        }
        Single::Hard => {
            const WRONG: &str = "hard mode is written as in `budget +30 ahead 20`";
            tried!(key(words, b"budget", WRONG));
            let budget = tried!(more(words, b"", WRONG));
            balance.hard = Hard {
                budget,
                ahead: amount!(words, "ahead"),
            };
        }
        Single::Group => balance.group = tried!(group(words)),
        Single::Corridor => {
            const WRONG: &str = "a corridor is as short and as long as in `40m..80m`";
            let length = tried!(range(words, b"m", WRONG));
            if length.0 < 1 {
                return Err("a sector has a length");
            }
            balance.corridor = length;
        }
        Single::Arena => balance.arena = tried!(arena(words)),
        Single::Room => balance.room = tried!(room(words)),
        Single::Unlock => {
            balance.unlock = Unlock {
                arena: amount!(words, "arena"),
                seed: amount!(words, "seed"),
                free: amount!(words, "free"),
                hard: amount!(words, "hard"),
                impossible: amount!(words, "impossible"),
            };
        }
    }
    Ok(())
}

/// The next word as a number that is added: it has a `+` in front.
const fn more(words: &mut Words, unit: &[u8], wrong: &'static str) -> Result<u32, &'static str> {
    let Some(word) = words.next() else {
        return Err(wrong);
    };
    let Some((nothing, number)) = halves(word, b"+") else {
        return Err(wrong);
    };
    let Some(number) = before(number, unit) else {
        return Err(wrong);
    };
    match whole(number) {
        Some(value) if nothing.is_empty() => Ok(value),
        _ => Err(wrong),
    }
}

/// What the levels of a stat give, and what they cost.
const fn stat(
    balance: &mut Balance,
    found: &mut Found,
    words: &mut Words,
) -> Result<(), &'static str> {
    const WHICH: &str = "`stat` is followed by a stat: engine, tracks, turret, muzzle, magazine, \
                         loader, ricochet, mines, shield, piercing, seeking, splitting, \
                         exploding, brain or ballistics";
    const GIVES: &str = "this stat says what its levels give first: `speed` for an engine and a \
                         muzzle, `turn` for tracks and a turret, `reload` for a loader and \
                         `cooldown` for a shield";
    const LEVEL: &str = "a stat has a number for each of its levels, level 0 included";
    const COSTS: &str = "a piece of hardware says what its levels cost after `cost`, and \
                         software after `share`";
    let Some(stat) = stat_named(words.next()) else {
        return Err(WHICH);
    };
    let index = stat as usize;
    if found.stats[index] {
        return Err(TWICE);
    }
    found.stats[index] = true;
    let levels = stat.levels();
    let given: &[u8] = match stat {
        Stat::Engine | Stat::Muzzle => b"speed",
        Stat::Tracks | Stat::Turret => b"turn",
        Stat::Loader => b"reload",
        Stat::Shield => b"cooldown",
        _ => b"",
    };
    let mut level = 0;
    if !given.is_empty() {
        tried!(key(words, given, GIVES));
        while level < levels {
            let Some(word) = words.next() else {
                return Err(LEVEL);
            };
            // A tank without a shield has no cooldown, and one without an engine no speed.
            let value = if matches!(stat, Stat::Shield) && level == 0 {
                if !same(word, b"-") {
                    return Err("level 0 of the shield is no shield: its cooldown is written `-`");
                }
                0.0
            } else {
                match decimal(word) {
                    Some(value) => value,
                    None => return Err(LEVEL),
                }
            };
            let stands = matches!(stat, Stat::Engine) && level == 0;
            if stands && value != 0.0 {
                return Err("an engine of level 0 doesn't drive: its speed is 0");
            }
            if !stands && !matches!(stat, Stat::Shield if level == 0) && value <= 0.0 {
                return Err("what a level gives is more than 0");
            }
            balance.stats.gives[index][level] = value;
            level += 1;
        }
    }
    let software = stat.is_software();
    let priced: &[u8] = if software { b"share" } else { b"cost" };
    tried!(key(words, priced, COSTS));
    let (mut last, most) = (0, if software { 100 } else { 255 });
    level = 0;
    while level < levels {
        let cost = match words.next() {
            Some(word) => whole(word),
            None => None,
        };
        let Some(cost) = cost else {
            return Err(LEVEL);
        };
        if level == 0 && cost != 0 {
            return Err("level 0 is free: it costs 0");
        }
        if cost < last {
            return Err("a level costs no less than the one before it");
        }
        if cost > most {
            return Err("a level costs 255 points at most, and a tier 100% of a budget");
        }
        balance.stats.costs[index][level] = cost as u8;
        last = cost;
        level += 1;
    }
    Ok(())
}

/// The standard tank: its stats with their levels.
const fn player(words: &mut Words) -> Result<[u8; Stat::COUNT], &'static str> {
    const WRONG: &str = "the standard tank is written as its stats with their levels, as in \
                         `engine=4 tracks=5`";
    let mut levels = [0; Stat::COUNT];
    let mut written = [false; Stat::COUNT];
    while let Some(word) = words.next() {
        let Some((name, level)) = halves(word, b"=") else {
            return Err(WRONG);
        };
        let (Some(stat), Some(level)) = (stat_named(Some(name)), whole(level)) else {
            return Err(WRONG);
        };
        if written[stat as usize] {
            return Err("a stat of the standard tank is written twice");
        }
        written[stat as usize] = true;
        let mut garage = false;
        let mut index = 0;
        while index < Stat::GARAGE.len() {
            garage |= Stat::GARAGE[index] as usize == stat as usize;
            index += 1;
        }
        if !garage {
            return Err("the tank of a player has no turret, no brain and no ballistics");
        }
        if level as usize >= stat.levels() {
            return Err("a level of the standard tank is higher than its stat goes");
        }
        levels[stat as usize] = level as u8;
    }
    if levels[Stat::Engine as usize] == 0 {
        return Err("the standard tank needs an engine, to reach the barriers");
    }
    let (mut types, mut index) = (0, 0);
    while index < Ammo::STATS.len() {
        types += (levels[Ammo::STATS[index] as usize] > 0) as u32;
        index += 1;
    }
    if types > 1 {
        return Err("a tank fires one type of bullet");
    }
    Ok(levels)
}

/// Two numbers with `to` between them: where something starts and where it ends.
macro_rules! slope {
    ($words:expr, $read:ident, $unit:literal, $wrong:expr) => {{
        let first = tried!($read($words, $unit.as_bytes(), $wrong));
        tried!(key($words, b"to", $wrong));
        (first, tried!($read($words, $unit.as_bytes(), $wrong)))
    }};
}

const fn budget(words: &mut Words) -> Result<Budget, &'static str> {
    const WRONG: &str = "a budget is written as in `10 to 100 every 15`";
    let (first, cap) = slope!(words, count, "", WRONG);
    tried!(key(words, b"every", WRONG));
    let every = tried!(count(words, b"", WRONG));
    if first > cap || cap > 9999 {
        return Err("a budget goes up from its first value to its cap, which is 9999 at most");
    }
    if every == 0 {
        return Err("a budget takes a sector at least to get closer to its cap");
    }
    Ok(Budget { first, cap, every })
}

const fn experience(words: &mut Words) -> Result<Experience, &'static str> {
    const WRONG: &str = "experience is written as in `every 20 reaction 0.5s to 0.12s aim 8deg \
                         to 1deg`";
    tried!(key(words, b"every", WRONG));
    let every = tried!(count(words, b"", WRONG));
    if every == 0 {
        return Err("experience takes a sector at least to grow");
    }
    tried!(key(words, b"reaction", WRONG));
    let reaction = slope!(words, measure, "s", WRONG);
    tried!(key(words, b"aim", WRONG));
    let aim = slope!(words, measure, "deg", WRONG);
    Ok(Experience {
        every,
        reaction,
        aim,
    })
}

/// Something that grows by steps: its value in sector 1, then the sectors where it goes up.
const fn step(
    balance: &mut Balance,
    found: &mut Found,
    words: &mut Words,
) -> Result<(), &'static str> {
    const WHICH: &str = "`step` is followed by what grows: tanks, or a stat";
    const WRONG: &str = "a step is its value in sector 1, then the sectors where it goes up by \
                         one, as in `1  3 6 9`";
    // The group and the three caps that every balance has, or a stat that this one holds
    // back.
    let word = words.next();
    let (index, other) = match (named(word, &STEPS), stat_named(word)) {
        (Some(index), _) => (index, None),
        (None, Some(stat)) => (STEPS.len(), Some(stat)),
        (None, None) => return Err(WHICH),
    };
    match other {
        Some(stat) if balance.steps.others[stat as usize].is_some() => return Err(TWICE),
        None if found.steps[index] => return Err(TWICE),
        Some(_) => {}
        None => found.steps[index] = true,
    }
    let mut step = Step {
        first: tried!(count(words, b"", WRONG)),
        ..NO_STEP
    };
    let mut last = 1;
    while let Some(word) = words.next() {
        let Some(sector) = whole(word) else {
            return Err(WRONG);
        };
        if sector <= last {
            return Err("the sectors of a step come in rising order, after sector 1");
        }
        if step.rises as usize == RISES {
            return Err("a step goes up eight times at most");
        }
        step.sectors[step.rises as usize] = sector;
        step.rises += 1;
        last = sector;
    }
    // A cap ends no higher than its stat goes, and a group has a tank.
    let highest = match (index, other) {
        (_, Some(stat)) => stat.levels() - 1,
        (1, _) => Stat::Brain.levels() - 1,
        (2, _) => Stat::Ballistics.levels() - 1,
        (3, _) => Stat::Muzzle.levels() - 1,
        _ => usize::MAX,
    };
    if step.last() as usize > highest {
        return Err("this step ends above the highest level of its stat");
    }
    match index {
        0 if step.first == 0 => return Err("a group has one tank at least"),
        0 => balance.steps.tanks = step,
        1 => balance.steps.brain = step,
        2 => balance.steps.ballistics = step,
        3 => balance.steps.muzzle = step,
        _ => {
            if let Some(stat) = other {
                balance.steps.others[stat as usize] = Some(step);
            }
        }
    }
    Ok(())
}

const fn group(words: &mut Words) -> Result<Group, &'static str> {
    const WEIGHT: &str = "the weights of a group are whole numbers, eight of them at most, and \
                          `favorite` comes after them";
    let mean = after!(
        words,
        "mean",
        range,
        "%",
        "the least and the most, as in `70%..90%`"
    );
    let (least, leader) = (share!(words, "least"), share!(words, "leader"));
    tried!(key(words, b"weights", WEIGHT));
    let mut group = Group {
        mean,
        least,
        leader,
        weights: [0; WEIGHTS],
        drawn: 0,
        favorite: 0,
    };
    let mut sum = 0;
    loop {
        let weight = match words.ahead() {
            Some(word) => whole(word),
            None => None,
        };
        let Some(weight) = weight else {
            break;
        };
        words.next();
        if group.drawn as usize == WEIGHTS {
            return Err(WEIGHT);
        }
        group.weights[group.drawn as usize] = weight;
        group.drawn += 1;
        sum += weight;
    }
    if sum == 0 {
        return Err("a group needs a weight that isn't 0");
    }
    group.favorite = amount!(words, "favorite");
    if group.favorite == 0 {
        return Err("a favorite stat weighs 1 at least, as the others do");
    }
    if (least as i32) > mean.0 {
        return Err("the least that a tank has is no more than what a tank has on average");
    }
    if leader < 100 {
        return Err("the leader of a group can have the budget of its sector at least: 100%");
    }
    Ok(group)
}

const fn class_named(word: Option<&[u8]>) -> Option<Class> {
    let Some(word) = word else {
        return None;
    };
    let mut index = 0;
    while index < Class::ALL.len() {
        if same(word, Class::ALL[index].name().as_bytes()) {
            return Some(Class::ALL[index]);
        }
        index += 1;
    }
    None
}

/// The most that a class has of a stat, written as in `ricochet<=3`.
const fn cap(words: &mut Words, stat: Stat) -> Result<u8, &'static str> {
    const WRONG: &str = "a class ends with the most that it has of ricochet and of ballistics, \
                         as in `ricochet<=1 ballistics<=2`";
    let Some(word) = words.next() else {
        return Err(WRONG);
    };
    let Some((name, most)) = halves(word, b"<=") else {
        return Err(WRONG);
    };
    let Some(most) = whole(most) else {
        return Err(WRONG);
    };
    if !same(name, stat.name().as_bytes()) {
        return Err(WRONG);
    }
    if most as usize >= stat.levels() {
        return Err("a class has no more of a stat than the stat goes");
    }
    Ok(most as u8)
}

/// A class: its weight in the draw, the stats that it favors and the most that it has of
/// the stats that are kept for some.
const fn class(
    balance: &mut Balance,
    found: &mut Found,
    words: &mut Words,
) -> Result<(), &'static str> {
    const WHICH: &str = "`class` is followed by a class: all-rounder, sniper, scout, tactician, \
                         trickster, lancer, sapper, gunner, seeker, flanker or grenadier";
    let Some(class) = class_named(words.next()) else {
        return Err(WHICH);
    };
    let index = class as usize;
    if found.classes[index] {
        return Err(TWICE);
    }
    found.classes[index] = true;
    const WEIGHT: &str = "a class has its weight in the draw after its name, a whole number";
    let mut rule = ClassRule {
        weight: tried!(count(words, b"", WEIGHT)),
        ..NO_CLASS
    };
    // The stats that it favors come as plain names. Its caps have a `<=` in them.
    while let Some(stat) = stat_named(words.ahead()) {
        words.next();
        if rule.favors(stat) {
            return Err("a class favors a stat once");
        }
        if rule.favored as usize == rule.favorites.len() {
            return Err("a class favors two stats at most");
        }
        rule.favorites[rule.favored as usize] = stat;
        rule.favored += 1;
    }
    rule.ricochet = tried!(cap(words, Stat::Ricochet));
    rule.ballistics = tried!(cap(words, Stat::Ballistics));
    let (mut types, mut ammo) = (0, 0);
    while ammo < Ammo::STATS.len() {
        types += rule.favors(Ammo::STATS[ammo]) as u32;
        ammo += 1;
    }
    if types > 1 {
        return Err("a class fires one type of bullet");
    }
    if rule.favors(Stat::Seeking) && rule.ricochet == 0 {
        return Err("a bullet seeks at its bounces: a class that favors seeking has a bounce");
    }
    // A bank shot off a wall needs a bounce, and one off two walls needs two.
    if rule.ballistics > rule.ricochet + 1 {
        return Err("ballistics 2 needs a bounce, and ballistics 3 needs two");
    }
    balance.classes[index] = rule;
    Ok(())
}

const fn terrain(
    terrain: &mut Terrain,
    found: &mut Found,
    words: &mut Words,
) -> Result<(), &'static str> {
    const WHICH: &str = "`terrain` is followed by what it says: harshest, width, cover, crates, \
                         angled or spawn";
    let Some(index) = named(words.next(), &TERRAIN) else {
        return Err(WHICH);
    };
    if found.terrain[index] {
        return Err(TWICE);
    }
    found.terrain[index] = true;
    match index {
        0 => {
            const WRONG: &str = "the harshest sector is a whole number, 2 at least";
            terrain.harshest = tried!(count(words, b"", WRONG));
            if terrain.harshest < 2 {
                return Err(WRONG);
            }
        }
        1 => {
            const WRONG: &str = "a width is written as in `14m to 8m spread 6m`";
            let (first, last) = slope!(words, count, "m", WRONG);
            terrain.width = (first as i32, last as i32);
            terrain.spread = meters!(words, "spread");
            if first == 0 || last == 0 {
                return Err("a corridor has a width");
            }
        }
        2 => {
            const WRONG: &str = "the cover of a sector is written as in `9 to 3`";
            let (first, last) = slope!(words, count, "", WRONG);
            terrain.cover = (first as i32, last as i32);
        }
        3 => {
            const WRONG: &str = "the crates of a sector are written as in `0..1 to 3..6`";
            terrain.crates = slope!(words, range, "", WRONG);
        }
        4 => {
            const WRONG: &str = "the chance of an angled wall is written as in `10% to 70%`";
            let (first, last) = slope!(words, count, "%", WRONG);
            if first > 100 || last > 100 {
                return Err("a chance is 100% at most");
            }
            terrain.angled = (first as i32, last as i32);
        }
        _ => terrain.spawn = (meters!(words, "entry"), meters!(words, "barrier")),
    }
    Ok(())
}

const fn arena(words: &mut Words) -> Result<Arena, &'static str> {
    let arena = Arena {
        every: after!(
            words,
            "every",
            range,
            "",
            "the fewest and the most sectors, as in `5..8`"
        ),
        tanks: share!(words, "tanks"),
        waves: amount!(words, "waves"),
        wait: time!(words, "wait"),
        mark: time!(words, "mark"),
        clear: length!(words, "clear"),
        life: meters!(words, "life"),
        reach: length!(words, "reach"),
    };
    if arena.every.0 < 1 {
        return Err("an arena comes after a sector at least");
    }
    if arena.waves == 0 {
        return Err("an arena has a wave at least");
    }
    Ok(arena)
}

const fn room(words: &mut Words) -> Result<Room, &'static str> {
    const SIZE: &str = "the room is as wide and as deep as in `size 36m..42m 22m..24m`, in even \
                        numbers of meters";
    let mut room = Room {
        at_once: share!(words, "at-once"),
        lull: time!(words, "lull"),
        life_every: amount!(words, "life-every"),
        rise: time!(words, "rise"),
        apart: length!(words, "apart"),
        cover: share!(words, "cover"),
        width: (0, 0),
        depth: (0, 0),
    };
    tried!(key(words, b"size", SIZE));
    room.width = tried!(range(words, b"m", SIZE));
    room.depth = tried!(range(words, b"m", SIZE));
    // Even sizes keep the middle of the room on the lattice.
    let sizes = [room.width.0, room.width.1, room.depth.0, room.depth.1];
    let mut index = 0;
    while index < sizes.len() {
        if sizes[index] % 2 != 0 || sizes[index] == 0 {
            return Err(SIZE);
        }
        index += 1;
    }
    if room.at_once == 0 || room.at_once > 100 {
        return Err("the room holds some of a group at once, and all of it at most");
    }
    if room.life_every == 0 {
        return Err("a life comes back after a group at least");
    }
    Ok(room)
}

const fn enemy(
    enemy: &mut Enemy,
    found: &mut Found,
    words: &mut Words,
) -> Result<(), &'static str> {
    const WHICH: &str = "`enemy` is followed by what it says: keep, mines or ace";
    let Some(index) = named(words.next(), &ENEMY) else {
        return Err(WHICH);
    };
    if found.enemy[index] {
        return Err(TWICE);
    }
    found.enemy[index] = true;
    match index {
        0 => {
            const WRONG: &str = "the distances that a brain keeps are written as in `8m..14m`";
            enemy.keep = tried!(span(words, b"m", WRONG));
            enemy.hold = after!(words, "hold", measure, "", "a number, as in `1.25`");
            enemy.foresight = time!(words, "foresight");
            enemy.last_moment = time!(words, "last-moment");
            enemy.room = length!(words, "room");
            enemy.parry = length!(words, "parry");
        }
        1 => {
            enemy.mine_reflex = length!(words, "reflex");
            enemy.mine_range = length!(words, "range");
            enemy.mine_apart = length!(words, "apart");
            enemy.charge = after!(words, "charge", measure, "m/s", "a speed, as in `3m/s`");
        }
        _ => {
            enemy.bait = reach!(words, "bait");
            enemy.press = reach!(words, "press");
            let until = amount!(words, "until");
            if until > 255 {
                return Err("an ace presses a player until they have a few rounds again");
            }
            enemy.until = until as u8;
            enemy.retreat = reach!(words, "retreat");
            enemy.crossfire = radians(angle!(words, "crossfire"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The file of the tests: its lines are those that these tests write otherwise, whatever
    /// the balance of the game becomes.
    const FILE: &str = include_str!("../../test.balance");

    /// What the reader says of the file of the game with one piece of it written otherwise.
    fn fault(written: &str, otherwise: &str) -> Fault {
        assert_eq!(FILE.matches(written).count(), 1, "{written}");
        read(&FILE.replace(written, otherwise)).expect_err(otherwise)
    }

    /// Whether a line starts with these words.
    fn starts(text: &str, words: &str) -> bool {
        let wanted = words.split_whitespace();
        text.split_whitespace()
            .take(wanted.clone().count())
            .eq(wanted)
    }

    /// The line of a text that starts with these words.
    fn line_of(text: &str, words: &str) -> u32 {
        let found = text.lines().position(|line| starts(line, words));
        found.expect(words) as u32 + 1
    }

    #[test]
    fn a_file_is_read_into_its_numbers() {
        let balance = read(FILE).unwrap();
        // A few of its numbers, of each kind that the file writes.
        assert_eq!(balance.stats.gives[Stat::Muzzle as usize][1], 7.5);
        assert_eq!(balance.stats.gives[Stat::Shield as usize][..2], [0.0, 10.0]);
        assert_eq!(
            balance.stats.costs[Stat::Loader as usize],
            [0, 1, 2, 4, 6, 9, 12, 16]
        );
        assert_eq!(
            balance.stats.costs[Stat::Brain as usize][..5],
            [0, 8, 16, 24, 32]
        );
        assert_eq!(balance.standard[Stat::Loader as usize], 6);
        assert_eq!((balance.garage, balance.solo.lives), (59, 3));
        assert_eq!(balance.mine.warn, 0.6);
        assert_eq!(balance.steps.tanks.sectors(), [3, 6, 9, 12, 15]);
        assert_eq!(
            (balance.steps.muzzle.first, balance.steps.muzzle.last()),
            (2, 7)
        );
        assert_eq!(balance.group.weights(), [1, 1, 2, 2, 3, 8]);
        assert_eq!(balance.group.mean, (70, 90));
        assert_eq!(balance.terrain.crates, ((0, 1), (3, 6)));
        assert_eq!(balance.room.width, (36, 42));
        assert_eq!(balance.enemy.retreat, (16.0, 22.0));
        assert_eq!(balance.unlock.hard, 30);
        let seeker = balance.classes[Class::Seeker as usize];
        assert_eq!(seeker.favorites(), [Stat::Ricochet, Stat::Seeking]);
        assert_eq!(
            (seeker.weight, seeker.ricochet, seeker.ballistics),
            (1, 3, 1)
        );
        assert!(balance.classes[Class::None as usize].favorites().is_empty());
    }

    #[test]
    fn something_that_grows_by_steps_goes_up_at_its_sectors() {
        let tanks = read(FILE).unwrap().steps.tanks;
        let told: Vec<u32> = [0, 1, 2, 3, 5, 6, 14, 15, 16, 1000]
            .into_iter()
            .map(|sector| tanks.at(sector))
            .collect();
        assert_eq!(told, [1, 1, 1, 2, 2, 3, 5, 6, 6, 6]);
        assert_eq!(tanks.last(), 6);
        // One that never goes up keeps its first value.
        let flat = read(&FILE.replace("step ballistics  0  3 8 13", "step ballistics  2")).unwrap();
        assert_eq!(flat.steps.ballistics.at(1), 2);
        assert_eq!(flat.steps.ballistics.at(500), 2);
    }

    #[test]
    fn any_stat_can_have_a_step() {
        // No stat but the three has one unless the file says so.
        let balance = read(FILE).unwrap();
        assert!(balance.steps.of(Stat::Exploding).is_none());
        assert_eq!(balance.steps.of(Stat::Brain), Some(balance.steps.brain));
        // A stat that has one is held back until its sectors.
        let line = "step muzzle      2  4 9 14 19 24\n";
        let more = "step exploding   0  8\nstep ricochet 1 10 20\n";
        let held = read(&FILE.replace(line, &format!("{line}{more}"))).unwrap();
        let exploding = held.steps.of(Stat::Exploding).unwrap();
        assert_eq!([exploding.at(7), exploding.at(8)], [0, 1]);
        let ricochet = held.steps.of(Stat::Ricochet).unwrap();
        assert_eq!(
            [ricochet.at(1), ricochet.at(10), ricochet.at(99)],
            [1, 2, 3]
        );
        assert!(held.steps.of(Stat::Engine).is_none());
        assert_ne!(
            fingerprint(FILE),
            fingerprint(&FILE.replace(line, &format!("{line}{more}")))
        );
        // It is written once, ends no higher than its stat, and names something that grows.
        for (more, why) in [
            ("step exploding 0 8\nstep exploding 0 9\n", "twice"),
            ("step brain 0 3\n", "twice"),
            ("step exploding 0 8 12\n", "highest level"),
            ("step ricochet 2 5 9\n", "highest level"),
            ("step armor 0 3\n", "what grows"),
        ] {
            let fault = read(&FILE.replace(line, &format!("{line}{more}"))).expect_err(more);
            assert!(fault.to_string().contains(why), "{more}: {fault}");
        }
    }

    #[test]
    fn numbers_are_the_nearest_to_what_is_written() {
        // The reader must give the numbers that the code of the game wrote as literals.
        for (written, wanted) in [
            ("0.225", 0.225),
            ("7.5", 7.5),
            ("3.2", 3.2),
            ("1.25", 1.25),
            ("0.12", 0.12),
            ("0.15", 0.15),
            ("0.6", 0.6),
            ("1.4", 1.4),
            ("0.8", 0.8),
            ("19", 19.0),
            ("0.3", 0.3),
            ("0.2", 0.2),
        ] {
            assert_eq!(decimal(written.as_bytes()), Some(wanted), "{written}");
        }
        for wrong in [
            "", ".", ".5", "5.", "1..2", "1.2.3", "-1", "1e3", "12345678",
        ] {
            assert_eq!(decimal(wrong.as_bytes()), None, "{wrong}");
        }
        assert_eq!(whole(b"0042"), Some(42));
        assert_eq!(whole(b"4.2"), None);
        // The angles that the rules wrote as parts of a half turn.
        assert_eq!(radians(720.0), 4.0 * PI);
        assert_eq!(radians(45.0), PI / 4.0);
        assert_eq!(radians(30.0), PI / 6.0);
        assert_eq!(radians(90.0), PI / 2.0);
    }

    #[test]
    fn a_number_comes_right_before_its_unit() {
        assert_eq!(before(b"1.5s", b"s"), Some(&b"1.5"[..]));
        assert_eq!(before(b"3m/s", b"m/s"), Some(&b"3"[..]));
        // A speed is no length, and a unit alone is no number.
        assert_eq!(before(b"3m/s", b"m"), None);
        assert_eq!(before(b"m", b"m"), None);
        assert_eq!(before(b"12", b""), Some(&b"12"[..]));
        assert_eq!(halves(b"8m..14m", b".."), Some((&b"8m"[..], &b"14m"[..])));
        assert_eq!(halves(b"0.5..1.5", b".."), Some((&b"0.5"[..], &b"1.5"[..])));
        assert_eq!(halves(b"0.5", b".."), None);
    }

    #[test]
    fn a_file_that_can_not_be_read_says_why_and_on_which_line() {
        // A record that is wrong in itself.
        for (written, otherwise, line, what) in [
            (
                "cost 0 2 4 6 8 11 14 17",
                "cost 0 2 4 6 8 11 14",
                "stat engine",
                "each of its levels",
            ),
            (
                "cost 0 2 4 6 8 11 14 17",
                "cost 0 2 4 6 8 7 14 17",
                "stat engine",
                "no less than",
            ),
            (
                "cost 0 2 4 6 8 11 14 17",
                "cost 1 2 4 6 8 11 14 17",
                "stat engine",
                "level 0 is free",
            ),
            (
                "cost 0 2 4 6 8 11 14 17",
                "share 0 2 4 6 8 11 14 17",
                "stat engine",
                "after `cost`",
            ),
            (
                "speed 0 2 3 4 5 6 7 8",
                "speed 1 2 3 4 5 6 7 8",
                "stat engine",
                "doesn't drive",
            ),
            (
                "speed 0 2 3 4 5 6 7 8",
                "0 2 3 4 5 6 7 8",
                "stat engine",
                "what its levels give",
            ),
            (
                "cooldown - 10 9",
                "cooldown 11 10 9",
                "stat shield",
                "written `-`",
            ),
            (
                "share 0 8 16 24 32",
                "share 0 8 16 24 132",
                "stat brain",
                "100% of a budget",
            ),
            (
                "cost 0 4 10 18",
                "cost 0 4 10 18 30",
                "stat ricochet",
                "a word too many",
            ),
            (
                "stat mines",
                "stat armor",
                "stat armor",
                "followed by a stat",
            ),
            (
                "loader=6",
                "loader=9",
                "player",
                "higher than its stat goes",
            ),
            ("engine=4 ", "", "player", "needs an engine"),
            ("ricochet=1 mines", "turret=1 mines", "player", "no turret"),
            (
                "mines=2 shield=2",
                "mines=2 mines=2",
                "player",
                "written twice",
            ),
            (
                "shield=2",
                "piercing=1 seeking=1",
                "player",
                "one type of bullet",
            ),
            ("shield=2", "shield", "player", "stats with their levels"),
            (
                "respawn 1.5s",
                "respawn 1.5",
                "solo",
                "a time, as in `1.5s`",
            ),
            ("lives 3", "lives 0", "solo", "one life at least"),
            ("lives 3", "life 3", "solo", "`lives` is expected here"),
            (
                "turret 720deg/s",
                "turret 720deg",
                "tank",
                "a turn, as in `720deg/s`",
            ),
            (
                "bullet 0.225m",
                "bullet 0.225s",
                "gun",
                "a length, as in `8m`",
            ),
            ("warn 0.6s", "warn 12s", "mine", "last moments of its fuse"),
            ("blast   3m", "blast   3", "blast", "as in `3m`"),
            (
                "10 to 100  every 15",
                "100 to 10  every 15",
                "budget",
                "up from its first value",
            ),
            (
                "10 to 100  every 15",
                "10 to 100  every 0",
                "budget",
                "a sector at least",
            ),
            (
                "10 to 100  every 15",
                "10 100 15",
                "budget",
                "as in `10 to 100 every 15`",
            ),
            (
                "aim 8deg to 1deg",
                "aim 8 to 1",
                "experience",
                "experience is written as",
            ),
            (
                "1  3 6 9 12 15",
                "1  3 6 6 12 15",
                "step tanks",
                "rising order",
            ),
            ("1  3 6 9 12 15", "1  1 3", "step tanks", "after sector 1"),
            ("1  3 6 9 12 15", "0  3", "step tanks", "one tank at least"),
            (
                "1  3 6 9 12 15",
                "1  2 3 4 5 6 7 8 9 10",
                "step tanks",
                "eight times",
            ),
            (
                "1  6 11 16",
                "2  6 11 16",
                "step brain",
                "above the highest level",
            ),
            ("step muzzle ", "step armor ", "step armor", "what grows"),
            ("team    +50%", "team    50%", "team", "as in `+50%`"),
            (
                "budget +30  ahead 20",
                "budget 30  ahead 20",
                "hard",
                "`budget +30 ahead 20`",
            ),
            (
                "mean 70%..90%",
                "mean 90%..70%",
                "group",
                "smaller of the two",
            ),
            (
                "least 30%",
                "least 80%",
                "group",
                "no more than what a tank has on average",
            ),
            ("leader 150%", "leader 90%", "group", "100%"),
            ("weights 1 1 2 2 3 8", "weights 0 0", "group", "isn't 0"),
            (
                "weights 1 1 2 2 3 8",
                "weights 1 1 1 1 1 1 1 1 1",
                "group",
                "eight of them",
            ),
            (
                "favorite 20",
                "favourite 20",
                "group",
                "`favorite` is expected",
            ),
            (
                "1  muzzle turret ",
                "muzzle turret ",
                "class sniper",
                "its weight in the draw",
            ),
            (
                "1  muzzle turret ",
                "1  muzzle turret engine ",
                "class sniper",
                "two stats at most",
            ),
            (
                "1  muzzle turret ",
                "1  muzzle muzzle ",
                "class sniper",
                "a stat once",
            ),
            (
                "1  splitting ",
                "1  splitting piercing ",
                "class flanker",
                "one type of bullet",
            ),
            (
                "ricochet<=0  ballistics<=1",
                "ricochet<=0  ballistics<=2",
                "class grenadier",
                "needs a bounce",
            ),
            (
                "ricochet<=0  ballistics<=1",
                "ricochet<=9  ballistics<=1",
                "class grenadier",
                "than the stat goes",
            ),
            (
                "ricochet<=0  ballistics<=1",
                "ricochet 0  ballistics 1",
                "class grenadier",
                "as in `ricochet<=1",
            ),
            (
                "class lancer",
                "class knight",
                "class knight",
                "followed by a class",
            ),
            (
                "harshest 30",
                "harshest 1",
                "terrain harshest",
                "2 at least",
            ),
            (
                "14m to 8m  spread 6m",
                "14 to 8  spread 6",
                "terrain width",
                "as in `14m to 8m",
            ),
            (
                "0..1 to 3..6",
                "0..1 to 6..3",
                "terrain crates",
                "smaller of the two",
            ),
            (
                "10% to 70%",
                "10% to 170%",
                "terrain angled",
                "100% at most",
            ),
            (
                "entry 15m",
                "entry 15.5m",
                "terrain spawn",
                "a whole number of meters",
            ),
            (
                "terrain cover ",
                "terrain trees ",
                "terrain trees",
                "what it says",
            ),
            (
                "corridor 40m..80m",
                "corridor 40m",
                "corridor",
                "as in `40m..80m`",
            ),
            (
                "every 5..8",
                "every 0..8",
                "arena",
                "after a sector at least",
            ),
            ("waves 3", "waves 0", "arena", "a wave at least"),
            (
                "36m..42m 22m..24m",
                "35m..42m 22m..24m",
                "room",
                "even numbers",
            ),
            ("at-once 50%", "at-once 150%", "room", "all of it at most"),
            (
                "hold 1.25",
                "hold 1.25m",
                "enemy keep",
                "a number, as in `1.25`",
            ),
            (
                "charge 3m/s",
                "charge 3m",
                "enemy mines",
                "a speed, as in `3m/s`",
            ),
            (
                "bait 11m..14m",
                "bait 14m..11m",
                "enemy ace",
                "smaller of the two",
            ),
            (
                "enemy ace ",
                "enemy king ",
                "enemy king",
                "keep, mines or ace",
            ),
            (
                "hard 30  impossible 20",
                "hard 30",
                "unlock",
                "`impossible` is expected",
            ),
            (
                "mark    1.5s",
                "mark    1.5s soon",
                "mark",
                "a word too many",
            ),
            ("garage  59", "treasure  59", "treasure", "name of a record"),
            (
                "tanku-balance 1",
                "tanku-balance 2",
                "tanku-balance",
                "version 1",
            ),
        ] {
            let found = fault(written, otherwise);
            assert!(found.what.contains(what), "{otherwise}: {found}");
            // The fault is on the line that was written otherwise.
            let wanted = line_of(&FILE.replace(written, otherwise), line);
            assert_eq!(found.line, wanted, "{otherwise}: {found}");
        }
    }

    #[test]
    fn a_file_lacks_no_line_and_has_none_twice() {
        // The standard tank is checked once the prices are read: its fault is on its line.
        let dear = fault("garage  59", "garage  58");
        assert_eq!((dear.line, dear.about), (line_of(FILE, "player"), ""));
        assert!(dear.what.contains("more points than the garage has"));
        // A tank must draw a class.
        let classes = FILE.lines().filter(|text| starts(text, "class")).count();
        let weightless: Vec<String> = FILE
            .lines()
            .map(|text| match text.split_once("  1  ") {
                Some((class, rest)) if starts(text, "class") => format!("{class}  0  {rest}"),
                _ => text.to_string(),
            })
            .collect();
        assert_eq!(classes, Class::ALL.len());
        let none = read(&weightless.join("\n")).unwrap_err();
        assert!(none.what.contains("a weight that isn't 0"), "{none}");
        // A line that is missing is named.
        for (line, what, about) in [
            ("stat seeking", "no `stat` line for", "seeking"),
            ("solo ", "no line for", "solo"),
            ("step brain", "no `step` line for", "brain"),
            ("class sapper", "no `class` line for", "sapper"),
            ("terrain angled", "no `terrain` line for", "angled"),
            ("enemy mines", "no `enemy` line for", "mines"),
            ("unlock", "no line for", "unlock"),
        ] {
            let kept: Vec<&str> = FILE.lines().filter(|text| !starts(text, line)).collect();
            assert_eq!(kept.len() + 1, FILE.lines().count(), "{line}");
            let found = read(&kept.join("\n")).unwrap_err();
            assert!(found.what.contains(what), "{line}: {found}");
            assert_eq!(found.about, about, "{line}: {found}");
            assert_eq!(
                found.to_string(),
                format!("line {}: {}", found.line, found.what) + " " + about
            );
        }
        // A line that is there twice is the second one.
        for line in [
            "stat tracks",
            "blast",
            "step muzzle",
            "class scout",
            "terrain cover",
            "enemy ace",
        ] {
            let text = FILE.lines().find(|text| starts(text, line)).unwrap();
            let found = read(&format!("{FILE}{text}\n")).unwrap_err();
            assert_eq!(found.what, TWICE, "{line}");
            assert_eq!(found.line as usize, FILE.lines().count() + 1);
        }
        // The heading comes first, and once.
        let late = read(&FILE.replacen("tanku-balance 1\n", "", 1)).unwrap_err();
        assert_eq!(
            (late.what, late.line),
            (HEADING, line_of(FILE, "stat engine") - 1)
        );
        assert_eq!(read("").unwrap_err().what, HEADING);
        assert_eq!(read("# Nothing.\n\n").unwrap_err().what, HEADING);
        let again = read(&format!("{FILE}tanku-balance 1\n")).unwrap_err();
        assert_eq!(again.what, TWICE);
    }

    #[test]
    #[should_panic(expected = "game.balance, line 37: the file has no `stat` line for engine")]
    fn a_fault_stops_with_its_line_and_what_it_is_about() {
        let fault = Fault {
            line: 37,
            what: "the file has no `stat` line for",
            about: "engine",
        };
        assert_eq!(
            fault.to_string(),
            "line 37: the file has no `stat` line for engine"
        );
        fault.stop("game.balance");
    }

    #[test]
    #[should_panic(expected = "game.balance, line 4: this line has a word too many")]
    fn a_fault_that_is_about_nothing_stops_with_its_line() {
        let fault = Fault {
            line: 4,
            what: "this line has a word too many",
            about: "",
        };
        assert_eq!(fault.to_string(), "line 4: this line has a word too many");
        fault.stop("game.balance");
    }

    #[test]
    fn the_fingerprint_is_of_the_words_and_of_nothing_else() {
        let whole = read(FILE).unwrap();
        let print = fingerprint(FILE);
        // Comments, empty lines and the room between two words are no part of it.
        let spaced = FILE.replace("  ", " ").replace("\n\n", "\n");
        assert_ne!(spaced, FILE);
        assert_eq!(fingerprint(&spaced), print);
        let commented = format!("# Another word.\n{FILE}\n   # And one more.\n");
        assert_eq!(fingerprint(&commented), print);
        let windows = FILE.replace('\n', "\r\n");
        assert_eq!(fingerprint(&windows), print);
        assert_eq!(read(&windows).unwrap(), whole);
        // Any number is, and so is the order of the words.
        assert_ne!(
            fingerprint(&FILE.replace("blast   3m", "blast   4m")),
            print
        );
        assert_ne!(fingerprint(&FILE.replace("lives 3", "lives  4")), print);
        assert_ne!(fingerprint("a b\n"), fingerprint("ab\n"));
        assert_ne!(fingerprint("a\nb\n"), fingerprint("a b\n"));
    }
}
