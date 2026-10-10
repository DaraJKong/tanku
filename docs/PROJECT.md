# Tanku: game specification

Tanku is a cooperative top-down tank game for one to four players. The players fight their way through an endless corridor of procedurally generated enemy tanks and try to get as far as possible. A single hit destroys a tank, so survival comes from reading enemies, dodging and aiming, never from upgrades.

The core loop is: advance, read the enemies, fight, survive, learn, advance again. Learning is what the players earn each time around: they see what each kind of enemy does and prepare better for the next one. Nothing carries over between runs except what they have understood.

All numbers in this document are starting values to tune in playtests.

## Overview

| Topic | Summary |
|---|---|
| Genre | Cooperative arcade shooter played in roguelike runs |
| Players | 1 to 4. Solo is a complete mode, not a fallback. |
| Run length | 10 to 30 minutes for most runs |
| Verbs | Move, aim, shoot, place mines, shield, repair (multiplayer only) |
| View | 3D, seen from above. North, the direction of progress, is at the top of the screen unless a player rotates the view. |
| Platforms | Linux, Windows and the web |
| Technology | Rust and the Bevy engine |

### Pillars

1. **Skill over stats.** One hit destroys any tank, and player tanks never get stronger during a run. Players get further by playing better.
2. **Readable danger.** Everything an enemy can do is visible on its model before the fight starts, and every bullet can be dodged.
3. **Same seed, same challenge.** A seed always produces the same map and the same enemies, so it can be learned, shared and raced like a level.
4. **Co-op first, solo complete.** Every rule works for one to four players.
5. **Instant retry.** Getting from game over back to driving takes a few seconds.

### Non-goals

Tanku has no player-versus-player mode, no loot, no upgrades during a run, no story and no meta-progression. What players unlock adds ways to play and never makes a run easier.

### Inspirations

| Game | What Tanku takes from it |
|---|---|
| Tanks! (Wii Play) | One-hit kills, ricochets, mines, enemy types that can be recognized at a glance |
| Nuclear Throne | Short and intense runs, permanent death, instant restarts |

## Glossary

| Term | Meaning |
|---|---|
| Run | One attempt, from the start bay to game over |
| Seed | The text that determines a run's map and enemies |
| North | The direction of progress along the corridor |
| Sector | One stretch of corridor holding one group of enemies |
| Barrier | The energy wall that closes the end of a sector |
| Flag | Stands near the end of some sectors. A solo player captures it and respawns there. |
| Checkpoint | The last flag captured |
| Wreck | A destroyed player tank, which a teammate can repair |
| Budget | The number of points a tank has to spend on stats |
| Sector budget | The most points an enemy of a given sector can have |
| Genome | A tank's full list of stat levels |
| Hardware | The stats of the tank itself, paid in points |
| Software | The stats of an enemy's controller (brain and ballistics), paid as a share of its budget |
| Experience | How well enemy brains execute, which rises with the sector number |
| Round | One bullet in a tank's magazine |

## Run structure

A run starts in the start bay, an empty sector, and continues until game over. All progress of the run is then lost. Only unlocked modes and records persist. The score is the number of sectors cleared.

### Sectors and barriers

The corridor is a chain of sectors. Each sector holds one group of enemies and ends with a barrier, an energy wall that spans the full width of the corridor. Barriers follow four rules:

1. Bullets and blasts never pass a barrier, and enemies never cross one.
2. A barrier unlocks when every enemy in its sector is destroyed and every living player is within 8 m of it. It then stays unlocked.
3. Players cross an unlocked barrier forward only.
4. The enemies of a sector are dormant until the first player crosses into it. Dormant enemies are visible through the barrier. Until then, the sector is drawn darker than the rest of the corridor, and its enemies are outlines in dashes.

These rules let the players study the next group, pick the moment to engage and enter together. They also rule out both driving past enemies and fighting from a doorway. When the last living player has crossed, any wreck left behind is towed across and the sectors further back are unloaded. When two or fewer enemies remain in a sector, markers at the screen edge point to them. Each end of a barrier bears a tag with the numbers of the two sectors it parts, and a caption says whether the barrier is locked or open.

### Solo

The solo player has three lives, and lives are never refilled. When the tank is destroyed, the player loses a life and respawns 1.5 s later at the last checkpoint, between its flag and the barrier. Every sector beyond it is regenerated from the seed, so its enemies and obstacles are back. Before the first checkpoint, the player respawns in the start bay. Losing the third life ends the run.

Checkpoints are flags. A flag stands 4 m in front of the middle of its sector's barrier, inside a circle 2 m in radius drawn on the ground, and it is gray at first. Once every enemy of the sector is destroyed, the player captures the flag by driving into the circle: it turns green and becomes the checkpoint. A flag that the player drives past stays gray and is lost, because nobody crosses a barrier backward.

Flags become sparser with distance:

| Sectors | Flags |
|---|---|
| 1–8 | Every sector |
| 9–16 | Every 2nd sector |
| 17–24 | Every 3rd sector |
| 25 and beyond | Every 4th sector |

The count starts over in each row. After sector 8, the flags are therefore in sectors 10, 12, 14, 16, 19, 22, 28, 32 and every fourth one from there.

### Multiplayer

Multiplayer has no lives and no checkpoints. A destroyed player tank becomes a wreck, and its player watches a living teammate until repaired. The run ends when all players are wrecked at the same time. Repairs are unlimited but slow, which makes them safe between fights and risky during one (see [Repair](#repair)). A wreck is drawn as the outline of its tank in dashes.

Enemy groups grow with the number of players (see [Progression](#progression)). The map doesn't change.

### Modes

| Mode or option | Effect | Unlocked by |
|---|---|---|
| Normal | The rules of this document | Available from the start |
| Chosen seed | The players type a seed instead of getting a random one. | Reaching sector 10 |
| Free mode | Each player redistributes the 58 points of the player tank before the run. | Reaching sector 20 |
| Hard mode | Sector budgets start at 40 and are capped at 130, all software tiers are available from sector 1, and experience is that of 20 sectors further. | Reaching sector 30 |
| Impossible mode | Later. Enemies adapt to the players during the run (see [Impossible mode](#impossible-mode)). | Reaching sector 20 in hard mode |

Normal, hard and impossible are the three difficulties. Chosen seed and free mode are options that combine with any of them.

### Playtest

A playtest is a way to test the game faster, not a mode for players. It starts with the `--playtest` option on the command line and lets the solo player cheat from the menu in two ways:

- **Infinite lives.** The tank comes back as often as it is destroyed. This is on at the start and can be turned off.
- **Skipping to a sector.** The player picks any sector and comes back at the end of the one before it, as after capturing a flag there.

A playtest can also choose its seed without unlocking that option: `--seed` followed by a seed starts every run on it, which brings back a situation that a player reported.

A playtest says on screen that it is one. It sets no record and earns no achievement.

## Controls

| Action | Keyboard and mouse | Notes |
|---|---|---|
| Move | WASD or arrow keys | Eight directions, relative to the screen |
| Aim | Mouse position | The turret follows the cursor. |
| Shoot | Left click | Hold to keep firing. |
| Place a mine | Right click | |
| Shield | Space | |
| Repair | Hold Left Shift next to a wreck | Multiplayer only |
| Rotate the view | Q and E | A quarter turn each way |
| Menu | Esc | Pauses the game in solo only |

All bindings can be changed in the settings. Controller support comes later.

A tank drives along its hull axis only. The hull turns toward the input direction at its turn rate and takes the shorter way, driving in reverse when the target direction is behind it. It pivots in place while it is more than 45° off, and drives while turning otherwise. Tanks reach top speed in about 0.15 s. The player's turret follows the cursor at 720°/s, which feels instant for small corrections.

## Combat

### Damage

Any hit destroys any tank. There are no hit points and no armor: an active shield is the only protection. Destroyed enemies vanish. A destroyed player tank leaves a wreck that blocks movement and absorbs bullets. Tanks are solid to each other, and ramming does no damage.

The game tells a player what destroyed their tank: the tank that fired the bullet, laid the mine or set off the crate.

### Bullets

A bullet flies in a straight line at the speed set by its tank's Muzzle stat. It reflects off walls and blocks as many times as the Ricochet stat allows, and the next impact destroys it. A bullet has no range limit. It starts at the turret and travels down the barrel, so a tank pressed against a wall can't shoot through it: the bullet hits the wall at once.

Firing takes one round from the magazine. Rounds reload one at a time, automatically, each taking the Loader time, and firing doesn't interrupt reloading. Every gun also needs 0.25 s between two shots. This cooldown is the same for all tanks and isn't a stat. A player's tank can't shoot during the first 0.5 s after it appears, at the start of a run or after a respawn, so that the click that started the run fires nothing.

### What hits what

| Hits → | Player tank | Enemy tank | Player bullet | Enemy bullet | Mine or crate | Block or wall | Barrier |
|---|---|---|---|---|---|---|---|
| **Player bullet** | Destroys it | Destroys it | Both destroyed | Both destroyed | Detonates it | Bounces | Absorbed |
| **Enemy bullet** | Destroys it | Passes through | Both destroyed | Both destroyed | Detonates it | Bounces | Absorbed |
| **Blast** | Destroys it | Destroys it | Destroys it | Destroys it | Detonates it | Destroys blocks, stopped by walls | Stopped |

Three rules sum up the bullets:

1. A player's bullet destroys any tank: enemies, teammates and, after a ricochet, the player who fired it.
2. An enemy's bullet destroys only player tanks and passes through enemy tanks.
3. Any two bullets that touch destroy each other, whoever fired them.

Piercing is the exception to the third rule. A tank with the Piercing stat fires piercing bullets. A piercing bullet destroys the first normal bullet it touches and flies on, but it is a normal bullet from then on, so it takes two bullets to stop it. Two piercing bullets destroy each other.

Blasts are neutral. They destroy every unshielded tank in range, whoever caused them. A tank is in range when its center is within the radius of the blast and no wall or barrier stands between them.

### Mines

A tank drops a mine at its own position, at most one every 0.5 s, and can have as many armed mines as its Mines stat allows. A mine arms after 1 s. It then detonates when an opposing tank comes within 2 m (after a 0.6 s warning) or when its 10 s fuse runs out. A bullet or a blast detonates it at any time, even before it is armed. A shot passes over a mine that lies under the barrel of the tank that fires it. The blast has a radius of 3 m. An armed mine shows the circle within which an opposing tank sets it off. A triggered mine shows the circle that its blast will reach. Blasts are the only way to remove destructible blocks.

### Shield

The shield is a bubble around the tank that lasts 0.5 s. It destroys every bullet that touches it, piercing or not, and protects against blasts. The tank keeps moving and shooting while the bubble is up: its own shots leave the bubble, but a ricochet doesn't come back through it. When it ends, the tank overloads: it can't move, shoot, place mines or shield for the stun time. It also can't shield again before the cooldown ends. Higher Shield levels shorten both.

The overload is the drawback. A shield saves a tank from one threat and leaves it helpless against the next. It beats single fast shots, and it loses to bursts and to patient opponents who bait the shield and fire again.

### Repair

A player repairs a wrecked teammate by holding the repair key within 3 m of the wreck for 4 s. The repairer can't shoot, place mines or shield while repairing, and all progress is lost if the repair is interrupted. The repaired tank returns with a full magazine and 1 s of protection.

### Constants

| Constant | Value |
|---|---|
| Tank | 2.0 m long, 1.6 m wide, circular collider of radius 0.9 m, the same for all tanks |
| Bullet radius | 0.225 m |
| Fire cooldown | 0.25 s |
| Time without shooting after a player's tank appears | 0.5 s |
| Mine arming time and fuse | 1 s and 10 s |
| Time between two mines of a tank | 0.5 s |
| Mine trigger | Opposing tank within 2 m, then 0.6 s |
| Blast radius of mines and crates | 3 m |
| Shield duration | 0.5 s |
| Flag | 4 m in front of the barrier, captured within 2 m |
| Repair | 4 s within 3 m |

## Tanks

Every tank, player or enemy, is described by a genome: one level per stat. Levels are bought with the tank's budget of points. Players and enemies follow the same rules with three differences. Player tanks have a fixed appearance, a fixed genome outside free mode, and no software stats, because the player is the brain.

### Hardware stats

Level 0 of each stat is free. Costs are cumulative: each number is the total price of that level.

| Stat | Sets | Value at level 0, 1, 2, … | Cost at level 0, 1, 2, … |
|---|---|---|---|
| Engine | Top speed in m/s | 0, 2, 3, 4, 5, 6, 7, 8 | 0, 2, 4, 6, 8, 11, 14, 17 |
| Tracks | Hull turn rate in °/s | 90, 135, 180, 225, 270, 315, 360 | 0, 1, 2, 3, 4, 5, 6 |
| Turret | Turret turn rate in °/s (enemies only) | 45, 60, 90, 120, 180, 270, 360 | 0, 1, 2, 3, 5, 7, 10 |
| Muzzle | Bullet speed in m/s | 6, 7.5, 9, 12, 14, 16, 18, 20 | 0, 1, 2, 4, 6, 8, 11, 14 |
| Magazine | Rounds held | 1, 2, 3, 4, 5, 6 | 0, 3, 6, 9, 12, 15 |
| Loader | Seconds to reload one round | 4.0, 3.2, 2.5, 2.0, 1.6, 1.25, 1.0, 0.8 | 0, 1, 2, 4, 6, 9, 12, 16 |
| Ricochet | Bounces per bullet | 0, 1, 2, 3 | 0, 4, 10, 18 |
| Mines | Mines armed at once | 0, 1, 2, 3, 4 | 0, 4, 6, 8, 10 |
| Shield | Stun / cooldown in s | none, 1.5 / 10, 1.2 / 9, 0.9 / 8, 0.6 / 7, 0.3 / 6 | 0, 6, 9, 12, 15, 18 |
| Piercing | Bullets that each of its bullets pierces | 0, 1 | 0, 4 |

Every stat stops at its highest level. These limits keep the game sound: a bullet, for instance, never gets fast enough to pass through a wall. Buying every hardware stat at its highest level costs 128 points.

### Software stats

Software is the enemy's controller. Its price is a share of the tank's total budget, rounded up. Each tier includes the tiers below it. A tier is a set of hand-written behavior, so more tiers can be added when these aren't enough.

| Stat | Tier | Share of budget | Adds |
|---|---|---|---|
| Brain | 0, Sentry | 0% | Fires when a player is in its line of fire. Wanders if it has an engine. |
| Brain | 1, Grunt | 8% | Keeps its distance, avoids walls and mines, uses mines and shield by reflex. |
| Brain | 2, Soldier | 16% | Dodges bullets flying straight at it, reloads behind cover, spreads out from its allies. |
| Brain | 3, Veteran | 24% | Dodges ricochets, shoots down incoming bullets, mines the players' routes, shields only when it can't dodge. |
| Brain | 4, Ace | 32% | Coordinates with allies for crossfire, baits shots, retreats when alone. |
| Ballistics | 0 | 0% | Aims at where the target is. |
| Ballistics | 1 | 5% | Leads moving targets. |
| Ballistics | 2 | 10% | Finds single-bounce shots. |
| Ballistics | 3 | 15% | Finds double-bounce shots, and shoots mines and crates near players. |

### Why prices differ

A price should reflect how much more dangerous a level makes a tank, and not every stat grows the same way.

| Pricing | Stats | Reason |
|---|---|---|
| Flat per level | Tracks, Magazine | Each level is worth about as much as the last. |
| Rising per level | Engine above level 4, Turret, Muzzle, Loader, Ricochet | High levels cross thresholds: faster than a player tank, too fast to dodge, or enough bounces to fill a room. |
| Entry fee | Mines, Shield, Piercing | The first level buys a new ability. Later levels, where there are any, only improve it. |
| Share of budget | Brain, Ballistics | Software multiplies the value of the hardware it drives: the same brain is worth more in a better tank. |
| Prerequisite | Tracks, Mines, Shield, Ballistics 2 and 3 | The stat is worthless alone, so the generator never buys it alone. |

The prerequisites are: Tracks need Engine 1. On enemies, Mines and Shield need Brain 1. Ballistics 2 needs Ricochet 1, and Ballistics 3 needs Ricochet 2.

Prices are right when two enemy tanks of equal budget are about equally dangerous. The check is a headless build that runs many fights between random genomes and a reference bot, then compares the results per budget.

### Player tank

The player tank spends 58 points. Free mode lets each player redistribute them over every hardware stat except Turret.

| Stat | Level | Value | Cost |
|---|---|---|---|
| Engine | 4 | 5 m/s | 8 |
| Tracks | 5 | 315°/s | 5 |
| Muzzle | 2 | 9 m/s | 2 |
| Magazine | 4 | 5 rounds | 12 |
| Loader | 6 | 1.0 s per round | 12 |
| Ricochet | 1 | 1 bounce | 4 |
| Mines | 2 | 2 mines | 6 |
| Shield | 2 | 1.2 s stun, 9 s cooldown | 9 |
| **Total** | | | **58** |

### Enemy generation

Each enemy is generated from the seed, its sector and its index in the group:

1. Draw the tank's budget, uniformly between 60% and 100% of the sector budget.
2. Draw its software tiers within the sector's tier caps, and pay their shares.
3. Draw a specialization: two or three favorite hardware stats that will receive most of the points.
4. Spend the remaining points one level at a time, at random with the favorites weighted up, respecting prerequisites, until nothing affordable is left.

Specialization matters more than balance. A tank with a few strong stats has a recognizable shape and asks the players a clear question.

### Reference builds

The price list can express the enemy roster of Tanks!, in roughly the order in which that game introduces it. This is a sanity check of the prices, not a list of tanks to ship. The roster is taken from the [StrategyWiki page on Tanks!](https://strategywiki.org/wiki/Wii_Play/Tanks!).

| Build | Key stats | Budget | Possible from sector |
|---|---|---|---|
| Sentry (brown) | Stationary, 9 m/s bullets with 1 bounce, no brain | 10 | 1 |
| Rover (gray) | 2 m/s, same gun, brain 1 | 15 | 2 |
| Sniper (teal) | 2 m/s, 18 m/s bullets without bounce, brain 2, ballistics 1 | 25 | 6 |
| Burster (pink) | 4 m/s, 3 rounds, brain 2 | 33 | 6 |
| Miner (yellow) | 6 m/s, 4 mines, brain 1 | 37 | 7 |
| Banker (green) | Stationary, 18 m/s bullets with 2 bounces, ballistics 3 | 44 | 13 |
| Raider (purple) | 6 m/s, 5 rounds, 2 mines, brain 3 | 70 | 18 |
| Hunter (black) | 8 m/s, 18 m/s bullets, 2 mines, brain 4 | 89 | 32 |

### Appearance

An enemy's model is assembled from its genome, so players can read a tank before fighting it. Every stat has its own visual channel, and none relies on color alone.

| Stat | Visual channel |
|---|---|
| Engine | Track size. Stationary tanks sit on a fixed base. |
| Tracks | Hull length. Agile tanks are short. |
| Turret | Turret size. Small turrets turn fast. |
| Muzzle | Barrel length |
| Magazine | Ammo drum behind the turret, one segment per round |
| Loader | Vents on the turret sides, more with level |
| Ricochet | Rings around the muzzle, one per bounce |
| Mines | Mine rack at the rear, one mine per slot |
| Shield | White disk under the turret, of which only the edge shows. The edge is a little wider with level. |
| Piercing | Pointed tip on the barrel. Its bullets have a pointed tip too, until they have pierced. |
| Brain | Antennas, one per tier |
| Ballistics | Sight on the turret, one lens per tier |
| Total budget | Color intensity. Weak tanks are pale, strong ones saturated. |
| Largest spending group | Hue: one for mobility, one for firepower, one for mines and shield, one for software |

The lenses of the sight and the pointed tip of the barrel are a pale tint of the tank's own hue.

Player tanks have fixed models in reserved colors that no enemy uses. All tanks have the same size.

## Enemy brains

A brain is the controller of one enemy tank. It observes the fight (its own state, the tanks, bullets and mines around it, the nearby walls) and issues the same commands a player has: move, aim, shoot, place a mine, shield. Brains don't cheat: their tanks obey the same hardware limits and the same rules as player tanks. Enemies in an active sector always know where the players are. What they do with that knowledge depends on their software.

Two things make a brain better. Its tiers, bought with points, decide what it is able to do. Its experience, which depends only on the sector, decides how well it does it: experience shortens the reaction delay from 0.50 s to 0.12 s and shrinks the aiming error from 8° to 1°.

Brains are hand-written, one set of behavior per tier. In normal and hard mode they never learn during play, because that would break the promise that a seed always gives the same challenge. Brains that learn belong to [impossible mode](#impossible-mode).

## Progression

Difficulty depends only on the sector number $n$, the mode and the number of players $P$. Five things grow with $n$: the enemies' budget, their number, their software tiers, their experience and the harshness of the terrain. In solo, flags also become sparser.

The sector budget is the most points an enemy of sector $n$ can have:

$$B(n) = B_{\max} - (B_{\max} - B_0)\, e^{-(n-1)/15}$$

with $B_0 = 10$ and $B_{\max} = 100$ in normal mode. It rises fast at first and flattens toward the cap. The cap stays below the 128 points of a fully equipped tank, so even late enemies must specialize. Every stat also stops at its own highest level.

The number of enemies in a sector is $N(n) = \min(6,\ 1 + \lfloor n/3 \rfloor)$ in solo and $\lceil N(n)\,(1 + 0.5\,(P - 1)) \rceil$ with $P$ players. Arenas hold 1.5 times as many.

Experience is $E(n) = 1 - e^{-(n-1)/20}$, where 0 is a novice and 1 a flawless brain. It keeps closing in on flawless play without ever reaching it. The highest brain tier allowed is $\min(4,\ 1 + \lfloor (n-1)/5 \rfloor)$ and the highest ballistics tier is $\min(3,\ \lfloor (n+2)/5 \rfloor)$.

| Sector | Sector budget | Enemies in solo | Highest brain tier | Highest ballistics tier | Experience |
|---|---|---|---|---|---|
| 1 | 10 | 1 | 1 | 0 | 0.00 |
| 3 | 21 | 2 | 1 | 1 | 0.10 |
| 5 | 31 | 2 | 1 | 1 | 0.18 |
| 10 | 51 | 4 | 2 | 2 | 0.36 |
| 15 | 65 | 6 | 3 | 3 | 0.50 |
| 20 | 75 | 6 | 4 | 3 | 0.61 |
| 30 | 87 | 6 | 4 | 3 | 0.77 |
| 50 | 97 | 6 | 4 | 3 | 0.91 |

Every lever has a limit, so difficulty flattens after about sector 50. From there, a run is a test of endurance at full difficulty.

The terrain gets harsher along four levers:

| Lever | Sector 1 | Sector 30 and beyond |
|---|---|---|
| Corridor width | 14–20 m | 8–14 m |
| Cover blocks | Plentiful | Sparse |
| Explosive crates per sector | 0–1 | 3–6 |
| Angled walls | Rare | Common |

## Impossible mode

Impossible mode comes later and starts as an experiment. In this mode, enemies adapt to the players during the run. A seed still fixes the map, but it no longer fixes the enemies, which makes this the one mode that sets the third pillar aside.

Enemies are bred. The game scores every enemy on how it did against the players: destroying a player counts most, then forcing a shield, then surviving. Each new group descends from the best-scoring tanks so far, with their genomes recombined and slightly mutated within the sector budget. A team that struggles against fast bullets therefore meets more of them.

Brains that learn are a second step. An enemy's brain becomes a neural network whose size is a stat, and the networks are bred along with the genomes. They start from weights trained offline, never from random ones, so that enemies play decently from the first sector.

## Map

The map is an endless corridor that winds its way north. It usually heads north, occasionally runs east or west, and rarely doubles back south before turning north again. Its width varies, and it sometimes opens into an arena. Nothing exists outside the corridor: beyond the walls is the void.

| Sector type | Frequency | Size | Notes |
|---|---|---|---|
| Corridor | Most sectors | 40–80 m long, 8–20 m wide | Straight or bending |
| Switchback | Rare | Same as a corridor | Runs south, for one sector at most |
| Arena | Every 5 to 8 sectors | 32–36 m long, up to 36 m wide | Holds 1.5 times the enemies |

| Element | Tanks | Bullets | Blasts |
|---|---|---|---|
| Wall | Blocked | Bounce | Stopped |
| Destructible block, 2 × 2 m | Blocked | Bounce | Destroyed |
| Explosive crate, 2 × 2 m | Blocked | Detonate it | Detonate it |

The generator guarantees five things:

1. **A way through.** Every sector has a path from its entry to its barrier that is at least 3 m wide, without destroying anything.
2. **No overlap.** Parts of the corridor that aren't neighbors stay at least 6 m apart.
3. **Net progress.** The corridor never runs more than 60 m south of the northernmost point it has reached.
4. **A fair start.** Enemies start at least 15 m from the barrier the players come through, and at least 3 m from the barrier that ends their sector.
5. **Exact geometry.** Every wall vertex lies on a 1 m lattice. This gives walls at 0°, 45° and 90° plus the slopes 1:2 and 2:1, and it keeps generation identical on every platform.

## Multiplayer

Every game, solo included, is run by a server that has the final say on everything that happens. In solo and hosted games, the server runs inside a player's client.

| Session | How it works |
|---|---|
| Solo | The client runs its own server. Nothing goes over the network. |
| Hosted game | One player's client hosts up to three others. The game is announced on the local network. It can also be joined by address from the internet if the host's UDP port is reachable. An optional password protects it. |
| Official server | Later. Dedicated servers run by the project, which also remove the need to open a port. |

Players in a browser can play solo and join games. They can't host, because a browser can't accept connections.

Players gather in a lobby, where the host picks the mode and the seed, and the run starts for everyone at once. Nobody can join a run in progress. A player who disconnects loses their tank, and later sectors are generated for the remaining number of players. If the host leaves, the run ends. All players must run the same game version.

The game must stay playable up to a round trip of 120 ms. A player's own tank responds to input immediately, and the server confirms it afterward.

## Determinism

Players get a random seed, or type their own once that option is unlocked. A seed is a short text. Together with the game version, it identifies a level.

| What | Depends on | Guarantee |
|---|---|---|
| Map: walls, blocks, crates, barriers | Seed, version | Identical on every platform |
| Enemies: number, genomes, positions | Seed, version, mode, number of players | Identical on every platform |
| Enemy behavior | All of the above and the players' actions | Not guaranteed to repeat |
| Effects: particles, camera shake | Nothing | Random |

Solo and co-op players on the same seed therefore drive through the same map, on a desktop or in a browser. A new game version may change what a seed generates, so seeds and records always show their version.

Impossible mode keeps the first row of this table and gives up the second: its enemies depend on how the run has gone.

## Presentation

### Camera

The camera looks down at the ground at a steep angle. It shows about 30 m of ground from the bottom to the top of the screen, and more from side to side. Each player has their own view. It follows that player's tank smoothly and shifts a little toward the cursor, so players see further where they aim. Markers at the screen edge point to teammates who are out of view.

Once a sector has no enemy left and none of their bullets in flight, the view of a player whose tank comes close to its barrier runs up to 10 m ahead of the tank, past the barrier, so that the next group can be studied before crossing. It comes back to the tank once the tank is through.

North is at the top of the screen by default. A player can rotate their own view a quarter turn at a time, for instance to put north on the right and see further ahead on a wide screen. The view never rotates on its own, and the movement keys always follow the screen.

### Visual style

The game is drawn as a technical drawing on a dark sheet, and nothing is lit: a solid shows its height by its darker sides. The floor carries a grid with a line every meter and a heavier one every 5 m. Walls are bands between two pale lines, destructible blocks are hatched and explosive crates are crossed in red. Tanks are assembled from parts by the game, not modeled by hand: the body is in the tank's color, and the other parts are dark and edged in pale lines. Bullets are the brightest objects on screen, in one color for the players' bullets and another for the enemies', and each leaves a short line behind it and a patch of light on the floor. A bullet is an elongated shell with one dark stripe for each bounce it has left: it loses a stripe at every bounce, and a plain bullet won't bounce again. The void outside the corridor is empty and dark.

A line in dashes stands for what isn't there. A dormant enemy is the outline of its tank in dashes, flat on the floor and in its color, with every part that tells a stat. A bullet that is destroyed leaves a short burst of strokes in its color.

A fight leaves marks on the floor: a scalloped outline where a blast went off, a square in dashes where it took a block away, and a cross where a tank was destroyed. They last as long as their sector is loaded.

### Interface

The interface is the margin of the drawing: ruled panels with square corners, labels in capitals, numbers in a monospaced typeface, and one accent color per player.

| Screen | Content |
|---|---|
| Main menu | Solo, host, join, achievements, settings, quit |
| Lobby | Players, mode, seed, start |
| Heads-up display | Rounds and reload progress, mines left, shield state, sector number, lives (solo), seed and version, a scale and a north arrow, a ruler along the edge of the screen that gives the distance to the barrier, teammates' state (multiplayer). Rounds, reload and mines are also on an arc under the player's own tank. |
| Menu | Resume, settings, quit, and the seed with its version |
| Game over | Sectors cleared, time, kills, seed and version, the tank that ended the run with the list of its parts, retry, new run |
| Settings | Controls, video, audio, accessibility |

The game opens in full screen the first time. After that, it opens the way the player left it.

### Audio

Sound carries information the eyes can miss: a shot fired off screen, a ricochet, a mine about to blow, a shield overloading. A basic set of such cues arrives with the solo loop (M4), in time for the first real playtest. Music and the full sound pass come later.

### Accessibility

Color is never the only carrier of information. Camera shake can be turned off, the interface can be scaled, and all controls can be rebound.

## Achievements and persistence

Achievements unlock modes, and their conditions are listed under [Modes](#modes). An unlocked mode is another way to play, not an advantage: nothing a player unlocks makes a run easier. An achievement earned by a team is awarded to each member. Runs on a chosen seed don't count toward achievements, so a seed can't be practiced to unlock them.

The game stores settings, bindings, unlocks and records on the player's machine, or in the browser's storage on the web. Records keep the best sector reached per mode and number of players, with the seed and the version.

## Technical requirements

| Topic | Requirement |
|---|---|
| Simulation | Fixed 60 ticks per second. Gameplay is 2D on the ground plane, and only the rendering is 3D. |
| Authority | The server decides. Solo and hosted games embed it in the client. |
| Network | Up to 4 players, playable up to a 120 ms round trip. Desktop clients connect over UDP, browsers over WebTransport. |
| Generation | Fixed-width integer arithmetic and lookup tables only, with a separate random stream per purpose |
| Performance | 60 frames per second at 1080p on integrated graphics |
| Platforms | Linux and Windows on x86-64, and current desktop browsers through WebAssembly |
| Restart | Under 3 s from game over to a new run |

## Roadmap

Each milestone is playable and answers a question. From M1 on, the game runs as a fixed-tick simulation owned by a server, even in solo, so that co-op adds a transport and not a rewrite.

| Milestone | Content | Question it answers |
|---|---|---|
| M0, Foundation | Working workspace, first commit, passing checks | None |
| M1, Toy | One fixed room, a player tank, bullets, ricochets, piercing, mines, shield, one hand-written enemy | Do the speeds, the one-hit kills and the shield feel right? Is piercing fair to face? |
| M2, Corridor | Seeded sectors, barriers, loading and unloading, view rotation, generation tests | Is an endless corridor interesting to drive through? Which way do players turn the view? |
| M3, Genome | Stats, budgets, enemy generation, generated models, brain tiers 0 to 2 | Can players read a tank at a glance? Does equal budget mean equal danger? |
| M4, Solo loop | Lives, flags, progression curves, heads-up display, main menu, settings, game over, records, basic sound cues, first web build | Is a 20-minute run fun? This is the first real playtest. |
| M5, Co-op | Lobby, hosting, joining, local discovery, repair, enemy scaling, joining from a browser | Is it fun with friends at 100 ms of latency? |
| M6, Modes | Achievements, chosen seed, free mode, hard mode, brain tiers 3 and 4 | Do the unlocks give reasons to come back? |
| M7, Impossible mode | An experiment: enemies bred from the tanks that did best against the players | Do players feel the enemies adapting to them, and is it fun? |

Later, in no fixed order: controller support, local co-op on one screen (split screen, or one camera that follows all players), music and the full sound pass, interface polish and more options, a performance pass, the official server, new bullet types (splitting, seeking, exploding), and brains that learn (see [Impossible mode](#impossible-mode)).

## Open questions

- How do internet games connect for hosts who can't open a port, before the official server exists?
- Can a browser join a game hosted on a player's machine, which needs a certificate the browser accepts, or only games on the official server?
- In impossible mode, how exactly are enemies scored, and can brains learn fast enough within one run to matter?
- Which exact colors do players, enemies and bullets use?
- How is the game distributed, and is the name free to use?
