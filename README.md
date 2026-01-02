# Midi and Virtual Book jukebox Player

A cross-platform MIDI and virtual book jukebox player. It only includes the necessary functionalities to play MIDIfied instruments, such as Yamaha pianos and barrel/fair/dance organs. This program transforms a standard x64 touch PC tablet into a playback screen device.

This software is a simplier "Vanbasco player" alternative.


![](doc/tablet.png)

## Features:

- Play MIDI files from a folder, in 1 click
- Easy send Midi file to midi Output (midi virtual hub, or other midi cables)
- Play organ specific Book Files (barrelorgandiscovery aprint software), with custom midi translation
- View the playback in piano roll format, and book layout
- Large and clear tree view for files.
- Load prepopulated playlists (text files containing lists of files).
- Add files to the current playlist using a number and the enter key. (midi files with numbers)
- Manage a playlist Remove a file from the playlist by clicking on it.
- Skip the current playing file for exploration.
- Send panic or note-off messages to all channels when starting/stopping playback.
- Add stars to files to see your best midi files
- Number of play of Midi file remembered
- Dark and Light theme
- Launchable with fullscreen mode to get rid of the windows
- Available in en/fr/nl, translations may also be added 
- Adjust zoom factor for touch and reading from far await.
- Work on linux, windows (macos untested), single executable soft (all in one), implemented in rust, hardware display acceleration

## Download and install

The software is available for linux, windows [https://github.com/barrelorgandiscovery/virtualbookplayer/releases](https://github.com/barrelorgandiscovery/virtualbookplayer/releases). 

![](doc/20230623_195212.jpg)


## Other setup :

![](doc/20230617_194321.jpg)

## Screen shots :

Fileview, and playlist construction :
![](doc/ss1.png) 

Piano Roll display :
![](doc/ss2.png) 

Midi output selection : (don't need to restart the application, plug and choose the output port)
![](doc/ss3.png) 


## launch using command line

several options are available thought command line, 

```
    virtualbookplayer --help
```

this returns : 

```
Usage: virtualbookplayerapp [OPTIONS]

Options:
  -r, --reset-preferences      Name of the person to greet
  -l, --lang-code <LANG_CODE>  
  -h, --help                   Print help
  -V, --version                Print version

```

### option -r, --reset-preferences  

this option reset the saved preferences (window size, output midi port, ... )

### option  -l, --lang-code <LANG_CODE>  

Overload the window language, and take this value instead of trying to detect it

There are two languages available : french and english, the software try to detect it.
if not detected english is taken

codes are two char ISO country code in lower case
feel free to create a pull request and provide other languages translations

