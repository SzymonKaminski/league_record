# League Record

This program automatically detects when a League of Legends game is running and records it.
Currently only supports Windows and is only tested for Windows 10 with an AMD GPU.

![screenshot](https://user-images.githubusercontent.com/37913466/167213695-295f5abc-02bd-471a-a31a-d65e530564f5.png)

## Keybindings

| Key         | Function           |
|:-----------:|:------------------:|
| Space       | Play/Pause         |
| Arrow Right | +5 seconds         |
| Arrow Left  | -5 seconds         |
| Arrow Up    | +10% volume        |
| Arrow Down  | -10% volume        |
| f           | toggle fullscreen  |
| m           | toggle mute        |
| >           | +0.25 playbackrate |
| <           | -0.25 playbackrate |
| Esc         | exit fullscreen    |

## Settings

To adjust the settings create/change the settings.json file in the installation path. There should be an example-settings.json file for reference.

|       Name       |                                              Value                                              | Description                                                                                                                                                                                                                                                                                |
|:----------------:|:-----------------------------------------------------------------------------------------------:| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| recordingsFolder |                          String (only valid symbols for a foldername)                           | The name of the folder in which the recordings are stored                                                                                                                                                                                                                                  |
|  filenameFormat  |                               String (with special placeholders)                                | Format string for naming new recordings. Can contain [special placeholders](https://docs.rs/chrono/latest/chrono/format/strftime/index.html) in order to make each name unique. If a new recording has the same name as an already existing recording, the old recording gets overwritten! | 
| encodingQuality  |                                positive whole number from 0-50                                  | Determines the size vs. quality tradeoff for the mp4 files. Zero means best encoding quality with a big filesize. 50 means heavily compressed with a small filesize.                                                                                                                       |
| outputResolution |                      ['480p', '720p', '1080p', '1440p', '2160p', '4320p']                       | Sets the output resolution of the recordings.                                                                                                                                                                                                                                              |
| outputFramerate  |                              [whole number > 0, whole number > 0]                               | Sets the framerate of the recordings as a fraction (numerator/denominator). <br> e.g. [30, 1] => 30fps, [30, 2] => 15fps                                                                                                                                                                   |
|   recordAudio    |                                          true \| false                                          | Determines if audio gets recorded.                                                                                                                                                                                                                                                         |
|   markerFlags    |{ 'kill', 'death', 'assist', 'turret', 'inhibitor', 'dragon', 'herald', 'baron' } : true \| false| Choose which events are shown by default in the timeline when playing a recording.                                                                                                                                                                                                         |

## Performance

|                             | CPU | RAM    | GPU   |
| --------------------------- |:---:|:------:|:-----:|
| idle                        | ~0% | ~5MB   | 0%    |
| background recording        | ~2% | ~140MB | ~2%   |
| play recording     |        | ~3% | ~130MB | ~1%   |
| recording and video playing | ~5% | ~280MB | ~3%   | 

When nothing is happening - no league game is being recorded and the program is minimized to the taskbar - the app is idling and takes up minimal resources.
During recording a subprocess "lol_rec" is started which records the game. This subprocess takes up most of the resources.

## Release / Build

**Currently [static-file-server](https://github.com/halverneus/static-file-server) is used as a replacement for the broken tauri asset protocol. This will be removed when the asset protocol is fixed**

There is a release for Windows-x64, but you can build the project on your own.
This project relies on libobs (27.2.4) to record the game.
For build prerequisites look at [libobs-recorder](https://github.com/FFFFFFFXXXXXXX/libobs-recorder)
Build with `cargo tauri build`.
Package up with `tar -cvzf LeagueRecord.tar.gz -C src-tauri libobs/data libobs/obs-plugins -C libobs *.dll *.exe -C ../target/release LeagueRecord*` (assuming that you have all your obs .dll's and the data/plugin folders in src-tauri/libobs/)

## License

The GPLv2 license of the Obs project applies to all the .dll files as well as the files in the ./data and ./obs-plugins folders in the distributed version.

The static-file-server executable is licensed under the MIT License.

This project is distributed under the GNU General Public License v2.
