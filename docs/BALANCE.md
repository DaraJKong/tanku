# Tanku: balance

The numbers that decide how hard Tanku is are written in one text file, `tanku_core/game.balance`. This document describes that file: its notation, and what each of its lines says. [PROJECT.md](PROJECT.md) says what the numbers mean in the game and holds the same values. When the two disagree, one of them is out of date.

## Overview

| Topic | Summary |
|---|---|
| File | `tanku_core/game.balance`, written by hand |
| When it is read | While the game is compiled. The game carries its balance in itself and opens no file when it starts. |
| A file that can't be read | It stops the build, and the error says on which line and why |
| What it holds | Numbers: what the levels of the stats give and cost, the standard tank, the weapons, how the sectors get harder, how a group shares its points, the classes, the terrain, the arenas, arena mode, how the brains fight, and what unlocks the modes |
| What it doesn't hold | Rules. Which stats, classes and tiers there are, and what each of them does, is said by the code that reads the numbers. |
| Reader | `tanku_core/src/balance/read.rs`: a function that the compiler runs. A program can call it too. |
| Tests | They have a balance of their own, `tanku_core/test.balance`: a change to the balance of the game fails one test, which says so, and no other (see [Tests](#tests)) |

## Notation

The file is text. It has one record per line, with its words apart by spaces: as many spaces as line up the columns. An empty line is ignored, and so is a line that starts with `#`.

The first record is `tanku-balance 1`: the format of the file and its version. Every other record is there once, in any order, and none is missing.

A number is written right before its unit, without a space:

| What | Unit | Example |
|---|---|---|
| A time, in seconds | `s` | `1.5s` |
| A length, in meters | `m` | `8m` |
| A speed, in meters per second | `m/s` | `3m/s` |
| An angle, in degrees | `deg` | `45deg` |
| A turn, in degrees per second | `deg/s` | `720deg/s` |
| A share, in percent | `%` | `150%` |

A number without a unit is a count, a level, a sector or a number of points. Three signs join numbers:

| Sign | Meaning | Example |
|---|---|---|
| `..` | The least and the most of something | `8m..14m` |
| `to` | Where something starts and where it ends: in sector 1 and in the harshest sector, or without experience and with all of it | `9 to 3` |
| `+` | What is added to something | `+50%` |

Times, lengths that are not sizes of the map, speeds, angles and the numbers of a `stat` line can have decimals. Everything else is a whole number: points, shares, sectors, and the sizes of the map, which stand on a lattice of 1 m.

## Stats

```
stat engine      speed 0 2 3 4 5 6 7 8               cost 0 2 4 6 8 11 14 17
stat magazine                                        cost 0 3 6 9 12 15
stat shield      cooldown - 10 9 8 7 6               cost 0 6 9 12 15 18
stat brain                                           share 0 8 16 24 32
```

A `stat` line names a stat, then says what each of its levels gives and what each costs, from level 0 on. It has exactly as many numbers as the stat has levels:

| Stat | Levels | What a level gives | Then |
|---|---|---|---|
| `engine` | 0 to 7 | `speed`: its top speed, in m/s. Level 0 doesn't drive: its speed is 0. | `cost` |
| `tracks` | 0 to 6 | `turn`: the turn rate of the hull, in degrees per second | `cost` |
| `turret` | 0 to 6 | `turn`: the turn rate of the turret, in degrees per second | `cost` |
| `muzzle` | 0 to 7 | `speed`: the speed of its bullets, in m/s | `cost` |
| `magazine` | 0 to 5 | Its level and one more, in rounds | `cost` |
| `loader` | 0 to 7 | `reload`: the time to reload a round, in seconds | `cost` |
| `ricochet` | 0 to 3 | Its level, in bounces | `cost` |
| `mines` | 0 to 4 | Its level, in mines | `cost` |
| `shield` | 0 to 5 | `cooldown`: the time before the shield comes back, in seconds. Level 0 is no shield, and is written `-`. | `cost` |
| `piercing`, `seeking`, `splitting`, `exploding` | 0 and 1 | The type of the bullets | `cost` |
| `brain` | 0 to 4 | What its tier does | `share` |
| `ballistics` | 0 to 3 | What its tier does | `share` |

After `cost` come the points that each level costs in all, 255 at most. After `share` comes the share of the budget of its tank that each tier of software costs, in percent: a tank with a budget of $b$ pays $\lceil b\,s / 100 \rceil$ points for a tier of share $s$. Level 0 is free, and no level costs less than the one before it.

The file says what the levels are worth, and not how many there are: the parts of a tank are drawn for these levels.

## Players

```
player  engine=4 tracks=5 muzzle=2 magazine=4 loader=6 ricochet=1 mines=2 shield=2
garage  59
solo    lives 3  respawn 1.5s
tank    turret 720deg/s  speed-up 0.15s  pivot 45deg  hold-fire 0.5s
```

| Line | What it says |
|---|---|
| `player` | The standard tank: its stats with their levels. A stat that isn't named is at level 0. It is a tank that a player may build: it has an engine, one type of bullet at most, neither turret nor software, and the points of the garage pay for it. |
| `garage` | The points of a tank that a player builds in free mode |
| `solo` | The lives of a single player, and how long after its tank was destroyed the player is back |
| `tank` | What a tank does whatever its stats: the turn rate of the turret of a player, the time a tank takes to reach its top speed, how far its hull is from where it wants to go when it stops to pivot, and how long the tank of a player can't shoot after it appears |

## Weapons

```
gun     cooldown 0.25s  bullet 0.225m
mine    arm 1s  fuse 10s  warn 0.6s  trigger 2m  every 0.5s
blast   3m
shield  lasts 1.0s  radius 1.4m
split   after 1s  angle 30deg
mark    1.5s
repair  within 3m  takes 2s  safe 1s
```

| Line | What it says |
|---|---|
| `gun` | The time between two shots of any gun, and the radius of a bullet |
| `mine` | How long a mine takes to arm, how long after it was laid it goes off by itself, how long it warns first, how close another tank than the one that laid it comes to set it off, and the time between two mines of a tank. The warning is no longer than the fuse. |
| `blast` | How far the blast of a mine, of a crate and of an exploding bullet reaches |
| `shield` | How long the bubble lasts, and its radius |
| `split` | How long a splitting bullet flies before it becomes two, and how far to each side of its line of flight the two leave |
| `mark` | How long before it happens the bounce of a seeking bullet is marked, and the end of an exploding one on what stands still |
| `repair` | How close to a wreck a tank repairs it, how long that takes, and how long nothing destroys the tank that was repaired |

## Progression

```
budget      10 to 135  every 25
experience  every 20  reaction 0.5s to 0.12s  aim 8deg to 1deg
step tanks       1  4 8 12 16 20
step brain       0  2 6 14 22
step ballistics  0  3 10 15
step muzzle      2  5 7 13 18 25
step piercing    0  9
step splitting   0  11
step exploding   0  17
step seeking     0  19
team    +50%
hard    budget +30  ahead 20
```

**`budget`** is the budget of a sector: its first value, its cap, and the number of sectors $k$ over which it covers 63% of what is left to the cap. In sector $n$:

$$B(n) = B_{cap} - (B_{cap} - B_1)\, e^{-(n-1)/k}$$

rounded to the nearest point. The compiler works the budgets of the first 256 sectors out into a table, so generation reads whole numbers. The sectors after them have the budget of the last one.

**`experience`** is how well the brains execute. It goes from 0 toward 1 in the same way, $E(n) = 1 - e^{-(n-1)/k}$, and takes the delay before a brain reacts and its largest aiming error from their first value to their last: a brain of experience $E$ reacts after $r_0 - (r_0 - r_1)\,E$.

**`step`** is something that grows by steps: its value in sector 1, then each sector where it goes up by one, in rising order. It goes up eight times at most.

| Step | What grows |
|---|---|
| `tanks` | The tanks of the group of a single player |
| `brain` | The highest tier of brain that an enemy can have |
| `ballistics` | The highest tier of ballistics |
| `muzzle` | The highest level of Muzzle |
| Any other stat, as `exploding` or `ricochet` | The highest level of that stat. This line is optional: a stat without one has every level from sector 1. |

The first four lines are in every balance. The balance of the game has one more for each special bullet, which keeps its class for the sector of that line. A cap ends no higher than its stat goes, and it is for the enemies alone: a player's tank has what its points pay for. A cap of 0 keeps the stat out of a sector altogether, and with it the classes that favor it (see [Classes](#classes)). In hard mode, a cap of the hardware is the one of some sectors further, as the cap on Muzzle is.

**`team`** is what each player after the first adds to the tanks of a group: $P$ players face $\lceil N\,(1 + t\,(P - 1)) \rceil$ tanks where one faces $N$.

**`hard`** is what hard mode adds: points on top of the budget of every sector, and the number of sectors by which its experience and its cap on Muzzle are ahead of normal mode. Its software has every tier from the first sector, which is a rule and not a number.

## Groups

```
group   mean 70%..90%  least 30%  leader 125%  weights 1 1 2 2 3 8  favorite 15
```

How a group shares its points. The shares are in percent of the budget of its sector.

| Word | What it says |
|---|---|
| `mean` | What a tank of a group has on average: each group draws its own between the two |
| `least` | The least that a tank has. It is no more than the smaller `mean`. |
| `leader` | The most that the leader of a group has, 100% at least. The others stop at the budget of the sector. |
| `weights` | The weights by which a group shares what is left once every tank has the least: each tank draws one. Eight at most. |
| `favorite` | How much likelier the stats that a class favors are to receive the next point, all of them together, than any other stat |

## Classes

```
class all-rounder  3                      ricochet<=1  ballistics<=2
class sniper       3  muzzle turret       ricochet<=1  ballistics<=2  keeps muzzle
class seeker       1  ricochet seeking    ricochet<=2  ballistics<=1
```

A `class` line names a class, then says three things, and a fourth for some classes:

- **Its weight** when a tank draws its class. A class of weight 2 comes twice as often as one of weight 1, and one of weight 0 never comes.
- **The stats that it favors**, two at most, the one that it takes first in front. A class that favors one of the four special bullets is the one class that fires it.
- **The most that it has** of ricochet and of ballistics, the two stats that are kept for some classes.
- **What it keeps**, after `keeps`: stats that it favors, whose highest level is its alone. That level is the highest that the sector allows, which a `step` line may hold back. `keeps muzzle` gives the sniper the fastest gun of every sector, and a tank of another class has a gun one level slower. A special bullet is its class's already, and is kept by no line.

Ballistics 2 needs a bounce and ballistics 3 needs two, so a class has no more ballistics than its ricochet and one. A class that favors `seeking` has a bounce.

A class comes from the first sector that allows a level of every stat that it favors, and of what those stats need: mines and a shield need a brain, tracks an engine, and a bullet that seeks a bounce. A sapper therefore waits for the first brain. The `step` lines say which sectors those are: with `step brain 0  2 6 14 22` and `step ballistics 0  3 10 15`, the brain comes at sector 2 and ballistics at sector 3, so the first tactician is in sector 3. Until then its weight counts for nothing, and the other classes share the draw. Hard mode has every tier from its first sector, and still takes its classes from the sectors of normal mode: a tank is of the same class in both. Any stat can have a `step` line, so any class can be held back: `step exploding 0  17` keeps the grenadiers for sector 17, and `step splitting 0  11` the flankers for sector 11.

The classes are `all-rounder`, `sniper`, `scout`, `tactician`, `trickster`, `lancer`, `sapper`, `gunner`, `seeker`, `flanker` and `grenadier`. What else tells them apart is a rule: a tactician takes the best software that its sector allows, and a sapper has a brain.

## Terrain

```
terrain harshest 30
terrain width    14m to 8m  spread 6m
terrain cover    9 to 3
terrain crates   0..1 to 3..6
terrain angled   10% to 70%
terrain spawn    entry 15m  barrier 3m
corridor 40m..80m
```

What gets harsher goes from its value in sector 1 to its value in the harshest sector, in equal steps, and stays there.

| Line | What it says |
|---|---|
| `terrain harshest` | The sector from which the terrain is as harsh as it gets, 2 at least |
| `terrain width` | The narrowest that a corridor gets, and how much wider than that it gets |
| `terrain cover` | The groups of cover blocks of a sector |
| `terrain crates` | The fewest and the most explosive crates that a sector draws. Fewer stand: a crate that finds no room, or that closes the way through, is left out (see [the progression](PROJECT.md#progression)). |
| `terrain angled` | The chance that a change of width is an angled wall and not a square step |
| `terrain spawn` | How far from the entry of their sector the enemies start at least, and how far from its barrier |
| `corridor` | The shortest and the longest that a sector of the corridor gets |

The generator keeps its promises whatever these numbers are, as far as it can: its tests say when a change leaves a sector without a way through.

## Arenas and arena mode

```
arena   every 5..8  tanks 150%  waves 3  wait 2s  mark 1.0s  clear 4m  life 4m  reach 1.5m
room    at-once 50%  lull 3s  life-every 5  rise 2s  apart 2m  cover 150%  size 36m..42m 22m..24m
```

| Word of `arena` | What it says |
|---|---|
| `every` | An arena comes every so many sectors: the fewest and the most |
| `tanks` | The tanks of its group, in percent of those of a corridor, rounded up |
| `waves` | The waves that its group comes in |
| `wait` | The time between the entry of the first player and the warning of the first wave |
| `mark` | How long the place of a tank is marked before the tank appears |
| `clear` | A mark waits while the tank of a player is this close to its place |
| `life` | How far inside its barrier an arena that is cleared leaves a life |
| `reach` | How close a tank comes to a life to take it |

| Word of `room` | What it says |
|---|---|
| `at-once` | The share of a group that the room holds at once, rounded up. A group of two comes whole, whatever the share. |
| `lull` | The time before a group comes |
| `life-every` | A single player gets a life back for every so many groups cleared |
| `rise` | How long the place of a block or of a crate is drawn before it stands there |
| `apart` | A tank doesn't appear this close to a tank of its group |
| `cover` | Its groups of cover, in percent of those of the sector of the same number |
| `size` | The narrowest and the widest that the room gets, then the least and the most deep, in even numbers of meters |

## Brains

```
enemy keep   8m..14m  hold 1.25  foresight 0.7s  last-moment 0.2s  room 0.15m  parry 0.3m
enemy mines  reflex 6m  range 20m  apart 5m  charge 3m/s
enemy ace    bait 11m..14m  press 5m..8m  until 3  retreat 16m..22m  crossfire 90deg
enemy sight  30m
```

| Line | What it says |
|---|---|
| `enemy keep` | The distances that a brain of tier 1 or more keeps from the player. `hold`: it keeps its target until another player is this many times nearer. `foresight`: how far ahead it sees a bullet coming. `last-moment`: how long before a bullet hits a soldier or a veteran raises its shield. `room`: the room that a veteran wants between its tank and a bullet to count on a miss. `parry`: a veteran fires at a bullet once its shot will pass this close to the middle of it. |
| `enemy mines` | `reflex`: a brain lays a mine when the player is this close. `range`: a veteran lays mines in the way of a player who is no further than this. `apart`: it lays none this close to another one. `charge`: a player who comes at a tank this fast charges it. |
| `enemy ace` | The distances that an ace keeps from a player who has rounds (`bait`) and from one who has none (`press`), until that player has so many rounds again (`until`), and those that it keeps when it is the last of its group (`retreat`). `crossfire`: the turn around the player that it wants between itself and the nearest ally. |
| `enemy sight` | How far an enemy fires at a player. A file may leave this line out. |

**`foresight`** is how long before a bullet arrives a brain starts to see it. A soldier sees a bullet that flies straight at its tank once the bullet is that long from hitting it. A veteran follows every bullet for that long, bounces included, and sees those that will pass over its tank. An ace also sees those that will pass where it can drive to in that time, which is how it gets out of the way of a shot that is aimed ahead of it. A brain acts on a bullet once it has seen it for the reaction of its sector, so what it has left to get out of the way is the foresight less that reaction. With `foresight 0.7s`, a shot of the standard tank, which flies at 9 m/s, is seen from 6.3 m away. With a foresight of 1 s, which the game had first, it was seen from 9 m: a brain that keeps 8 to 14 m from the player then saw it almost as it was fired, and any tank that has an engine had the time to move aside. A shorter foresight leaves the slow tanks too little time, and the fast ones still enough.

**`sight`** is how far an enemy fires. With the line, an enemy fires at a player who is no further from its tank than this. Further than that, it holds its fire: it still knows where the player is, keeps its gun on them and drives toward them as its brain says, and it fires once the player is within its sight, after the reaction of its sector. It still shoots down a bullet that comes at it, wherever the player is. Without the line, an enemy fires at a player that it has a line of fire on, however far. A sight is no shorter than the furthest that a brain keeps from the player in `enemy keep`, or the brains that keep that distance would never fire.

## Unlocks

```
unlock  arena 10  seed 15  free 20  hard 30  impossible 20
```

The sector that a run must clear to unlock each mode. The last one is cleared in hard mode.

## What stays in the code

- **Sizes that drawings and the map stand on**: the radius of a tank, the side of a block, the length of a barrel.
- **How the walls of the corridor are shaped**, and the lengths of the sectors that turn.
- **What a brain does at each tier**, and the timers inside it.
- **What a reader of numbers can't change**: the tick, the search for a bank shot, how far a brain looks ahead.

A number of these that turns out to matter gets a word in the file, and the version on its first line goes up when a file of today would no longer read.

## Changing the balance

A change is tried with the trials of the balance, which the README describes: the report of a trial as the game is, a change to a line of the file, the report again, and the comparison of the two. Each report names the balance that it was made with by its fingerprint, a number made of the words of the file: comments and the room between two words change nothing of it.

Two builds of different balances don't play together. Each machine works out the tank of its own player by its balance, so two that differ would never agree on it. A hosted game says which balance it plays by: the join screen lists a game of another balance as one of another version, and such a game doesn't answer whoever joins it by its address or by its link. After a change to the file, every machine that plays together is built again, and so is the page for browsers.

A change that is kept is written down in two more places:

1. **[PROJECT.md](PROJECT.md)**, which holds the same numbers in its tables and its text.
2. **The test that watches the file**, `the_balance_of_the_game_is_the_one_that_the_specification_describes` in `tanku_core/src/balance.rs`. It holds the fingerprint of the balance that the specification describes, and fails when the file has another, with the fingerprint to give it once the specification is up to date.

## Tests

The tests of the rules have a balance of their own, `tanku_core/test.balance`: the numbers that the game had at version 0.9.0, in the same notation. `tanku_core` is compiled with that file for its own tests and for those of the other crates, which ask for it with the feature `testing`. A test that says that group 12 has four tanks, or that a tank of 58 points fits the garage, holds numbers of that file. So a change to the balance of the game fails no test of a rule: it fails the one test that watches the file, and nothing else.

What the tests then don't say is that the rules hold with the numbers of the game. Three things say it:

- **The reader** refuses a file whose numbers can't work together, while the game is compiled.
- **The trials** play the game as it is compiled, with the balance of the game.
- **The tests themselves, on request**, run on the balance of the game:

```bash
cargo test --workspace --features tanku_core/game-balance
```

There, a test that holds a number fails where the two files differ, and the others say whether the rules still hold. The tests of the generator are among them: after a change to the terrain, those that hold no width say whether every sector still has its way through.

The benchmarks are built as the tests are, so they measure the balance of the tests unless they are given the same feature: `cargo bench -p tanku_core -p tanku_server -p tanku_audio --features tanku_core/game-balance` measures the game. Whatever `cargo run` starts, the game and the trials, is compiled with the balance of the game.

The balance of the tests doesn't follow the one of the game. Bringing it up to date is a change of its own, with every test that holds one of its numbers: it is worth it when the two have grown so far apart that the tests no longer look like the game. It was done once, when the balance of version 0.8.0 became the game's: 55 tests held a number of the file of before.

