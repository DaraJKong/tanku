# Tanku: ideas

Ideas that aren't in the specification. They are ranked by likely usefulness and none has been explored in depth.

| Idea | Rank | Notes | How to validate |
|---|---|---|---|
| Elite tank in arenas | Medium | One tank at 1.5 times the sector's budget cap. A boss moment without a new system, and something that can keep growing past the difficulty cap. | Playtest once the solo loop exists |
| Pits | Medium | Holes that stop tanks but not bullets, as in Tanks!. Adds work to map generation and pathfinding. | Corridor prototype |
| Aiming laser | Medium | Tanks with Ballistics 2 or more show their line of fire, bounces included. Makes bank shots readable and fairer. | Playtest against a bank-shot enemy |
| Daily seed | Medium | A seed derived from the date. Worth more with a leaderboard on the official server. | After the modes exist |
| Pings | Medium | A key that marks a spot or an enemy for teammates. | Co-op playtest |
| Mine circles | Medium | An armed mine draws its 2 m trigger circle in dashes, and a triggered mine draws its 3 m blast circle during the 0.6 s warning. Makes both radii readable, and adds circles to a busy screen. | Playtest against a tank that lays mines, with the circles on and off |
| Dormant enemies as outlines | Medium | A dormant enemy is a dashed outline in its hue, filled in when it wakes. An outline carries less of the saturation that shows the budget. | Compare screenshots with the dull paint used now |
| Drawing of the tank that ended the run | Medium | The game over screen shows the enemy that destroyed the player, with its stats as a parts list. The simulation must report whose bullet or mine it was. | Watch whether playtesters read it before they retry |
| Rounds around the player's tank | Medium | Rounds, reload and mines on an arc under the tank, where the player is looking, in addition to the corner of the screen. | Playtest with the arc on and off |
| View past a barrier | Medium | Enemies start at least 15 m beyond a barrier and the view reaches about 16 m north of the tank, so the next group is off screen when north is at the top. The view could slide forward while a player waits at a barrier. | Corridor playtest: can players study the next group before they cross? |
| Camera that turns with the corridor | Low | The view rotates by itself to keep progress pointing one way. Saves key presses, but takes control away and can disorient. | Compare with manual rotation in the corridor prototype |
| More piercing levels | Low | A piercing bullet pierces one bullet. Each further level would pierce one more. | Playtest against enemies that pierce, once the price of piercing is settled |
| Hitbox as long as the bullet | Low | Bullets are drawn 0.9 m long but collide as a circle 0.45 m across, so the two ends of the drawing touch nothing. A capsule would match the drawing and make every collision test longer to write. | Playtest: watch for bullets that seem to pass through each other |
| Glow around bullets | Low | Bloom on bullets and lights. Bullets are bright and flat for now, which already reads well. Costs frame time on the web. | Compare screenshots with and without it |
| Smaller web build | Low | The web build is 38 MB, 9 MB compressed. `wasm-opt` would shrink it. | Measure the loading time first |
| Reflecting shield | Low | The shield sends bullets back. Fun, but hard to balance with friendly fire. | Variant of the shield prototype |
| Life refill | Low | A checkpoint gives one life back, up to three. A tuning lever if solo is too harsh. | Solo playtests |
| Repair kits | Low | A limited, shared number of repairs. A tuning lever if co-op is too easy. | Co-op playtests |
| Friendly-fire option | Low | A lobby switch. Splits records and achievements in two. | Co-op playtests |
| Armor | Low | Tanks that survive several hits. Contradicts the one-hit pillar. | None planned |
| Stealth | Low | A stat that makes a tank fade out, as the white tank of Tanks! does. Contradicts readable danger. | None planned |
| Replays | Low | Recording inputs and replaying a run. Needs a simulation that is deterministic on every platform, not only generation. | None planned |
| Realistic military theme | Low | Camouflage, mud, weathered steel. Camouflage hides the hue and saturation that tell the players what an enemy is, and it needs textures the generated models don't have. | None planned |
| Synthwave theme | Low | Neon on black with a horizon grid. Everything glows, so bullets stop being the most visible objects, and it depends on bloom, which costs frame time on the web. | Compare screenshots with the chosen theme |
| Voxel or pixel-art theme | Low | Chunky retro look. The 1:2 slopes of the walls and the thin stat parts (antennas, muzzle rings) turn into staircases and noise. | None planned |
| Wireframe blueprint | Low | Tanks as colored lines only, on a black ground. Thin lines carry too little color to show hue and budget, and they shimmer when they move. | None planned |
| Blast scars | Low | A scalloped outline stays where a blast went off, with the removed blocks in dashes and a cross for each tank destroyed. A record of the fight, and more lines on the floor. | Compare screenshots after a long fight |
| Seed and version during a run | Low | The specification shows them in the menu. A corner block could show them at all times, which helps when sharing a screenshot. | None planned |
| Ruler along the corridor | Low | Ticks in the void that give the distance to the barrier. Decoration that happens to be true. | None planned |
