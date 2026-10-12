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
shield  lasts 0.5s  radius 1.4m
split   after 1s  angle 30deg
mark    1.5s
repair  within 3m  takes 2s  safe 1s
```

| Line | What it says |
|---|---|
| `gun` | The time between two shots of any gun, and the radius of a bullet |
| `mine` | How long a mine takes to arm, how long after it was laid it goes off by itself, how long it warns first, how close an opposing tank comes to set it off, and the time between two mines of a tank. The warning is no longer than the fuse. |
| `blast` | How far the blast of a mine, of a crate and of an exploding bullet reaches |
| `shield` | How long the bubble lasts, and its radius |
| `split` | How long a splitting bullet flies before it becomes two, and how far to each side of its line of flight the two leave |
| `mark` | How long before it happens the bounce of a seeking bullet is marked, and the end of an exploding one on what stands still |
| `repair` | How close to a wreck a tank repairs it, how long that takes, and how long nothing destroys the tank that was repaired |

## Progression

```
budget      10 to 100  every 15
experience  every 20  reaction 0.5s to 0.12s  aim 8deg to 1deg
step tanks       1  3 6 9 12 15
step brain       1  6 11 16
step ballistics  0  3 8 13
step muzzle      2  4 9 14 19 24
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

A cap ends no higher than its stat goes.

**`team`** is what each player after the first adds to the tanks of a group: $P$ players face $\lceil N\,(1 + t\,(P - 1)) \rceil$ tanks where one faces $N$.

**`hard`** is what hard mode adds: points on top of the budget of every sector, and the number of sectors by which its experience and its cap on Muzzle are ahead of normal mode. Its software has every tier from the first sector, which is a rule and not a number.

## Groups

```
group   mean 70%..90%  least 30%  leader 150%  weights 1 1 2 2 3 8  favorite 20
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
class all-rounder  1                      ricochet<=1  ballistics<=2
class seeker       1  ricochet seeking    ricochet<=3  ballistics<=1
```

A `class` line names a class, then says three things:

- **Its weight** when a tank draws its class. A class of weight 2 comes twice as often as one of weight 1, and one of weight 0 never comes.
- **The stats that it favors**, two at most, the one that it takes first in front. A class that favors one of the four special bullets is the one class that fires it.
- **The most that it has** of ricochet and of ballistics, the two stats that are kept for some classes.

Ballistics 2 needs a bounce and ballistics 3 needs two, so a class has no more ballistics than its ricochet and one. A class that favors `seeking` has a bounce.

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
| `terrain crates` | The fewest and the most explosive crates of a sector |
| `terrain angled` | The chance that a change of width is an angled wall and not a square step |
| `terrain spawn` | How far from the entry of their sector the enemies start at least, and how far from its barrier |
| `corridor` | The shortest and the longest that a sector of the corridor gets |

The generator keeps its promises whatever these numbers are, as far as it can: its tests say when a change leaves a sector without a way through.

## Arenas and arena mode

```
arena   every 5..8  tanks 150%  waves 3  wait 2s  mark 1s  clear 4m  life 4m  reach 1.5m
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
| `at-once` | The share of a group that the room holds at once, rounded up |
| `lull` | The time before a group comes |
| `life-every` | A single player gets a life back for every so many groups cleared |
| `rise` | How long the place of a block or of a crate is drawn before it stands there |
| `apart` | A tank doesn't appear this close to a tank of its group |
| `cover` | Its groups of cover, in percent of those of the sector of the same number |
| `size` | The narrowest and the widest that the room gets, then the least and the most deep, in even numbers of meters |

## Brains

```
enemy keep   8m..14m  hold 1.25  foresight 1s  last-moment 0.2s  room 0.15m  parry 0.3m
enemy mines  reflex 6m  range 20m  apart 5m  charge 3m/s
enemy ace    bait 11m..14m  press 5m..8m  until 3  retreat 16m..22m  crossfire 90deg
```

| Line | What it says |
|---|---|
| `enemy keep` | The distances that a brain of tier 1 or more keeps from the player. `hold`: it keeps its target until another player is this many times nearer. `foresight`: how far ahead it sees a bullet coming. `last-moment`: how long before a bullet hits a soldier or a veteran raises its shield. `room`: the room that a veteran wants between its tank and a bullet to count on a miss. `parry`: a veteran fires at a bullet once its shot will pass this close to the middle of it. |
| `enemy mines` | `reflex`: a brain lays a mine when the player is this close. `range`: a veteran lays mines in the way of a player who is no further than this. `apart`: it lays none this close to another one. `charge`: a player who comes at a tank this fast charges it. |
| `enemy ace` | The distances that an ace keeps from a player who has rounds (`bait`) and from one who has none (`press`), until that player has so many rounds again (`until`), and those that it keeps when it is the last of its group (`retreat`). `crossfire`: the turn around the player that it wants between itself and the nearest ally. |

## Unlocks

```
unlock  arena 5  seed 10  free 20  hard 30  impossible 20
```

The sector that a run must clear to unlock each mode. The last one is cleared in hard mode.

## What stays in the code

- **Sizes that drawings and the map stand on**: the radius of a tank, the side of a block, the length of a barrel.
- **How the walls of the corridor are shaped**, and the lengths of the sectors that turn.
- **What a brain does at each tier**, and the timers inside it.
- **What a reader of numbers can't change**: the tick, the search for a bank shot, how far a brain looks ahead.

A number of these that turns out to matter gets a word in the file, and the version on its first line goes up when a file of today would no longer read.
