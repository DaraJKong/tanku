// With "?probe" after its address, the page shows how long its frames are and whether its
// sound has holes. Without it, this file does nothing.
//
// A page hands its sound to the browser in pieces of 46 ms, between two frames, and says when
// each piece is to start. The sound has a hole when a piece starts after the one before it has
// ended, because the page was busy for too long to hand it over in time. The cues are handed
// over one piece at a time, so a frame that keeps the page busy for longer than about 40 ms
// leaves a hole in them. The music is handed over further ahead, and reaches further still
// after each hole.
if (new URLSearchParams(location.search).has("probe")) {
  const LONG = 40;
  const sound = () => ({ pieces: 0, holes: 0, hole: 0, total: 0, end: 0 });
  const fresh = () => ({ frames: 0, longest: 0, long: 0, music: sound(), cues: sound(), ahead: null });
  // A script can read these numbers too, and empty them.
  const probe = (window.probe = {
    seen: fresh(),
    reset: () => {
      const ends = [probe.seen.music.end, probe.seen.cues.end];
      probe.seen = fresh();
      [probe.seen.music.end, probe.seen.cues.end] = ends;
    },
  });

  const start = AudioBufferSourceNode.prototype.start;
  AudioBufferSourceNode.prototype.start = function (when = 0, ...rest) {
    // The game gives the source of its music a name.
    const seen = this.context.tanku === "music" ? probe.seen.music : probe.seen.cues;
    // A piece that is to start in the past starts now.
    const from = Math.max(when, this.context.currentTime);
    const hole = 1000 * (from - seen.end);
    seen.pieces += 1;
    if (seen.end > 0 && hole > 0.5) {
      seen.holes += 1;
      seen.total += hole;
      seen.hole = Math.max(seen.hole, hole);
    }
    seen.end = from + (this.buffer ? this.buffer.duration : 0);
    if (this.context.tanku === "music") {
      probe.seen.ahead = this.context.ahead;
    }
    return start.call(this, when, ...rest);
  };

  // The page is busy while the game loads, which says nothing of the game: the count starts
  // over with the first click or key, as the sound does.
  for (const action of ["keydown", "pointerdown"]) {
    addEventListener(action, probe.reset, { capture: true, once: true });
  }

  let last = performance.now();
  const frame = (now) => {
    const length = now - last;
    last = now;
    probe.seen.frames += 1;
    probe.seen.longest = Math.max(probe.seen.longest, length);
    probe.seen.long += length > LONG;
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);

  const panel = document.createElement("pre");
  panel.style.cssText =
    "position: fixed; top: 8px; right: 8px; margin: 0; padding: 6px 10px; z-index: 10;" +
    "font: 13px monospace; color: #dfe6f2; background: #0b1220cc; border: 1px solid #5d7397;" +
    "cursor: pointer; user-select: none;";
  panel.title = "Click to start counting again";
  panel.addEventListener("click", probe.reset);
  addEventListener("DOMContentLoaded", () => document.body.append(panel));
  const line = (name, seen) =>
    `${name}  ${seen.pieces} pieces   holes: ${seen.holes}   longest ${seen.hole.toFixed(0)} ms`;
  setInterval(() => {
    const seen = probe.seen;
    const ahead = seen.ahead === null ? "" : `   ${Math.round(1000 * seen.ahead)} ms ahead`;
    panel.textContent =
      `frames  ${seen.frames}   longest ${seen.longest.toFixed(0)} ms   over ${LONG} ms: ${seen.long}\n` +
      line("music ", seen.music) + ahead + "\n" +
      line("cues  ", seen.cues);
  }, 500);
}
