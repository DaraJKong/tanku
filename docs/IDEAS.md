# Tanku: ideas

Ideas that aren't in the specification. They are ranked by likely usefulness and none has been explored in depth.

| Idea | Rank | Notes | How to validate |
|---|---|---|---|
| Elite tank in arenas | Medium | One tank at 1.5 times the sector's budget cap. A boss moment without a new system, and something that can keep growing past the difficulty cap. | Playtest once the solo loop exists |
| Pits | Medium | Holes that stop tanks but not bullets, as in Tanks!. Adds work to map generation and pathfinding. | Corridor prototype |
| Aiming laser | Medium | Tanks with Ballistics 2 or more show their line of fire, bounces included. Makes bank shots readable and fairer. | Playtest against a bank-shot enemy |
| Daily seed | Medium | A seed derived from the date. Worth more with a leaderboard on the official server. | After the modes exist |
| Pings | Medium | A key that marks a spot or an enemy for teammates. | Co-op playtest |
| View past a barrier | Medium | Enemies start at least 15 m beyond a barrier and the view reaches about 16 m north of the tank: with north at the top, a tank that waited at a barrier had 3% of the next group on screen. Built: once a sector is quiet, the view runs up to 10 m ahead of a tank that comes close to its barrier, which puts 31% of the next group on screen. The specification doesn't describe it yet. | Describe it in the specification, then drop this row |
| Cause of each loss | Medium | The game over screen shows the tank that ended the run, but nothing tells a solo player what destroyed their tank while lives remain. A line during the respawn delay could name the build and say whether it was a bullet, a mine or a crate. The simulation already reports it. | Solo playtest: ask players what destroyed them after each loss |
| Chicanes | Low | Both walls take the shallowest slope together, so the corridor jumps 8 m or more sideways within a few meters. The throat between the two walls is less than half as wide as the corridor, so it only fits the widest stretches. | Corridor prototype: force one into a wide sector and drive through it |
| Camera that turns with the corridor | Low | The view rotates by itself to keep progress pointing one way. Saves key presses, but takes control away and can disorient. | Compare with manual rotation in the corridor prototype |
| Wider view at a barrier | Low | While the view runs ahead of a tank at a barrier, it could also show 45 m of ground instead of 30. That puts 66% of the next group on screen instead of 31%, and draws everything a third smaller. Tried in the prototype of the view past a barrier and not kept. | None planned |
| Shield state on the model | Low | The cue on a tank shows whether its shield is ready, recharging or overloaded, for instance with lights that come back one by one. The white rim only shows the level. | Playtest: watch whether players can tell when an enemy's shield is back |
| What to expect from a tank | Low | A few sentences under the drawing of the tank that ended the run, on how it fights: what its brain does and how it aims. Each sentence must be checked against what the brain tiers really do, and reading them delays the retry. | Watch whether playtesters read the parts list first |
| Dark plate under the sight | Low | The lenses are a pale tint of the tank's hue, so they are faint on a weak tank, which is pale itself, once it has woken up. A small dark plate under them makes them clear on every tank, and makes the turret busier. | Playtest: watch whether players miss the sight of weak tanks |
| More playtest cheats | Low | A tank that nothing destroys, destroying the enemies of the sector, slow motion, a free camera, placing an enemy of a chosen genome. On the web, a playtest could start from the address of the page. | Add one when a playtest shows the need |
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
