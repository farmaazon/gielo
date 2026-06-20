# Simple Curling Simlator

A simulation of curling game, for training giving right calls - the execution (including sweeping) is outside control of players.
Be sure to consider all possible outcomes! Written in Rust for 

You may play [browser version](https://farmaazon.github.io/gielo/): set-up all player names and their accuracy (average error of weight and angle), and press Start.
Click at sheet to put marker and see where is most probable position of the stone (do not adjust marker for sweeping - it is is assumed to be "included" in delivering player's accuracy). Right-click for quick switching rotation. The weight is expressed in hog-to-hog time - you may check "automatic" to set weight to the level of the marker.

## Building desktop version

```
$ cargo run --release
```
Tested mostly on Linux, but should work on all platforms supported by Slint. 

## Simulation details

To make simulation fast for analysis (e.g. for quick creating "heatmaps" of possible outcomes), it is tracked not step-by-step, but from-event-to-event - having one starting
situation, we compute when next event happens, which may be collision, escape or stopping of the stones - after event new vectors of velocity and accelleration are computed.

The "curling" aspect of stone trajectory is simulated by constant* force applied perpendicularly to the movement. As this accelleration rotates over time, it too hard math
to keep it accurate - instead, it is updated after each time quantum (the length of such quantum is adaptive - it ends when the accuracy vector becomes too off the actual one, so it's longer for high velocities).

*Which is not how actual curling stones behave, which tend to curl more when their rotation slows down. Something to be improved in the future.