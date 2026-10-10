//! How far ahead the web build schedules its music.
//!
//! A browser plays the sound that a page has handed over, and the page can only hand some over
//! between two frames. While a frame keeps the page busy, nothing new arrives: the sound that
//! was handed over before must last until the frame is done, or there is a hole in it. So the
//! music is handed over ahead of time, a few pieces at once.
//!
//! The price is that the music hears of the game that much later. It therefore starts close
//! to the present, and reaches further only on a machine whose frames turn out to be long.
//!
//! Everything is counted in frames of sound, so that each piece starts exactly where the last
//! one ends.

use tanku_audio::SAMPLE_RATE;

/// Frames in a piece of music, which is handed over at once: 46 ms.
pub const PIECE: u64 = 2048;
/// How far ahead of the present the music is scheduled at first: a tenth of a second.
const NEAR: u64 = SAMPLE_RATE as u64 / 10;
/// How much further it reaches after each hole.
const MORE: u64 = SAMPLE_RATE as u64 / 10;
/// How far it reaches at most: four tenths of a second.
const FAR: u64 = 4 * SAMPLE_RATE as u64 / 10;
/// A first piece starts this long after the present, which gives the browser the time to
/// take it: 30 ms.
const LEAD: u64 = 3 * SAMPLE_RATE as u64 / 100;

/// What is scheduled so far, and how far ahead the music is kept.
pub struct Ahead {
    /// The frame up to which the music is scheduled, counted from the start of the sound.
    until: u64,
    reach: u64,
}

impl Default for Ahead {
    fn default() -> Self {
        Self {
            until: 0,
            reach: NEAR,
        }
    }
}

impl Ahead {
    /// The pieces to schedule now: the frame at which the first one starts, and how many
    /// there are. Each starts where the one before it ends.
    ///
    /// `now` is the frame that the sound has reached. `away` says that the page wasn't shown
    /// for a while: sound that ran out then says nothing about the machine.
    pub fn due(&mut self, now: u64, away: bool) -> (u64, u32) {
        if self.until < now {
            // The sound ran out: a frame took longer than the music reached. After the first
            // piece there is nothing to learn from that; later, the music reaches further.
            if self.until > 0 && !away {
                self.reach = (self.reach + MORE).min(FAR);
            }
            self.until = now + LEAD;
        }
        let first = self.until;
        let mut pieces = 0;
        while self.until < now + self.reach {
            self.until += PIECE;
            pieces += 1;
        }
        (first, pieces)
    }

    /// How far ahead the music is kept, in seconds.
    pub fn reach(&self) -> f32 {
        self.reach as f32 / SAMPLE_RATE as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Frames of sound in some milliseconds.
    fn ms(ms: u64) -> u64 {
        ms * SAMPLE_RATE as u64 / 1000
    }

    /// Plays some frames of the game, each of some milliseconds, and tells how long the holes
    /// in the music were, in frames of sound.
    fn play(ahead: &mut Ahead, now: &mut u64, frames: &[u64]) -> u64 {
        let mut holes = 0;
        for &frame in frames {
            *now += ms(frame);
            // Before the first piece, there is no music to have a hole in.
            if ahead.until > 0 {
                holes += now.saturating_sub(ahead.until);
            }
            let (first, pieces) = ahead.due(*now, false);
            // What is scheduled starts in the future, and reaches as far as it should.
            assert!(first >= *now);
            assert!(ahead.until >= *now + ahead.reach);
            assert_eq!(ahead.until, first + pieces as u64 * PIECE);
        }
        holes
    }

    #[test]
    fn the_music_starts_just_ahead_of_the_present() {
        let mut ahead = Ahead::default();
        let now = ms(5000);
        // Two pieces of 46 ms, from 30 ms after the present, reach past a tenth of a second.
        assert_eq!(ahead.due(now, false), (now + LEAD, 2));
        assert_eq!(ahead.reach(), 0.1);
        // Nothing more is due until some of it has played.
        assert_eq!(ahead.due(now, false).1, 0);
        assert_eq!(ahead.due(now + ms(16), false).1, 0);
        // Each piece starts where the last one ended.
        let (next, pieces) = ahead.due(now + ms(40), false);
        assert_eq!((next, pieces), (now + LEAD + 2 * PIECE, 1));
    }

    #[test]
    fn steady_frames_leave_no_hole_and_keep_the_music_close() {
        let (mut ahead, mut now) = (Ahead::default(), ms(1000));
        assert_eq!(play(&mut ahead, &mut now, &[16; 600]), 0);
        // Frames of up to a tenth of a second go by without a hole too.
        assert_eq!(play(&mut ahead, &mut now, &[16, 16, 95, 16, 60, 90, 16]), 0);
        assert_eq!(ahead.reach(), 0.1);
    }

    #[test]
    fn a_long_frame_leaves_one_hole_and_the_music_reaches_further() {
        let (mut ahead, mut now) = (Ahead::default(), ms(1000));
        play(&mut ahead, &mut now, &[16; 60]);
        // The page is held up for 180 ms: the music had 100 ms to 146 ms in hand.
        let hole = play(&mut ahead, &mut now, &[180]);
        assert!(hole > ms(30) && hole <= ms(80), "{hole}");
        assert_eq!(ahead.reach(), 0.2);
        // The next frame of that length goes by without a hole.
        play(&mut ahead, &mut now, &[16; 30]);
        assert_eq!(play(&mut ahead, &mut now, &[180, 16, 16, 190, 16]), 0);

        // The reach stops growing at four tenths of a second.
        for _ in 0..6 {
            play(&mut ahead, &mut now, &[16, 16, 700]);
        }
        assert_eq!(ahead.reach(), 0.4);
        assert_eq!(play(&mut ahead, &mut now, &[16, 390, 16, 380]), 0);
    }

    #[test]
    fn a_page_that_was_away_teaches_nothing() {
        let (mut ahead, mut now) = (Ahead::default(), ms(1000));
        play(&mut ahead, &mut now, &[16; 60]);
        // The player looked at another page for a minute.
        now += ms(60_000);
        let (first, pieces) = ahead.due(now, true);
        assert_eq!((first, pieces), (now + LEAD, 2));
        assert_eq!(ahead.reach(), 0.1);
    }
}
