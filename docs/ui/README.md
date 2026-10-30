# Wireroom window concept

[`concept.html`](concept.html) is an interactive mock-up of the Wireroom main window,
with its style in `concept.css` and its behaviour in `concept.js`. Open it in a browser. It is accepted as a first approximation: it fixes the layout and
the links between panels, and its known problems are corrected during implementation.
The demodulators, their masks, parameters and result types follow the signal core; the
recording, the sonogram, the message texts and the phase points are illustrative.

The interface is built with GPUI through GPUI Kit, on the component principles of
[Argand](https://github.com/o-kos/argand): standard controls wherever they exist, custom
drawing only for the domain canvases (spectrum, sonogram, minimap, constellation), and
settings in popovers anchored to their summaries.

## Layout

- **Title bar and toolbar.** The application button opens the menu. The toolbar holds the
  demodulator selector, its parameter summary, which opens the parameter editor with
  developer parameters behind a toggle, the channel readout, play/pause and stop, and the
  playback speed: 1×, 2×, 4×, 8×, 16× and Max.
- **Plot.** The spectrum at the playhead sits above the sonogram on a shared frequency
  axis; time runs downwards. A full-recording minimap is on the left and the time ruler
  on the right. The unprocessed part is dimmed, and Follow keeps the playhead in view.
- **Results.** The right column shows results in one tab per result type the
  demodulator declares (text, bits, ALE, files). Selecting a result brings its span into
  view on the sonogram, where every result is outlined.
- **Log and constellation.** Below the results, the log with a filter takes the width;
  the constellation stays square, its side equal to the row height.
- **Status bar.** Processing state, progress, position, achieved rate, sample format and
  selected executable implementation.

The mock-up’s legacy backend labels are illustrative. The accepted runtime architecture
launches a selected `dmd` executable; replay and core are separate implementations,
without a backend selector inside this application.

## Decisions

- Processing runs as playback, from realtime through intermediate speeds to the maximum
  the core allows.
- There is exactly one processing channel. The demodulator mask draws it: bandwidth,
  information markers and the centre service marker. It can be dragged or typed, and
  retuned during playback.
- Changing the demodulator stops processing and clears the results.
- The constellation shows the phase points at the current processing position; a
  demodulator without phase points shows that instead.
- Parameter names and descriptions are shown as the core provides them.
