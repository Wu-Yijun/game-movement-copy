# Game Movement Copy

Many games, especially platform jumping games, require you to perform the same operation repeatedly to advance, but it is difficult for ordinary players to advance to the last position in one go, and we often make _mistakes_ in the middle. This is very frustrating, especially when we need to repeat the same action over and over again to reach the last progress, advance a little, and then die... We play games, not to train hand muscle memory!

Repeating the same action over and over is difficult and boring, so this project aims to help players complete the last successful operation with accurate memory, and then players can take over the program at any time and continue the game based on the last time.

## Default Setting

`config.yaml` contains: 
- interval: Interval of Controller listener.
- enable_mouse: Listen to mouse.
- enable_keyboard: Listen to keyboard.
- enable_controller: Listen to any of the four controller.
- screen_scale: screen scale of system, used to align listen mouse and emulated mouse.

## Default Short Cuts

| Key bindings                   | Descriptions                                           |
| ------------------------------ | ------------------------------------------------------ |
| `Alt` + `1`                    | Start Recording                                        |
| `Shift` + `Alt` + `1`          | Stop Recording                                         |
| `Shift` + `Escape`             | Stop Recording and Discard                             |
| `Alt` + `2`                    | Start Playback                                         |
| `Shift` + `Alt` + `2`          | Stop Playback                                          |
| `Shift` + `Escape`             | Stop Playback                                          |
| `Escape`                       | Stop Playback and Record at current position.          |
| `Ctrl` + `Alt` + `1`           | Start Recording And Append to _Last Recording Result_. |
| `Ctrl` + `Shift` + `RMB` + `S` | Save Last Recording Result to file.                    |

----

Now it is basically done, but I found that emulated controller may be detected as P2 by game, thus may not control your character.
