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
2. **Readable danger.** Everything an enemy can do is visible on its model, and every bullet can be dodged. A group is read through its barrier before the fight starts. An arena is the one exception: its enemies show only when they appear, each after a warning.
3. **Same seed, same challenge.** A seed always produces the same map and the same enemies, so it can be learned, shared and raced like a level.
4. **Co-op first, solo complete.** Every rule works for one to four players.
5. **Instant retry.** Getting from game over back to driving takes a few seconds.

### Non-goals

Tanku has no player-versus-player mode, no loot, no upgrades during a run, no story and no meta-progression. What players unlock adds ways to play and never makes the ways they had easier.

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
| Arena | A wide sector whose enemies come in waves |
| Wave | One part of the group of an arena. It appears when the part before it is destroyed. |
| Checkpoint | The last sector cleared. A solo player respawns in front of its barrier. |
| Life | What a solo player loses with their tank. A cleared arena leaves one to pick up. |
| Wreck | A destroyed player tank, which a teammate can repair |
| Budget | The number of points a tank has to spend on stats |
| Sector budget | The measure of an enemy's budget in a given sector. No tank has more, except the leader of its group. |
| Group | The enemies of one sector, which share one budget unevenly |
| Leader | The tank with the largest share of its group's budget: up to 1.5 times the sector budget |
| Class | What an enemy favors when it spends its points. Its hue tells it. |
| Genome | A tank's full list of stat levels |
| Standard tank | The tank that every player has outside free mode |
| Garage | The screen where a player spends the points of their tank, in free mode |
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
4. The enemies of a sector are dormant until the first player crosses into it. Dormant enemies are visible through the barrier. Until then, the sector is drawn darker than the rest of the corridor, and its enemies are outlines in dashes. An arena is the exception (see [Arenas](#arenas)).

These rules let the players study the next group, pick the moment to engage and enter together. They also rule out both driving past enemies and fighting from a doorway. When the last living player has crossed, any wreck left behind is towed across and the sectors further back are unloaded. When two or fewer enemies remain in a sector, markers at the screen edge point to them. Each end of a barrier bears a tag with the numbers of the two sectors it parts. A caption says whether the barrier is locked or open, and whether an arena lies behind it.

### Arenas

An arena is a wide and short sector that holds 1.5 times the enemies of a corridor (see [Map](#map)). All of them at once in so little room are more than the players can read, so an arena sends its group in waves and shows none of it beforehand.

Seen through its barrier, an arena is empty. Two circles in dashes on its floor and the caption of the barrier tell what it is. Its group comes in three waves of the same size, give or take one tank:

1. The first wave is announced 2 s after the first player has crossed, and each of the others when the last tank of the wave before it is destroyed. The announcement is a message with the number of the wave, and a sound.
2. The place of each tank of the wave is then marked on the floor for 1 s, by a circle that closes in on it. The mark is short on purpose: a longer one lets the players aim at the place and destroy the tank as it appears. A mark waits for as long as a player's tank is within 4 m of the place.
3. The tank appears on its mark, awake.

The barrier of an arena unlocks like any other, once the last tank of its last wave is destroyed. In an arena, the markers at the screen edge point to the last two tanks of the wave that is there.

The danger of an arena can't be read before the fight: that is its point, and the one exception to the second pillar. What the players can't study, they are warned of, one wave at a time.

### Solo

The solo player has three lives. When the tank is destroyed, the player loses a life and respawns 1.5 s later at the checkpoint: in front of the barrier of the last sector cleared, or in the start bay before any sector is cleared. Losing the last life ends the run.

Every sector that is cleared is a checkpoint, and the sector ahead of it keeps what the player did there:

- The enemies that were destroyed stay destroyed, and the kills stay counted.
- The enemies that are left go back to where they started, dormant, with full magazines.
- Every bullet and every mine is removed.
- The blocks that were destroyed and the crates that went off stay gone.

The player then crosses the barrier again and fights what is left of the group. An arena sends again what was left of the wave that was on, after its 2 s and its marks, and then the waves that had not come.

An arena that is cleared leaves a life on its floor, 4 m in front of the middle of its barrier. A player who has lost a life takes it by driving within 1.5 m of it. Nobody has more than three lives: a player who has all three can't take it, and sees it in dashes. A life that is left on the floor is lost when the player crosses the barrier, because nobody crosses a barrier backward.

### Multiplayer

Multiplayer has no lives and no checkpoints, and an arena leaves nothing on its floor. A destroyed player tank becomes a wreck, and its player watches a living teammate until repaired. The run ends when all players are wrecked at the same time. Repairs are unlimited and take 2 s next to the wreck, which makes them safe between fights and risky during one (see [Repair](#repair)). A wreck is drawn as the outline of its tank in dashes.

Enemy groups grow with the number of players (see [Progression](#progression)). The map doesn't change.

### Modes

| Mode or option | Effect | Unlocked in solo by |
|---|---|---|
| Normal | The rules of this document | Available from the start |
| Chosen seed | The solo player, or the host of a game, types the seed of the run instead of getting a random one. | Reaching sector 10 |
| Free mode | Each player spends the 59 points of their tank as they like before the run (see [Garage](#garage)). | Reaching sector 20 |
| Hard mode | Sector budgets start at 40 and are capped at 130, all software tiers are available from sector 1, and experience and the cap on Muzzle are those of 20 sectors further. | Reaching sector 30 |
| Impossible mode | Later. Enemies adapt to the players during the run (see [Impossible mode](#impossible-mode)). | Reaching sector 20 in hard mode |

Normal, hard and impossible are the three difficulties. Chosen seed and free mode are options that combine with any of them.

The locks are solo's. A solo player sets the difficulty and the options of a run on the solo screen, where each one that is locked says what unlocks it. The host of a game of several players sets them in the lobby, and has all of them from the start (see [Lobby](#lobby)).

### Playtest

A playtest is a way to test the game faster, not a mode for players. It starts with the `--playtest` option on the command line and lets the solo player cheat from the menu in two ways:

- **Infinite lives.** The tank comes back as often as it is destroyed. This is on at the start and can be turned off.
- **Skipping to a sector.** The player picks any sector and comes back in front of the barrier of the one before it, as if that sector had just been cleared.

A playtest can also choose its seed without unlocking that option: `--seed` followed by a seed starts every run on it, which brings back a situation that a player reported.

A playtest says on screen that it is one. It sets no record and earns no achievement. A playtest can host a game: the cheats are the host's alone, and the run sets no record for anyone in it.

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

Any hit destroys any tank. There are no hit points and no armor: an active shield is the only protection. Destroyed enemies vanish. A destroyed player tank leaves a wreck, which is in nobody's way: tanks drive over it and bullets fly over it. Tanks that run are solid to each other, and ramming does no damage.

The game tells a player what destroyed their tank: the tank that fired the bullet, laid the mine or set off the crate.

### Bullets

A bullet flies in a straight line at the speed set by its tank's Muzzle stat. It reflects off walls and blocks as many times as the Ricochet stat allows, and the next impact destroys it. A bullet has no range limit. It starts at the turret and travels down the barrel, so a tank pressed against a wall can't shoot through it. A bullet doesn't bounce inside the barrel: it ends on the wall, the block or the barrier that it meets there, and a crate that it meets there goes off. A bounce so close would bring the bullet back before the tank could move out of its way. The round is spent either way.

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

A tank drops a mine at its own position, at most one every 0.5 s, and can have as many armed mines as its Mines stat allows. A mine arms after 1 s. It then detonates when an opposing tank comes within 2 m or when its 10 s fuse runs out, and it warns for 0.6 s first either way: the last 0.6 s of the fuse are that warning. A bullet or a blast detonates it at any time, without a warning, even before it is armed. A shot passes over a mine that lies under the barrel of the tank that fires it. The blast has a radius of 3 m. An armed mine shows the circle within which an opposing tank sets it off. A mine that warns shows the circle that its blast will reach instead. Blasts are the only way to remove destructible blocks.

### Shield

The shield is a bubble around the tank that lasts 0.5 s. It destroys every bullet that touches it, piercing or not, and protects against blasts. The tank keeps moving and shooting while the bubble is up: its own shots leave the bubble, but a ricochet doesn't come back through it. Once the bubble is gone, the tank can't shield again before the cooldown ends. Higher Shield levels shorten the cooldown.

The cooldown is the only drawback. A shield saves a tank from one threat, and for several seconds it can't save it from the next. It beats single shots, and it loses to bursts that outlast the bubble and to patient opponents who bait the shield and fire again.

### Repair

A player repairs a wrecked teammate by holding the repair key within 3 m of the wreck for 2 s. The repairer keeps every other verb: the tank drives within that reach, shoots, places mines and shields as usual. All progress is lost if the repair is interrupted: when the key is let go, when the tank leaves the reach, or when it is destroyed. The repaired tank returns with a full magazine and 1 s of protection, during which bullets end on it and blasts leave it alone.

A tank repairs one wreck at a time. With several wrecks within reach, it works on the nearest one and stays with it, wherever it drives within that reach, until that tank is back or the repair is interrupted. If the key is still held then, it goes on with the nearest wreck that is left, from nothing. Two tanks at the same wreck repair it no faster than one.

Nothing destroys a wreck again, and a wreck is in nobody's way: tanks drive over it, bullets fly over it, it sets off no mine and it stays through any blast. It therefore gives no cover to the tank that repairs it. A tank that comes back under another one pushes it aside over a few ticks, without a jump.

A wreck lies in a circle in dashes and keeps the number of its player over it: a tank whose center is inside the circle can repair it. The circle is inked in from the top as the repair goes on, and starts over when the repair is interrupted. While a teammate's tank is a wreck, a player is told on screen which key to hold, and how long the repair still takes. The player whose tank is the wreck is told when a teammate is at it.

### Constants

| Constant | Value |
|---|---|
| Tank | 2.0 m long, 1.6 m wide, circular collider of radius 0.9 m, the same for all tanks |
| Barrel | Its tip is 1.3 m from the center of the tank |
| Bullet radius | 0.225 m |
| Fire cooldown | 0.25 s |
| Time without shooting after a player's tank appears | 0.5 s |
| Mine arming time and fuse | 1 s and 10 s |
| Time between two mines of a tank | 0.5 s |
| Mine trigger | Opposing tank within 2 m |
| Mine warning | 0.6 s before the blast, after a tank sets the mine off and at the end of its fuse |
| Blast radius of mines and crates | 3 m |
| Shield duration | 0.5 s |
| Lives of a solo player | 3 at the start, and never more |
| Life of an arena | 4 m in front of the barrier, taken within 1.5 m |
| Waves of an arena | 3, the first announced 2 s after the first player has crossed |
| Mark of a tank that is about to appear | 1 s, once no player's tank is within 4 m of the place |
| Repair | 2 s within 3 m |

## Tanks

Every tank, player or enemy, is described by a genome: one level per stat. Levels are bought with the tank's budget of points. Players and enemies follow the same rules with four differences. Player tanks have a color of their own, a fixed genome outside free mode, no software stats, because the player is the brain, and no class.

### Hardware stats

Level 0 of each stat is free. Costs are cumulative: each number is the total price of that level.

| Stat | Sets | Value at level 0, 1, 2, … | Cost at level 0, 1, 2, … |
|---|---|---|---|
| Engine | Top speed in m/s | 0, 2, 3, 4, 5, 6, 7, 8 | 0, 2, 4, 6, 8, 11, 14, 17 |
| Tracks | Hull turn rate in °/s | 90, 135, 180, 225, 270, 315, 360 | 0, 1, 2, 3, 4, 5, 6 |
| Turret | Turret turn rate in °/s (enemies only) | 45, 60, 90, 120, 180, 270, 360 | 0, 1, 2, 3, 5, 7, 10 |
| Muzzle | Bullet speed in m/s | 6, 7.5, 9, 11, 13, 15, 17, 19 | 0, 1, 3, 6, 9, 12, 16, 21 |
| Magazine | Rounds held | 1, 2, 3, 4, 5, 6 | 0, 3, 6, 9, 12, 15 |
| Loader | Seconds to reload one round | 4.0, 3.2, 2.5, 2.0, 1.6, 1.25, 1.0, 0.8 | 0, 1, 2, 4, 6, 9, 12, 16 |
| Ricochet | Bounces per bullet | 0, 1, 2, 3 | 0, 4, 10, 18 |
| Mines | Mines armed at once | 0, 1, 2, 3, 4 | 0, 4, 6, 8, 10 |
| Shield | Cooldown in s | none, 10, 9, 8, 7, 6 | 0, 6, 9, 12, 15, 18 |
| Piercing | Bullets that each of its bullets pierces | 0, 1 | 0, 4 |

Every stat stops at its highest level. These limits keep the game sound: a bullet, for instance, never gets fast enough to pass through a wall. Buying every hardware stat at its highest level costs 135 points. An enemy's Muzzle level is also limited by its sector (see [Progression](#progression)), and its Piercing and Ricochet levels by its class (see [Classes](#classes)).

### Software stats

Software is the enemy's controller. Its price is a share of the tank's total budget, rounded up. Each tier includes the tiers below it. A tier is a set of hand-written behavior, so more tiers can be added when these aren't enough.

| Stat | Tier | Share of budget | Adds |
|---|---|---|---|
| Brain | 0, Sentry | 0% | Fires when a player is in its line of fire. Wanders if it has an engine. |
| Brain | 1, Grunt | 8% | Keeps its distance, avoids walls and mines, uses mines and shield by reflex. |
| Brain | 2, Soldier | 16% | Dodges bullets flying straight at it, reloads behind cover, spreads out from its allies. |
| Brain | 3, Veteran | 24% | Dodges ricochets, shoots down a bullet that it can't dodge, mines the players' routes, shields only when it can't dodge. |
| Brain | 4, Ace | 32% | Dodges shots that are aimed ahead of it, coordinates with allies for crossfire, baits shots, retreats when alone. |
| Ballistics | 0 | 0% | Aims at where the target is. |
| Ballistics | 1 | 5% | Leads moving targets. |
| Ballistics | 2 | 10% | Finds single-bounce shots. |
| Ballistics | 3 | 15% | Finds double-bounce shots, and shoots mines and crates near players. |

Ballistics 3 is kept for the two classes whose bullets bounce twice (see [Classes](#classes)).

### Why prices differ

A price should reflect how much more dangerous a level makes a tank, and not every stat grows the same way.

| Pricing | Stats | Reason |
|---|---|---|
| Flat per level | Tracks, Magazine | Each level is worth about as much as the last. |
| Rising per level | Engine above level 4, Turret, Muzzle, Loader, Ricochet | High levels cross thresholds: faster than a player tank, too fast to dodge, or enough bounces to fill a room. |
| Entry fee | Mines, Shield, Piercing | The first level buys a new ability. Later levels, where there are any, only improve it. |
| Share of budget | Brain, Ballistics | Software multiplies the value of the hardware it drives: the same brain is worth more in a better tank. |
| Prerequisite | Tracks, Mines, Shield, Ballistics 2 and 3 | The stat is worthless alone, so the generator never buys it alone. |
| Cap by sector | Muzzle | A sniper puts its points into its gun, and one enemy in eight is a sniper. The cap decides how fast that gun is in each sector, and the price decides what else such a tank can afford. |
| Kept for a class | Piercing, Ricochet above level 1, Ballistics 3 | Bullets that pierce and bullets that bounce twice are the hardest to read. They stay rare, and the hue of the tank announces them. |

The prerequisites are: Tracks need Engine 1. On enemies, Mines and Shield need Brain 1. Ballistics 2 needs Ricochet 1, and Ballistics 3 needs Ricochet 2.

Prices are right when two enemy tanks of equal budget are about equally dangerous. The check is a headless build that runs many fights between random genomes and a reference bot, then compares the results per budget.

### Player tank

Outside free mode, every player has the same tank: the standard tank, which spends 59 points.

| Stat | Level | Value | Cost |
|---|---|---|---|
| Engine | 4 | 5 m/s | 8 |
| Tracks | 5 | 315°/s | 5 |
| Muzzle | 2 | 9 m/s | 3 |
| Magazine | 4 | 5 rounds | 12 |
| Loader | 6 | 1.0 s per round | 12 |
| Ricochet | 1 | 1 bounce | 4 |
| Mines | 2 | 2 mines | 6 |
| Shield | 2 | 9 s cooldown | 9 |
| **Total** | | | **59** |

### Garage

In free mode, each player spends the 59 points as they like, in the garage, before the run. The garage lists every hardware stat except Turret, each with its level, what that level gives and its price, and the points that are left. Next to them it shows the tank as it will look, and that drawing follows every change at once (see [Appearance](#appearance)).

A tank from the garage follows the rules of every tank, with three of its own:

1. It needs Engine 1 at least. A tank that can't drive would never come within reach of a barrier, and the barrier would stay locked.
2. It has no Turret stat: its turret follows the cursor, as on the standard tank.
3. It can leave points unspent, and it can't spend more than 59.

What holds an enemy back doesn't hold a player. The cap on Muzzle and the stats kept for a class are for enemies only, and Mines and Shield need no brain. A player can have the fastest gun, piercing bullets and three bounces, at their price.

The garage opens on the tank that the player built last, which is the standard tank the first time, and a button brings the standard tank back. In a game of several players, each player has their own garage and their own tank, and the others see that tank in the run as its player built it.

### Classes

An enemy has a class: the stats that it favors when it spends its points. The class gives the tank its hue, so that it is read from afar, before the parts of the model are.

| Class | Favors | Hue |
|---|---|---|
| All-rounder | Nothing: a bit of everything | Gray |
| Sniper | Muzzle and Turret | Teal |
| Scout | Engine and Tracks | Yellow |
| Tactician | Brain and Ballistics | Green |
| Trickster | Ricochet | Purple |
| Lancer | Piercing | Orange |
| Sapper | Mines and Shield | Pink |
| Gunner | Magazine and Loader | Red |

Blue is the players'. Some stats are kept for the classes that are about them:

- Only a lancer has Piercing.
- Only a trickster and a tactician have Ricochet above level 1, and Ballistics 3, which needs it.
- A tactician has the highest Brain and Ballistics tiers that its sector allows.
- A sapper has Brain 1 at least, without which it could use neither its mines nor its shield.

### Enemy generation

The enemies of a sector are generated together, from the seed, the sector and the number of tanks in the group. The group has one budget, which its tanks share unevenly, so that weak tanks stand next to strong ones:

1. Draw the group's budget: between 70% and 90% of the sector budget for each of its tanks.
2. Give every tank 30% of the sector budget. Each tank then draws a weight of 1, 1, 2, 2, 3 or 8, and the rest of the group's budget is shared by weight.
3. The heaviest tank is the leader, which can have up to 1.5 times the sector budget. No other tank has more than the sector budget. What a tank can't take goes to the others.

A tank that is alone in its sector has the whole budget of its group, which is less than the sector budget. In a group of six, the strongest tank has about 2.6 times the points of the weakest.

Each tank then spends its budget, from the seed, its sector and its index in the group:

1. Draw its class, each of the eight with the same chance.
2. Draw its software tiers within the sector's tier caps and what its class allows, and pay their shares. A Ballistics tier whose bounces the tank can't pay for drops until it can.
3. Buy one level of each stat that its class favors.
4. Spend the remaining points one level at a time, at random, respecting prerequisites, the sector's cap on Muzzle and what its class allows, until nothing affordable is left. The favorite stats weigh 20 together, and every other stat weighs 1.

Specialization matters more than balance. A tank with a few strong stats has a recognizable shape and asks the players a clear question. In a group of six, four or five classes stand side by side.

### Reference builds

The price list can express the enemy roster of Tanks!, in roughly the order in which that game introduces it. This is a sanity check of the prices, not a list of tanks to ship. The guns of the Sniper and of the Banker are the fastest that their sectors allow. The names and the colors are those of Tanks!, not those of the classes. The roster is taken from the [StrategyWiki page on Tanks!](https://strategywiki.org/wiki/Wii_Play/Tanks!).

| Build | Key stats | Budget | Possible from sector |
|---|---|---|---|
| Sentry (brown) | Stationary, 9 m/s bullets with 1 bounce, no brain | 10 | 2 |
| Rover (gray) | 2 m/s, same gun, brain 1 | 15 | 3 |
| Sniper (teal) | 2 m/s, 11 m/s bullets without bounce, brain 2, ballistics 1 | 19 | 6 |
| Burster (pink) | 4 m/s, 3 rounds, brain 2 | 34 | 6 |
| Miner (yellow) | 6 m/s, 4 mines, brain 1 | 39 | 5 |
| Banker (green) | Stationary, 13 m/s bullets with 2 bounces, ballistics 3 | 42 | 13 |
| Raider (purple) | 6 m/s, 5 rounds, 2 mines, brain 3 | 72 | 11 |
| Hunter (black) | 8 m/s, 17 m/s bullets, 2 mines, brain 4 | 96 | 19 |

A build is possible in a sector whose caps allow its tiers and its gun, and where a tank of a solo group in a corridor can have as many points. The Miner, the Raider and the Hunter first come as the leader of their group: no other tank can have their budget before sectors 7, 19 and 46. The Banker is a trickster or a tactician, the two classes whose bullets bounce twice.

### Appearance

An enemy's model is assembled from its genome, so players can read a tank before fighting it. Every stat has its own visual channel, and none relies on color alone. The hue tells nothing that the parts don't: a class is the stats that it favors.

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
| Total budget | Color intensity. Weak tanks are pale, strong ones saturated, and even the weakest shows its hue. |
| Class | Hue: one for each class, and gray for an all-rounder (see [Classes](#classes)) |

The lenses of the sight and the pointed tip of the barrel are a pale tint of the tank's own hue.

A player's tank is assembled from its genome like an enemy's. The standard tank therefore always looks the same, and a tank from the garage shows what its player bought. Two things never change on a player's tank: its turret keeps the size of the standard tank's, because Turret isn't a stat of a player's tank, and it is always the same blue, which no enemy uses, however many points it spends. Among several players, a tag over each of their tanks bears the number of its player, from P1 to P4. All tanks have the same size.

## Enemy brains

A brain is the controller of one enemy tank. It observes the fight (its own state, the tanks, bullets and mines around it, the nearby walls) and issues the same commands a player has: move, aim, shoot, place a mine, shield. Brains don't cheat: their tanks obey the same hardware limits and the same rules as player tanks. Enemies in an active sector always know where the players are. What they do with that knowledge depends on their software. An enemy fights the nearest player in its sector whose tank still runs, and keeps that target until another player is clearly nearer.

Two things make a brain better. Its tiers, bought with points, decide what it is able to do. Its experience, which depends only on the sector, decides how well it does it: experience shortens the reaction delay from 0.50 s to 0.12 s and shrinks the aiming error from 8° to 1°.

Brains are hand-written, one set of behavior per tier. In normal and hard mode they never learn during play, because that would break the promise that a seed always gives the same challenge. Brains that learn belong to [impossible mode](#impossible-mode).

## Progression

Difficulty depends only on the sector number $n$, the mode and the number of players $P$. Six things grow with $n$: the enemies' budget, their number, their software tiers, the speed of their fastest bullets, their experience and the harshness of the terrain.

The sector budget is the measure of an enemy's budget in sector $n$. No tank has more, except the leader of its group, which can have up to 1.5 times as much (see [Enemy generation](#enemy-generation)):

$$B(n) = B_{\max} - (B_{\max} - B_0)\, e^{-(n-1)/15}$$

with $B_0 = 10$ and $B_{\max} = 100$ in normal mode. It rises fast at first and flattens toward the cap. The cap stays below the 135 points of a fully equipped tank, so even late enemies must specialize. Only a leader can have more than the cap, and some leaders then have more points than they can spend (see [Open questions](#open-questions)). Every stat also stops at its own highest level.

The number of enemies in a sector is $N(n) = \min(6,\ 1 + \lfloor n/3 \rfloor)$ in solo and $\lceil N(n)\,(1 + 0.5\,(P - 1)) \rceil$ with $P$ players. Arenas hold 1.5 times as many, in three waves (see [Arenas](#arenas)). The group of a sector is set when the sector is loaded, one sector ahead of the players. A sector that is too small for its whole group holds as many enemies as fit. That happens in the smallest arenas: to about one in a thousand with four players, and almost never with fewer.

Experience is $E(n) = 1 - e^{-(n-1)/20}$, where 0 is a novice and 1 a flawless brain. It keeps closing in on flawless play without ever reaching it. The highest brain tier allowed is $\min(4,\ 1 + \lfloor (n-1)/5 \rfloor)$ and the highest ballistics tier is $\min(3,\ \lfloor (n+2)/5 \rfloor)$.

The highest Muzzle level an enemy can have is $\min(7,\ 2 + \lfloor (n+1)/5 \rfloor)$: the level of the player tank in the first three sectors, then one more at sectors 4, 9, 14, 19 and 24, between the steps of the software tiers. The cap is for enemies only.

The cap follows what a player can dodge. A player's tank that keeps moving needs about 0.55 s to react to a shot and move out of its way, which a bullet covers in 5 m at 9 m/s, 7.2 m at 13 m/s and 8.3 m at 15 m/s. Enemies with a brain keep 8 to 14 m away. Until sector 13, a player who keeps moving can therefore dodge every bullet at that distance. From sector 14, the fastest guns beat reaction at close range, and the players need distance, cover or the shield.

| Sector | Sector budget | Most points of a tank in solo | Enemies in solo | Highest brain tier | Highest ballistics tier | Highest Muzzle level | Experience |
|---|---|---|---|---|---|---|---|
| 1 | 10 | 9 | 1 | 1 | 0 | 2 | 0.00 |
| 3 | 21 | 29 | 2 | 1 | 1 | 2 | 0.10 |
| 5 | 31 | 42 | 2 | 1 | 1 | 3 | 0.18 |
| 10 | 51 | 76 | 4 | 2 | 2 | 4 | 0.36 |
| 15 | 65 | 97 | 6 | 3 | 3 | 5 | 0.50 |
| 20 | 75 | 112 | 6 | 4 | 3 | 6 | 0.61 |
| 30 | 87 | 130 | 6 | 4 | 3 | 7 | 0.77 |
| 50 | 97 | 145 | 6 | 4 | 3 | 7 | 0.91 |

The most points of a tank are those of the leader of a group in a corridor. The tank of sector 1 is alone and has no group to lead.

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

Enemies are bred. The game scores every enemy on how it did against the players: destroying a player counts most, then forcing a shield, then surviving. Each new group descends from the best-scoring tanks so far, with their genomes recombined and slightly mutated within the sector budget and the cap on Muzzle. A team that struggles against fast bullets therefore meets more of them.

Brains that learn are a second step. An enemy's brain becomes a neural network whose size is a stat, and the networks are bred along with the genomes. They start from weights trained offline, never from random ones, so that enemies play decently from the first sector.

## Map

The map is an endless corridor that winds its way north. It usually heads north, occasionally runs east or west, and rarely doubles back south before turning north again. Its width varies, and it sometimes opens into an arena. Nothing exists outside the corridor: beyond the walls is the void.

| Sector type | Frequency | Size | Notes |
|---|---|---|---|
| Corridor | Most sectors | 40–80 m long, 8–20 m wide | Straight or bending |
| Switchback | Rare | Same as a corridor | Runs south, for one sector at most |
| Arena | Every 5 to 8 sectors | 32–36 m long, up to 36 m wide | Holds 1.5 times the enemies, in three waves |

| Element | Tanks | Bullets | Blasts |
|---|---|---|---|
| Wall | Blocked | Bounce | Stopped |
| Destructible block, 2 × 2 m | Blocked | Bounce | Destroyed |
| Explosive crate, 2 × 2 m | Blocked | Detonate it | Detonate it |

The generator guarantees five things:

1. **A way through.** Every sector has a path from its entry to its barrier that is at least 3 m wide, without destroying anything.
2. **No overlap.** Parts of the corridor that aren't neighbors stay at least 6 m apart.
3. **Net progress.** The corridor never runs more than 60 m south of the northernmost point it has reached.
4. **A fair start.** Enemies start at least 15 m from the barrier the players come through, and at least 3 m from the barrier that ends their sector. In an arena, these are the places where the tanks of its waves appear.
5. **Exact geometry.** Every wall vertex lies on a 1 m lattice. This gives walls at 0°, 45° and 90° plus the slopes 1:2 and 2:1, and it keeps generation identical on every platform.

## Multiplayer

Every game, solo included, is run by a server that has the final say on everything that happens. In solo and hosted games, the server runs inside a player's client.

| Session | How it works |
|---|---|
| Solo | The client runs its own server. Nothing goes over the network. |
| Hosted game | One player's client hosts up to three others. The game is announced on the local network, where the join screen of the others lists it. A game that isn't listed is joined by the address of the host's machine, which the lobby shows. A browser joins by a link, which the lobby shows too. From the internet, the host's UDP port 28405 must be reachable, and port 28406 for browsers. The host can give the game a name and a password. |
| Official server | Later. Dedicated servers run by the project, which also remove the need to open a port. |

Players in a browser can play solo and join games. They can't host, because a browser can't accept connections.

### Hosting and joining

A host opens a game from the host screen, and can give it two things first:

- **A name** of up to 24 characters, which the join screen and the lobby show. A game without a name is listed by the address of its host alone.
- **A password** of up to 24 characters. A game that has one only lets in a player who types it, whichever way they join. A wrong password is refused, and the player is told so. The password itself never goes over the network, and the game doesn't keep it: the host types it again for the next game.

Both stay as they are until the host closes the game.

The join screen lists the games of the local network, each with its name, the address of its host and its number of players, and a click joins one. A game that has a password is listed as locked, and joining it asks for the password. A game whose run is in progress, a full one and one of another version are listed for what they are, and can't be joined. A game is gone from the list a few seconds after its host closed it.

A browser doesn't join as a desktop game does. It can't look for games on a network, and it only speaks to a game that shows it a certificate. Nobody vouches for the certificate that a hosted game makes for itself, so a browser takes it by its print alone. The host therefore hands out a link: the address of the game's page, followed by where the game is and by the print of its certificate. A page that is opened by that link joins the game at once, or asks for the password first: the link never carries it. What a host hands out stays the same from one game to the next, until the host quits. A browser only joins from a page that it loaded from a secure address, or from the player's own machine.

### Lobby

Players gather in a lobby before a run. Nobody can join a run in progress.

Each player has a seat, which gives them their number, from P1 to P4. Every player's tank is the same blue, and the number tells them apart. The host has the first seat, and the others take the next ones in the order they joined. When a player leaves the lobby, those behind them move up. Seats don't change during a run: they move up again when the next one starts.

The host sets what the run is, and the others see it. Nothing is locked for a host (see [Modes](#modes)):

| Option | Choices |
|---|---|
| Difficulty | Normal or hard, and impossible once it exists |
| Seed | One that the game draws, and draws again when the host asks, or one that the host types, which makes it a chosen seed |
| Free mode | On or off. When it is on, each player builds their tank in their own garage (see [Garage](#garage)). |

A run starts once every player has said that they are ready, the host included. It then starts by itself after a countdown of 3 s, which the lobby shows. A player can take their word back at any time, which stops the countdown. Two changes take it back for them: a player who changes their tank in the garage is no longer ready, and when the host changes an option, nobody is. A player who joins isn't ready either.

A host who starts alone plays a solo run, with its lives, its checkpoints and its locks: a host who is alone can't be ready while the difficulty or an option is one that they haven't unlocked for solo.

When a run is over, the host decides what comes next: the same seed again, a new run, or the lobby. The first two start at once for everyone, with the same options and the same tanks: only the lobby asks who is ready. The host can also end a run from the menu, which brings everyone back to the lobby. Nobody is ready there until they say so again.

### Connection and latency

A player who disconnects loses their tank, and later sectors are generated for the remaining number of players. A player whose machine stays silent for 5 s has disconnected. If the host leaves, the run ends, and the other players are back at their main menu. All players must run the same game version: a game of another version doesn't answer.

The game must stay playable up to a round trip of 120 ms. What the host tells comes late, so a player who joined a game is shown three things in three ways:

- **Their own tank** responds to input immediately: it drives, aims, fires, lays mines and raises its shield at once, with the sounds of each. Their machine does all of it as the host will. When the host tells otherwise, the tank is brought to its place without a jump, and a shot or a mine that the host doesn't have goes away.
- **Bullets** are shown where they are at that moment, in the time of the player's own tank. What comes at a tank is where the screen shows it: a bullet that is dodged on screen is dodged, and a bullet ends where it meets the player's own tank or its bubble.
- **The other tanks** are shown about 50 ms in the past, between two tellings of the host, so that they move evenly.

## Determinism

Players get a random seed, or type their own: a solo player once that option is unlocked, and a host from the start. A seed is a short text. Together with the game version, it identifies a level.

| What | Depends on | Guarantee |
|---|---|---|
| Map: walls, blocks, crates, barriers | Seed, version | Identical on every platform |
| Enemies: number, genomes, positions | Seed, version, mode, number of players | Identical on every platform |
| Enemy behavior | All of the above and the players' actions | Not guaranteed to repeat |
| Music: notes, patterns, sounds | Version | Identical on every platform |
| Effects: particles, camera shake | Nothing | Random |

Solo and co-op players on the same seed therefore drive through the same map, on a desktop or in a browser. A new game version may change what a seed generates, so seeds and records always show their version.

Impossible mode keeps the first row of this table and gives up the second: its enemies depend on how the run has gone.

## Presentation

### Camera

The camera looks down at the ground at a steep angle. It shows about 30 m of ground from the bottom to the top of the screen, and more from side to side. Each player has their own view. It follows that player's tank smoothly and shifts a little toward the cursor, so players see further where they aim.

Among several players, a marker at the screen edge points to each teammate whose tank is out of view. It bears the number of its player, and it is full for a tank that runs and hollow for a wreck, as the squares of the team are on the display. A player whose own tank is a wreck is shown where it lies in the same way. Markers that would hide each other, or the marker of one of the last enemies, stand side by side.

Once a sector has no enemy left and none of their bullets in flight, the view of a player whose tank comes close to its barrier runs up to 10 m ahead of the tank, past the barrier, so that the next group can be studied before crossing. It comes back to the tank once the tank is through.

North is at the top of the screen by default. A player can rotate their own view a quarter turn at a time, for instance to put north on the right and see further ahead on a wide screen. The view never rotates on its own, and the movement keys always follow the screen.

### Visual style

The game is drawn as a technical drawing on a dark sheet, and nothing is lit: a solid shows its height by its darker sides. The floor carries a grid with a line every meter and a heavier one every 5 m. Walls are bands between two pale lines, destructible blocks are hatched and explosive crates are crossed in red. Tanks are assembled from parts by the game, not modeled by hand: the body is in the tank's color, and the other parts are dark and edged in pale lines. Bullets are the brightest objects on screen. They are white whoever fired them, because any bullet can destroy a player's tank, and each casts a patch of light on the floor. A bullet is an elongated shell with one dark stripe for each bounce it has left: it loses a stripe at every bounce, and a plain bullet won't bounce again. The void outside the corridor is empty and dark.

A line in dashes stands for what isn't there. A dormant enemy is the outline of its tank in dashes, flat on the floor and in its color, with every part that tells a stat. A bullet that is destroyed leaves a short burst of white strokes. A life that the player can't take is the outline of itself in dashes.

The floor of an arena bears two circles in dashes around its middle, in a dull red. The place where a tank is about to appear is marked by a red circle that closes in on it. A life on the floor is a square in the players' blue, like those that count the lives on the display, inside the circle within which it is taken.

A fight leaves marks on the floor: a scalloped outline where a blast went off, a square in dashes where it took a block away, and a cross where a tank was destroyed. They last as long as their sector is loaded.

### Interface

The interface is the margin of the drawing: ruled panels with square corners, labels in capitals, numbers in a monospaced typeface, and one accent color, the blue of the players.

| Screen | Content |
|---|---|
| Main menu | Solo, host, join, achievements, settings, quit |
| Solo | Difficulty, seed, free mode with its garage, what unlocks each of them while it is locked, a note when the run will earn no achievement, start |
| Host | The name of the game, its password, open |
| Join | The games of the local network with their names and which of them have a password, the address of the host, the password of a game that has one, join. In a browser: how to join by the link of a host, and that link again once it was followed. |
| Lobby | The name of the game, the players with who is ready, the difficulty, the seed and free mode, which the host sets, the garage in free mode, a note when the run will earn no achievement, ready and the countdown to the run, the address to join at or the link for browsers with a button that copies it (host) |
| Garage | Every stat of the player's tank with its level, what it gives and its price, the points that are left, the tank as it will look, back to the standard tank |
| Heads-up display | Rounds and reload progress, mines left, shield state, sector number, lives (solo), seed and version, a note on a run that earns no achievement, a scale and a north arrow, a ruler along the edge of the screen that gives the distance to the barrier, the number of each wave of an arena as it is announced, a caption over a life on the floor (solo), a numbered square for each teammate's state, a tag with the number of each player over their tank, a marker at the screen edge for each teammate out of view and what to do about a wreck (multiplayer). Rounds, reload and mines are also on an arc under the player's own tank. |
| Menu | Resume, settings, quit, and the seed with its version. In multiplayer, the host goes back to the lobby or closes the game, and the others leave it. |
| Game over | Sectors cleared, time, kills, seed and version, the tank that ended the run with its class and the list of its parts, a note on a run that earned no achievement, retry with a note that it will earn none, new run. In multiplayer, the choice is the host's. |
| Settings | Controls, video, audio, accessibility |

The game opens in full screen the first time. After that, it opens the way the player left it.

### Audio

Sound carries information the eyes can miss: a shot fired off screen, a ricochet, a mine about to blow, a shield coming down, a wave on its way, a tank that appears. A basic set of such cues arrives with the solo loop (M4), in time for the first real playtest. The full sound pass comes later.

The music is one piece that never stops, made by the game as it plays, the same for every seed and on every platform. Its drums and its mode follow the place and the fight, and its melody follows the highest brain tier awake. It never covers a cue. Its specification is in [MUSIC.md](MUSIC.md).

### Accessibility

Color is never the only carrier of information. Camera shake can be turned off, the interface can be scaled, and all controls can be rebound.

## Achievements and persistence

Achievements unlock modes and options for solo, and their conditions are listed under [Modes](#modes). A game of several players needs none of them: its host has every difficulty and every option from the start. An unlocked mode is another way to play, not an advantage: nothing a player unlocks makes the next achievement easier to earn. Two kinds of run therefore earn none:

- A run on a seed that the game didn't just draw for it: a chosen seed, or the seed of an earlier run that is played again. A seed can't be practiced to unlock them.
- A run in free mode, so that a tank can't be built to unlock them.

Every other run earns them, alone or as a team, and an achievement earned by a team is awarded to each member. A run that earns none says so on screen: before it starts, while it lasts and when it is over. The game over screen says it of a retry before the player picks one. A difficulty that is unlocked unlocks the ones below it: a team can reach sector 20 in hard mode before any of its members has unlocked hard mode.

The game stores settings, bindings, unlocks, records and the tank of the garage on the player's machine, or in the browser's storage on the web. It also keeps what a solo player and a host chose last: the difficulty, free mode and the name of the game, and never the password. Records keep the best sector reached for each difficulty and number of players, with the seed and the version, and apart for the runs in free mode. The number of players is the one at the start of the run, and each player keeps the record on their own machine.

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
| M4, Solo loop | Lives, checkpoints, progression curves, heads-up display, main menu, settings, game over, records, basic sound cues, first web build | Is a 20-minute run fun? This is the first real playtest. |
| M5, Co-op | Lobby, hosting, joining, local discovery, repair, enemy scaling, joining from a browser | Is it fun with friends at 100 ms of latency? |
| M6, Modes | Achievements, the solo screen, chosen seed, free mode with its garage, hard mode, the name, the password and the options of a hosted game, the ready check of its lobby, brain tiers 3 and 4 | Do the unlocks give reasons to come back? |
| M7, Impossible mode | An experiment: enemies bred from the tanks that did best against the players | Do players feel the enemies adapting to them, and is it fun? |

The question of M5 waits for more players than the project has now. An early access release will bring them. It needs two things that the game doesn't have yet: a way for players to update the game, and a way for them to report bugs and feedback.

Later, in no fixed order: controller support, local co-op on one screen (split screen, or one camera that follows all players), the full sound pass, interface polish and more options, a performance pass, the official server, new bullet types (splitting, seeking, exploding), and brains that learn (see [Impossible mode](#impossible-mode)).

## Open questions

- How do internet games connect for hosts who can't open a port, before the official server exists?
- In impossible mode, how exactly are enemies scored, and can brains learn fast enough within one run to matter?
- Which exact shades do the hues of the classes take? The players' blue and the white of the bullets are settled.
- From about sector 20, some leaders have more points than they can spend: a tank without a brain buys neither mines nor a shield, and most classes buy neither Piercing nor a second bounce. Should a leader's share stop at what its tank can buy, or should what is left over raise its software?
- How is the game distributed, at which secure address is its page published, and is the name free to use?
- How do the players of an early access release update the game, and how do they report bugs and feedback?
- Is a tank from the garage so much better than the standard tank, in the hands of a good player, that free mode becomes the easy way through? Its runs earn no achievement and keep records of their own, which limits the harm. The prices were set for enemies, whose danger also depends on their brains. The check comes before the garage is built: a playtest that takes its tank from the command line, and the same seed played with the standard tank and with two or three extreme ones.
