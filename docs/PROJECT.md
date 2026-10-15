# Tanku: game specification

Tanku is a cooperative top-down tank game for one to four players. The players fight their way through an endless corridor of procedurally generated enemy tanks and try to get as far as possible. A single hit destroys a tank, so survival comes from reading enemies, dodging and aiming, never from upgrades.

The core loop is: advance, read the enemies, fight, survive, learn, advance again. Learning is what the players earn each time around: they see what each kind of enemy does and prepare better for the next one. Nothing carries over between runs except what they have understood.

All numbers in this document are starting values to tune in playtests. Those that decide how hard the game is are written in one file, `tanku_core/game.balance`, which the game is compiled with: [BALANCE.md](BALANCE.md) describes it. The file and this document hold the same numbers, and a change to one is a change to the other.

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
| Run | One attempt, from its start to game over |
| Seed | The text that determines a run's map and enemies |
| North | The direction of progress along the corridor |
| Sector | One stretch of corridor holding one group of enemies |
| Barrier | The energy wall that closes the end of a sector |
| Arena | A wide sector whose enemies come in waves |
| Wave | One part of the group of an arena. It appears when the part before it is destroyed. |
| Arena mode | An option of a run: one room in place of the corridor, and one group after another |
| Room | The place of a run in arena mode: a closed arena that the run never leaves |
| Lull | In arena mode, the time before a group comes |
| Reserve | In arena mode, the tanks of a group that have not come yet. They wait outside the room. |
| Checkpoint | The last sector cleared. A solo player respawns in front of its barrier. |
| Life | What a solo player loses with their tank. A cleared arena leaves one to pick up. |
| Wreck | A destroyed player tank, which a teammate can repair |
| Budget | The number of points a tank has to spend on stats |
| Sector budget | The measure of an enemy's budget in a given sector. No tank has more, except the leader of its group. |
| Group | The enemies of one sector, which share one budget unevenly |
| Leader | The tank with the largest share of its group's budget: up to 1.25 times the sector budget |
| Class | What an enemy favors when it spends its points. Its color tells it. |
| Type of bullet | What a tank's bullets do besides flying straight: nothing more, or pierce, seek, split or explode. A tank fires one type. |
| Genome | A tank's full list of stat levels |
| Standard tank | The tank that every player has outside free mode |
| Garage | The screen where a player spends the points of their tank, in free mode |
| Hardware | The stats of the tank itself, paid in points |
| Software | The stats of an enemy's controller (brain and ballistics), paid as a share of its budget |
| Experience | How well enemy brains execute, which rises with the sector number |
| Round | One bullet in a tank's magazine |

## Run structure

A run starts in the start bay, an empty sector, and continues until game over. All progress of the run is then lost. Only unlocked modes, records and a line of the journal persist. The score is the number of sectors cleared. A run in [arena mode](#arena-mode) has neither bay nor corridor, and its score is the number of groups cleared.

### Sectors and barriers

The corridor is a chain of sectors. Each sector holds one group of enemies and ends with a barrier, an energy wall that spans the full width of the corridor. Barriers follow four rules:

1. Bullets and blasts never pass a barrier, and enemies never cross one.
2. A barrier unlocks when every enemy in its sector is destroyed and every living player is within 8 m of it. It then stays unlocked.
3. Players cross an unlocked barrier forward only.
4. The enemies of a sector are dormant until the first player crosses into it. Dormant enemies are visible through the barrier. Until then, the sector is drawn darker than the rest of the corridor, its blocks and its crates are plans that lie flat on its floor, and its enemies are outlines in dashes. When the first player crosses, the sector wakes: its blocks, its crates and its enemies stand up. That takes a third of a second on the screen and no time in the rules: from the first instant, the blocks are in the way and the enemies are awake. An arena is the exception (see [Arenas](#arenas)).

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

Multiplayer has no lives and no checkpoints, and an arena leaves nothing on its floor. A destroyed player tank becomes a wreck, and its player watches a living teammate until repaired: the view stays on the wreck for a moment, then goes over to that teammate. The run ends when all players are wrecked at the same time. Repairs are unlimited and take 2 s next to the wreck, which makes them safe between fights and risky during one (see [Repair](#repair)). A wreck is drawn as the outline of its tank in dashes.

Enemy groups grow with the number of players (see [Progression](#progression)). The map doesn't change.

### Arena mode

A run in arena mode has no corridor. It is fought in one room, a closed arena that the seed draws, against one group after another, without end. What this document says of a run holds there, with what follows in place of sectors and barriers.

**Groups.** The groups come in the order of their numbers. Group $n$ is the group of sector $n$ for as many players: its tanks, their budgets and their brains are those of that sector (see [Progression](#progression)), and it has the number of tanks of a corridor, not the 1.5 times of an arena. It doesn't come in waves. Half of its tanks, rounded up, come when the lull is over, and the group is announced then, by a message and a sound. Both tanks of a group of two come then: one and then the other would be fought as two groups of one. From then on, a tank of the group comes for each one that is destroyed: the room holds as many tanks as that half for as long as any waits in reserve, which is as many as a wave of an arena of the corridor has. The place of each tank is marked for 1 s before the tank appears, awake, as in an arena, and the tanks that come later are told by their marks alone. A mark there is in the color that its tank has in reserve, so it tells which tank comes. No tank appears within 4 m of a player's tank, or within 2 m of a tank of its group. In an arena of the corridor, a mark waits for the player to move. In the room, where nobody could tell why a tank doesn't come, the tank takes another place that has room, and waits only if there is none.

**The lull and the reserve.** A lull of 3 s comes before the first group and after each group that is cleared. It is short on purpose: arena mode is meant to be fast before it is readable. The tanks that have not come yet wait in reserve: they stand outside the south wall of the room, as outlines in dashes with every part that tells a stat, and nothing reaches them there. During a lull, the reserve holds the whole next group, with a gap between the tanks that come first and the others. During a fight, it holds the tanks that each wait for one to be destroyed, and they leave it in the order they stand in. A tank leaves it when its place in the room is marked. A group is shown there as one is through a barrier, and the places of its tanks are not told before their marks.

**The room.** It is laid out anew for each group. What the first group is fought among stands when the run starts. When a group is cleared, the blocks and the crates that are left go, with the marks that the fight left on the floor, and those of the next group are planned: the place of each is drawn on the floor for 2 s, and the block or the crate stands there once that time is over and no tank, no wreck and no mine is in its place. A bullet that flies there at that moment ends. The room of group $n$ is as bare and as explosive as sector $n$ (see [Progression](#progression)), and the tanks of a group appear at places that are drawn for that group.

**Lives and wrecks.** A single player has three lives, and gets one back for every fifth group that is cleared, never more than three. A tank that is destroyed comes back in the middle of the room 1.5 s later. The room keeps what was done to it, as a sector does: the tanks that were destroyed stay destroyed, the blocks that were destroyed and the crates that went off stay gone, and every bullet and every mine is removed. Nobody comes for a tank that is destroyed while the player is away. The tanks that are left go back into reserve, and the room fills again 2 s after the player is back: the group is announced, and its tanks come after their marks, up to as many at once as the first time. A lull waits for a player who fell. Several players have wrecks and repairs, as in the corridor.

**Score.** The score is the number of groups cleared. Arena mode has records of its own, and its runs earn no achievement (see [Achievements and persistence](#achievements-and-persistence)).

Arena mode trades reading for pace. The reserve shows every tank before it comes, as a barrier shows the group of a sector, so nothing appears that was not shown. But the lull is too short to study a group, and a tank that waits comes the moment another one is destroyed. What a run in arena mode gives up is the choice of the moment: nothing waits for the players to be ready.

### Modes

| Mode or option | Effect | Unlocked in solo by |
|---|---|---|
| Normal | The rules of this document | Available from the start |
| Chosen seed | The solo player, or the host of a game, types the seed of the run instead of getting a random one. | Clearing sector 15 |
| Arena mode | The run is fought in one room, against one group after another (see [Arena mode](#arena-mode)). | Clearing sector 10 |
| Free mode | Each player spends the 59 points of their tank as they like before the run (see [Garage](#garage)). | Clearing sector 20 |
| Hard mode | Sector budgets start at 40 and are capped at 165, all software tiers are available from sector 1, and experience and the cap on Muzzle are those of 20 sectors further. | Clearing sector 30 |
| Impossible mode | Later. Enemies adapt to the players during the run (see [Impossible mode](#impossible-mode)). | Clearing sector 20 in hard mode |

Normal, hard and impossible are the three difficulties. Chosen seed, arena mode and free mode are options that combine with any of them, and with each other.

The locks are solo's. A solo player sets the difficulty and the options of a run on the solo screen, where each one that is locked says what unlocks it. The host of a game of several players sets them in the lobby, and has all of them from the start (see [Lobby](#lobby)).

### Playtest

A playtest is a way to test the game faster, not a mode for players. It starts with the `--playtest` option on the command line and lets the solo player cheat from the menu in three ways:

- **Infinite lives.** The tank comes back as often as it is destroyed. This is on at the start and can be turned off.
- **Skipping to a sector.** The player picks any sector and comes back in front of the barrier of the one before it, as if that sector had just been cleared. In arena mode, the player picks a group, and the run goes on with the lull before it.
- **Fake achievements.** The machine sets its achievements and its records aside and plays as on its first time: what a run unlocks is locked again. What the playtest then earns by the rules of a run that counts, achievements and records, is fake: it is kept in memory until the game is closed, and no file keeps it. This is off at the start. Turned off, the machine has its own achievements and records back.

A playtest can also choose its seed without unlocking that option: `--seed` followed by a seed starts every run on it, which brings back a situation that a player reported.

It can bring a tank of its own too: `--tank` followed by the stats of a tank with their levels starts every run in free mode, in that tank, under the rules of the [garage](#garage). A tank is tried that way without building it in the garage, and the tank of the garage stays as it is.

It can bring teammates as well, so that one person can try a game of several players. On the solo screen of a playtest, the player picks up to three tanks that the game drives beside theirs, and how well they play: novice, regular or expert, which differ in how soon they see a bullet coming, whether they follow it off a wall and whether they shield. The run is then one of several players, with its wrecks, its repairs and the groups of as many players. A teammate fights the nearest enemy of its sector, and holds a shot that would take a teammate. The one nearest to a wreck repairs it. Once a sector is cleared, the teammates wait in front of its barrier and cross it after the player, never first. Otherwise they keep near the player.

A playtest says on screen that it is one. It sets no record and earns no achievement, but fake ones. A playtest can host a game: the cheats are the host's alone, and the run sets no record for anyone in it, but fake ones for its host.

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
| Menu | Esc | Pauses the game in solo, in a playtest with teammates, and for everyone when the host of a game opens it |

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
| **Enemy bullet** | Destroys it | Destroys it | Both destroyed | Both destroyed | Detonates it | Bounces | Absorbed |
| **Blast** | Destroys it | Destroys it | Destroys it | Destroys it | Detonates it | Destroys blocks, stopped by walls | Stopped |

Two rules sum up the bullets, and neither asks who fired:

1. A bullet destroys any tank that it touches: a player's destroys enemies and teammates, and an enemy's destroys players and other enemies. It spares only the tank that fired it, and only until it has bounced.
2. Any two bullets that touch destroy each other.

Piercing is the exception to the second rule. A tank with the Piercing stat fires piercing bullets. A piercing bullet destroys the first normal bullet it touches and flies on, but it is a normal bullet from then on, so it takes two bullets to stop it. Two piercing bullets destroy each other.

Blasts are neutral. They destroy every unshielded tank in range, whoever caused them. A tank is in range when its center is within the radius of the blast and no wall or barrier stands between them.

### Special bullets

A tank fires one type of bullet. A normal bullet does what the rules above say. Four types are special: each is a stat that a tank has or doesn't have, and no tank has two of them (see [Hardware stats](#hardware-stats)). A special bullet is white like any other and destroys what a normal bullet destroys. Its shape says what else it does, and so does the muzzle that it came from (see [Appearance](#appearance)).

- **Piercing.** It destroys the first normal bullet that it touches and flies on, as said above.
- **Seeking.** It flies straight, and it seeks when it bounces. Off a wall or a block, it doesn't leave at the mirror angle: it leaves toward the nearest tank in sight of that place that its side fights. A player's bullet seeks the enemies, never a teammate or the tank that fired it, and an enemy's bullet seeks the players' tanks that run. It aims at where that tank is then, not ahead of it. With no such tank in sight, it bounces as a normal bullet does. It seeks at each bounce that its tank's Ricochet gives it, and never without Ricochet.
- **Splitting.** After 1 s of flight, it becomes two normal bullets that leave from where it is, 30° to each side of its line of flight. The two don't destroy each other as they part. The bullet itself doesn't bounce: a wall or a block that it meets before it splits ends it. The two bullets have the bounces of their tank's Ricochet.
- **Exploding.** It never bounces, and it goes off wherever it ends: on a wall, a block, a crate, a barrier, a tank, a shield, a mine or another bullet, and in a blast that reaches it. Its blast is that of a mine, with a radius of 3 m, and it is the doing of the tank that fired the bullet. Like every blast, it is neutral: a tank that fires at a wall within 3 m destroys itself.

Each of the last three says what it is about to do, in dashes (see [Visual style](#visual-style)):

- From 1.5 s before a seeking bullet bounces, a line runs from the bullet to the wall, and a second one from there to the tank that it would seek: its way out, which follows that tank until the bounce.
- Two lines ahead of a splitting bullet show its two ways, and go with the bullet. Nothing counts the time down: for the last 0.6 s before the split, as long as a mine warns, the two lines are solid and flash.
- From 1.5 s before an exploding bullet ends on a wall, a block, a crate or a barrier, a line runs from the bullet to that place, and a circle around the place shows what the blast will reach. A tank, a mine or a bullet that gets in its way ends it sooner, and nothing marks that.

A time gives every gun the same warning, whatever its Muzzle: 1.5 s is 13.5 m at 9 m/s and 28.5 m at 19 m/s, so in a corridor most of these shots are marked from the barrel.

### Mines

A tank drops a mine at its own position, at most one every 0.5 s, and can have as many armed mines as its Mines stat allows. A mine arms after 1 s. It then detonates when another tank than the one that laid it comes within 2 m, whichever side that tank is on, or when its 10 s fuse runs out, and it warns for 0.6 s first either way: the last 0.6 s of the fuse are that warning. A bullet or a blast detonates it at any time, without a warning, even before it is armed. A shot passes over a mine that lies under the barrel of the tank that fires it. The blast has a radius of 3 m. An armed mine shows the circle within which another tank sets it off. A mine that warns shows the circle that its blast will reach instead. Blasts are the only way to remove destructible blocks.

### Shield

The shield is a bubble around the tank that lasts 1 s. It destroys every bullet that touches it, piercing or not, and protects against blasts. The tank keeps moving and shooting while the bubble is up: its own shots leave the bubble, but a ricochet doesn't come back through it. Once the bubble is gone, the tank can't shield again before the cooldown ends. Higher Shield levels shorten the cooldown.

The cooldown is the only drawback. A shield saves a tank from one threat, and for several seconds it can't save it from the next. It beats single shots, and it loses to bursts that outlast the bubble and to patient opponents who bait the shield and fire again.

### Repair

A player repairs a wrecked teammate by holding the repair key within 3 m of the wreck for 2 s. The repairer keeps every other verb: the tank drives within that reach, shoots, places mines and shields as usual. All progress is lost if the repair is interrupted: when the key is let go, when the tank leaves the reach, or when it is destroyed. The repaired tank returns with a full magazine and 1 s of protection, during which bullets end on it and blasts leave it alone.

A tank repairs one wreck at a time. With several wrecks within reach, it works on the nearest one and stays with it, wherever it drives within that reach, until that tank is back or the repair is interrupted. If the key is still held then, it goes on with the nearest wreck that is left, from nothing. Two tanks at the same wreck repair it no faster than one.

Nothing destroys a wreck again, and a wreck is in nobody's way: tanks drive over it, bullets fly over it, it sets off no mine and it stays through any blast. It therefore gives no cover to the tank that repairs it. A tank that comes back under another one pushes it aside over a few ticks, without a jump.

A wreck lies in a circle in a phantom line, as a reach is drawn: a tank whose center is inside the circle can repair it. The circle is inked in from the top as the repair goes on, and starts over when the repair is interrupted. The wreck of a teammate keeps the balloon of its player over it, in dashes, and the balloon is inked in with the circle. While a teammate's tank is a wreck, a player is told on screen which key to hold, and how long the repair still takes. The player whose tank is the wreck is told when a teammate is at it.

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
| Mine trigger | Another tank than the one that laid it within 2 m |
| Mine warning | 0.6 s before the blast, after a tank sets the mine off and at the end of its fuse |
| Blast radius of mines, crates and exploding bullets | 3 m |
| Seeking bullet | At each bounce, toward the nearest tank in sight that its side fights. Its bounce is marked from 1.5 s before. |
| Splitting bullet | Splits after 1 s, 30° to each side. Its lines flash for the last 0.6 s. |
| Exploding bullet | Goes off where it ends. Its end on what stands still is marked from 1.5 s before. |
| Shield duration | 1 s |
| Lives of a solo player | 3 at the start, and never more |
| Life of an arena | 4 m in front of the barrier, taken within 1.5 m |
| Waves of an arena | 3, the first announced 2 s after the first player has crossed |
| Mark of a tank that is about to appear | 1 s, once no player's tank is within 4 m of the place |
| Sight of an enemy | 30 m: it holds its fire at a player who is further |
| Repair | 2 s within 3 m |
| Group of arena mode | Half of its tanks at once, rounded up, and both tanks of a group of two, after a lull of 3 s. Then one for each tank that is destroyed. |
| Place of a tank that appears in arena mode | 4 m or more from a player's tank, and 2 m or more from a tank of its group |
| Life of arena mode | 1 for every fifth group cleared, and never more than 3 |
| Block or crate of arena mode | Its place is drawn for 2 s before it stands there |

## Tanks

Every tank, player or enemy, is described by a genome: one level per stat. Levels are bought with the tank's budget of points. Players and enemies follow the same rules with four differences. Player tanks have a color of their own, a fixed genome outside free mode, no software stats, because the player is the brain, and no class.

### Hardware stats

Level 0 of each stat is free. Costs are cumulative: each number is the total price of that level.

| Stat | Sets | Value at level 0, 1, 2, … | Cost at level 0, 1, 2, … |
|---|---|---|---|
| Engine | Top speed in m/s | 0, 2, 3, 4, 5, 6, 7, 8 | 0, 2, 4, 6, 8, 11, 14, 17 |
| Tracks | Hull turn rate in °/s | 90, 135, 180, 225, 270, 315, 360 | 0, 1, 2, 3, 4, 5, 6 |
| Turret | Turret turn rate in °/s (enemies only) | 45, 60, 90, 120, 180, 270, 360 | 0, 1, 2, 3, 5, 7, 10 |
| Muzzle | Bullet speed in m/s | 6, 7.5, 9, 11, 13, 15, 17, 19 | 0, 1, 3, 6, 11, 17, 24, 32 |
| Magazine | Rounds held | 1, 2, 3, 4, 5, 6 | 0, 3, 6, 9, 12, 15 |
| Loader | Seconds to reload one round | 4.0, 3.2, 2.5, 2.0, 1.6, 1.25, 1.0, 0.8 | 0, 1, 2, 4, 6, 9, 12, 16 |
| Ricochet | Bounces per bullet | 0, 1, 2, 3 | 0, 4, 10, 18 |
| Mines | Mines armed at once | 0, 1, 2, 3, 4 | 0, 4, 6, 8, 10 |
| Shield | Cooldown in s | none, 10, 9, 8, 7, 6 | 0, 6, 9, 12, 15, 18 |
| Piercing | Bullets that each of its bullets pierces | 0, 1 | 0, 6 |
| Seeking | Whether its bullets seek at their bounces | no, yes | 0, 10 |
| Splitting | Whether its bullets split | no, yes | 0, 9 |
| Exploding | Whether its bullets explode | no, yes | 0, 10 |

Piercing, Seeking, Splitting and Exploding are the four special types of bullet (see [Special bullets](#special-bullets)). A tank has one of them at most.

Every stat stops at its highest level. These limits keep the game sound: a bullet, for instance, never gets fast enough to pass through a wall. Buying every hardware stat at its highest level, with the dearest type of bullet, costs 152 points. An enemy's Muzzle level is also limited by its sector, and each special bullet waits for a sector of its own (see [Progression](#progression)). Its type of bullet and its Ricochet level are limited by its class (see [Classes](#classes)).

### Software stats

Software is the enemy's controller. Its price is a share of the tank's total budget, rounded up. Each tier includes the tiers below it. A tier is a set of hand-written behavior, so more tiers can be added when these aren't enough.

| Stat | Tier | Share of budget | Adds |
|---|---|---|---|
| Brain | 0, Sentry | 0% | Fires when a player is in its line of fire. Wanders if it has an engine. |
| Brain | 1, Grunt | 8% | Keeps its distance, avoids walls and mines, uses mines and shield by reflex. |
| Brain | 2, Soldier | 16% | Dodges bullets flying straight at it, whoever fired them, reloads behind cover, spreads out from its allies. |
| Brain | 3, Veteran | 24% | Dodges ricochets, shoots down a bullet that it can't dodge, mines the players' routes, shields only when it can't dodge. |
| Brain | 4, Ace | 32% | Dodges shots that are aimed ahead of it, coordinates with allies for crossfire, baits shots, retreats when alone. |
| Ballistics | 0 | 0% | Aims at where the target is. |
| Ballistics | 1 | 5% | Leads moving targets. |
| Ballistics | 2 | 10% | Finds single-bounce shots. |
| Ballistics | 3 | 15% | Finds double-bounce shots, and shoots mines and crates near players. |

Ballistics 3 is kept for two classes, and three others stop at Ballistics 1 (see [Classes](#classes)).

### Why prices differ

A price should reflect how much more dangerous a level makes a tank, and not every stat grows the same way.

| Pricing | Stats | Reason |
|---|---|---|
| Flat per level | Tracks, Magazine | Each level is worth about as much as the last. |
| Rising per level | Engine above level 4, Turret, Muzzle, Loader, Ricochet | High levels cross thresholds: faster than a player tank, too fast to dodge, or enough bounces to fill a room. |
| Entry fee | Mines, Shield, Piercing, Seeking, Splitting, Exploding | The first level buys a new ability. Later levels, where there are any, only improve it. |
| Share of budget | Brain, Ballistics | Software multiplies the value of the hardware it drives: the same brain is worth more in a better tank. |
| Prerequisite | Tracks, Mines, Shield, Seeking, Ballistics 2 and 3 | The stat is worthless alone, so the generator never buys it alone. |
| Cap by sector | Muzzle, Piercing, Seeking, Splitting, Exploding | A sniper puts its points into its gun, and one enemy in eight is a sniper, more in the first sectors. The cap decides how fast that gun is in each sector, and the price decides what else such a tank can afford. Each special bullet waits for a sector of its own, so the classes that fire them come one after another. |
| Kept for a class | Piercing, Seeking, Splitting, Exploding, Ricochet above level 1, Ballistics 3 | The special bullets and the bullets that bounce twice are the hardest to read. Each belongs to a class, and the color of the tank announces it. |

The prerequisites are: Tracks need Engine 1. On enemies, Mines and Shield need Brain 1. Ballistics 2 needs Ricochet 1, and Ballistics 3 needs Ricochet 2. Seeking needs Ricochet 1: a bullet that doesn't bounce never seeks.

The four special bullets are priced by what they add. A piercing bullet costs the least, a splitting one more, and a seeking one and an exploding one the most.

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

In free mode, each player spends the 59 points as they like, in the garage, before the run. The garage lists every hardware stat except Turret, each with its level, what that level gives, its price and the price of one more level, and the points that are spent and those that are left. The four special bullets are not four lines there. They are one, the type of the bullets: a choice among normal, piercing, seeking, splitting and exploding, each with its price, because a tank fires one type. Next to the list, the garage shows the tank as it will look, and that drawing follows every change at once (see [Appearance](#appearance)).

A tank from the garage follows the rules of every tank, with three of its own:

1. It needs Engine 1 at least. A tank that can't drive would never come within reach of a barrier, and the barrier would stay locked.
2. It has no Turret stat: its turret follows the cursor, as on the standard tank.
3. It can leave points unspent, and it can't spend more than 59.

What holds an enemy back doesn't hold a player. The cap on Muzzle and the stats kept for a class are for enemies only, and Mines and Shield need no brain. A player can have the fastest gun, any type of bullet and three bounces, at their price. Two purchases buy nothing, and the garage says so: Ricochet on a tank whose bullets explode, and seeking bullets on a tank without Ricochet.

The garage opens on the tank that the player built last, which is the standard tank the first time, and a button brings the standard tank back. In a game of several players, each player has their own garage and their own tank, and the others see that tank in the run as its player built it.

### Classes

An enemy has a class: the stats that it favors when it spends its points. The class gives the tank its color, so that it is read from afar, before the parts of the model are.

| Class | Favors | Color | Weight | From sector |
|---|---|---|---|---|
| All-rounder | Nothing: a bit of everything | Gray | 3 | 1 |
| Sniper | Muzzle and Turret | Teal | 3 | 1 |
| Scout | Engine and Tracks | Yellow | 3 | 1 |
| Tactician | Brain and Ballistics | Green | 2 | 3 |
| Trickster | Ricochet | Purple | 2 | 1 |
| Lancer | Piercing | Orange | 2 | 9 |
| Seeker | Seeking and Ricochet | Dark magenta | 1 | 19 |
| Flanker | Splitting | Dark blue | 1 | 11 |
| Grenadier | Exploding | Brown, almost black | 1 | 17 |
| Sapper | Mines and Shield | Pink | 3 | 2 |
| Gunner | Magazine and Loader | Red | 3 | 1 |

A tank draws its class by these weights, among the classes that its sector has. A class waits for the sector that allows a first level of every stat that it favors, and of what those stats need: a tactician for the first brain and the first tier of ballistics, a sapper for the first brain, and each class that fires a special bullet for the sector of that bullet (see [Progression](#progression)). The sectors of normal mode decide whatever the difficulty, so a tank is of the same class in hard mode. Once every class has come, at sector 19, five tanks in 24 are of the four classes that fire a special bullet.

The bright blue is the players'. The blue of a flanker is darker and of another hue. The seeker, the flanker and the grenadier are told from the other classes by how dark they are as much as by their hue: the wheel of hues had no room left for three more.

Some stats are kept for the classes that are about them:

- Each special bullet is one class's alone: only a lancer has Piercing, only a seeker Seeking, only a flanker Splitting and only a grenadier Exploding.
- Only a trickster, a tactician and a seeker have Ricochet above level 1. Only the first two have Ballistics 3, which needs it.
- A seeker, a flanker and a grenadier have Ballistics 1 at most. The bank shots of the higher tiers are no use to them: a seeker's bullet finds its own way off a wall, and the bullets of the other two don't bounce.
- A grenadier has no Ricochet, which its bullets couldn't use.
- A tactician has the highest Brain and Ballistics tiers that its sector allows.
- A sapper has Brain 1 at least, without which it could use neither its mines nor its shield.

### Enemy generation

The enemies of a sector are generated together, from the seed, the sector and the number of tanks in the group. The group has one budget, which its tanks share unevenly, so that weak tanks stand next to strong ones:

1. Draw the group's budget: between 70% and 90% of the sector budget for each of its tanks.
2. Give every tank 30% of the sector budget. Each tank then draws a weight of 1, 1, 2, 2, 3 or 8, and the rest of the group's budget is shared by weight.
3. The heaviest tank is the leader, which can have up to 1.25 times the sector budget. No other tank has more than the sector budget. What a tank can't take goes to the others.

A tank that is alone in its sector has the whole budget of its group, which is less than the sector budget. In a group of six, the strongest tank has about 2.3 times the points of the weakest.

Each tank then spends its budget, from the seed, its sector and its index in the group:

1. Draw its class by its weight, among the classes that its sector has (see [Classes](#classes)).
2. Draw its software tiers within the sector's tier caps and what its class allows, and pay their shares. A Ballistics tier whose bounces the tank can't pay for drops until it can.
3. Buy one level of each stat that its class favors.
4. Spend the remaining points one level at a time, at random, respecting prerequisites, the sector's cap on Muzzle and what its class allows, until nothing affordable is left. The favorite stats weigh 15 together, and every other stat weighs 1.
5. If points are left, raise its brain by a tier, or else its ballistics with the bounce that the tier needs, within the sector's tier caps and what its class allows, and go back to step 4: a first brain opens mines and a shield. The tank is done when what is left pays for neither.

The last step is for the tanks that have more points than their hardware can take. A tank without a brain has neither mines nor a shield, and most classes have neither a special bullet nor a second bounce, which leaves it 100 points of hardware to buy. A leader can have more than that from sector 22 on and the other tanks from sector 34 on, and both sooner in hard mode. What such a tank has left makes it smarter, and what stays unspent is less than a tier costs it.

Specialization matters more than balance. A tank with a few strong stats has a recognizable shape and asks the players a clear question. In a group of six, four or five classes stand side by side.

### Reference builds

The price list can express the enemy roster of Tanks!, in roughly the order in which that game introduces it. This is a sanity check of the prices, not a list of tanks to ship. The gun of the Sniper is the fastest that its sector allows. The names and the colors are those of Tanks!, not those of the classes. The roster is taken from the [StrategyWiki page on Tanks!](https://strategywiki.org/wiki/Wii_Play/Tanks!).

| Build | Key stats | Budget | Possible from sector |
|---|---|---|---|
| Sentry (brown) | Stationary, 9 m/s bullets with 1 bounce, no brain | 10 | 2 |
| Rover (gray) | 2 m/s, same gun, brain 1 | 15 | 3 |
| Sniper (teal) | 2 m/s, 11 m/s bullets without bounce, brain 2, ballistics 1 | 19 | 6 |
| Burster (pink) | 4 m/s, 3 rounds, brain 2 | 34 | 6 |
| Miner (yellow) | 6 m/s, 4 mines, brain 1 | 39 | 6 |
| Banker (green) | Stationary, 13 m/s bullets with 2 bounces, ballistics 3 | 44 | 15 |
| Raider (purple) | 6 m/s, 5 rounds, 2 mines, brain 3 | 72 | 14 |
| Hunter (black) | 8 m/s, 17 m/s bullets, 2 mines, brain 4 | 109 | 26 |

A build is possible in a sector whose caps allow its tiers and its gun, and where a tank of a solo group in a corridor can have as many points. The Burster, the Miner, the Raider and the Hunter first come as the leader of their group: no other tank can have their budget before sectors 7, 8, 18 and 40. The Banker is a trickster or a tactician, the two classes that have Ballistics 3.

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
| Seeking | Chevrons around the muzzle, in the place of the rings: one per bounce. Its bullets have a chevron in the place of each stripe. |
| Splitting | Fork on the muzzle, in front of the rings: two prongs that point where the two bullets will go. Its bullets have a seam along them and no stripe. |
| Exploding | Muzzle that flares into a bell. Its bullets have a flat end, with a red line that cuts them near it, and a trail. |
| Brain | Antennas, one per tier |
| Ballistics | Sight on the turret, one lens per tier |
| Total budget | Color intensity. Weak tanks are pale, strong ones saturated, and even the weakest shows its hue. |
| Class | Color: a hue for each class, gray for an all-rounder, and a darker color for the seeker, the flanker and the grenadier (see [Classes](#classes)) |

The lenses of the sight and what marks the muzzle of a special bullet (the pointed tip, the chevrons, the fork and the bell) are a pale tint of the tank's own hue.

The mark of a bullet that seeks agrees with the marks of ricochet instead of standing beside them: a seeking bounce is a bounce, so a chevron is drawn where its ring and its stripe would be. A tank whose bullets seek but can't bounce has neither.

A player's tank is assembled from its genome like an enemy's. The standard tank therefore always looks the same, and a tank from the garage shows what its player bought. Two things never change on a player's tank: its turret keeps the size of the standard tank's, because Turret isn't a stat of a player's tank, and it is always the same bright blue, however many points it spends. No enemy has that blue: the flanker's is darker and of another hue. Among several players, a balloon over each teammate's tank bears the number of its player, from 1 to 4, as the balloons of a drawing name its parts. A player's own tank has none: their own number is in the title block of their display. All tanks have the same size.

## Enemy brains

A brain is the controller of one enemy tank. It observes the fight (its own state, the tanks, bullets and mines around it, the nearby walls) and issues the same commands a player has: move, aim, shoot, place a mine, shield. Brains don't cheat: their tanks obey the same hardware limits and the same rules as player tanks. Enemies in an active sector always know where the players are. What they do with that knowledge depends on their software. An enemy fights the nearest player in its sector whose tank still runs, and keeps that target until another player is clearly nearer. It fires at a player who is within 30 m of its tank, which is its sight. Further than that, it holds its fire: it keeps its gun on the player and drives as its brain says, and it fires once the player is within its sight, after its reaction delay. It still shoots down a bullet that comes at it, wherever the player is.

Two things make a brain better. Its tiers, bought with points, decide what it is able to do. Its experience, which depends only on the sector, decides how well it does it: experience shortens the reaction delay from 0.50 s to 0.12 s and shrinks the aiming error from 8° to 1°.

A bullet spares nobody either, so a brain that dodges minds the bullets of the other enemies as it minds the players': only its own don't count, until they have bounced. A tank holds its fire, whatever its tier, while another enemy stands in the way of its shot, between its gun and what it fires at, and it fires as soon as the way is clear. What a shot does once it has missed is for the others to get out of the way of. No brain lays a mine next to another enemy, and the best ballistics fire at no mine and no crate whose blast would reach one.

A blast spares nobody, so the bullets that explode ask three things of the brains. A tank that fires them holds its fire, whatever its tier, while the blast of its shot would reach its own tank or another enemy. It also holds it while a bullet that explodes is so close to its gun that the blast of one would set off the other: a burst would go off one bullet after the other, back to the gun that fired it. And from tier 1, a brain stays out of the circle that an exploding bullet marks, as it stays away from mines.

Brains are hand-written, one set of behavior per tier. In normal and hard mode they never learn during play, because that would break the promise that a seed always gives the same challenge. Brains that learn belong to [impossible mode](#impossible-mode).

## Progression

Difficulty depends only on the sector number $n$, the mode and the number of players $P$. Seven things grow with $n$: the enemies' budget, their number, their software tiers, the speed of their fastest bullets, the types of bullet that they fire, their experience and the harshness of the terrain. In arena mode, the number of a group stands for the sector number.

The sector budget is the measure of an enemy's budget in sector $n$. No tank has more, except the leader of its group, which can have up to 1.25 times as much (see [Enemy generation](#enemy-generation)):

$$B(n) = B_{\max} - (B_{\max} - B_0)\, e^{-(n-1)/25}$$

with $B_0 = 10$ and $B_{\max} = 135$ in normal mode. It rises fast at first and flattens toward the cap. The cap stays below the 152 points of a fully equipped tank, so even late enemies must specialize. Only a leader can have more than the cap. A tank that has more points than its hardware can take raises its software with the rest (see [Enemy generation](#enemy-generation)). Every stat also stops at its own highest level.

The number of enemies in a sector is $N(n) = \min(6,\ 1 + \lfloor n/4 \rfloor)$ in solo and $\lceil N(n)\,(1 + 0.5\,(P - 1)) \rceil$ with $P$ players. Arenas hold 1.5 times as many, in three waves (see [Arenas](#arenas)). The group of a sector is set when the sector is loaded, one sector ahead of the players. A sector that is too small for its whole group holds as many enemies as fit. That happens in the smallest arenas: to about one in a thousand with four players, and almost never with fewer.

Experience is $E(n) = 1 - e^{-(n-1)/20}$, where 0 is a novice and 1 a flawless brain. It keeps closing in on flawless play without ever reaching it. The software tiers come by steps. The tank of sector 1 has no brain: the highest brain tier allowed is 0 there, and it goes up by one at sectors 2, 6, 14 and 22. The highest ballistics tier is 0 at first, and goes up by one at sectors 3, 10 and 15.

The highest Muzzle level an enemy can have is the level of the player tank in the first four sectors, then one more at sectors 5, 7, 13, 18 and 25. The cap is for enemies only.

The special bullets come one at a time: piercing from sector 9, splitting from sector 11, exploding from sector 17 and seeking from sector 19. The class that fires each of them comes with it (see [Classes](#classes)).

No two of these steps fall on the same sector. From sector 2 to sector 20, every sector brings one thing more than the one before it: a tank, a tier, a faster gun or a type of bullet. The last tier of brain comes at sector 22 and the fastest gun at sector 25.

The cap follows what a player can dodge. A player's tank that keeps moving needs about 0.55 s to react to a shot and move out of its way, which a bullet covers in 5 m at 9 m/s, 7.2 m at 13 m/s and 8.3 m at 15 m/s. Enemies with a brain keep 8 to 14 m away. Until sector 12, a player who keeps moving can therefore dodge every bullet at that distance. From sector 13, the fastest guns beat reaction at close range, and the players need distance, cover or the shield.

| Sector | Sector budget | Most points of a tank in solo | Enemies in solo | Highest brain tier | Highest ballistics tier | Highest Muzzle level | Experience |
|---|---|---|---|---|---|---|---|
| 1 | 10 | 9 | 1 | 0 | 0 | 2 | 0.00 |
| 3 | 20 | 18 | 1 | 1 | 1 | 2 | 0.10 |
| 5 | 28 | 35 | 2 | 1 | 1 | 3 | 0.18 |
| 10 | 48 | 60 | 3 | 2 | 2 | 4 | 0.36 |
| 15 | 64 | 80 | 4 | 3 | 3 | 5 | 0.50 |
| 20 | 77 | 96 | 6 | 3 | 3 | 6 | 0.61 |
| 30 | 96 | 120 | 6 | 4 | 3 | 7 | 0.77 |
| 50 | 117 | 146 | 6 | 4 | 3 | 7 | 0.91 |
| 75 | 129 | 161 | 6 | 4 | 3 | 7 | 0.98 |

The most points of a tank are those of the leader of a group in a corridor. A tank of the first three sectors is alone and has no group to lead.

Every lever has a limit, so difficulty flattens after about sector 75. From there, a run is a test of endurance at full difficulty.

The terrain gets harsher along four levers:

| Lever | Sector 1 | Sector 30 and beyond |
|---|---|---|
| Corridor width | 14–20 m | 8–14 m |
| Cover blocks | Plentiful | Sparse |
| Explosive crates per sector | 0–1 | 3–6 |
| Angled walls | Rare | Common |

The room of arena mode follows the two levers that are about what stands on the floor: for group $n$, it has the crates of sector $n$, and half as much cover again as that sector, because it has more floor. Its walls don't change.

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
| Room | The whole run, in arena mode only | 36–42 m wide, 22–24 m deep | Closed all around. Holds one group after another, and half of a group at once. |

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

The room of arena mode is one closed sector, small enough to fit on a screen with its reserve. Its corners are cut on the slopes of the lattice, and each of its two long walls has, one time in two, a stretch that stands out into the room. It has no entry and no barrier, so the first and the fourth guarantee read differently there: nothing stands within 5 m of the middle of the room, where the players appear, and every place where a tank appears is reached from there along a path at least 3 m wide, without destroying anything. A tank appears no closer to a player's tank than its mark allows.

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

Both are made of the characters that the interface draws: the letters of the Latin alphabet with their accents, figures and signs. What a player types then shows as it was typed on every machine. Both stay as they are until the host closes the game.

The password keeps out those who were not given it: a player who has not typed it is sent nothing of the game. It hides nothing from someone who listens to the network, and a short word that others could guess keeps out the honest alone.

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
| Arena mode | On or off (see [Arena mode](#arena-mode)) |
| Free mode | On or off. When it is on, each player builds their tank in their own garage (see [Garage](#garage)). |

A run starts once every player has said that they are ready, the host included. A player says so with a button of the lobby, or with Enter. The run then starts by itself after a countdown of 3 s, which the lobby shows. A player can take their word back at any time, which stops the countdown. Two changes take it back for them: a player who changes their tank in the garage is no longer ready, and when the host changes an option, nobody is. A player who joins isn't ready either.

A host who starts alone plays a solo run, with its lives, its checkpoints and its locks: a host who is alone can't be ready while the difficulty or an option is one that they haven't unlocked for solo.

When a run is over, the host decides what comes next: the same seed again, a new run, or the lobby. The first two start at once for everyone, with the same options and the same tanks: only the lobby asks who is ready. The host can also end a run from the menu, which brings everyone back to the lobby. Nobody is ready there until they say so again.

A host whom the others have left by then would go on alone, in a solo run. The locks of solo hold for that run too: the host is offered neither the same seed nor a new run while the difficulty or an option is one that they haven't unlocked for solo.

### Connection and latency

The menu of the host pauses the run for everyone: nothing moves until the host closes it, and every player is told on screen that the host paused. The menu of another player pauses nothing.

A player who disconnects loses their tank, and later sectors are generated for the remaining number of players. A player whose machine stays silent for 5 s has disconnected. If the host leaves, the run ends, and the other players are back at their main menu. All players must run the same game version, compiled with the same balance (see [BALANCE.md](BALANCE.md)): a game of another one doesn't answer, and the join screen lists it as one of another version.

The game must stay playable up to a round trip of 120 ms. What the host tells comes late, so a player who joined a game is shown three things in three ways:

- **Their own tank** responds to input immediately: it drives, aims, fires, lays mines and raises its shield at once, with the sounds of each. Their machine does all of it as the host will. When the host tells otherwise, the tank is brought to its place without a jump, and a shot or a mine that the host doesn't have goes away.
- **Bullets** are shown where they are at that moment, in the time of the player's own tank. What comes at a tank is where the screen shows it: a bullet that is dodged on screen is dodged, and a bullet ends where it meets the player's own tank or its bubble.
- **The other tanks** are shown about 50 ms in the past, between two tellings of the host, so that they move evenly.

## Determinism

Players get a random seed, or type their own: a solo player once that option is unlocked, and a host from the start. A seed is a short text. Together with the game version and the difficulty, it identifies a level.

| What | Depends on | Guarantee |
|---|---|---|
| Map: walls, blocks, crates, barriers | Seed, version | Identical on every platform |
| Room of arena mode: its walls, and the blocks, the crates and the places of the tanks of each group | Seed, version | Identical on every platform |
| Enemies: number, genomes, positions | Seed, version, mode, number of players | Identical on every platform |
| Enemy behavior | All of the above and the players' actions | Not guaranteed to repeat |
| Music: notes, patterns, sounds | Version | Identical on every platform |
| Effects: particles, camera shake | Nothing | Random |

Solo and co-op players on the same seed therefore drive through the same map, on a desktop or in a browser. A new game version may change what a seed generates, so seeds and records always show their version. In arena mode, a seed gives one room, and group $n$ there is drawn as the group of sector $n$ of that seed is, for as many players and with the number of tanks of a corridor: where that sector is a corridor, it is the same group, tank for tank.

Impossible mode keeps the first row of this table and gives up the second: its enemies depend on how the run has gone.

## Presentation

### Camera

The camera looks down at the ground at a steep angle. It shows about 30 m of ground from the bottom to the top of the screen, and more from side to side. Each player has their own view. It follows that player's tank smoothly and shifts a little toward the cursor, so players see further where they aim.

Among several players, the balloon of each teammate whose tank is out of view stands at the screen edge, toward that tank, without its leader. It is drawn in a full line for a tank that runs and in dashes for a wreck, as the balloons of the team are on the display. A player whose own tank is a wreck is shown where it lies in the same way. Markers that would hide each other, or the marker of one of the last enemies, stand side by side.

Once a sector has no enemy left and none of their bullets in flight, the view of a player whose tank comes close to its barrier runs up to 10 m ahead of the tank, past the barrier, so that the next group can be studied before crossing. It comes back to the tank once the tank is through.

North is at the top of the screen by default. A player can rotate their own view a quarter turn at a time, for instance to put north on the right and see further ahead on a wide screen. The view never rotates on its own, and the movement keys always follow the screen.

In arena mode, the view doesn't follow the tank: it holds on the room and on its reserve, which fit on a wide screen together, so that a single player sees all of the fight and the next group at once. Where the screen is too narrow, the view follows the tank as far as the walls. Among several players, whose groups take two rows in reserve, the view shows the reserve whole unless the tank is far in the north of the room.

### Visual style

The game is drawn as a technical drawing on a dark sheet, and nothing is lit: a solid shows its height by its darker sides. The floor carries a grid with a line every meter and a heavier one every 5 m. Walls are bands between two pale lines, destructible blocks are hatched and explosive crates are crossed in red. Tanks are assembled from parts by the game, not modeled by hand: the body is in the tank's color, and the other parts are dark and edged in pale lines. Bullets are the brightest objects on screen. They are white whoever fired them, because any bullet can destroy a player's tank, and each casts a patch of light on the floor. A bullet is an elongated shell with one dark stripe for each bounce it has left: it loses a stripe at every bounce, and a plain bullet won't bounce again. A special bullet has a shape of its own: a pointed tip while it pierces, a chevron in the place of each stripe when it seeks, a seam along it when it will split, and a flat end, with a red line that cuts it near that end, when it explodes. The exploding bullet is the only one with a trail, a thin one. The void outside the corridor is dark, and carries what a draftsman writes around a plan, in the finest line: the length of each wall and the angle of each wall that slopes, each given once, and short strokes along the outside of the walls. A line closes the mouth of a notch, along the walls on each side of it, so that its depth is given once. Where a corner of a wall hides under the wall of the next sector, as where the corridor turns, the length runs to the outer face of that wall and says what it spans.

The lines are those of a technical drawing: a wide line for the edges of what stands, a narrow one for the floor and what is written in the void, dashes for what isn't there, a chain line, a long dash and a dot, for a barrier and for the circles of an arena, and a phantom line, a long dash and two dots, for a reach: the circle within which a mine goes off, the one within which a wreck is repaired, and the one that a blast will reach.

A line in dashes stands for what isn't there. A dormant enemy is the outline of its tank in dashes, flat on the floor and in its color, with every part that tells a stat. The seeker, the flanker and the grenadier are too dark for that: dashes in their color would not show on the sheet, so their outlines are in the pale tint of it. Dashes also draw the way that a special bullet is about to fly, in white (see [Special bullets](#special-bullets)), and the circle that its blast will reach is a phantom line in red. A bullet that is destroyed leaves a short burst of white strokes. A life that the player can't take is the outline of itself in dashes. A tank in reserve is the outline of its tank in dashes too, on the void outside the room.

A block or a crate that is not in play yet lies flat, as its plan: the outline of a block with its hatching, the outline of a crate with its red cross. The plans are drawn as sharply as what stands, and the darker floor under them says that their sector waits. They stand up when the first player crosses the barrier, and they are plans again in a sector that a single player comes back in front of. The walls of the corridor are no plans: they stand in a sector that waits as in any other, drawn darker. In the room of arena mode, the blocks and the crates of the next group are planned the same way before they stand.

A sector takes a third of a second to wake, and everything in it starts at the same moment. Its light comes up. Each plan takes its face, pale at first, as ink that is still wet, and is pulled up off the sheet: a little past its height and back, a crate further than a block. Marks flick out from the corners of its foot when it lands. Each enemy is inked in from under its turret while its tracks, its hull and its turret rise one after the other, and its antennas last. The drawing holds nothing back: a block is at its height a tenth of a second after it wakes, and the nearest enemy starts 15 m from the barrier, which the fastest bullet needs 0.8 s to cover. Only what waited rises. The blocks and the crates of the room of arena mode waited as plans, and each rises in the same way when its plan is over. A tank that appears in an arena or in arena mode has its mark and its flash.

The floor of an arena bears two circles in chain lines around its middle, with the center lines that cross there, in a dull red, as the circles of holes are drawn. The place where a tank is about to appear is marked by a circle that closes in on it: a red one in an arena of the corridor, which tells nothing of the tank, and in arena mode one in the color of the tank's outline in reserve. The tank appears with a flash of the same color. A life on the floor is a square in the players' blue, like those that count the lives on the display, inside the circle within which it is taken. The room of arena mode bears no circles: nothing else is there to tell it from.

A fight leaves marks on the floor: a scalloped outline where a blast went off, a square in dashes where it took a block away, and a cross where a tank was destroyed. They last as long as their sector is loaded.

### Interface

Every screen is a sheet of one set, as a set of drawings is: a frame with its zones and its centring marks, and a title block in the bottom right corner that names what the screen draws, who it is drawn for, the version as its revision and the day as its date of issue. Its controls are the rows of a list, 56 px high at 1080p, and boxes of 44 px or more, and what it tells stands in ruled blocks, each with a label over it. A tank that a screen shows is drawn as an assembly: a balloon on each part that tells a stat, and its parts list over the title block. Everything is lettered in Tanku Lettering, a face of single strokes as on a drawing, in capitals no smaller than 14.8 px at 1080p, and one accent color stands out, the blue of the players.

| Screen | Content |
|---|---|
| Main menu | Solo, host, join, achievements, settings, quit, and the best runs of one difficulty at a time, in the corridor or in arena mode, in the standard tank or in free mode |
| Solo | Difficulty, seed, arena mode, free mode with its garage, what unlocks each of them while it is locked, a note when the run will earn no achievement, the teammates of a playtest and how well they play, start |
| Host | The name of the game, its password, open |
| Join | The games of the local network with their names and which of them have a password, the address of the host, the password of a game that has one, join. In a browser: how to join by the link of a host, and that link again once it was followed. |
| Lobby | The name of the game, the seats of the players, each signed by its player once they are ready, the difficulty, the seed, arena mode and free mode, which the host sets, the garage in free mode, a note when the run will earn no achievement, ready and the countdown to the run, the address to join at or the link for browsers with a button that copies it (host) |
| Garage | Every stat of the player's tank with its level, what it gives, its price and the price of one more level, the type of its bullets as one choice among five, the points that are spent and those that are left, the tank as it will look, back to the standard tank |
| Heads-up display | The parts list of the player's tank, in the bottom left corner: rounds and reload progress, mines left, shield state. The title block, in the bottom right corner: sector number, lives (solo) or the balloon of each player of the team, which tells how their tank is (multiplayer), seed, difficulty and version, a scale and a north arrow. Around them: a note on a run that earns no achievement, a ruler along the edge of the screen that gives the distance to the barrier, the number of each wave of an arena as it is announced, a caption over a life on the floor (solo), a balloon over each teammate's tank, one at the screen edge for each teammate out of view, and what to do about a wreck (multiplayer). Rounds, reload and mines are also on an arc under the player's own tank. In arena mode: the number of the group in place of the sector number, the next one during a lull with its last two seconds counted down, a message when a group comes, and no ruler. |
| Menu | Resume, settings, quit, and the seed with its version and the difficulty of a run that isn't in normal mode. In multiplayer, the host goes back to the lobby or closes the game, and the others leave it. |
| Game over | Sectors cleared, or groups in arena mode, time, kills, seed and version, the difficulty of a run that isn't in normal mode, the tank that ended the run with its class and the list of its parts, a note on a run that earned no achievement, retry with a note that it will earn none, new run. In multiplayer, the choice is the host's. |
| Settings | Controls, video, audio, accessibility |

The game opens in full screen the first time. After that, it opens the way the player left it.

### Audio

Sound carries information the eyes can miss: a shot fired off screen, a ricochet, a mine about to blow, a shield coming down, a wave on its way, a tank that appears. A few cues answer what the players do instead, and are among the quietest: a mine that is placed, and a sector of the corridor that wakes when the first of them crosses its barrier. The end of a fight is told the moment its last enemy is destroyed: bullets that still fly are no fight. A basic set of such cues arrives with the solo loop (M4), in time for the first real playtest. The full sound pass comes later.

The music is one piece that never stops, made by the game as it plays, the same for every seed and on every platform. Its drums and its mode follow the place and the fight, and its melody follows the highest brain tier awake. It never covers a cue. Its specification is in [MUSIC.md](MUSIC.md).

### Accessibility

Color is never the only carrier of information. Camera shake can be turned off, the interface can be scaled, and all controls can be rebound.

## Achievements and persistence

Achievements unlock modes and options for solo, and their conditions are listed under [Modes](#modes). A game of several players needs none of them: its host has every difficulty and every option from the start. An unlocked mode is another way to play, not an advantage: nothing a player unlocks makes the next achievement easier to earn. Three kinds of run therefore earn none:

- A run on a seed that the game didn't just draw for it: a chosen seed, or the seed of an earlier run that is played again. A seed can't be practiced to unlock them.
- A run in free mode, so that a tank can't be built to unlock them.
- A run in arena mode. The achievements are earned in the corridor: the groups of a room are fought without the way to them, and are no sectors cleared.

Every other run earns them, alone or as a team, and an achievement earned by a team is awarded to each member. A run that earns none says so on screen: before it starts, while it lasts and when it is over. The game over screen says it of a retry before the player picks one. A difficulty that is unlocked unlocks the ones below it: a team can clear sector 20 in hard mode before any of its members has unlocked hard mode. Arena mode came to the game after the other achievements: a player who has any of them has cleared its sector, and has it.

The game stores settings, bindings, unlocks, records and the tank of the garage on the player's machine, or in the browser's storage on the web. It also keeps what a solo player and a host chose last: the difficulty, arena mode, free mode and the name of the game, and never the password. Records keep the most sectors cleared for each difficulty and number of players, with the seed and the version, and apart for the runs in free mode. The runs of arena mode have records of their own in the same way, which keep the most groups cleared. The number of players is the one at the start of the run, and each player keeps the record on their own machine.

The game also keeps a journal on the player's machine: a line for each run that entered a sector, with its seed, its mode, the sectors that it entered, where each tank was lost and to what, and how it ended. The web build keeps none.

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

Arena mode came after M6 and is on no milestone: it is an experiment, which answers whether the fights of the game hold a run without the corridor between them. It is also where M7 can be tried first, since what changes there from one group to the next is the enemies alone.

The bullets that seek, split and explode came after M6 too, each with a class of its own, and are on no milestone either. They answer whether a group still reads when four classes in eleven fire a bullet of their own.

Later, in no fixed order: controller support, local co-op on one screen (split screen, or one camera that follows all players), the full sound pass, interface polish and more options, a performance pass, the official server, and brains that learn (see [Impossible mode](#impossible-mode)).

## Open questions

- How do internet games connect for hosts who can't open a port, before the official server exists?
- In impossible mode, how exactly are enemies scored, and can brains learn fast enough within one run to matter?
- Which exact shades do the hues of the classes take? The players' blue and the white of the bullets are settled.
- Are the classes that fire a special bullet still too many? Piercing alone was on one tank in eight. The classes are now drawn with weights, and those four weigh less and come late: five tanks in 24 are of them once all have come, at sector 19.
- Are the prices of the special bullets right? They were set by rank at first, and raised after the trials and after play: 6 points for piercing, 9 for splitting, 10 for seeking and for exploding.
- Do the three dark classes read on the dark sheet? A dark body has a pale frame, and their outlines in dashes take a pale tint. Whether a dormant flanker in that tint is told from a teammate's wreck is to be checked in a game of several players.
- How is the game distributed, at which secure address is its page published, and is the name free to use?
- How do the players of an early access release update the game, and how do they report bugs and feedback?
- Is a tank from the garage so much better than the standard tank, in the hands of a good player, that free mode becomes the easy way through? Its runs earn no achievement and keep records of their own, which limits the harm. The prices were set for enemies, whose danger also depends on their brains. The check comes before the garage is built: a playtest that takes its tank from the command line, and the same seed played with the standard tank and with two or three extreme ones.
- In arena mode, is a life for every fifth group enough for a single player, and are the eight tanks that four players face at once too many for one screen? A room that is kept full is harder than the corridor: the same groups land about 30% more shots in a minute on a crude player there.
