# Translate a .book file to MIDI

A `.book` file describes a cardboard music book. This player speaks MIDI. The YAML file is the small table that says which cardboard track becomes which MIDI note.

You write one YAML file per instrument scale. Every book punched for that scale can share it.

## What a .book file contains

`.book` is the virtual-book format of [Barrel Organ Discovery](https://www.barrel-organ-discovery.org/) and [APrint](https://github.com/barrelorgandiscovery/aprintproject). It is plain XML. You can open it in a text editor. The layout is described in [Carton virtuel](https://barrelorgandiscovery.github.io/APrintDoc/product/CartonVirtuel/). The XML namespace is `http://barrelorgandiscovery.org/virtualbook/2016`.

Two parts matter for playback:

- The **scale** (`gamme`) names the instrument and lists its tracks: melody notes, percussion, and register commands. Public scale files are listed on [Gammes](https://www.barrel-organ-discovery.org/instruments/gamma/).
- The **holes** are the punches. Each hole has a track number, a start time, and a length. This player reads those times as microseconds, the same unit as the [APrint scripting console](https://barrelorgandiscovery.github.io/APrintDoc/product/scripting/scripting_first_steps_quickscripts/).

A short excerpt looks like this:

```xml
<ns:scale name="27 notes"
    xmlns:ns="http://barrelorgandiscovery.org/virtualbook/2016">
  <ns:definition>
    <ns:tracks>
      <ns:track xsi:type="ns:TrackNoteDef"
          xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
        <ns:no>0</ns:no>
        <ns:note>G3</ns:note>
      </ns:track>
    </ns:tracks>
  </ns:definition>
</ns:scale>
<ns:holes>
  <ns:hole timestamp="900000" length="200000" track="0"/>
</ns:holes>
```

Track `0` is a G3 key. The hole starts at 0.9 s and stays open for 0.2 s. The player will send a note-on at the start and a note-off at the end, at full velocity (127).

The scale name is the `name` attribute. Here it is `27 notes`. That name chooses the YAML file.

## Where to put the YAML file

Name the file after the scale, and add `.yml`:

```text
27 notes.yml
```

Put that file in the folder from which you start the program. The player looks there, using the scale name stored inside the book. A book named `waltz.book` with scale `27 notes` still uses `27 notes.yml`.

Spaces and capitals must match the scale name. `52 Limonaire` needs a file called `52 Limonaire.yml`. A full example for that scale is in the repository as [`sample 52 Limonaire.yml`](../sample%2052%20Limonaire.yml). Copy it and drop the `sample ` prefix so the player can find it.

When the file is absent, the player builds a default map from the scale. That default is described at the end.

## The smallest useful file

```yaml
name: 27 notes
mapping:
  0:
    midi_channel: 0
    note: 55
  1:
    midi_channel: 0
    note: 59
```

Read it this way:

| Line | Meaning |
| --- | --- |
| `name` | A title for this translation. Any short text is fine. |
| `mapping` | The table of tracks. |
| `0:` and `1:` | Cardboard track numbers. They are the `<ns:no>` values, the same numbers as `track` on a hole. |
| `midi_channel` | MIDI channel, counted from 0. `0` is the first channel. |
| `note` | MIDI note number from 0 to 127. `60` is middle C (C4). `69` is A4. |

A track you leave out of `mapping` stays silent. That is how you drop a key the MIDI instrument cannot play.

Indent with spaces, as in the example. The track number is a bare number, and `note` is a number too. The file does not accept names such as `C4` in the `note` field.

Handy note numbers:

| Written pitch | MIDI number |
| --- | --- |
| C3 | 48 |
| G3 | 55 |
| C4 | 60 |
| A4 | 69 |
| C5 | 72 |

On a General MIDI sound module, channel `9` is the drum kit (often labelled channel 10 when the display counts from 1). Melody stays on `0` to `8` or `10` to `15`.

## Melody, drums, and registers

Design the map by role. The scale already sorts the tracks. In the book XML, `xsi:type` tells you which role a track has:

- `ns:TrackNoteDef` is a pipe or a reed. Its `<ns:note>` is the pitch, written `C`, `C#`, `D`, and so on, plus an octave. Copy that pitch into a MIDI number.
- `ns:TrackDrum` is a percussion. The book stores a percussion code and, on some instruments, a shift, because the blow can fall at the end of the hole. Put this track on channel `9` and choose the drum note yourself. Common General MIDI choices are `36` for a bass drum and `38` for a snare.
- `ns:TrackRegisterControlStartDef` and `ns:TrackRegisterControlResetDef` are register commands: a stop, a tremolo, a swell. Give the track a note that your MIDI instrument uses for that control. The note stays on while the hole is open.

```yaml
name: 27 notes
mapping:
  # chant, track 0, written G3 in the scale
  0:
    midi_channel: 0
    note: 55
  # bass drum
  24:
    midi_channel: 9
    note: 36
  # a register, held while its hole passes the reader
  26:
    midi_channel: 0
    note: 24
```

Comments starting with `#` are allowed. The player ignores them.

Two extra fields may appear in a file, as they do in the Limonaire sample:

```yaml
global_parameters:
  read_size: 2.0
mapping:
  0:
    midi_channel: 0
    note: 55
    modifier: MECHANICAL_READ
```

`read_size` is optional. `modifier` is optional and is either `MECHANICAL_READ` or `PERCUSSION_TRIGGERED_AT_END`. Playback holds the chosen note from the start of the hole to the end of the hole in both cases. To change what you hear, change `midi_channel` and `note`.

## When there is no YAML file

The player then builds a map from the scale definition:

- Each note track goes to channel `0`. The MIDI number is the written pitch, one octave lower. A scale note `C4` (number 60) is sent as `48`.
- Each drum track goes to channel `9`, note `45`.
- Register tracks are left silent.

Write a YAML file when that default is not the instrument you want to drive: another octave, a real drum note, drums on their own channel, or a register that must send a control note.

## References

- [Barrel Organ Discovery](https://www.barrel-organ-discovery.org/), the project behind the virtual book.
- [Carton virtuel, the .book format](https://barrelorgandiscovery.github.io/APrintDoc/product/CartonVirtuel/).
- [Gammes](https://www.barrel-organ-discovery.org/instruments/gamma/), the shared scale files.
- [APrint](https://github.com/barrelorgandiscovery/aprintproject), the program that reads and writes `.book` files.
- [APrint scripting: holes, tracks, and microseconds](https://barrelorgandiscovery.github.io/APrintDoc/product/scripting/scripting_first_steps_quickscripts/).
